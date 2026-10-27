//! Per-traject corpus state.
//!
//! Each traject owns a federated corpus config in the database
//! (`traject_corpus_sources`) that mirrors the shape of
//! `corpus-registry.yaml`. When the active traject changes, the editor
//! routes reads and writes through that traject's [`TrajectCorpus`] instead
//! of the globally configured [`crate::state::CorpusState`].
//!
//! Construction is lazy: the cache holds a slot per traject, and the
//! first request that needs the traject pays the clone cost. The slot's
//! build lock means concurrent first-touches on the same traject share
//! one clone; first-touches on *different* trajects never block each
//! other.
//!
//! The cached index snapshot expires after [`TRAJECT_INDEX_TTL`]: the
//! first request past the TTL kicks off a background re-enumeration of
//! the sources that swaps in a fresh [`SourceMap`] (new laws merged
//! upstream, re-harvests, saves on another replica become visible
//! without a process restart) while every request — including that first
//! one — keeps being served the stale snapshot (stale-while-revalidate;
//! nobody waits out the GitHub round-trips). The backends, the post-save
//! overlay (reconciled against the branch — see [`reconcile_overlay`]),
//! the implements index/memo and the changed-laws cache carry over so
//! in-flight saves, read-your-writes semantics and implementor lookups
//! are unaffected.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use regelrecht_corpus::backend::create_backend;
use regelrecht_corpus::models::{GitHubSource, LocalSource, Source, SourceType};
use regelrecht_corpus::source_map::collect_law_implements;
use regelrecht_corpus::timing;
use regelrecht_corpus::{CorpusRegistry, SourceMap};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::state::{BackendEntry, CorpusState};
use crate::traject_index_diagnosis::OwnSourceTarget;

/// Resolved corpus state for a single traject, plus per-source write
/// routing.
pub struct TrajectCorpus {
    pub corpus: CorpusState,
    /// The traject this snapshot belongs to. Carried on the snapshot so
    /// every log line written from a read path can be tied back to one
    /// traject — a warning without it cannot be correlated with anything.
    pub traject_id: Uuid,
    /// The branch the writable-own source branches from (the
    /// `gh_base_branch` column), when that source is GitHub-backed.
    /// Kept outside the registry's [`Source`] because it is
    /// traject-flow-specific; the diagnosis of a failed index scan needs
    /// it to tell "the base branch is gone" apart from the other causes.
    pub own_base_branch: Option<String>,
    /// Maps the `source_id` a law was loaded from to the `source_id`
    /// whose backend should receive the write. When the read source is
    /// itself writable (local source, or a GitHub source that doesn't
    /// need a traject-specific branch override), there's no entry and
    /// the caller falls back to the read `source_id` directly.
    ///
    /// Today this map carries one entry: the writable-own's `auth_ref`
    /// (which points at the seed source it shadows, e.g. `minbzk-central`)
    /// mapped to the writable-own's own `source_id`. That gives "save
    /// the law back where it came from" for laws read from the seed,
    /// routed through the traject's own branch on the same upstream
    /// repo.
    pub write_target_for_source: HashMap<String, String>,
    /// Source id of the writable-own backend in this traject. Used by
    /// handlers that have no per-law context (e.g. the documents CRUD
    /// endpoints) to address the traject's own branch directly without
    /// having to reverse-engineer it out of `write_target_for_source`.
    pub writable_own_source_id: String,
    /// Read-your-writes overlay for law YAML content: only bodies that
    /// went through a successful `save_law` (see [`record_save`]). After a
    /// save we mirror the persisted body here so subsequent reads in the
    /// same traject (any session, any user) see the new content without
    /// forcing a full source_map rebuild against GitHub. The overlay is
    /// content-only — `LoadedLaw` metadata (source_id/relative_path)
    /// doesn't change with a content edit, so backend resolution keeps
    /// using `corpus.source_map`.
    ///
    /// Shared (via the `Arc`) across TTL index refreshes: a refreshed
    /// snapshot must never resurrect pre-save content, and an in-flight
    /// save that calls `record_save` on the pre-refresh instance must be
    /// visible on the post-refresh one. Each refresh *reconciles* the
    /// entries against the write-target branch (see [`reconcile_overlay`]):
    /// an entry whose branch file no longer matches the saved body was
    /// overwritten by someone else (another replica, a direct push) and is
    /// dropped, so reads converge to the branch instead of pinning this
    /// process's save forever — read-your-writes-or-newer, not
    /// read-your-writes-forever. The whole map is dropped when the traject
    /// cache entry is invalidated (source config change).
    ///
    /// Unbounded growth is intentional: in practice the size is bounded
    /// by the number of distinct laws edited in this traject, with a
    /// memory budget of roughly N laws × YAML size (KBs). If a bulk-edit
    /// flow is ever added that touches many laws per traject, revisit
    /// with an LRU cap. (Lazily-fetched read bodies live in the bounded
    /// `body_cache` below, not here.)
    overlay: Arc<RwLock<HashMap<String, String>>>,
    /// Cache of law bodies fetched lazily on first read (see `law_yaml`),
    /// so a re-read of an unloaded law (read-only article view, reload,
    /// another tab) doesn't spend another Contents API call.
    ///
    /// Unlike `overlay` this is per-snapshot: a TTL index refresh starts
    /// with an empty cache, so upstream content changes become visible
    /// within [`TRAJECT_INDEX_TTL`] instead of being served stale until
    /// process restart. Bounded at [`BODY_CACHE_MAX_ENTRIES`] with FIFO
    /// eviction so a crawl across a large federated corpus can't grow it
    /// without limit.
    body_cache: RwLock<BoundedCache<String>>,
    /// Law-scoped sidecar files (annotations, individual scenario
    /// `.feature` files) fetched lazily on first read, keyed by a
    /// caller-prefixed string (see `corpus_handlers`). `None` entries are
    /// cached too — "this law has no annotations" costs a full GitHub
    /// round-trip to rediscover, and the editor asks on every law open.
    ///
    /// Per-snapshot like `body_cache` (upstream edits become visible
    /// within [`TRAJECT_INDEX_TTL`]); writes keep it correct within the
    /// window via [`Self::store_sidecar`] / [`Self::invalidate_sidecar`]
    /// (read-your-writes, like the law-body `overlay`).
    sidecar_cache: RwLock<BoundedCache<Option<String>>>,
    /// Per-law scenario listing (filenames + extracted target law ids),
    /// the result of the union-listing in `list_scenarios_in_scope`. One
    /// entry costs 2 `list_files` calls plus a read per scenario file to
    /// rebuild, so the editor's per-law-open listing is the single most
    /// GitHub-round-trip-heavy read. Per-snapshot; invalidated by
    /// scenario save/delete.
    scenario_list_cache: RwLock<BoundedCache<Vec<ScenarioListEntry>>>,
    /// Write-generation counter guarding `sidecar_cache` against a
    /// read/write race: a GET-path backend read can take seconds, and a
    /// save/delete that lands in the cache meanwhile must not be
    /// overwritten when that older read finally completes — the pre-save
    /// bytes would then serve (with a pre-save ETag, 412-looping the
    /// saver's next If-Match) until the snapshot swap. Write-initiated
    /// mutations bump this under the cache write lock; read-path stores
    /// capture it *before* their cache probe (a capture after the probe
    /// would let a write landing in between go unnoticed — the read
    /// would see the post-write generation and re-cache possibly stale
    /// backend bytes over the write's entry) and insert only while it is
    /// unchanged. One counter per cache family (see
    /// `scenario_list_write_gen`) so a write in one family never drops a
    /// concurrent read-path store in the other.
    sidecar_write_gen: AtomicU64,
    /// Write-generation counter guarding `scenario_list_cache` — the
    /// listing analogue of `sidecar_write_gen`, with the same capture-
    /// before-probe contract. Bumped by [`Self::record_scenario_saved`] /
    /// [`Self::record_scenario_deleted`] so an in-flight listing rebuild
    /// discards its (possibly pre-write) result instead of masking the
    /// mutation.
    scenario_list_write_gen: AtomicU64,
    /// "Law → laws it `implements`" index, built on demand by
    /// [`implementors_of`]. A TTL refresh carries the previous snapshot's
    /// index over as a *stale* value (see `implements_index_stale`) and
    /// rebuilds it lazily against the new snapshot — stale-while-
    /// revalidate, like the snapshot itself: a lookup is never blocked on
    /// a corpus rescan when a previous index exists. The rebuild itself
    /// is cheap thanks to `implements_memo`.
    implements_index: RwLock<Option<Arc<ImplementsIndex>>>,
    /// Whether the value in `implements_index` was built against a
    /// *previous* snapshot (carried over by a TTL refresh). While true,
    /// the first lookup to win `implements_build_lock` rebuilds against
    /// this snapshot; concurrent lookups keep serving the carried index.
    implements_index_stale: AtomicBool,
    /// Single-flight gate for building `implements_index`. Held across
    /// the (potentially fetching) scan WITHOUT holding `implements_index`
    /// itself, so `record_save` — which runs while the writable-own
    /// backend mutex is held — can update the index without any lock
    /// cycle against the scan's backend fetches.
    implements_build_lock: Mutex<()>,
    /// Cross-snapshot content-addressed memo for the implements scan:
    /// `(source_id, blob sha) → parsed implements list`. The blob sha
    /// comes from the Trees enumeration that built the index snapshot
    /// ([`regelrecht_corpus::source_map::LoadedLaw::content_sha`]), so a
    /// rebuild after a TTL refresh only fetches/parses bodies whose
    /// content actually changed — without it the first implementors
    /// lookup after EVERY refresh would re-fetch the entire federated
    /// corpus over the Contents API (the scan cost PR #762 removed from
    /// the request path). Shared (via the `Arc`) across refreshes like
    /// the overlay; each rebuild swaps in a map containing only the
    /// entries of the current snapshot, so it cannot grow beyond
    /// O(corpus) across content churn.
    implements_memo: Arc<RwLock<ImplementsMemo>>,
    /// Short-lived cache of the "edited in this traject" diff
    /// ([`changed_law_ids`]). Each `changed-laws` request would otherwise
    /// fire a GitHub Compare API call, and the library sidebar re-requests
    /// it on every load / traject switch — on a shared, rate-limited token
    /// that adds up fast. The cache collapses a burst of library loads into
    /// one Compare call per [`CHANGED_LAWS_TTL`] window. `save_law`
    /// invalidates it so the saving user sees their own edit appear
    /// immediately (read-your-writes, like `overlay`); other members /
    /// replicas converge within the TTL. A stale entry is also served as a
    /// fallback when a fresh Compare call fails (e.g. token throttled), so
    /// the section degrades to slightly-stale rather than vanishing.
    /// Shared (via the `Arc`) across TTL index refreshes, like `overlay`:
    /// a refresh must not undo `save_law`'s update of this cache.
    changed_cache: Arc<RwLock<Option<ChangedLawsCache>>>,
    /// Single-flight gate for the *background* changed-laws recompute
    /// spawned by [`Self::changed_law_ids`] when the cache entry has
    /// expired: only one task per traject recomputes, everyone else keeps
    /// serving the stale ids. Shared across snapshots like the cache it
    /// guards, so a refresh mid-recompute cannot admit a second Compare
    /// call.
    changed_refresh_in_flight: Arc<AtomicBool>,
    /// Single-flight gate for the *synchronous* changed-laws refresh — the
    /// leg taken when only the caller's own token gets into the repo, so
    /// there is no background task that could do the work. Without it every
    /// linked member loading the library inside the same expired-TTL window
    /// fires its own Compare call against a rate-limited token.
    ///
    /// A `Mutex` rather than the `AtomicBool` above because here the losers
    /// must *wait* for the winner instead of returning stale: after taking
    /// the gate each caller re-reads the cache, and a fresh entry left by the
    /// winner short-circuits the Compare call. That double-check is what
    /// makes the gate safe across callers with different credentials — an
    /// empty result produced without a usable token is never cached (see
    /// [`Self::changed_law_ids`]), so a token-carrying caller can never
    /// inherit one, while a token-less caller waiting here is upgraded from
    /// "empty" to the winner's real diff.
    ///
    /// Shared across snapshots like the cache it guards.
    changed_refresh_gate: Arc<Mutex<()>>,
    /// Whether this snapshot's index scan had a per-request user token
    /// available as fallback for the writable-own source. Only a bool —
    /// the token itself is **never** stored on this shared, cross-user
    /// structure (nor in any cache key). [`TrajectCorpusCache::get_or_build`]
    /// uses it to decide whether a snapshot whose writable-own scan failed
    /// is worth rebuilding now that a request *does* carry a token: a
    /// failure scanned *without* one may succeed with it; a failure
    /// scanned *with* one waits for the TTL instead of re-scanning on
    /// every request.
    own_index_scanned_with_user_token: bool,
}

/// One scenario file in a law's cached listing: the filename plus the law
/// ids its `I evaluate … of "…"` steps target (the data
/// `corpus_handlers::ScenarioEntry` is serialised from).
#[derive(Clone)]
pub struct ScenarioListEntry {
    pub filename: String,
    pub target_law_ids: Vec<String>,
}

/// Cached result of [`TrajectCorpus::changed_law_ids`] with the instant it
/// was computed, for TTL expiry.
struct ChangedLawsCache {
    computed_at: Instant,
    law_ids: Vec<String>,
}

/// RAII reset for a single-flight `AtomicBool` held by a spawned task:
/// clears the flag when the task ends, *including* on panic — a latched
/// flag would permanently disable the background path it guards.
struct ResetOnDrop(Arc<AtomicBool>);

impl Drop for ResetOnDrop {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// How long a cached changed-laws diff is served before a fresh GitHub
/// Compare call is made. Short enough that another member's save shows up
/// promptly in the sidebar, long enough to collapse a burst of library
/// loads (mount + tab switches + retries) into a single Compare call.
const CHANGED_LAWS_TTL: Duration = Duration::from_secs(60);

/// How long a caller waits for the synchronous-refresh gate before giving up
/// and computing for itself. See the use site: this caps the added latency of
/// the gate at roughly one Compare round-trip instead of letting concurrent
/// callers queue behind a persistently failing upstream.
const CHANGED_REFRESH_GATE_WAIT: Duration = Duration::from_secs(10);

/// How long a traject's cached index snapshot (its [`SourceMap`] plus the
/// derived body / sidecar / implements caches) is served before the first
/// request past the deadline triggers a *background* re-enumeration of
/// the sources (stale-while-revalidate — the triggering request itself
/// keeps serving the stale snapshot). Same convergence target as
/// [`CHANGED_LAWS_TTL`]: upstream changes (new laws merged, re-harvests,
/// saves on another replica) show up within roughly a minute instead of
/// requiring a process restart, while a burst of library loads still hits
/// the snapshot. The refresh keeps backends, the post-save overlay and the
/// changed-laws cache (see [`TrajectCorpusCache::get_or_build`]).
const TRAJECT_INDEX_TTL: Duration = Duration::from_secs(60);

/// Upper bound on lazily-fetched law bodies cached per traject snapshot
/// (see [`TrajectCorpus::body_cache`]). At a typical body size of a few
/// tens of KB this caps the cache at low tens of MB per traject — enough
/// to cover an implements-index scan over a mid-sized federated corpus
/// without refetching, while a full-corpus crawl merely evicts FIFO
/// instead of growing without bound.
const BODY_CACHE_MAX_ENTRIES: usize = 1024;

/// When an implements-index build would otherwise fetch at least this many
/// law bodies one-by-one from a *single* source, ask that source for its
/// whole implements map in one go instead (see
/// [`RepoBackend::read_all_implements`] — the precomputed index committed in
/// the corpus repo, or a repo-archive download when no index describes the
/// branch). Below the threshold the per-law lazy fetch is cheaper. This is
/// what turns the cold build over a large GitHub-backed traject from
/// O(corpus) Contents calls (a 504) into a couple of requests.
///
/// [`RepoBackend::read_all_implements`]: regelrecht_corpus::backend::RepoBackend::read_all_implements
const BULK_FETCH_THRESHOLD: usize = 16;

/// Lazily-fetched per-snapshot values with a FIFO size cap, keyed by a
/// caller-chosen string (law id for bodies, a prefixed key for sidecar
/// files). FIFO (not LRU) keeps reads lock-cheap: a cache hit only needs
/// the outer `RwLock`'s read guard, no per-hit reordering under a write
/// lock. Eviction order only matters once a traject's read set exceeds
/// [`BODY_CACHE_MAX_ENTRIES`], which is already crawl territory.
#[derive(Default)]
pub(crate) struct BoundedCache<V> {
    map: HashMap<String, V>,
    /// Insertion order of the keys in `map`, oldest first.
    order: VecDeque<String>,
}

impl<V> BoundedCache<V> {
    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        self.map.get(key)
    }

    pub(crate) fn insert(&mut self, key: String, value: V) {
        if !self.map.contains_key(&key) {
            while self.map.len() >= BODY_CACHE_MAX_ENTRIES {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                self.map.remove(&oldest);
            }
            self.order.push_back(key.clone());
        }
        self.map.insert(key, value);
    }

    /// Drop one key, including its `order` entry. The O(n) deque scan is
    /// fine — invalidations are save/delete-driven, so rare. Leaving the
    /// order entry behind would be subtly wrong at capacity: a re-insert
    /// of the same key appends a duplicate, and eviction popping the
    /// stale front duplicate would `map.remove` the *live* re-inserted
    /// value while its newer order entry survives.
    fn remove(&mut self, key: &str) {
        if self.map.remove(key).is_some() {
            self.order.retain(|k| k != key);
        }
    }
}

/// Content-addressed memo behind [`TrajectCorpus::implements_memo`]:
/// `(source_id, blob sha) → parsed implements list`.
///
/// The sha is the one the Trees enumeration reported at source-map build
/// time, while the body is fetched separately (clone read or, for the
/// writable-own API backend, the tarball). A branch push between those two
/// steps can key an entry under sha A while the body parsed was sha B — a
/// bounded staleness, the same TOCTOU the per-law fetch path has always
/// had. It is harmless: the implements index only drives the editor's
/// implementor panel (never legal calculations), and it self-heals once
/// the law's sha changes again and a rebuild overwrites the memo wholesale.
type ImplementsMemo = HashMap<(String, String), Vec<String>>;

/// Per-snapshot forward index: each law's parsed `implements` list, plus
/// the laws whose body couldn't be fetched during the scan (throttling,
/// token expiry, …) and therefore couldn't be checked.
///
/// `Clone` exists for `Arc::make_mut` in [`TrajectCorpus::record_save`]:
/// a post-save entry update copy-on-writes when a lookup still holds the
/// previous `Arc`.
#[derive(Clone)]
struct ImplementsIndex {
    /// `law_id` → `$id`s of the higher laws it declares it implements.
    /// Laws with an empty list are omitted.
    implements_by_law: HashMap<String, Vec<String>>,
    /// Laws skipped because their body fetch failed. Kept so lookups can
    /// report partiality instead of silently passing off an incomplete
    /// scan as "no implementors". Self-heals at the next snapshot
    /// (TTL refresh / invalidation), which rebuilds the index.
    failed_law_ids: Vec<String>,
}

/// Result of [`TrajectCorpus::implementors_of`]: the implementing law ids
/// plus the ids that could not be checked because their body fetch failed
/// when the index was built.
pub struct ImplementorsResult {
    pub implementors: Vec<String>,
    /// Number of laws whose body could not be read **anywhere in the index
    /// build** — this is an index-wide count, NOT scoped to the queried
    /// law. It signals that the index (and therefore *any* implementor
    /// lookup against it) may be incomplete, not that these specific
    /// failures implement the queried law. Reported so callers can surface
    /// partiality instead of passing an incomplete scan off as "no
    /// implementors". Self-heals at the next snapshot rebuild.
    pub skipped_count: usize,
}

impl TrajectCorpus {
    /// The index-scan failure of the **writable-own** source in this
    /// snapshot, if any. `Some` means the traject's own laws are missing
    /// from the index — every law would silently resolve from the seed
    /// sources — so callers serving the traject-scoped library gate on
    /// this and fail loudly instead of presenting a normal-looking
    /// central-corpus-only list.
    pub fn own_index_error(&self) -> Option<&str> {
        self.corpus
            .index_failures
            .get(&self.writable_own_source_id)
            .map(String::as_str)
    }

    /// The registry entry of this traject's **writable-own** source — the
    /// repo the editor commits to. `None` only when the traject's source
    /// config no longer lists it (a shape the build path doesn't produce).
    ///
    /// Shared by every caller that needs the source's own coordinates
    /// rather than a law's: the index-failure diagnosis
    /// ([`Self::own_source_target`]) and the integrity scan, which reads
    /// the repo's whole file tree instead of one law.
    pub fn own_source(&self) -> Option<&Source> {
        self.corpus
            .registry
            .get_source(&self.writable_own_source_id)
    }

    /// The GitHub coordinates of the writable-own source, for the in-band
    /// diagnosis of a failed index scan (see
    /// [`crate::traject_index_diagnosis`]). `None` when that source isn't
    /// GitHub-backed (a local dev/preview stack), which leaves nothing to
    /// ask GitHub about.
    ///
    /// The branch comes straight from the stored source config rather than
    /// being re-derived from the traject name, so the probe asks about the
    /// branch the scan actually read.
    pub fn own_source_target(&self) -> Option<OwnSourceTarget> {
        let source = self.own_source()?;
        let SourceType::GitHub { github } = &source.source_type else {
            return None;
        };
        Some(OwnSourceTarget {
            traject_id: self.traject_id,
            source_id: self.writable_own_source_id.clone(),
            owner: github.owner.clone(),
            repo: github.repo.clone(),
            branch: github.branch.clone(),
            // Mirrors `build_traject_github_backend`'s default: a NULL
            // `gh_base_branch` means "main".
            base_branch: self
                .own_base_branch
                .clone()
                .unwrap_or_else(|| "main".to_string()),
        })
    }

    /// Re-resolve the server-side token configured for the writable-own
    /// source, if any.
    ///
    /// For the callers that talk to GitHub directly instead of through the
    /// backend: when a request carries no personal token, this is the
    /// credential the backend itself would have used, so they have to
    /// re-resolve it or they walk a different access path than the ordinary
    /// reads. The index diagnosis needs that to probe the path that actually
    /// broke ([`crate::traject_index_diagnosis`]); the integrity scan needs
    /// it to enumerate the branch at all ([`crate::traject_integrity`]).
    /// Resolved on demand and dropped by the caller — no token is ever
    /// stored on this shared, cross-user snapshot.
    pub fn own_server_token(&self) -> Option<String> {
        let source = self.own_source()?;
        regelrecht_corpus::auth::CredentialResolver::new(self.corpus.auth_file.as_deref())
            .resolve_source(source)
            .ok()
            .and_then(regelrecht_corpus::auth::TokenDecision::into_token)
    }

    /// Resolve the YAML content for a law in this traject, preferring the
    /// post-save overlay over the source_map snapshot built at traject
    /// activation time.
    ///
    /// The source_map is a lightweight **index** — for GitHub-backed sources
    /// its entries carry only metadata (no body). When a metadata-only law is
    /// read for the first time, its body is fetched lazily from the law's own
    /// source backend (one Contents API call), rather than every law's content
    /// being fetched up front at traject-activation time.
    ///
    /// `Ok(None)` is a genuine miss (the law isn't in this traject's corpus, or
    /// its source's backend never initialised). A lazy-fetch failure (GitHub
    /// throttling, token expiry, a network blip) is returned as `Err` so the
    /// caller can answer "failed to load" instead of masking a transient error
    /// as a 404 "not found".
    pub async fn law_yaml(
        &self,
        law_id: &str,
    ) -> Result<Option<String>, regelrecht_corpus::error::CorpusError> {
        self.law_yaml_with_read_token(law_id, None).await
    }

    /// [`Self::law_yaml`] with a per-request read token for the
    /// **writable-own** source (the acting user's own GitHub token,
    /// resolved by the request handler). The token is applied *only* when
    /// the law's body resolves from the writable-own source — seed sources
    /// never see it — and it is passed through per call, never stored on
    /// this (shared, cross-user) structure. Cached *content* is fine: every
    /// traject route runs a membership check before reaching this.
    pub async fn law_yaml_with_read_token(
        &self,
        law_id: &str,
        own_read_token: Option<&str>,
    ) -> Result<Option<String>, regelrecht_corpus::error::CorpusError> {
        if let Some(text) = self.overlay.read().await.get(law_id) {
            return Ok(Some(text.clone()));
        }
        if let Some(text) = self.body_cache.read().await.get(law_id) {
            return Ok(Some(text.clone()));
        }

        // Pull the bits we need out of the index entry, then drop the borrow
        // before the await below.
        let (source_id, relative_path) = {
            let Some(law) = self.corpus.source_map.get_law(law_id) else {
                return Ok(None);
            };
            if law.is_loaded() {
                return Ok(Some(law.yaml_content.clone()));
            }
            (law.source_id.clone(), law.relative_path.clone())
        };

        // The source is indexed but its backend was skipped at build time
        // — `ensure_ready` failed, which is as likely a GitHub hiccup as a
        // misconfiguration. This law exists (the index enumerated it), so
        // "no backend" is a failure to read it, not a law that isn't
        // there; returning a miss would render the whole source as
        // "leeg maar in orde" for a TTL window.
        let Some(entry) = self.corpus.backends.get(&source_id) else {
            return Err(regelrecht_corpus::error::CorpusError::Config(format!(
                "source '{source_id}' has no usable backend in this snapshot, \
                 so law '{law_id}' cannot be read (backend init failed at build time)"
            )));
        };

        // The personal token authenticates exclusively against the
        // traject's own repo; a seed read must never carry it.
        let token = (source_id == self.writable_own_source_id)
            .then_some(own_read_token)
            .flatten();
        let content = {
            let backend = entry.backend.lock().await;
            // `?` propagates a read error rather than collapsing it to None.
            backend
                .read_file_with_token(std::path::Path::new(&relative_path), token)
                .await?
        };
        let Some(content) = content else {
            return Ok(None);
        };

        // Cache the lazily-fetched body so re-reads of this unloaded law don't
        // each spend another Contents API call (read-only views, reloads, a
        // second tab). Per-snapshot: discarded on a TTL index refresh so
        // upstream content changes converge, and bounded so a corpus-wide
        // crawl can't grow it without limit. Also makes a genuinely-empty
        // body a one-shot fetch rather than re-fetching every call (the
        // empty-`yaml_content` "unloaded" sentinel can't tell them apart,
        // but this cache short-circuits before that check).
        self.body_cache
            .write()
            .await
            .insert(law_id.to_string(), content.clone());
        Ok(Some(content))
    }

    /// Like [`Self::law_yaml`], but returns **every** version of the law
    /// rather than the single "best for today" one.
    ///
    /// The scenario dependency loader feeds all of these to the engine so its
    /// `select_in` can pick the version in force on the scenario's calculation
    /// date — fixing the case where a referenced law has a future-dated version
    /// that would otherwise be the only one loaded (and so "not yet in force").
    ///
    /// Read-your-writes: the overlay / body cache hold the single saved (branch)
    /// body, keyed by `law_id`; it is applied to the version whose source +
    /// source-relative path matches the saved entry (`get_law`'s winner). Other
    /// versions are served from the index body (local sources) or lazily fetched
    /// via their own backend + `relative_path` (GitHub sources). Returns an empty
    /// vec when no version of the id is indexed.
    ///
    /// Accepts the per-request writable-own read token — same contract as
    /// [`Self::law_yaml_with_read_token`]: the token is only ever applied
    /// to versions whose source is the writable-own, per call, never
    /// stored. Pass `None` outside a user-request context.
    pub async fn law_yaml_versions_with_read_token(
        &self,
        law_id: &str,
        own_read_token: Option<&str>,
    ) -> Result<Vec<String>, regelrecht_corpus::error::CorpusError> {
        // Snapshot the per-version index metadata, then drop the borrow before
        // any await below.
        let entries: Vec<(String, String, Option<String>)> = {
            let Some(versions) = self.corpus.source_map.get_law_versions(law_id) else {
                return Ok(Vec::new());
            };
            versions
                .iter()
                .map(|v| {
                    let body = v.is_loaded().then(|| v.yaml_content.clone());
                    (v.source_id.clone(), v.relative_path.clone(), body)
                })
                .collect()
        };

        // The overlay / body cache hold the saved body for the single best
        // version (`get_law`'s winner); identify it so it's applied to the
        // right version entry rather than all of them.
        let saved_key = self
            .corpus
            .source_map
            .get_law(law_id)
            .map(|l| (l.source_id.clone(), l.relative_path.clone()));
        let saved_body: Option<String> = if let Some(text) = self.overlay.read().await.get(law_id) {
            Some(text.clone())
        } else {
            self.body_cache.read().await.get(law_id).cloned()
        };

        let mut out = Vec::with_capacity(entries.len());
        for (source_id, relative_path, body) in entries {
            // Read-your-writes for the saved/branch version. Compare by
            // reference (no per-iteration clone of the two owned Strings).
            if saved_key
                .as_ref()
                .is_some_and(|(s, r)| s == &source_id && r == &relative_path)
            {
                if let Some(body) = &saved_body {
                    out.push(body.clone());
                    continue;
                }
            }
            // Eagerly-loaded (local) version body is already in the index.
            if let Some(body) = body {
                out.push(body);
                continue;
            }
            // Metadata-only (GitHub) version — lazily fetch its body. A source
            // whose backend was skipped at build time is treated as a missing
            // version rather than failing the whole set: this path feeds the
            // scenario dependency loader, where one unfetchable future-dated
            // version must not take down a law whose in-force version is
            // right here. Unlike `law_yaml`, which answers a single law and
            // therefore errors, this stays per-version isolation — but it is
            // no longer silent, because a dropped version changes which
            // redaction the engine considers in force.
            let Some(entry) = self.corpus.backends.get(&source_id) else {
                tracing::warn!(
                    law_id = %law_id,
                    source_id = %source_id,
                    "skipping a law version whose source has no usable backend \
                     in this snapshot (backend init failed at build time)"
                );
                continue;
            };
            // Isolate per-version fetch failures: a transient backend error for
            // one version (e.g. a central GitHub blip on a future-dated version)
            // must not fail the whole set and take down a law whose in-force
            // version is locally available. Skip + log, mirroring the frontend's
            // per-version `loadLawVersions` isolation. A law that ends up with no
            // fetchable version surfaces to the loader as a missing dependency
            // (retried on the next run), not a hard 502.
            // Personal token only for the writable-own source, like
            // `law_yaml_with_read_token`.
            let token = (source_id == self.writable_own_source_id)
                .then_some(own_read_token)
                .flatten();
            let content = {
                let backend = entry.backend.lock().await;
                match backend
                    .read_file_with_token(std::path::Path::new(&relative_path), token)
                    .await
                {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::warn!(
                            law_id = %law_id,
                            source_id = %source_id,
                            error = %e,
                            "skipping a law version that failed to fetch"
                        );
                        continue;
                    }
                }
            };
            match content {
                Some(content) => out.push(content),
                // Enumerated in the index but the blob is gone — an index/backend
                // inconsistency. Skip like the Err path, but log it: same
                // diagnosability concern, and this path is new.
                None => tracing::warn!(
                    law_id = %law_id,
                    source_id = %source_id,
                    relative_path = %relative_path,
                    "skipping a law version whose blob was not found on the backend"
                ),
            }
        }
        Ok(out)
    }

    /// Mirror a freshly-saved law's content into the read-your-writes
    /// overlay. Called by `save_law` after a successful `backend.persist`,
    /// so the next GET on the same law (or a refresh) sees the new body.
    ///
    /// Also keeps the per-snapshot implements index coherent: a save can
    /// add or drop `implements` declarations, so when the index has been
    /// built its entry for this law is replaced with the new body's list.
    /// (When a save races an in-flight index build the scan may have read
    /// the pre-save body after this update ran; the entry is then stale
    /// until the next snapshot — bounded by [`TRAJECT_INDEX_TTL`].)
    pub async fn record_save(&self, law_id: String, body: String) {
        let implements = collect_law_implements(&body);
        self.overlay.write().await.insert(law_id.clone(), body);
        if let Some(index) = self.implements_index.write().await.as_mut() {
            let index = Arc::make_mut(index);
            // The law's body is now known, so it can no longer count as
            // "skipped due to fetch failure" from an earlier scan.
            index.failed_law_ids.retain(|id| id != &law_id);
            if implements.is_empty() {
                index.implements_by_law.remove(&law_id);
            } else {
                index.implements_by_law.insert(law_id, implements);
            }
        }
    }

    /// Cached sidecar file content for `key` (annotations / scenario
    /// files; the caller prefixes the key by kind — see
    /// `corpus_handlers`). The outer `Option` is the cache verdict
    /// (miss vs hit), the inner one the cached read result — `Some(None)`
    /// means "we asked GitHub and the file does not exist", which is as
    /// expensive to rediscover as a hit and far more common (most laws
    /// have no annotations yet).
    pub async fn cached_sidecar(&self, key: &str) -> Option<Option<String>> {
        self.sidecar_cache.read().await.get(key).cloned()
    }

    /// Current write-generation of `sidecar_cache`. GET-path callers
    /// capture this *before* their cache probe + backend read and pass it
    /// to [`Self::store_sidecar_read`], which then drops the store when a
    /// save/delete bumped the counter in between — see
    /// `sidecar_write_gen`.
    pub fn sidecar_write_generation(&self) -> u64 {
        self.sidecar_write_gen.load(Ordering::SeqCst)
    }

    /// Current write-generation of `scenario_list_cache` — the listing
    /// counterpart of [`Self::sidecar_write_generation`], consumed by
    /// [`Self::store_scenario_list_read`].
    pub fn scenario_list_write_generation(&self) -> u64 {
        self.scenario_list_write_gen.load(Ordering::SeqCst)
    }

    /// Store a GET-path sidecar read result for `key`, unless a
    /// write-initiated mutation happened since the caller captured
    /// `gen_before` — a slow read completing after a save must not
    /// re-cache the pre-save bytes over the save's fresher entry.
    pub async fn store_sidecar_read(&self, key: String, content: Option<String>, gen_before: u64) {
        let mut cache = self.sidecar_cache.write().await;
        if self.sidecar_write_gen.load(Ordering::SeqCst) == gen_before {
            cache.insert(key, content);
        }
    }

    /// Store a just-written sidecar body for `key` — the sidecar analogue
    /// of [`Self::record_save`]'s read-your-writes. Bumps the write
    /// generation (under the cache write lock) so an in-flight GET-path
    /// read discards its possibly pre-save result instead of overwriting
    /// this entry.
    pub async fn store_sidecar(&self, key: String, content: Option<String>) {
        let mut cache = self.sidecar_cache.write().await;
        self.sidecar_write_gen.fetch_add(1, Ordering::SeqCst);
        cache.insert(key, content);
    }

    /// Drop one sidecar cache entry so the next read resolves through the
    /// real backend routing again. Used by scenario delete, where the
    /// next read may legitimately fall back to a seed copy — a negative
    /// cache entry would hide it. Bumps the write generation for the same
    /// reason [`Self::store_sidecar`] does.
    pub async fn invalidate_sidecar(&self, key: &str) {
        let mut cache = self.sidecar_cache.write().await;
        self.sidecar_write_gen.fetch_add(1, Ordering::SeqCst);
        cache.remove(key);
    }

    /// Cached scenario listing for `law_id` (see `scenario_list_cache`).
    pub async fn cached_scenario_list(&self, law_id: &str) -> Option<Vec<ScenarioListEntry>> {
        self.scenario_list_cache.read().await.get(law_id).cloned()
    }

    /// Store the scenario listing computed for `law_id` on the GET path,
    /// unless a write-initiated mutation happened since `gen_before` was
    /// captured (same guard as [`Self::store_sidecar_read`] — a listing
    /// computed before a save/delete must not mask it).
    pub async fn store_scenario_list_read(
        &self,
        law_id: String,
        entries: Vec<ScenarioListEntry>,
        gen_before: u64,
    ) {
        let mut cache = self.scenario_list_cache.write().await;
        if self.scenario_list_write_gen.load(Ordering::SeqCst) == gen_before {
            cache.insert(law_id, entries);
        }
    }

    /// Fold a just-saved scenario into the cached listing for `law_id`
    /// (upsert, keeping the filename sort) instead of invalidating it.
    /// Surgical on purpose: a full rebuild right after the commit would
    /// re-list GitHub's Contents API, whose directory view is eventually
    /// consistent — the rebuild can capture the pre-save view and cache
    /// it for the rest of the snapshot window. No cached listing → no-op
    /// (the next listing rebuilds and the file is on the branch). Bumps
    /// the write generation so an in-flight rebuild discards its
    /// (possibly pre-save) result.
    pub async fn record_scenario_saved(
        &self,
        law_id: &str,
        filename: &str,
        target_law_ids: Vec<String>,
    ) {
        let mut cache = self.scenario_list_cache.write().await;
        self.scenario_list_write_gen.fetch_add(1, Ordering::SeqCst);
        if let Some(mut entries) = cache.get(law_id).cloned() {
            match entries.iter_mut().find(|e| e.filename == filename) {
                Some(entry) => entry.target_law_ids = target_law_ids,
                None => {
                    entries.push(ScenarioListEntry {
                        filename: filename.to_string(),
                        target_law_ids,
                    });
                    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
                }
            }
            cache.insert(law_id.to_string(), entries);
        }
    }

    /// Remove a just-deleted scenario from the cached listing for
    /// `law_id`. Surgical for the same eventual-consistency reason as
    /// [`Self::record_scenario_saved`]: a rebuild racing GitHub's lagging
    /// directory view would resurrect the deleted file in the cache for
    /// up to a snapshot window. Bumps the write generation so an
    /// in-flight rebuild discards its result.
    pub async fn record_scenario_deleted(&self, law_id: &str, filename: &str) {
        let mut cache = self.scenario_list_cache.write().await;
        self.scenario_list_write_gen.fetch_add(1, Ordering::SeqCst);
        if let Some(mut entries) = cache.get(law_id).cloned() {
            entries.retain(|e| e.filename != filename);
            cache.insert(law_id.to_string(), entries);
        }
    }

    /// Law ids whose articles declare `implements` for `law_id` (the IoC
    /// reverse link), resolved against this snapshot's federated corpus.
    ///
    /// The first call of the process builds the [`ImplementsIndex`] — the
    /// one O(corpus) scan that lazily fetches the body of every
    /// metadata-only law — and every later call (any target law) is an
    /// in-memory reverse lookup. A TTL refresh does NOT repeat that scan:
    /// the previous index is served stale while the first post-refresh
    /// lookup rebuilds it, and the rebuild resolves unchanged bodies
    /// through the content-addressed `implements_memo` (blob shas from
    /// the Trees enumeration), fetching only what actually changed.
    /// Bodies fetched by a scan land in `body_cache`, so opening one of
    /// the scanned laws afterwards is also free. Laws whose body fetch
    /// failed are counted in [`ImplementorsResult::skipped_count`]
    /// instead of being silently indistinguishable from "doesn't
    /// implement anything"; the failed set is retried at the next
    /// rebuild.
    pub async fn implementors_of(&self, law_id: &str) -> ImplementorsResult {
        let index = self.implements_index_get_or_build().await;
        let mut implementors: Vec<String> = index
            .implements_by_law
            .iter()
            .filter(|(id, implemented)| {
                id.as_str() != law_id && implemented.iter().any(|i| i == law_id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        implementors.sort();
        ImplementorsResult {
            implementors,
            skipped_count: index.failed_law_ids.len(),
        }
    }

    /// Get the snapshot's implements index, building it on first use.
    ///
    /// Single-flighted on `implements_build_lock`; the (potentially
    /// fetching) build never holds the `implements_index` RwLock itself,
    /// so a concurrent `record_save` — which runs under the writable-own
    /// backend mutex that the scan may also need — can always complete.
    ///
    /// Stale-while-revalidate across snapshots: a TTL refresh carries the
    /// previous index over flagged stale. The first lookup to win the
    /// build lock rebuilds against this snapshot (cheap — the
    /// content-addressed `implements_memo` means only *changed* bodies
    /// are fetched/parsed); every other lookup keeps serving the carried
    /// index instead of queueing behind the rebuild. Only a traject that
    /// has never built an index at all (first lookup of the process)
    /// blocks all callers on the one initial scan.
    async fn implements_index_get_or_build(&self) -> Arc<ImplementsIndex> {
        let carried = {
            let guard = self.implements_index.read().await;
            match guard.as_ref() {
                Some(index) if !self.implements_index_stale.load(Ordering::SeqCst) => {
                    return index.clone();
                }
                other => other.cloned(),
            }
        };

        let _build = match &carried {
            // Nothing to serve: every caller waits for the one build.
            None => self.implements_build_lock.lock().await,
            // Stale index available: only one task rebuilds; the rest
            // serve the carried index rather than queueing up.
            Some(stale) => match self.implements_build_lock.try_lock() {
                Ok(guard) => guard,
                Err(_) => return stale.clone(),
            },
        };

        // Re-check under the build lock: the previous holder may have
        // (re)built while we waited.
        {
            let guard = self.implements_index.read().await;
            if let Some(index) = guard.as_ref() {
                if !self.implements_index_stale.load(Ordering::SeqCst) {
                    return index.clone();
                }
            }
        }

        let index = self.build_implements_index().await;
        *self.implements_index.write().await = Some(index.clone());
        self.implements_index_stale.store(false, Ordering::SeqCst);
        index
    }

    /// One pass over the snapshot's laws producing the implements index.
    /// Per law, in cost order:
    /// - **overlay** entries (saved in this traject) are parsed from the
    ///   overlay body — no fetch, O(locally saved laws);
    /// - **loaded** entries (local sources) reuse the `implements` list
    ///   the corpus parsed at load time — no fetch, no parse;
    /// - **metadata-only** entries hit the cross-snapshot
    ///   `implements_memo` by `(source_id, blob sha)` — only bodies whose
    ///   sha changed since the last build (or were never seen) are
    ///   fetched via [`law_yaml`] (which also feeds the body cache) and
    ///   parsed.
    ///
    /// The memo is swapped wholesale at the end so it only ever holds the
    /// current snapshot's entries (bounded by corpus size, and a build
    /// cancelled mid-flight — client disconnect — leaves the old memo
    /// intact).
    async fn build_implements_index(&self) -> Arc<ImplementsIndex> {
        let overlay = self.overlay.read().await.clone();
        let memo = self.implements_memo.read().await.clone();
        let mut new_memo: ImplementsMemo = HashMap::new();

        // Collect what we need first so the source_map borrow doesn't
        // live across the awaits below.
        struct ScanEntry {
            law_id: String,
            source_id: String,
            relative_path: String,
            content_sha: Option<String>,
            loaded_implements: Option<Vec<String>>,
        }
        let entries: Vec<ScanEntry> = self
            .corpus
            .source_map
            .laws()
            .map(|law| ScanEntry {
                law_id: law.law_id.clone(),
                source_id: law.source_id.clone(),
                relative_path: law.relative_path.clone(),
                content_sha: law.content_sha.clone(),
                loaded_implements: law.is_loaded().then(|| law.implements.clone()),
            })
            .collect();

        // Cold builds would fetch every metadata-only body. One Contents
        // call per law is O(corpus) and times out on large GitHub-backed
        // trajects, so when a single source has many bodies to fetch, pull
        // the whole source in one archive request and serve the scan from
        // memory. Holds parsed `implements` lists (NOT bodies — the backend
        // parses and discards bodies during the streamed archive extraction,
        // so neither side ever materialises the whole corpus), keyed by
        // (source_id, source-relative path) to match the loop's per-entry
        // lookup.
        let mut bulk: HashMap<(String, String), Vec<String>> = HashMap::new();
        {
            let mut misses: HashMap<&str, usize> = HashMap::new();
            for entry in &entries {
                if overlay.contains_key(&entry.law_id) || entry.loaded_implements.is_some() {
                    continue;
                }
                let memo_hit = entry
                    .content_sha
                    .as_ref()
                    .is_some_and(|sha| memo.contains_key(&(entry.source_id.clone(), sha.clone())));
                if !memo_hit {
                    *misses.entry(entry.source_id.as_str()).or_default() += 1;
                }
            }
            for (source_id, count) in misses {
                if count < BULK_FETCH_THRESHOLD {
                    continue;
                }
                let Some(backend_entry) = self.corpus.backends.get(source_id) else {
                    continue;
                };
                let result = {
                    let backend = backend_entry.backend.lock().await;
                    backend.read_all_implements().await
                };
                match result {
                    Ok(files) => {
                        tracing::info!(
                            source_id,
                            files = files.len(),
                            "implements scan: bulk-loaded source in one request"
                        );
                        for (rel_path, implements) in files {
                            bulk.insert((source_id.to_string(), rel_path), implements);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            source_id,
                            error = %e,
                            "implements scan: bulk source load failed; falling back to per-law fetch"
                        );
                    }
                }
            }
        }

        let mut implements_by_law = HashMap::new();
        let mut failed_law_ids = Vec::new();
        for entry in entries {
            let implements = if let Some(body) = overlay.get(&entry.law_id) {
                // A body saved in this traject supersedes both the loaded
                // content and the enumerated blob; never memo it under the
                // (pre-save) snapshot sha.
                collect_law_implements(body)
            } else if let Some(implements) = entry.loaded_implements {
                implements
            } else {
                let memo_key = entry.content_sha.map(|sha| (entry.source_id.clone(), sha));
                if let Some(hit) = memo_key.as_ref().and_then(|key| memo.get(key)) {
                    let implements = hit.clone();
                    if let Some(key) = memo_key {
                        new_memo.insert(key, implements.clone());
                    }
                    implements
                } else if let Some(implements) =
                    bulk.get(&(entry.source_id.clone(), entry.relative_path.clone()))
                {
                    // Implements list came (already parsed) from the one-shot
                    // archive scan above.
                    let implements = implements.clone();
                    if let Some(key) = memo_key {
                        new_memo.insert(key, implements.clone());
                    }
                    implements
                } else {
                    // Resolve through `law_yaml`, NOT `LoadedLaw::yaml_content`:
                    // metadata-only bodies are fetched lazily (and cached in
                    // `body_cache` for subsequent opens).
                    match self.law_yaml(&entry.law_id).await {
                        Ok(Some(yaml)) => {
                            let implements = collect_law_implements(&yaml);
                            if let Some(key) = memo_key {
                                new_memo.insert(key, implements.clone());
                            }
                            implements
                        }
                        // A genuine miss (the file vanished from the branch
                        // between enumeration and now) has nothing to index
                        // and nothing to retry. A source without a usable
                        // backend is not this case — `law_yaml` errors for
                        // that, so it lands in `failed_law_ids` below.
                        Ok(None) => continue,
                        Err(e) => {
                            tracing::debug!(law_id = %entry.law_id, error = %e, "implements scan: body fetch failed");
                            failed_law_ids.push(entry.law_id);
                            continue;
                        }
                    }
                }
            };
            if !implements.is_empty() {
                implements_by_law.insert(entry.law_id, implements);
            }
        }
        if !failed_law_ids.is_empty() {
            tracing::warn!(
                failed = failed_law_ids.len(),
                indexed = implements_by_law.len(),
                "implements index built with fetch failures; implementor lists may be incomplete until the next snapshot"
            );
        }
        failed_law_ids.sort();

        *self.implements_memo.write().await = new_memo;
        Arc::new(ImplementsIndex {
            implements_by_law,
            failed_law_ids,
        })
    }

    /// Law ids that have been edited in this traject: the diff between the
    /// writable-own source's traject branch and its base branch, mapped
    /// back to law ids via the source map.
    ///
    /// This is the durable source of truth for "what changed in this
    /// traject" — every save commits to the branch, so the branch-vs-base
    /// diff survives process restarts and is shared across all members
    /// (unlike the in-memory `overlay`, which is only a read-your-writes
    /// cache). Returns an empty list when nothing has been saved yet (the
    /// traject branch doesn't exist → the Compare API 404s → empty) or the
    /// writable-own backend isn't initialised.
    ///
    /// Cached for [`CHANGED_LAWS_TTL`] (see `changed_cache`) so the library
    /// sidebar's repeated loads don't each spend a GitHub Compare call. On a
    /// fresh-compute failure (e.g. token throttled) a stale cached value, if
    /// any, is served rather than propagating the error — the sidebar keeps
    /// showing the last-known edit set instead of dropping the section.
    ///
    /// `token` is the acting user's GitHub token, used when the writable-own
    /// source has no server-side credential of its own (the private-repo
    /// traject route — see [`Self::changed_diff_needs_user_token`]). The
    /// *diff* is not user-specific: every member sees the same branch-vs-base
    /// set, so the cache stays per traject. An **empty result caused by the
    /// absence of a usable token** is user-specific though, so that one is
    /// never written to the shared cache — it would otherwise be served for a
    /// full TTL window to members who do have a link, blanking their
    /// "Bewerkt" section.
    ///
    /// That also splits the refresh in two. When the backend can diff on its
    /// own credential the expired entry is refreshed in the background
    /// (stale-while-revalidate, nobody waits for GitHub). When only a
    /// personal token gets in, the background leg has no credential to use,
    /// so the refresh happens on the request path of the first caller past
    /// the TTL who actually carries one; a caller without a token keeps
    /// being served the stale entry rather than blanking it.
    pub async fn changed_law_ids(
        self: &Arc<Self>,
        token: Option<&str>,
    ) -> Result<Vec<String>, regelrecht_corpus::error::CorpusError> {
        // Cached and fresh: serve it. Expired: decide who refreshes.
        let stale = match self.changed_cache.read().await.as_ref() {
            None => None,
            Some(cached) if cached.computed_at.elapsed() < CHANGED_LAWS_TTL => {
                return Ok(cached.law_ids.clone());
            }
            Some(cached) => Some(cached.law_ids.clone()),
        };

        // Deliberately *after* the fresh-cache return: this takes the backend
        // mutex (to probe `supports_token_override`), and that same lock is
        // contended by the law-body reads. The sidebar asks for this diff on
        // every library load, so the overwhelmingly common case is the hit
        // above — which has no need for the answer.
        let needs_user_token = self.changed_diff_needs_user_token().await;

        if let Some(stale_ids) = &stale {
            if !needs_user_token {
                // The backend has its own credential, so ONE background
                // recompute converges the entry while this request returns
                // immediately — convergence stays within one extra TTL
                // window and nobody waits out the Compare round-trip.
                if self
                    .changed_refresh_in_flight
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let corpus = self.clone();
                    tokio::spawn(async move {
                        // RAII reset (not a trailing `store`): a panic inside
                        // the recompute must not leave the flag latched true,
                        // or no request would ever recompute again and the
                        // stale ids would serve until process restart — the
                        // AtomicBool analogue of the snapshot refresh's
                        // OwnedMutexGuard.
                        let _reset = ResetOnDrop(corpus.changed_refresh_in_flight.clone());
                        corpus.recompute_changed_cache().await;
                    });
                }
                return Ok(stale_ids.clone());
            }
            if token.is_none() {
                // Only a personal token gets into this repo and this caller
                // has none: recomputing would yield an empty list that says
                // nothing about the branch. Keep serving the stale entry
                // until a linked member refreshes it.
                return Ok(stale_ids.clone());
            }
        }

        // Either nothing is cached (first call of the process — there is
        // nothing stale to serve instead) or the entry expired and only this
        // request's personal token can refresh it. Both are request-path
        // work, so single-flight it: concurrent callers queue on the gate and
        // the double-check below lets everyone but the first return the
        // winner's freshly-cached diff instead of firing their own Compare.
        // Bounded: the gate is held across a GitHub Compare call, and that
        // call has its own multi-second timeout. Waiting unboundedly would
        // turn a persistently failing upstream (rate limit, outage) into a
        // queue — the Nth concurrent member would wait N timeouts where
        // before this gate existed they each waited one, and the sidebar
        // blocks on this request. On timeout we simply do what the code did
        // before the gate: compute for ourselves. The herd-collapsing still
        // works in the normal case, where the winner returns in well under
        // this budget.
        let _gate = tokio::time::timeout(
            CHANGED_REFRESH_GATE_WAIT,
            self.changed_refresh_gate.clone().lock_owned(),
        )
        .await
        .ok();
        if let Some(cached) = self.changed_cache.read().await.as_ref() {
            if cached.computed_at.elapsed() < CHANGED_LAWS_TTL {
                return Ok(cached.law_ids.clone());
            }
        }

        match self.compute_changed_law_ids(token).await {
            Ok(ids) => {
                // Only a result that could actually see the repo is worth
                // sharing with the other members of this traject.
                //
                //
                // Known and accepted limitation: on the private-repo leg an
                // empty diff is ambiguous. It is what a branch without edits
                // returns, but ALSO what a member gets whose token cannot see
                // the repo - GitHub answers 404 (not 403) for an invisible
                // private repo and `compare_files` maps that to "no files
                // changed". Such a member can therefore blank "Bewerkt" for
                // the others for up to one TTL window, after which any member
                // who *can* see the repo overwrites it with the truth. That
                // self-healing bound is why this is tolerated rather than
                // patched here: refusing to cache empty results instead would
                // leave a merged-away diff standing forever for token-less
                // members, which is worse. The real fix is upstream - teach
                // `compare_files` to distinguish 404 from "nothing changed" -
                // and is tracked separately.
                if token.is_some() || !needs_user_token {
                    *self.changed_cache.write().await = Some(ChangedLawsCache {
                        computed_at: Instant::now(),
                        law_ids: ids.clone(),
                    });
                }
                Ok(ids)
            }
            // Same degrade-to-stale stance as the background leg: a
            // throttled upstream must not drop the sidebar's section, and
            // re-arming the clock stops every subsequent request from
            // re-attempting against it.
            Err(e) => match stale {
                Some(stale_ids) => {
                    tracing::warn!(
                        error = %e,
                        "changed-laws refresh failed; serving stale cached value for another TTL"
                    );
                    if let Some(cached) = self.changed_cache.write().await.as_mut() {
                        cached.computed_at = Instant::now();
                    }
                    Ok(stale_ids)
                }
                None => Err(e),
            },
        }
    }

    /// Whether computing the branch-vs-base diff depends on the *caller's*
    /// GitHub token — i.e. the writable-own backend authenticates per call
    /// and has no credential at rest.
    ///
    /// This deliberately stops at the *capability* probe and does NOT mirror
    /// `TrajectCredentials::for_read` all the way down: that path additionally
    /// consults `write_requires_user_token` (config + the `github.user_oauth`
    /// flag) and hands back `None` when the user-token mode is off. Answering
    /// that question here would mean a DB round-trip on a hot path, and it
    /// would buy nothing: this predicate only ever guards *caching and
    /// background-refresh* decisions, and a token-less backend
    /// (`is_writable() == self.token.is_some()`) short-circuits the diff to an
    /// empty list without touching GitHub anyway. With the user-token mode off
    /// the effect is therefore a cheap empty recompute per request rather than
    /// a shared cache entry — never a wrong answer.
    ///
    /// `false` for everything else: a backend with a service token, a local
    /// backend, or a missing backend all produce a token-independent result
    /// that is safe to share across members and to recompute in the
    /// background.
    async fn changed_diff_needs_user_token(&self) -> bool {
        let Some(entry) = self.corpus.backends.get(&self.writable_own_source_id) else {
            return false;
        };
        if entry.writable {
            return false;
        }
        entry.backend.lock().await.supports_token_override()
    }

    /// The background leg of [`changed_law_ids`]'s stale-while-revalidate:
    /// recompute the diff and swap it in. On failure the *stale* entry is
    /// re-armed for another TTL window (matching the snapshot refresh's
    /// degrade-to-stale stance) so a throttled upstream isn't re-hit by
    /// every subsequent request.
    ///
    /// Known bounded race: the wholesale swap can undo a concurrent
    /// [`Self::record_changed_law`] fold — a save that commits while the
    /// Compare call is in flight may briefly drop out of the diff (the
    /// Compare predates the save's commit, and the path→id mapping runs
    /// through this possibly-older snapshot's `source_map`). Self-heals
    /// at the next recompute, ≤ one [`CHANGED_LAWS_TTL`] window; the
    /// pre-existing invalidate-based code had the same window around a
    /// save.
    ///
    /// Runs outside a request, so it has no user token — only started when
    /// the backend can diff on its own credential (see
    /// [`Self::changed_law_ids`]).
    async fn recompute_changed_cache(&self) {
        match self.compute_changed_law_ids(None).await {
            Ok(ids) => {
                *self.changed_cache.write().await = Some(ChangedLawsCache {
                    computed_at: Instant::now(),
                    law_ids: ids,
                });
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "changed-laws recompute failed; serving stale cached value for another TTL"
                );
                if let Some(cached) = self.changed_cache.write().await.as_mut() {
                    cached.computed_at = Instant::now();
                }
            }
        }
    }

    /// Fold a just-saved law into the cached changed-laws diff so the
    /// saving user sees their edit in the sidebar immediately
    /// (read-your-writes, the changed-laws analogue of [`record_save`]) —
    /// without the synchronous GitHub Compare call that invalidating the
    /// cache used to cost the next sidebar load. The entry's TTL clock is
    /// deliberately NOT reset: the next recompute reconciles against the
    /// real branch diff on schedule (covering e.g. a save that reverts a
    /// law back to its base content, which should eventually disappear
    /// from the list).
    pub async fn record_changed_law(&self, law_id: &str) {
        if let Some(cached) = self.changed_cache.write().await.as_mut() {
            if !cached.law_ids.iter().any(|id| id == law_id) {
                cached.law_ids.push(law_id.to_string());
                cached.law_ids.sort();
            }
        }
        // No cache entry yet: leave it absent — the next changed-laws
        // request computes the real diff (which includes this save).
    }

    /// Uncached computation behind [`changed_law_ids`]: ask the writable-own
    /// backend for its branch-vs-base diff — authenticating with `token` when
    /// the backend has no credential of its own — and map the changed paths
    /// to law ids via the source map.
    async fn compute_changed_law_ids(
        &self,
        token: Option<&str>,
    ) -> Result<Vec<String>, regelrecht_corpus::error::CorpusError> {
        let Some(entry) = self.corpus.backends.get(&self.writable_own_source_id) else {
            return Ok(Vec::new());
        };
        let changed = {
            let backend = entry.backend.lock().await;
            backend.changed_files_with_token(token).await?
        };
        if changed.is_empty() {
            return Ok(Vec::new());
        }
        // Match changed source-relative paths against loaded laws. Normalise
        // separators so a relative_path computed on a non-Unix host still
        // matches the forward-slash paths GitHub returns.
        let changed: std::collections::HashSet<String> =
            changed.into_iter().map(|p| p.replace('\\', "/")).collect();
        let mut ids: Vec<String> = self
            .corpus
            .source_map
            .laws()
            .filter(|law| changed.contains(&law.relative_path.replace('\\', "/")))
            .map(|law| law.law_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

/// A built traject corpus plus the instant its index snapshot was
/// (re)built, for TTL expiry.
struct CachedCorpus {
    corpus: Arc<TrajectCorpus>,
    built_at: Instant,
}

impl CachedCorpus {
    fn is_fresh(&self, ttl: Duration) -> bool {
        self.built_at.elapsed() < ttl
    }
}

/// Per-traject cache slot. `state` holds the current snapshot (None until
/// first build); `build_lock` single-flights both the initial build and
/// TTL refreshes so concurrent first-touches share one clone and a
/// refresh herd collapses to one source enumeration. Behind an `Arc` so
/// the background refresh task can carry its `OwnedMutexGuard` across
/// `tokio::spawn`.
#[derive(Default)]
struct TrajectSlot {
    state: RwLock<Option<CachedCorpus>>,
    build_lock: Arc<Mutex<()>>,
    /// Consecutive failed TTL refreshes. Serving stale stays correct
    /// indefinitely, but a permanently broken source would otherwise
    /// re-warn every TTL window — the count rate-limits the warn to the
    /// first failure and every [`FAILED_REFRESH_WARN_EVERY`]th after
    /// that (the rest log at debug). Reset on any successful
    /// build/refresh.
    refresh_failures: AtomicU32,
}

/// Every how many consecutive refresh failures the full warn is repeated
/// (between repeats the failure logs at debug). At the 60s TTL this is
/// one warn per ~10 minutes for a permanently broken source.
const FAILED_REFRESH_WARN_EVERY: u32 = 10;

/// Lazy registry of per-traject corpora, mirroring the
/// `CorpusState`-per-traject design. Concurrent first-touches on the same
/// traject share one build; first-touches on *different* trajects never
/// block each other. A built snapshot is served for `index_ttl` and then
/// refreshed in place (see [`Self::get_or_build`]).
pub struct TrajectCorpusCache {
    cells: RwLock<HashMap<Uuid, Arc<TrajectSlot>>>,
    /// How long an index snapshot is served before it is refreshed.
    /// [`TRAJECT_INDEX_TTL`] in production; tests inject shorter values
    /// via [`Self::with_index_ttl`].
    index_ttl: Duration,
}

impl Default for TrajectCorpusCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TrajectCorpusCache {
    pub fn new() -> Self {
        Self::with_index_ttl(TRAJECT_INDEX_TTL)
    }

    /// Cache with a caller-chosen snapshot TTL — the injection point for
    /// tests that need to force (or rule out) a refresh without waiting
    /// out the production TTL.
    pub fn with_index_ttl(index_ttl: Duration) -> Self {
        Self {
            cells: RwLock::new(HashMap::new()),
            index_ttl,
        }
    }

    /// Get-or-build the corpus state for a traject.
    ///
    /// On a cache miss the slow path queries `traject_corpus_sources`,
    /// instantiates a backend per source (cloning when needed via
    /// `ensure_ready`), and stitches them into a [`CorpusState`].
    ///
    /// When the cached snapshot is older than the index TTL, the first
    /// request past the deadline kicks off a *background* refresh (see
    /// [`refresh_traject_corpus`] for what carries over) and — like every
    /// concurrent request while that refresh is in flight — keeps being
    /// served the stale snapshot. No request ever waits out the source
    /// re-enumeration (stale-while-revalidate); only the very first build
    /// of a traject blocks, because there is nothing to serve yet. A
    /// failed refresh extends the stale snapshot for another TTL window
    /// instead of erroring reads (same degrade-to-stale stance as
    /// [`TrajectCorpus::changed_law_ids`]).
    ///
    /// `own_scan_token` is the acting user's linked GitHub token, used
    /// **transiently** as a fallback for the index scan of the traject's
    /// writable-own source when the server has no token for it (the scan
    /// mirror of the request-bound read fallback). It is never stored on
    /// the cached snapshot or in any key — only the *result* of a scan is
    /// shared, which is acceptable because the law metadata it produces is
    /// visible to every traject member anyway. One extra rule: a cached
    /// snapshot whose writable-own scan **failed without** a user token is
    /// rebuilt synchronously when a request finally carries one — serving
    /// the broken snapshot to that request would fail it loudly for no
    /// reason when the user's own access can heal the index right now.
    pub async fn get_or_build(
        &self,
        pool: &PgPool,
        traject_id: Uuid,
        auth_file: Option<PathBuf>,
        own_scan_token: Option<&str>,
    ) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
        // A snapshot whose writable-own scan failed while no user token was
        // available is worth one immediate retry now that this request
        // carries a token. (A failure scanned WITH a token is served as-is
        // — retrying it per request would hammer GitHub with scans that
        // keep failing; the TTL refresh retries on schedule.)
        let heals_own_index = |corpus: &TrajectCorpus| {
            own_scan_token.is_some()
                && corpus.own_index_error().is_some()
                && !corpus.own_index_scanned_with_user_token
        };

        let slot = {
            let mut map = self.cells.write().await;
            map.entry(traject_id)
                .or_insert_with(|| Arc::new(TrajectSlot::default()))
                .clone()
        };

        // Fast path: a fresh snapshot serves without touching the build
        // lock. A stale snapshot is served just as fast — expiry only
        // decides whether a background refresh is kicked off first.
        {
            let state = slot.state.read().await;
            match state.as_ref() {
                Some(cached) if heals_own_index(&cached.corpus) => {
                    // Fall through to the synchronous build below: this
                    // request's token can repair the broken index.
                }
                Some(cached) if cached.is_fresh(self.index_ttl) => {
                    tracing::debug!(traject = %traject_id, cache = "hit", "traject corpus served from cache");
                    return Ok(cached.corpus.clone());
                }
                Some(cached) => {
                    let stale = cached.corpus.clone();
                    drop(state);
                    // Stale: only one task refreshes (`try_lock`); everyone
                    // else — including the winner — serves stale rather than
                    // queueing up behind a network round-trip. The refresh
                    // borrows this request's scan token (owned copy for the
                    // spawned task; dropped with it).
                    if let Ok(guard) = slot.build_lock.clone().try_lock_owned() {
                        spawn_background_refresh(
                            guard,
                            slot.clone(),
                            stale.clone(),
                            traject_id,
                            self.index_ttl,
                            own_scan_token.map(str::to_string),
                        );
                    }
                    return Ok(stale);
                }
                None => {}
            }
        }

        // Nothing cached (or a broken-own-index snapshot this request's
        // token can heal): every caller must wait for the one build.
        let _build = slot.build_lock.clone().lock_owned().await;

        // Re-check under the build lock: the previous holder may have
        // built while we waited. A snapshot this request's token could
        // still heal is NOT served — it is exactly what we came to rebuild.
        if let Some(cached) = slot.state.read().await.as_ref() {
            if !heals_own_index(&cached.corpus) {
                tracing::debug!(traject = %traject_id, cache = "hit", "traject corpus served from cache");
                return Ok(cached.corpus.clone());
            }
        }

        tracing::debug!(traject = %traject_id, cache = "cold", "building traject corpus (cold)");
        let corpus = timing::measure(
            "build",
            build_traject_corpus(pool, traject_id, auth_file.as_deref(), own_scan_token),
        )
        .await?;

        slot.refresh_failures.store(0, Ordering::SeqCst);
        *slot.state.write().await = Some(CachedCorpus {
            corpus: corpus.clone(),
            built_at: Instant::now(),
        });
        Ok(corpus)
    }

    /// Drop the cached entry for a traject so the next request rebuilds.
    /// Used after the traject's sources change so stale clones aren't
    /// served further.
    pub async fn invalidate(&self, traject_id: Uuid) {
        self.cells.write().await.remove(&traject_id);
    }
}

/// The background leg of [`TrajectCorpusCache::get_or_build`]'s
/// stale-while-revalidate: refresh the index snapshot off the request
/// path and swap it into the slot. `guard` is the slot's build lock,
/// won by `try_lock` on the request path and held until the swap so a
/// concurrent expiry cannot start a second enumeration.
///
/// On failure the stale snapshot is re-armed for another TTL window —
/// the re-armed `built_at` stops every subsequent request from
/// re-attempting against a throttled upstream. The warn is rate-limited
/// via `refresh_failures`: a permanently broken source fails every TTL
/// window, and repeating the same warn each minute drowns the log
/// without new signal.
///
/// Races it tolerates: a [`TrajectCorpusCache::invalidate`] between
/// spawn and swap orphans the slot (it's no longer in `cells`), so the
/// swap writes into state nobody reads again — the next request builds
/// cold, which is exactly what invalidation asks for.
fn spawn_background_refresh(
    guard: tokio::sync::OwnedMutexGuard<()>,
    slot: Arc<TrajectSlot>,
    old: Arc<TrajectCorpus>,
    traject_id: Uuid,
    index_ttl: Duration,
    own_scan_token: Option<String>,
) {
    tokio::spawn(async move {
        let _guard = guard;
        // Between losing the state read lock and winning the build lock a
        // previous holder may already have refreshed — don't enumerate the
        // sources twice within one TTL window.
        if let Some(cached) = slot.state.read().await.as_ref() {
            if cached.is_fresh(index_ttl) {
                return;
            }
        }

        let span = tracing::info_span!("traject_index_refresh", traject = %traject_id);
        let outcome = tracing::Instrument::instrument(
            refresh_traject_corpus(&old, traject_id, own_scan_token.as_deref()),
            span,
        )
        .await;
        // `own_scan_token` dies with this task — the refreshed snapshot
        // only ever carries the scan *result*.
        apply_refresh_outcome(&slot, old, traject_id, outcome).await;
    });
}

/// Swap a refresh outcome into the slot: a fresh snapshot on success, the
/// re-armed stale one on failure. Factored out of
/// [`spawn_background_refresh`] so the failure stance is unit-testable
/// without needing a source that fails to enumerate.
async fn apply_refresh_outcome(
    slot: &TrajectSlot,
    old: Arc<TrajectCorpus>,
    traject_id: Uuid,
    outcome: Result<Arc<TrajectCorpus>, TrajectCorpusError>,
) {
    match outcome {
        Ok(refreshed) => {
            slot.refresh_failures.store(0, Ordering::SeqCst);
            *slot.state.write().await = Some(CachedCorpus {
                corpus: refreshed,
                built_at: Instant::now(),
            });
        }
        Err(e) => {
            let failures = slot.refresh_failures.fetch_add(1, Ordering::SeqCst) + 1;
            if failures == 1 || failures.is_multiple_of(FAILED_REFRESH_WARN_EVERY) {
                tracing::warn!(
                    traject = %traject_id,
                    error = %e,
                    consecutive_failures = failures,
                    "traject index refresh failed; serving stale snapshot for another TTL"
                );
            } else {
                tracing::debug!(
                    traject = %traject_id,
                    error = %e,
                    consecutive_failures = failures,
                    "traject index refresh failed again; serving stale snapshot for another TTL"
                );
            }
            *slot.state.write().await = Some(CachedCorpus {
                corpus: old,
                built_at: Instant::now(),
            });
        }
    }
}

#[derive(Debug)]
pub enum TrajectCorpusError {
    NotFound,
    NoWritableOwn,
    Db(sqlx::Error),
    Corpus(regelrecht_corpus::error::CorpusError),
}

impl std::fmt::Display for TrajectCorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "traject not found or has no sources"),
            Self::NoWritableOwn => write!(f, "traject has no writable-own source configured"),
            Self::Db(e) => write!(f, "database error: {e}"),
            Self::Corpus(e) => write!(f, "corpus error: {e}"),
        }
    }
}

impl std::error::Error for TrajectCorpusError {}

impl From<sqlx::Error> for TrajectCorpusError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

impl From<regelrecht_corpus::error::CorpusError> for TrajectCorpusError {
    fn from(e: regelrecht_corpus::error::CorpusError) -> Self {
        Self::Corpus(e)
    }
}

/// Build a fresh [`TrajectCorpus`]: load sources from DB, clone backends,
/// produce a [`SourceMap`].
///
/// `own_scan_token`: per-call user-token fallback for the index scan of
/// the writable-own source (see [`TrajectCorpusCache::get_or_build`]).
/// Consumed by the enumeration only — it is not handed to the backends
/// (request-bound reads/writes resolve their own per-request token) and
/// never stored on the returned structure.
#[tracing::instrument(name = "build_traject_corpus", skip_all, fields(traject = %traject_id))]
async fn build_traject_corpus(
    pool: &PgPool,
    traject_id: Uuid,
    auth_file: Option<&std::path::Path>,
    own_scan_token: Option<&str>,
) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
    let rows = sqlx::query_as::<_, TrajectSourceRow>(
        "SELECT source_id, name, source_type::text AS source_type,
                gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path, gh_ref,
                local_path, priority, auth_ref, scopes, is_writable_own
         FROM traject_corpus_sources
         WHERE traject_id = $1
         ORDER BY priority",
    )
    .bind(traject_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Err(TrajectCorpusError::NotFound);
    }

    // Confirm the traject has a writable_own source — without one we
    // can't route saves on laws read from the seed sources. The actual
    // routing happens via `write_target_for_source` below; the local
    // here is just the guard against a misconfigured traject.
    let writable_own_source_id = rows
        .iter()
        .find(|r| r.is_writable_own)
        .map(|r| r.source_id.clone())
        .ok_or(TrajectCorpusError::NoWritableOwn)?;

    // Traject-flow-specific, so it never made it into `Source`: the branch
    // the traject branch is cut from. Kept on the snapshot so a failed
    // index scan can be diagnosed without a second DB round trip.
    let own_base_branch = rows
        .iter()
        .find(|r| r.is_writable_own)
        .and_then(|r| r.gh_base_branch.clone());

    // Build the read-source → write-target-source map. Every non-
    // writable_own source (local seed, GitHub seed, …) is routed to the
    // writable_own's backend so a save on any read-only seed-loaded law
    // lands on the traject's branch. The earlier `auth_ref`-only
    // mapping only fired when the writable_own's auth_ref matched a
    // seed's source_id verbatim, which broke for preview/local-stack
    // deploys where the seed is `local` but auth_ref still resolves a
    // GitHub token — saves on those laws then silently fell back to
    // the local backend and never reached the traject branch.
    let mut write_target_for_source: HashMap<String, String> = HashMap::new();
    for row in &rows {
        if !row.is_writable_own {
            write_target_for_source.insert(row.source_id.clone(), writable_own_source_id.clone());
        }
    }

    let sources: Vec<Source> = rows.iter().map(|r| r.to_source()).collect();
    let registry = CorpusRegistry::from_sources(sources.clone());

    // Build a backend per source, scoped to a traject-specific clone path.
    let mut backends: HashMap<String, BackendEntry> = HashMap::new();
    for (row, source) in rows.iter().zip(sources.iter()) {
        // Token resolution honours `Source::strict_auth` (set by
        // `to_source` for the writable-own row): the writable-own resolves
        // strictly (no legacy `CORPUS_GIT_TOKEN` fallback), because its
        // `auth_ref` was derived from the create-request's repo coords and a
        // missing per-repo token MUST fail closed — not transparently ship
        // the central token to a user-chosen GitHub repo on the next push.
        // Seeded (non-writable) sources keep the legacy fallback so
        // pre-existing deployments that rely on a single global PAT for
        // read-only mirrors keep working. The index scan below
        // (`index_all_sources_async`) goes through the same resolver, so
        // scan and push can no longer disagree about the token.
        let token_result = regelrecht_corpus::auth::CredentialResolver::new(auth_file)
            .resolve_source(source)
            .map(regelrecht_corpus::auth::TokenDecision::into_token);
        let token = token_result.unwrap_or_else(|e| {
            tracing::warn!(
                traject = %traject_id,
                source_id = %source.id,
                error = %e,
                "failed to resolve auth token for traject source"
            );
            None
        });
        // Diagnostic: token=None on the writable-own source means git
        // push will hit "could not read Username" later, and — for a
        // private repo — the index scan and every lazy body read will 404
        // just as hard, silently resolving the traject's own laws from the
        // seed corpus instead. Surface it now with both the source_id and
        // the resolved auth_ref so an operator can see whether the row
        // carries the expected ref and whether the env var matches.
        if token.is_none() && source.id == writable_own_source_id {
            let expected_env = regelrecht_corpus::auth::token_env_name(
                source.auth_ref.as_deref().unwrap_or(&source.id),
            );
            tracing::error!(
                traject = %traject_id,
                source_id = %source.id,
                auth_ref = ?source.auth_ref,
                auth_file = ?auth_file,
                expected_env = %expected_env,
                user_scan_token = own_scan_token.is_some(),
                "traject writable-own source resolved NO server token — server-side pushes \
                 fail, and on a private repo reads/index scans only work through the acting \
                 user's linked GitHub token (user_scan_token says whether this build has one)"
            );
        }

        let is_writable_own = source.id == writable_own_source_id;

        // The writable-own GitHub source goes through
        // `build_traject_github_backend` because it needs a base branch to
        // create its traject branch from; read-only base/seed GitHub
        // sources (minbzk-central, …) go through `create_backend`, which
        // builds the same Contents-API backend without one. Neither clones:
        // the implements scan that used to justify a local checkout now
        // reads the precomputed index out of the corpus repo, and body
        // reads ride the Contents API with conditional GETs (a 304 costs no
        // rate-limit quota). Local sources keep their configured path.
        let backend_result = match &source.source_type {
            SourceType::GitHub { github } if is_writable_own => build_traject_github_backend(
                traject_id,
                source,
                github,
                row.gh_base_branch.as_deref(),
                token.as_deref(),
            ),
            SourceType::GitHub { .. } | SourceType::Local { .. } => {
                create_backend(source, token.as_deref())
            }
        };
        match backend_result {
            Ok(mut backend) => {
                if let Err(e) = backend.ensure_ready().await {
                    if is_writable_own {
                        // The traject's whole purpose is to push edits to
                        // this branch; falling through with a missing
                        // writable backend would make every save 503 with
                        // no signal that the underlying clone failed.
                        tracing::error!(
                            traject = %traject_id,
                            source_id = %source.id,
                            error = %e,
                            "traject writable-own backend init failed"
                        );
                        return Err(TrajectCorpusError::Corpus(e));
                    }
                    tracing::warn!(
                        traject = %traject_id,
                        source_id = %source.id,
                        error = %e,
                        "traject backend init failed, skipping"
                    );
                    continue;
                }
                let writable = backend.is_writable();
                backends.insert(
                    source.id.clone(),
                    BackendEntry {
                        backend: Arc::new(Mutex::new(backend)),
                        writable,
                    },
                );
            }
            Err(e) => {
                if is_writable_own {
                    tracing::error!(
                        traject = %traject_id,
                        source_id = %source.id,
                        error = %e,
                        "failed to create traject writable-own backend"
                    );
                    return Err(TrajectCorpusError::Corpus(e));
                }
                tracing::warn!(
                    traject = %traject_id,
                    source_id = %source.id,
                    error = %e,
                    "failed to create traject backend"
                );
            }
        }
    }

    // Build a lightweight INDEX of every law across the traject's sources —
    // the private writable-own repo and the seeded central corpus — so the
    // bibliotheek search can surface any law without fetching every law's body
    // up front (which meant N per-file GitHub Contents API calls per traject
    // build — slow and rate-limited). Bodies are fetched lazily on first read
    // (see `TrajectCorpus::law_yaml`). Per-source index failures are tolerated
    // inside `index_all_sources_async` (a bad seed source is skipped, not
    // fatal). A failure to enumerate the writable-own source is the one we care
    // about most — its laws are the point of the traject — so it's logged at
    // error level for operators. The traject still opens, though: it degrades
    // (those laws missing from search until the next rebuild) rather than
    // failing entirely on what is usually a transient GitHub hiccup. The hard
    // failure path is the writable-own *backend* init above, which returns
    // `Err` so a broken write target never opens silently.
    // The scan may fall back to the acting user's token for the
    // writable-own source — and only for that source, only when the
    // server-side resolution yields no token (both enforced corpus-side
    // by `ScanTokenOverride`).
    let scan_override = own_scan_token.map(|token| regelrecht_corpus::ScanTokenOverride {
        source_id: &writable_own_source_id,
        token,
    });
    let today = regelrecht_shared::dates::today_str();
    let (source_map, index_failures) = match timing::measure(
        "index",
        registry.index_all_sources_with_override(auth_file, scan_override, &today),
    )
    .await
    {
        Ok((map, failed)) => {
            if let Some(f) = failed
                .iter()
                .find(|f| f.source_id == writable_own_source_id)
            {
                tracing::error!(
                    traject = %traject_id,
                    source_id = %writable_own_source_id,
                    error = %f.error,
                    "traject writable-own source failed to index — the traject-scoped \
                     library refuses to serve (no silent fallback to seed-only laws) \
                     until a scan succeeds"
                );
            }
            let failures: HashMap<String, String> =
                failed.into_iter().map(|f| (f.source_id, f.error)).collect();
            (map, failures)
        }
        Err(e) => {
            tracing::warn!(
                traject = %traject_id,
                error = %e,
                "traject corpus load failed, falling back to local-only"
            );
            // Local-only fallback: every non-local source is absent from
            // the map, so record the enumeration error against each of
            // them — `/sources` then shows why their law_count is 0.
            let failures: HashMap<String, String> = registry
                .sources()
                .iter()
                .filter(|s| matches!(s.source_type, SourceType::GitHub { .. }))
                .map(|s| (s.id.clone(), e.to_string()))
                .collect();
            (
                registry
                    .load_local_sources(&today)
                    .unwrap_or_else(|_| SourceMap::new(regelrecht_shared::dates::today_str())),
                failures,
            )
        }
    };

    Ok(Arc::new(TrajectCorpus {
        corpus: CorpusState {
            registry,
            source_map,
            backends,
            auth_file: auth_file.map(|p| p.to_path_buf()),
            index_failures,
            lazy_bodies: Default::default(),
        },
        traject_id,
        own_base_branch,
        write_target_for_source,
        writable_own_source_id,
        overlay: Arc::new(RwLock::new(HashMap::new())),
        body_cache: RwLock::new(BoundedCache::default()),
        implements_index: RwLock::new(None),
        implements_index_stale: AtomicBool::new(false),
        implements_build_lock: Mutex::new(()),
        implements_memo: Arc::new(RwLock::new(HashMap::new())),
        changed_cache: Arc::new(RwLock::new(None)),
        changed_refresh_in_flight: Arc::new(AtomicBool::new(false)),
        changed_refresh_gate: Arc::new(Mutex::new(())),
        sidecar_cache: RwLock::new(BoundedCache::default()),
        scenario_list_cache: RwLock::new(BoundedCache::default()),
        sidecar_write_gen: AtomicU64::new(0),
        scenario_list_write_gen: AtomicU64::new(0),
        // Only the *fact* that a user token was available; the token
        // itself dies with this call.
        own_index_scanned_with_user_token: own_scan_token.is_some(),
    }))
}

/// Build a TTL-refreshed [`TrajectCorpus`] from an existing one: a fresh
/// index snapshot over the *same* sources and backends.
///
/// Carried over from `old` (so a refresh can never break in-flight save
/// semantics):
/// - the **backends map** — the same `Arc<Mutex<…>>` instances, so a save
///   holding a backend mutex across the refresh keeps excluding writers
///   and the writable-own → seed lock-ordering invariant is unaffected;
/// - the **post-save `overlay`** (shared `Arc`) — refreshed reads keep
///   seeing saved bodies (never resurrect pre-save content), and a save
///   that lands on the pre-refresh instance is visible post-refresh. The
///   entries are *reconciled* against the write-target branch first (see
///   [`reconcile_overlay`]) so a save overwritten externally stops being
///   pinned by this process;
/// - the **changed-laws cache** (shared `Arc`) — same reasoning for
///   `save_law`'s invalidation;
/// - the **implements index** (flagged stale) and the content-addressed
///   **implements memo** (shared `Arc`) — lookups keep serving the
///   previous index while the first post-refresh lookup rebuilds it, and
///   the rebuild only fetches bodies whose blob sha changed;
/// - the **write routing** (`write_target_for_source`,
///   `writable_own_source_id`) and the registry/auth config, which only
///   change through traject create/delete → [`TrajectCorpusCache::invalidate`].
///
/// Fresh in the new instance:
/// - the **`source_map` index snapshot** — the point of the refresh;
/// - the **`body_cache`** — so upstream body changes become visible.
///
/// Any source failing to enumerate fails the whole refresh: the caller
/// then serves the previous (complete) snapshot for another TTL, which
/// strictly beats swapping in a snapshot with thousands of laws missing.
async fn refresh_traject_corpus(
    old: &Arc<TrajectCorpus>,
    traject_id: Uuid,
    own_scan_token: Option<&str>,
) -> Result<Arc<TrajectCorpus>, TrajectCorpusError> {
    let registry = old.corpus.registry.clone();
    let auth_file = old.corpus.auth_file.clone();
    // Same user-token fallback as the cold build: without it, a traject
    // whose writable-own repo is only readable through the acting user's
    // token could never refresh its index (every TTL refresh would fail
    // and re-arm the stale snapshot forever). Transient, like everywhere
    // else — the refreshed snapshot never carries the token.
    let scan_override = own_scan_token.map(|token| regelrecht_corpus::ScanTokenOverride {
        source_id: &old.writable_own_source_id,
        token,
    });
    let (source_map, failed) = registry
        .index_all_sources_with_override(
            auth_file.as_deref(),
            scan_override,
            &regelrecht_shared::dates::today_str(),
        )
        .await?;
    if !failed.is_empty() {
        let details: Vec<String> = failed
            .iter()
            .map(|f| format!("{}: {}", f.source_id, f.error))
            .collect();
        return Err(TrajectCorpusError::Corpus(
            regelrecht_corpus::error::CorpusError::Config(format!(
                "index refresh for traject {traject_id} failed to enumerate sources: {details:?}"
            )),
        ));
    }

    // Only after a *successful* enumeration: a failed refresh must not
    // touch the overlay either.
    reconcile_overlay(old, &source_map).await;

    Ok(next_snapshot(old, source_map).await)
}

/// Assemble the next-snapshot [`TrajectCorpus`] over `source_map`,
/// carrying over everything [`refresh_traject_corpus`] documents.
/// Factored out of the refresh so tests can drive the carry-over
/// semantics with a hand-built source map (no registry enumeration).
async fn next_snapshot(old: &Arc<TrajectCorpus>, source_map: SourceMap) -> Arc<TrajectCorpus> {
    // Carry the previous implements index (even one this snapshot itself
    // inherited and never rebuilt) so implementor lookups keep being
    // served during the post-refresh rebuild.
    let carried_index = old.implements_index.read().await.clone();
    Arc::new(TrajectCorpus {
        corpus: CorpusState {
            registry: old.corpus.registry.clone(),
            source_map,
            backends: old.corpus.backends.clone(),
            auth_file: old.corpus.auth_file.clone(),
            // A refresh only lands when every source enumerated (see
            // `refresh_traject_corpus`), so the refreshed snapshot has no
            // scan failures to report.
            index_failures: HashMap::new(),
            lazy_bodies: Default::default(),
        },
        traject_id: old.traject_id,
        own_base_branch: old.own_base_branch.clone(),
        write_target_for_source: old.write_target_for_source.clone(),
        writable_own_source_id: old.writable_own_source_id.clone(),
        overlay: old.overlay.clone(),
        body_cache: RwLock::new(BoundedCache::default()),
        implements_index: RwLock::new(carried_index),
        implements_index_stale: AtomicBool::new(true),
        implements_build_lock: Mutex::new(()),
        implements_memo: old.implements_memo.clone(),
        changed_cache: old.changed_cache.clone(),
        changed_refresh_in_flight: old.changed_refresh_in_flight.clone(),
        changed_refresh_gate: old.changed_refresh_gate.clone(),
        sidecar_cache: RwLock::new(BoundedCache::default()),
        scenario_list_cache: RwLock::new(BoundedCache::default()),
        sidecar_write_gen: AtomicU64::new(0),
        scenario_list_write_gen: AtomicU64::new(0),
        // A refresh only lands when every source enumerated, so this
        // snapshot has no own-index failure the flag could qualify;
        // carrying the old value keeps the field truthful about how the
        // *last* failed scan (if the caller re-arms `old`) was attempted.
        own_index_scanned_with_user_token: old.own_index_scanned_with_user_token,
    })
}

/// Reconcile the read-your-writes overlay against the write-target
/// branch at refresh time: for every overlaid law, read the file the
/// save wrote (the writable-own branch for federated laws, the law's own
/// backend otherwise) and DROP the entry when the branch content no
/// longer equals the saved body — someone else (another replica, a
/// direct push) wrote after us, and serving their newer content restores
/// convergence. Without this, a law once saved on this process would
/// serve this process's body forever and every cross-replica save would
/// 412 against content the loser can never see.
///
/// Cost: O(locally saved laws), one backend read per entry, every TTL
/// window. Each backend lock is taken per entry — never across the loop
/// — and only the write-target backend is locked (no seed lock), so the
/// writable-own → seed lock-order invariant cannot be violated and an
/// in-flight save is never starved.
///
/// Failure stance: a read error keeps the entry (can't tell whether the
/// branch moved — keeping read-your-writes beats dropping a save on a
/// network blip). The removal itself is compare-and-remove: an entry
/// replaced by a concurrent `record_save` after this pass snapshotted it
/// is left alone.
///
/// Note the drop also fires on a *version rollover*, not just a direct
/// push: the path comes from the new `source_map`, which already picked
/// `best_per_law`. If a corpus update publishes a newer valid-from date
/// file for the same `law_id` (e.g. `2026-01-01.yaml` superseding the
/// user's `2025-01-01.yaml` save), the branch read returns the new
/// version, it differs from the saved body, and the overlay entry is
/// dropped — the newer version correctly supersedes the pinned save.
async fn reconcile_overlay(old: &Arc<TrajectCorpus>, source_map: &SourceMap) {
    let entries: Vec<(String, String)> = old
        .overlay
        .read()
        .await
        .iter()
        .map(|(law_id, body)| (law_id.clone(), body.clone()))
        .collect();

    for (law_id, overlay_body) in entries {
        // Resolve the write-target file in the NEW snapshot. If the law is
        // absent from this snapshot we KEEP the overlay entry rather than
        // dropping it: absence is ambiguous. A just-saved law can be missing
        // from a snapshot whose source enumeration ran between our branch
        // push and `record_save` returning (the GitHub Trees listing
        // predates the push) — dropping here would break read-your-writes for
        // a TTL window even though the save is safely on the branch. This
        // matches the read-error failure stance below: when we cannot read
        // the branch to compare, keep serving our save. A law genuinely
        // removed from the branch keeps its overlay entry until a later save
        // overwrites it — the read-your-writes trade-off this function makes.
        let branch_body = match source_map.get_law(&law_id) {
            None => continue,
            Some(law) => {
                let write_source_id = old
                    .write_target_for_source
                    .get(&law.source_id)
                    .cloned()
                    .unwrap_or_else(|| law.source_id.clone());
                let Some(entry) = old.corpus.backends.get(&write_source_id) else {
                    // Write backend gone (shouldn't happen outside config
                    // churn): nothing to compare against, keep the entry.
                    continue;
                };
                let read = {
                    let backend = entry.backend.lock().await;
                    backend
                        .read_file(std::path::Path::new(&law.relative_path))
                        .await
                };
                match read {
                    Ok(Some(body)) => body,
                    Ok(None) => {
                        // Trees enumeration found the law but the Contents
                        // read says it's gone — a TOCTOU gap symmetric with
                        // the Err arm below. We cannot tell a genuine delete
                        // from transient propagation lag, so keep the save
                        // (same "can't tell → keep" stance).
                        tracing::debug!(
                            law_id = %law_id,
                            "overlay reconcile: file not found on branch; keeping overlay entry"
                        );
                        continue;
                    }
                    Err(e) => {
                        tracing::debug!(
                            law_id = %law_id,
                            error = %e,
                            "overlay reconcile: branch read failed; keeping overlay entry"
                        );
                        continue;
                    }
                }
            }
        };

        if branch_body == overlay_body {
            // Our save is still what the branch holds — keep serving it.
            continue;
        }
        let mut overlay = old.overlay.write().await;
        if overlay.get(&law_id).map(String::as_str) == Some(overlay_body.as_str()) {
            overlay.remove(&law_id);
            tracing::info!(
                law_id = %law_id,
                "overlay entry superseded by branch content; dropping so reads converge"
            );
        }
    }
}

/// Build a [`GitHubApiBackend`] for a traject source — no clone, no
/// `/tmp` working tree. Reads, writes, branch-creation all go through
/// the GitHub REST API. The branch on the writable_own source is
/// created lazily (in `ensure_ready`) from `base_branch` if it doesn't
/// yet exist on the remote — preserving the "first save creates the
/// branch" behaviour the old `GitBackend` clone path had.
fn build_traject_github_backend(
    _traject_id: Uuid,
    _source: &Source,
    github: &GitHubSource,
    base_branch: Option<&str>,
    token: Option<&str>,
) -> Result<Box<dyn regelrecht_corpus::backend::RepoBackend>, regelrecht_corpus::error::CorpusError>
{
    use regelrecht_corpus::github_api_backend::GitHubApiBackend;

    // `traject_id` and `source.id` used to namespace the on-disk clone
    // path; with the API backend the branch + URL already disambiguate,
    // so the parameters are kept on the signature for call-site
    // symmetry only.
    let backend = GitHubApiBackend::new(
        github,
        Some(base_branch.unwrap_or("main").to_string()),
        token.map(|t| t.to_string()),
    )?;
    Ok(Box::new(backend))
}

/// DB row mirror for `traject_corpus_sources`. `gh_base_branch` is kept
/// outside [`Source`] because it's traject-flow-specific (clone-then-
/// branch-from) and the global [`Source`] type doesn't carry it.
#[derive(sqlx::FromRow, Debug, Clone)]
struct TrajectSourceRow {
    source_id: String,
    name: String,
    source_type: String,
    gh_owner: Option<String>,
    gh_repo: Option<String>,
    gh_branch: Option<String>,
    gh_base_branch: Option<String>,
    gh_path: Option<String>,
    gh_ref: Option<String>,
    local_path: Option<String>,
    priority: i32,
    auth_ref: Option<String>,
    scopes: serde_json::Value,
    is_writable_own: bool,
}

impl TrajectSourceRow {
    fn to_source(&self) -> Source {
        let source_type = match self.source_type.as_str() {
            "github" => SourceType::GitHub {
                github: GitHubSource {
                    owner: self.gh_owner.clone().unwrap_or_default(),
                    repo: self.gh_repo.clone().unwrap_or_default(),
                    branch: self.gh_branch.clone().unwrap_or_default(),
                    path: self.gh_path.clone(),
                    git_ref: self.gh_ref.clone(),
                },
            },
            _ => SourceType::Local {
                local: LocalSource {
                    path: PathBuf::from(self.local_path.clone().unwrap_or_default()),
                },
            },
        };

        let scopes = serde_json::from_value(self.scopes.clone()).unwrap_or_else(|e| {
            tracing::warn!(
                source_id = %self.source_id,
                error = %e,
                "traject_corpus_sources.scopes failed to deserialise, falling back to empty list"
            );
            Default::default()
        });

        Source {
            id: self.source_id.clone(),
            name: self.name.clone(),
            source_type,
            scopes,
            priority: self.priority.max(0) as u32,
            auth_ref: self.auth_ref.clone(),
            // The writable-own's `auth_ref` derives from user-supplied repo
            // coords (create-traject request), so EVERY token lookup for it
            // — backend construction here, but also the index scan inside
            // `CorpusRegistry::index_all_sources_async` — must resolve
            // strictly, without the legacy `CORPUS_GIT_TOKEN` fallback.
            strict_auth: self.is_writable_own,
            enriched: None,
            load_all: false,
            library_only: false,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the snapshot caches: the bounded lazy-body cache,
    //! the TTL freshness rule, the per-snapshot implements index (incl.
    //! the fetch-failure path) and the carry-over semantics of a TTL
    //! index refresh. The DB-backed `get_or_build` flow is covered by
    //! the `traject_reads_test` integration tests.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use regelrecht_corpus::backend::{
        FileEntry, PersistOutcome, RepoBackend, WriteContext as CorpusWriteContext,
    };
    use regelrecht_corpus::error::{CorpusError, Result as CorpusResult};
    use tokio::sync::Semaphore;

    /// In-memory backend stub: serves a fixed file set, optionally
    /// failing every read (the throttled-fetch path), and counts reads
    /// so tests can assert a cache hit skipped the backend.
    struct StubBackend {
        files: HashMap<String, String>,
        fail_reads: bool,
        reads: Arc<AtomicUsize>,
        /// Counts `read_all_files` (bulk) calls so a test can assert the
        /// scan pulled a source in one request instead of per-law.
        bulk_reads: Arc<AtomicUsize>,
    }

    impl StubBackend {
        fn with_files(files: &[(&str, &str)]) -> Self {
            Self {
                files: files
                    .iter()
                    .map(|(p, c)| (p.to_string(), c.to_string()))
                    .collect(),
                fail_reads: false,
                reads: Arc::new(AtomicUsize::new(0)),
                bulk_reads: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn failing() -> Self {
            Self {
                files: HashMap::new(),
                fail_reads: true,
                reads: Arc::new(AtomicUsize::new(0)),
                bulk_reads: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl RepoBackend for StubBackend {
        async fn read_file(&self, path: &Path) -> CorpusResult<Option<String>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.fail_reads {
                return Err(CorpusError::Git("simulated throttle".to_string()));
            }
            Ok(self.files.get(path.to_str().unwrap()).cloned())
        }
        async fn read_all_implements(&self) -> CorpusResult<Vec<(String, Vec<String>)>> {
            self.bulk_reads.fetch_add(1, Ordering::SeqCst);
            if self.fail_reads {
                return Err(CorpusError::Git("simulated throttle".to_string()));
            }
            Ok(self
                .files
                .iter()
                .map(|(p, c)| (p.clone(), collect_law_implements(c)))
                .collect())
        }
        async fn write_file(&self, _: &Path, _: &str) -> CorpusResult<()> {
            Ok(())
        }
        async fn delete_file(&self, _: &Path) -> CorpusResult<()> {
            Ok(())
        }
        async fn list_files(&self, _: &Path, _: Option<&str>) -> CorpusResult<Vec<FileEntry>> {
            Ok(Vec::new())
        }
        async fn changed_files(&self) -> CorpusResult<Vec<String>> {
            if self.fail_reads {
                return Err(CorpusError::Git("simulated throttle".to_string()));
            }
            Ok(Vec::new())
        }
        async fn persist(&self, _: &CorpusWriteContext) -> CorpusResult<PersistOutcome> {
            Ok(PersistOutcome::default())
        }
        async fn ensure_ready(&mut self) -> CorpusResult<()> {
            Ok(())
        }
        fn is_writable(&self) -> bool {
            true
        }
    }

    fn backend_entry(backend: StubBackend) -> BackendEntry {
        BackendEntry {
            backend: Arc::new(Mutex::new(Box::new(backend) as Box<dyn RepoBackend>)),
            writable: true,
        }
    }

    /// Bare `TrajectCorpus` over the given index + backends; no DB, no
    /// network.
    fn test_corpus(
        source_map: SourceMap,
        backends: HashMap<String, BackendEntry>,
    ) -> TrajectCorpus {
        TrajectCorpus {
            corpus: CorpusState {
                registry: CorpusRegistry::empty(),
                source_map,
                backends,
                auth_file: None,
                index_failures: HashMap::new(),
                lazy_bodies: Default::default(),
            },
            traject_id: Uuid::nil(),
            own_base_branch: None,
            write_target_for_source: HashMap::new(),
            writable_own_source_id: "own".to_string(),
            overlay: Arc::new(RwLock::new(HashMap::new())),
            body_cache: RwLock::new(BoundedCache::default()),
            implements_index: RwLock::new(None),
            implements_index_stale: AtomicBool::new(false),
            implements_build_lock: Mutex::new(()),
            implements_memo: Arc::new(RwLock::new(HashMap::new())),
            changed_cache: Arc::new(RwLock::new(None)),
            changed_refresh_in_flight: Arc::new(AtomicBool::new(false)),
            changed_refresh_gate: Arc::new(Mutex::new(())),
            sidecar_cache: RwLock::new(BoundedCache::default()),
            scenario_list_cache: RwLock::new(BoundedCache::default()),
            sidecar_write_gen: AtomicU64::new(0),
            scenario_list_write_gen: AtomicU64::new(0),
            own_index_scanned_with_user_token: false,
        }
    }

    fn metadata_entry(map: &mut SourceMap, law_id: &str, source_id: &str) {
        metadata_entry_with_sha(map, law_id, source_id, Some(&format!("sha-{law_id}-v1")));
    }

    fn metadata_entry_with_sha(
        map: &mut SourceMap,
        law_id: &str,
        source_id: &str,
        sha: Option<&str>,
    ) {
        map.load_metadata_entry(
            law_id,
            &format!("wet/{law_id}/2025-01-01.yaml"),
            None,
            source_id,
            source_id,
            // Distinct priorities per source: equal-priority conflicts
            // across sources are a hard SourceMap error.
            if source_id == "own" { 0 } else { 1 },
            sha,
        )
        .unwrap();
    }

    fn law_body(law_id: &str, implements: Option<&str>) -> String {
        match implements {
            Some(higher) => format!(
                "$id: {law_id}\narticles:\n  - number: '1'\n    machine_readable:\n      implements:\n        - law: {higher}\n"
            ),
            None => format!("$id: {law_id}\narticles: []\n"),
        }
    }

    // ---- BoundedCache ----

    #[test]
    fn body_cache_evicts_oldest_at_cap() {
        let mut cache = BoundedCache::default();
        for i in 0..BODY_CACHE_MAX_ENTRIES + 10 {
            cache.insert(format!("law_{i}"), "body".to_string());
        }
        assert_eq!(cache.map.len(), BODY_CACHE_MAX_ENTRIES);
        // The 10 oldest entries were evicted FIFO; the newest survive.
        assert!(cache.get("law_0").is_none());
        assert!(cache.get("law_9").is_none());
        assert!(cache.get("law_10").is_some());
        assert!(cache
            .get(&format!("law_{}", BODY_CACHE_MAX_ENTRIES + 9))
            .is_some());
    }

    #[test]
    fn body_cache_overwrite_does_not_grow_or_evict() {
        let mut cache = BoundedCache::default();
        cache.insert("a".to_string(), "v1".to_string());
        cache.insert("b".to_string(), "v1".to_string());
        cache.insert("a".to_string(), "v2".to_string());
        assert_eq!(cache.map.len(), 2);
        assert_eq!(cache.order.len(), 2);
        assert_eq!(cache.get("a").map(String::as_str), Some("v2"));
    }

    #[test]
    fn body_cache_overwrite_at_cap_does_not_evict() {
        let mut cache = BoundedCache::default();
        // Fill exactly to capacity.
        for i in 0..BODY_CACHE_MAX_ENTRIES {
            cache.insert(format!("law_{i}"), "v1".to_string());
        }
        assert_eq!(cache.map.len(), BODY_CACHE_MAX_ENTRIES);
        // Re-inserting an existing key at cap must NOT evict: the
        // `contains_key` guard skips the eviction loop and leaves `order`
        // untouched, so the size invariant holds and no live entry is lost.
        cache.insert("law_0".to_string(), "v2".to_string());
        assert_eq!(cache.map.len(), BODY_CACHE_MAX_ENTRIES);
        assert_eq!(cache.order.len(), BODY_CACHE_MAX_ENTRIES);
        // The next-oldest, untouched entry survives (no spurious eviction)…
        assert!(cache.get("law_1").is_some());
        // …and the overwritten value is updated in place.
        assert_eq!(cache.get("law_0").map(String::as_str), Some("v2"));

        // FIFO, not LRU: the overwrite did NOT refresh law_0's age, so it's
        // still the oldest. The next genuinely-new insert at cap evicts it.
        cache.insert("law_new".to_string(), "v1".to_string());
        assert_eq!(cache.map.len(), BODY_CACHE_MAX_ENTRIES);
        assert!(
            cache.get("law_0").is_none(),
            "an overwrite must not refresh age — law_0 stays first out"
        );
        assert!(cache.get("law_new").is_some());
    }

    // ---- TTL freshness ----

    #[test]
    fn cached_corpus_freshness_respects_ttl() {
        let cached = CachedCorpus {
            corpus: Arc::new(test_corpus(SourceMap::new("2026-06-01"), HashMap::new())),
            built_at: Instant::now(),
        };
        // A just-built snapshot is fresh under the production TTL…
        assert!(cached.is_fresh(TRAJECT_INDEX_TTL));
        // …and stale under a zero TTL (the injection tests use).
        assert!(!cached.is_fresh(Duration::ZERO));
    }

    // ---- law_yaml / body cache interplay ----

    #[tokio::test]
    async fn law_yaml_caches_lazy_fetch_and_prefers_overlay() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_a", "seed");
        let stub = StubBackend::with_files(&[("wet/wet_a/2025-01-01.yaml", "$id: wet_a\n")]);
        let reads = stub.reads.clone();
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        // First read fetches and caches…
        assert_eq!(
            corpus.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\n")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        // …second read is a body-cache hit, no backend round-trip.
        assert_eq!(
            corpus.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\n")
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        // A save wins over the cached body (read-your-writes).
        corpus
            .record_save("wet_a".to_string(), "$id: wet_a\nname: saved\n".to_string())
            .await;
        assert_eq!(
            corpus.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved\n")
        );
    }

    /// Backend gedraagt zich als een privé repo achter de Contents-API:
    /// zonder het verwachte per-call token leest elke file als 404
    /// (`Ok(None)`). Met `required_token: None` modelleert hij een
    /// seed-backend die het persoonlijke token nooit mag ontvangen.
    struct TokenGatedBackend {
        files: HashMap<String, String>,
        required_token: Option<String>,
        /// Laat de Compare-call falen (haperende upstream: rate limit, 5xx)
        /// terwijl het token gewoon klopt; de leespaden blijven werken.
        compare_fails: bool,
        /// Aantal Compare-aanroepen dat deze backend heeft gezien, zodat een
        /// test kan vaststellen dat gelijktijdige bellers er samen één doen.
        compare_calls: Arc<AtomicUsize>,
        /// Parkeert de Compare-call tot de test een permit uitdeelt. Daarmee
        /// is de single-flight-test niet afhankelijk van scheduler-timing:
        /// de eerste beller kan de poort niet loslaten voordat de test dat
        /// wil, dus elke interleaving levert dezelfde telling op.
        compare_hold: Option<Arc<Semaphore>>,
    }

    #[async_trait]
    impl RepoBackend for TokenGatedBackend {
        async fn read_file(&self, path: &Path) -> CorpusResult<Option<String>> {
            self.read_file_with_token(path, None).await
        }
        async fn read_file_with_token(
            &self,
            path: &Path,
            token: Option<&str>,
        ) -> CorpusResult<Option<String>> {
            match &self.required_token {
                // Privé repo: alleen het juiste token leest; anders 404.
                Some(required) if token != Some(required.as_str()) => return Ok(None),
                Some(_) => {}
                // Seed-bron: het persoonlijke token mag hier nooit landen.
                None => assert!(
                    token.is_none(),
                    "seed backend must never receive the personal read token"
                ),
            }
            Ok(self.files.get(path.to_str().unwrap()).cloned())
        }
        async fn write_file(&self, _: &Path, _: &str) -> CorpusResult<()> {
            Ok(())
        }
        async fn delete_file(&self, _: &Path) -> CorpusResult<()> {
            Ok(())
        }
        async fn list_files(&self, _: &Path, _: Option<&str>) -> CorpusResult<Vec<FileEntry>> {
            Ok(Vec::new())
        }
        /// Zelfde poort als `read_file_with_token`, nu voor de Compare-call:
        /// zonder bruikbaar token doet `GitHubApiBackend` geen enkele
        /// round-trip en komt er een lege lijst terug. Elk bestand van deze
        /// stub telt als gewijzigd, zodat een test aan de uitkomst kan zien
        /// of het token is doorgegeven.
        async fn changed_files_with_token(&self, token: Option<&str>) -> CorpusResult<Vec<String>> {
            self.compare_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(hold) = &self.compare_hold {
                let _permit = hold.acquire().await.expect("semafoor open");
            }
            match &self.required_token {
                Some(required) if token != Some(required.as_str()) => Ok(Vec::new()),
                _ if self.compare_fails => Err(CorpusError::Git("simulated throttle".to_string())),
                _ => Ok(self.files.keys().cloned().collect()),
            }
        }
        async fn persist(&self, _: &CorpusWriteContext) -> CorpusResult<PersistOutcome> {
            Ok(PersistOutcome::default())
        }
        async fn ensure_ready(&mut self) -> CorpusResult<()> {
            Ok(())
        }
        fn supports_token_override(&self) -> bool {
            true
        }
        fn is_writable(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn law_yaml_forwards_the_read_token_only_to_the_writable_own_source() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_eigen", "own");
        metadata_entry(&mut map, "wet_seed", "seed");
        let backends = HashMap::from([
            (
                "own".to_string(),
                BackendEntry {
                    backend: Arc::new(Mutex::new(Box::new(TokenGatedBackend {
                        files: HashMap::from([(
                            "wet/wet_eigen/2025-01-01.yaml".to_string(),
                            "$id: wet_eigen\n".to_string(),
                        )]),
                        required_token: Some("user-token".to_string()),
                        compare_fails: false,
                        compare_calls: Arc::new(AtomicUsize::new(0)),
                        compare_hold: None,
                    }) as Box<dyn RepoBackend>)),
                    writable: false,
                },
            ),
            (
                "seed".to_string(),
                BackendEntry {
                    backend: Arc::new(Mutex::new(Box::new(TokenGatedBackend {
                        files: HashMap::from([(
                            "wet/wet_seed/2025-01-01.yaml".to_string(),
                            "$id: wet_seed\n".to_string(),
                        )]),
                        required_token: None,
                        compare_fails: false,
                        compare_calls: Arc::new(AtomicUsize::new(0)),
                        compare_hold: None,
                    }) as Box<dyn RepoBackend>)),
                    writable: false,
                },
            ),
        ]);
        let corpus = test_corpus(map, backends);

        // Zonder token blijft de privé writable-own onleesbaar (GitHub's 404
        // → `Ok(None)`), en er wordt níets negatiefs gecachet…
        assert_eq!(
            corpus
                .law_yaml_with_read_token("wet_eigen", None)
                .await
                .unwrap(),
            None
        );
        // …met het per-request token leest dezelfde wet wél.
        assert_eq!(
            corpus
                .law_yaml_with_read_token("wet_eigen", Some("user-token"))
                .await
                .unwrap()
                .as_deref(),
            Some("$id: wet_eigen\n")
        );

        // Een seed-read binnen dezelfde request geeft het token NIET door
        // (de assert in de stub bewaakt dat) en slaagt gewoon.
        assert_eq!(
            corpus
                .law_yaml_with_read_token("wet_seed", Some("user-token"))
                .await
                .unwrap()
                .as_deref(),
            Some("$id: wet_seed\n")
        );
    }

    // ---- implements index ----

    #[tokio::test]
    async fn implementors_of_builds_index_once_and_reverse_looks_up() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "regeling_a", "seed");
        metadata_entry(&mut map, "wet_hoger", "seed");
        let stub = StubBackend::with_files(&[
            (
                "wet/regeling_a/2025-01-01.yaml",
                &law_body("regeling_a", Some("wet_hoger")),
            ),
            (
                "wet/wet_hoger/2025-01-01.yaml",
                &law_body("wet_hoger", None),
            ),
        ]);
        let reads = stub.reads.clone();
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        let result = corpus.implementors_of("wet_hoger").await;
        assert_eq!(result.implementors, vec!["regeling_a".to_string()]);
        assert_eq!(result.skipped_count, 0);
        let reads_after_build = reads.load(Ordering::SeqCst);
        assert_eq!(reads_after_build, 2, "index build fetches each body once");

        // A second lookup — different target — reuses the index without
        // re-fetching anything.
        let result = corpus.implementors_of("regeling_a").await;
        assert!(result.implementors.is_empty());
        assert_eq!(reads.load(Ordering::SeqCst), reads_after_build);
    }

    #[tokio::test]
    async fn implementors_of_reports_fetch_failures_as_skipped() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "regeling_a", "seed");
        metadata_entry(&mut map, "wet_kapot", "broken");
        let stub = StubBackend::with_files(&[(
            "wet/regeling_a/2025-01-01.yaml",
            &law_body("regeling_a", Some("wet_hoger")),
        )]);
        let corpus = test_corpus(
            map,
            HashMap::from([
                ("seed".to_string(), backend_entry(stub)),
                ("broken".to_string(), backend_entry(StubBackend::failing())),
            ]),
        );

        let result = corpus.implementors_of("wet_hoger").await;
        // The throttled law is reported as skipped — distinguishable from
        // "checked and implements nothing" — while the healthy law is
        // still found.
        assert_eq!(result.implementors, vec!["regeling_a".to_string()]);
        assert_eq!(result.skipped_count, 1);
    }

    #[tokio::test]
    async fn implements_index_bulk_loads_large_source_in_one_request() {
        // A source with many metadata-only laws (≥ BULK_FETCH_THRESHOLD)
        // must be pulled in ONE bulk request, not one fetch per law — this
        // is what stops the cold build 504-ing on large GitHub trajects.
        let n = BULK_FETCH_THRESHOLD;
        let mut map = SourceMap::new("2026-06-01");
        let mut files: Vec<(String, String)> = Vec::new();
        for i in 0..n {
            let law_id = format!("reg_{i}");
            metadata_entry(&mut map, &law_id, "seed");
            files.push((
                format!("wet/{law_id}/2025-01-01.yaml"),
                law_body(&law_id, Some("wet_target")),
            ));
        }
        let file_refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();
        let stub = StubBackend::with_files(&file_refs);
        let reads = stub.reads.clone();
        let bulk_reads = stub.bulk_reads.clone();
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        let result = corpus.implementors_of("wet_target").await;

        // Every law is found as an implementor…
        let mut expected: Vec<String> = (0..n).map(|i| format!("reg_{i}")).collect();
        expected.sort();
        assert_eq!(result.implementors, expected);
        assert_eq!(result.skipped_count, 0);
        // …via exactly one bulk request and zero per-law fetches.
        assert_eq!(bulk_reads.load(Ordering::SeqCst), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn record_save_updates_built_implements_index_in_place() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "regeling_a", "seed");
        let stub = StubBackend::with_files(&[(
            "wet/regeling_a/2025-01-01.yaml",
            &law_body("regeling_a", None),
        )]);
        let corpus = test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        );

        // Build the index against the pre-save body: no implementors.
        assert!(corpus
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());

        // The save adds an `implements` declaration; the lookup must see
        // it immediately — no snapshot rollover needed.
        corpus
            .record_save(
                "regeling_a".to_string(),
                law_body("regeling_a", Some("wet_hoger")),
            )
            .await;
        assert_eq!(
            corpus.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );

        // And the reverse: a save dropping the declaration removes it.
        corpus
            .record_save("regeling_a".to_string(), law_body("regeling_a", None))
            .await;
        assert!(corpus
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());
    }

    // ---- TTL index refresh ----

    /// Build a real local-source `TrajectCorpus` over `dir`, the manual
    /// equivalent of `build_traject_corpus` without the DB round-trip.
    async fn local_corpus(dir: &Path) -> Arc<TrajectCorpus> {
        let source = Source {
            id: "own".to_string(),
            name: "Own".to_string(),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: dir.to_path_buf(),
                },
            },
            scopes: vec![],
            priority: 0,
            auth_ref: None,
            strict_auth: false,
            enriched: None,
            load_all: false,
            library_only: false,
        };
        let registry = CorpusRegistry::from_sources(vec![source.clone()]);
        let source_map = registry.load_local_sources("2026-06-01").unwrap();
        let mut backend = create_backend(&source, None).unwrap();
        backend.ensure_ready().await.unwrap();
        let writable = backend.is_writable();
        let backends = HashMap::from([(
            "own".to_string(),
            BackendEntry {
                backend: Arc::new(Mutex::new(backend)),
                writable,
            },
        )]);
        Arc::new(TrajectCorpus {
            corpus: CorpusState {
                registry,
                source_map,
                backends,
                auth_file: None,
                index_failures: HashMap::new(),
                lazy_bodies: Default::default(),
            },
            traject_id: Uuid::nil(),
            own_base_branch: None,
            write_target_for_source: HashMap::new(),
            writable_own_source_id: "own".to_string(),
            overlay: Arc::new(RwLock::new(HashMap::new())),
            body_cache: RwLock::new(BoundedCache::default()),
            implements_index: RwLock::new(None),
            implements_index_stale: AtomicBool::new(false),
            implements_build_lock: Mutex::new(()),
            implements_memo: Arc::new(RwLock::new(HashMap::new())),
            changed_cache: Arc::new(RwLock::new(None)),
            changed_refresh_in_flight: Arc::new(AtomicBool::new(false)),
            changed_refresh_gate: Arc::new(Mutex::new(())),
            sidecar_cache: RwLock::new(BoundedCache::default()),
            scenario_list_cache: RwLock::new(BoundedCache::default()),
            sidecar_write_gen: AtomicU64::new(0),
            scenario_list_write_gen: AtomicU64::new(0),
            own_index_scanned_with_user_token: false,
        })
    }

    fn write_law_file(dir: &Path, law_id: &str, body: &str) {
        let law_dir = dir.join("wet").join(law_id);
        std::fs::create_dir_all(&law_dir).unwrap();
        std::fs::write(law_dir.join("2025-01-01.yaml"), body).unwrap();
    }

    #[tokio::test]
    async fn refresh_swaps_index_but_carries_backends_overlay_and_changed_cache() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: v1\n");
        let old = local_corpus(dir.path()).await;

        // A save lands before the refresh: like the real `save_law`, the
        // body is persisted to the backend AND mirrored into the overlay
        // (the refresh's reconcile pass keeps an entry only while the
        // branch still holds the saved body).
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: saved\n");
        old.record_save("wet_a".to_string(), "$id: wet_a\nname: saved\n".to_string())
            .await;
        // …and upstream gains a brand-new law the old snapshot misses.
        write_law_file(dir.path(), "wet_b", "$id: wet_b\nname: nieuw\n");
        assert!(old.corpus.source_map.get_law("wet_b").is_none());

        let refreshed = refresh_traject_corpus(&old, Uuid::new_v4(), None)
            .await
            .expect("local refresh must succeed");

        // Fresh snapshot: the new upstream law is now indexed.
        assert!(refreshed.corpus.source_map.get_law("wet_b").is_some());
        // The backends are the *same* mutexes — an in-flight save keeps
        // excluding writers across the swap.
        assert!(Arc::ptr_eq(
            &old.corpus.backends["own"].backend,
            &refreshed.corpus.backends["own"].backend
        ));
        // The overlay carried over: the refresh must never resurrect
        // pre-save content.
        assert_eq!(
            refreshed.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved\n")
        );
        // A save recorded on the OLD instance after the swap is visible
        // through the refreshed one (shared overlay, not a copy).
        old.record_save(
            "wet_a".to_string(),
            "$id: wet_a\nname: saved2\n".to_string(),
        )
        .await;
        assert_eq!(
            refreshed.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved2\n")
        );
        // The changed-laws cache is shared for the same reason.
        assert!(Arc::ptr_eq(&old.changed_cache, &refreshed.changed_cache));
    }

    #[tokio::test]
    async fn refresh_marks_implements_index_stale_and_rebuild_sees_new_laws() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_hoger", &law_body("wet_hoger", None));
        let old = local_corpus(dir.path()).await;
        assert!(old
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());

        // A new implementing regulation lands upstream (e.g. merged on
        // the central corpus / saved on another replica).
        write_law_file(
            dir.path(),
            "regeling_a",
            &law_body("regeling_a", Some("wet_hoger")),
        );
        let refreshed = refresh_traject_corpus(&old, Uuid::new_v4(), None)
            .await
            .expect("local refresh must succeed");

        // The refreshed snapshot carries the old index as stale; the
        // first lookup rebuilds against the new snapshot and finds the
        // new regulation. The old instance's own (fresh) index is
        // untouched.
        assert!(refreshed.implements_index_stale.load(Ordering::SeqCst));
        assert_eq!(
            refreshed.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );
        assert!(!refreshed.implements_index_stale.load(Ordering::SeqCst));
        assert!(old
            .implementors_of("wet_hoger")
            .await
            .implementors
            .is_empty());
    }

    // ---- stale-while-revalidate on the implements index ----

    #[tokio::test]
    async fn carried_stale_index_is_served_while_a_rebuild_is_in_flight() {
        // A snapshot whose (carried) index is stale must keep answering
        // implementor lookups from it while another task holds the build
        // lock — the post-refresh lookup herd must never queue behind the
        // rescan (the federated-panel hang of PR #762).
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "regeling_a", "seed");
        let corpus = Arc::new(test_corpus(
            map,
            HashMap::from([(
                "seed".to_string(),
                backend_entry(StubBackend::with_files(&[(
                    "wet/regeling_a/2025-01-01.yaml",
                    &law_body("regeling_a", Some("wet_hoger")),
                )])),
            )]),
        ));
        // Simulate the carried-over state a TTL refresh produces.
        *corpus.implements_index.write().await = Some(Arc::new(ImplementsIndex {
            implements_by_law: HashMap::from([(
                "carried_regeling".to_string(),
                vec!["wet_hoger".to_string()],
            )]),
            failed_law_ids: Vec::new(),
        }));
        corpus.implements_index_stale.store(true, Ordering::SeqCst);

        // Another task is rebuilding (holds the build lock)…
        let build_guard = corpus.implements_build_lock.lock().await;
        // …so the lookup must serve the carried index immediately, not
        // block on the lock.
        let result = corpus.implementors_of("wet_hoger").await;
        assert_eq!(result.implementors, vec!["carried_regeling".to_string()]);
        drop(build_guard);

        // With the lock free, the next lookup rebuilds from the snapshot.
        let result = corpus.implementors_of("wet_hoger").await;
        assert_eq!(result.implementors, vec!["regeling_a".to_string()]);
    }

    // ---- cross-snapshot implements memo ----

    #[tokio::test]
    async fn rebuild_after_refresh_skips_unchanged_bodies_via_memo() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "regeling_a", "seed");
        metadata_entry(&mut map, "wet_hoger", "seed");
        let stub = StubBackend::with_files(&[
            (
                "wet/regeling_a/2025-01-01.yaml",
                &law_body("regeling_a", Some("wet_hoger")),
            ),
            (
                "wet/wet_hoger/2025-01-01.yaml",
                &law_body("wet_hoger", None),
            ),
        ]);
        let reads = stub.reads.clone();
        let old = Arc::new(test_corpus(
            map,
            HashMap::from([("seed".to_string(), backend_entry(stub))]),
        ));

        // First build of the process: every body is fetched once.
        assert_eq!(
            old.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);

        // TTL refresh with an UNCHANGED enumeration (same blob shas):
        // the post-refresh rebuild must answer entirely from the memo —
        // zero body fetches.
        let mut same_map = SourceMap::new("2026-06-01");
        metadata_entry(&mut same_map, "regeling_a", "seed");
        metadata_entry(&mut same_map, "wet_hoger", "seed");
        let refreshed = next_snapshot(&old, same_map).await;
        assert_eq!(
            refreshed.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );
        assert_eq!(
            reads.load(Ordering::SeqCst),
            2,
            "an unchanged corpus must rebuild without refetching any body"
        );

        // Next refresh where ONE law's sha moved: only that body is
        // refetched.
        let mut changed_map = SourceMap::new("2026-06-01");
        metadata_entry_with_sha(
            &mut changed_map,
            "regeling_a",
            "seed",
            Some("sha-regeling_a-v2"),
        );
        metadata_entry(&mut changed_map, "wet_hoger", "seed");
        let refreshed2 = next_snapshot(&refreshed, changed_map).await;
        assert_eq!(
            refreshed2.implementors_of("wet_hoger").await.implementors,
            vec!["regeling_a".to_string()]
        );
        assert_eq!(
            reads.load(Ordering::SeqCst),
            3,
            "only the changed body may be refetched"
        );
    }

    // ---- overlay reconciliation at refresh ----

    #[tokio::test]
    async fn refresh_drops_overlay_entry_when_branch_moved_past_the_save() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: v1\n");
        let old = local_corpus(dir.path()).await;

        // Save through this process: persisted + overlaid.
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: saved\n");
        old.record_save("wet_a".to_string(), "$id: wet_a\nname: saved\n".to_string())
            .await;

        // Another replica / a direct push moves the branch past our save.
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: extern\n");

        let refreshed = refresh_traject_corpus(&old, Uuid::new_v4(), None)
            .await
            .expect("local refresh must succeed");

        // The overlay entry was dropped (branch content differs from the
        // saved body), so reads converge to the external content instead
        // of pinning our stale save forever — and the next If-Match save
        // can 412 against bytes the user can actually see.
        assert!(!refreshed.overlay.read().await.contains_key("wet_a"));
        assert_eq!(
            refreshed.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: extern\n")
        );
    }

    #[tokio::test]
    async fn refresh_keeps_overlay_entry_while_branch_matches_the_save() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: v1\n");
        let old = local_corpus(dir.path()).await;

        // A fresh local save: branch == overlay.
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: saved\n");
        old.record_save("wet_a".to_string(), "$id: wet_a\nname: saved\n".to_string())
            .await;

        let refreshed = refresh_traject_corpus(&old, Uuid::new_v4(), None)
            .await
            .expect("local refresh must succeed");

        // No external write happened: read-your-writes keeps holding.
        assert!(refreshed.overlay.read().await.contains_key("wet_a"));
        assert_eq!(
            refreshed.law_yaml("wet_a").await.unwrap().as_deref(),
            Some("$id: wet_a\nname: saved\n")
        );
    }

    /// Poll `probe` until it returns `Some` or ~2s elapse — the background
    /// tasks under test swap state asynchronously, so assertions await
    /// convergence instead of racing the spawn.
    async fn wait_for<T, F, Fut>(mut probe: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Option<T>>,
    {
        for _ in 0..200 {
            if let Some(v) = probe().await {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("background task did not converge within 2s");
    }

    #[tokio::test]
    async fn background_refresh_swaps_in_a_fresh_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: v1\n");
        let old = local_corpus(dir.path()).await;

        let slot = Arc::new(TrajectSlot::default());
        *slot.state.write().await = Some(CachedCorpus {
            corpus: old.clone(),
            // Expired: eligible for refresh.
            built_at: Instant::now() - Duration::from_secs(3600),
        });

        // Upstream gains a law the stale snapshot misses.
        write_law_file(dir.path(), "wet_b", "$id: wet_b\nname: nieuw\n");

        let guard = slot.build_lock.clone().try_lock_owned().unwrap();
        spawn_background_refresh(
            guard,
            slot.clone(),
            old.clone(),
            Uuid::new_v4(),
            Duration::from_secs(60),
            None,
        );

        let refreshed = wait_for(|| {
            let slot = slot.clone();
            let old = old.clone();
            async move {
                let state = slot.state.read().await;
                let cached = state.as_ref().unwrap();
                (!Arc::ptr_eq(&cached.corpus, &old)).then(|| cached.corpus.clone())
            }
        })
        .await;

        assert!(refreshed.corpus.source_map.get_law("wet_b").is_some());
        // The build lock is free again: the next expiry can refresh.
        assert!(slot.build_lock.clone().try_lock_owned().is_ok());
    }

    #[tokio::test]
    async fn failed_refresh_outcome_rearms_the_stale_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        write_law_file(dir.path(), "wet_a", "$id: wet_a\nname: v1\n");
        let old = local_corpus(dir.path()).await;

        let slot = TrajectSlot::default();
        let expired = Instant::now() - Duration::from_secs(3600);
        *slot.state.write().await = Some(CachedCorpus {
            corpus: old.clone(),
            built_at: expired,
        });

        apply_refresh_outcome(
            &slot,
            old.clone(),
            Uuid::new_v4(),
            Err(TrajectCorpusError::Corpus(
                regelrecht_corpus::error::CorpusError::Config("enumeration failed".to_string()),
            )),
        )
        .await;

        // The stale snapshot is re-armed (fresh `built_at`), not replaced
        // and not dropped: reads keep working against the last good state,
        // and the fresh TTL window stops every subsequent request from
        // re-attempting against a broken upstream.
        let state = slot.state.read().await;
        let cached = state.as_ref().unwrap();
        assert!(Arc::ptr_eq(&cached.corpus, &old));
        assert!(cached.built_at > expired);
        assert_eq!(slot.refresh_failures.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn expired_changed_cache_serves_stale_and_recomputes_in_background() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_a", "own");
        let corpus = Arc::new(test_corpus(
            map,
            HashMap::from([(
                "own".to_string(),
                backend_entry(StubBackend::with_files(&[])),
            )]),
        ));

        // An expired cache entry with a value the real diff (empty — the
        // stub's `changed_files` default) no longer contains.
        *corpus.changed_cache.write().await = Some(ChangedLawsCache {
            computed_at: Instant::now() - Duration::from_secs(3600),
            law_ids: vec!["wet_a".to_string()],
        });

        // Served immediately, stale value and all — no Compare round-trip
        // on the request path.
        let ids = corpus.changed_law_ids(None).await.unwrap();
        assert_eq!(ids, vec!["wet_a".to_string()]);

        // …while the background recompute converges to the real diff and
        // releases the single-flight flag (checked inside the same poll:
        // the flag reset runs after the cache write, so asserting it
        // outside the wait would race a multi-threaded runtime).
        wait_for(|| {
            let corpus = corpus.clone();
            async move {
                let cache = corpus.changed_cache.read().await;
                let cached = cache.as_ref().unwrap();
                (cached.law_ids.is_empty()
                    && !corpus.changed_refresh_in_flight.load(Ordering::SeqCst))
                .then_some(())
            }
        })
        .await;
    }

    #[tokio::test]
    async fn failed_changed_recompute_rearms_stale_and_releases_the_flag() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_a", "own");
        let corpus = Arc::new(test_corpus(
            map,
            HashMap::from([("own".to_string(), backend_entry(StubBackend::failing()))]),
        ));

        let expired = Instant::now() - Duration::from_secs(3600);
        *corpus.changed_cache.write().await = Some(ChangedLawsCache {
            computed_at: expired,
            law_ids: vec!["wet_a".to_string()],
        });

        // Stale value serves; the background recompute fails against the
        // throttled backend.
        let ids = corpus.changed_law_ids(None).await.unwrap();
        assert_eq!(ids, vec!["wet_a".to_string()]);

        // Failure stance: the stale ids survive, the TTL clock is
        // re-armed (no per-request re-attempts against a broken
        // upstream), and the single-flight flag is released so a later
        // expiry can retry.
        wait_for(|| {
            let corpus = corpus.clone();
            async move {
                let cache = corpus.changed_cache.read().await;
                let cached = cache.as_ref().unwrap();
                (cached.computed_at > expired
                    && cached.law_ids == vec!["wet_a".to_string()]
                    && !corpus.changed_refresh_in_flight.load(Ordering::SeqCst))
                .then_some(())
            }
        })
        .await;
    }

    /// Traject op een privé, door de gebruiker aangedragen repo: de server
    /// heeft er geen eigen token voor, dus alleen het persoonlijke token van
    /// de handelende gebruiker komt binnen.
    fn private_own_corpus() -> Arc<TrajectCorpus> {
        private_own_corpus_with(false)
    }

    /// Als [`private_own_corpus`], maar `compare_fails` bepaalt of de
    /// Compare-call van een correct getokende beller alsnog stukloopt.
    fn private_own_corpus_with(compare_fails: bool) -> Arc<TrajectCorpus> {
        private_own_corpus_full(compare_fails, Arc::new(AtomicUsize::new(0)), None)
    }

    /// Als [`private_own_corpus_with`], met een teller voor het aantal
    /// Compare-aanroepen en een optionele semafoor die de Compare-call
    /// parkeert — samen genoeg om de single-flight-poort vast te pinnen.
    fn private_own_corpus_full(
        compare_fails: bool,
        compare_calls: Arc<AtomicUsize>,
        compare_hold: Option<Arc<Semaphore>>,
    ) -> Arc<TrajectCorpus> {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_eigen", "own");
        Arc::new(test_corpus(
            map,
            HashMap::from([(
                "own".to_string(),
                BackendEntry {
                    backend: Arc::new(Mutex::new(Box::new(TokenGatedBackend {
                        files: HashMap::from([(
                            "wet/wet_eigen/2025-01-01.yaml".to_string(),
                            "$id: wet_eigen\n".to_string(),
                        )]),
                        required_token: Some("user-token".to_string()),
                        compare_fails,
                        compare_calls: compare_calls.clone(),
                        compare_hold: compare_hold.clone(),
                    }) as Box<dyn RepoBackend>)),
                    writable: false,
                },
            )]),
        ))
    }

    #[tokio::test]
    async fn changed_law_ids_uses_the_callers_token_on_a_private_own_source() {
        // De kern van de "Bewerkt is stuk bij privé-trajecten"-fix: zonder
        // per-call token blijft de diff leeg (het oude, permanente gedrag);
        // mét token levert dezelfde backend de gewijzigde wet.
        let corpus = private_own_corpus();

        assert!(corpus.changed_law_ids(None).await.unwrap().is_empty());
        assert_eq!(
            corpus.changed_law_ids(Some("user-token")).await.unwrap(),
            vec!["wet_eigen".to_string()]
        );
    }

    #[tokio::test]
    async fn tokenless_empty_diff_is_never_written_to_the_shared_cache() {
        // `changed_cache` is per traject (TTL 60 s), niet per gebruiker. Een
        // lege uitkomst die alleen ontstond doordat *deze* gebruiker geen
        // GitHub-koppeling heeft, mag daarom niet 60 s lang aan alle leden
        // worden geserveerd — anders blijft "Bewerkt" leeg voor iemand die
        // wél gekoppeld is.
        let corpus = private_own_corpus();

        assert!(corpus.changed_law_ids(None).await.unwrap().is_empty());
        assert!(
            corpus.changed_cache.read().await.is_none(),
            "een token-loze lege diff hoort niet in de gedeelde cache"
        );

        // Het gekoppelde lid berekent wél een deelbare uitkomst…
        assert_eq!(
            corpus.changed_law_ids(Some("user-token")).await.unwrap(),
            vec!["wet_eigen".to_string()]
        );
        assert_eq!(
            corpus.changed_cache.read().await.as_ref().unwrap().law_ids,
            vec!["wet_eigen".to_string()]
        );

        // …en die uitkomst blijft staan als een ongekoppeld lid langskomt.
        assert_eq!(
            corpus.changed_law_ids(None).await.unwrap(),
            vec!["wet_eigen".to_string()]
        );
    }

    #[tokio::test]
    async fn expired_private_cache_waits_for_a_linked_caller_instead_of_blanking() {
        // De achtergrond-recompute draait buiten een request en heeft dus
        // geen gebruikerstoken; op een privé-bron zou hij de cache leegvegen.
        // Hij wordt daarom niet gestart: een ongekoppelde beller krijgt de
        // stale waarde, een gekoppelde ververst hem op het request-pad.
        let corpus = private_own_corpus();
        let expired = Instant::now() - Duration::from_secs(3600);
        *corpus.changed_cache.write().await = Some(ChangedLawsCache {
            computed_at: expired,
            law_ids: vec!["wet_oud".to_string()],
        });

        assert_eq!(
            corpus.changed_law_ids(None).await.unwrap(),
            vec!["wet_oud".to_string()]
        );
        let cache = corpus.changed_cache.read().await;
        let cached = cache.as_ref().unwrap();
        assert_eq!(cached.law_ids, vec!["wet_oud".to_string()]);
        assert_eq!(cached.computed_at, expired, "geen achtergrond-recompute");
        assert!(!corpus.changed_refresh_in_flight.load(Ordering::SeqCst));
        drop(cache);

        // Een gekoppeld lid ververst de verlopen entry meteen (synchroon:
        // er is geen achtergrond-leg met een bruikbaar token).
        assert_eq!(
            corpus.changed_law_ids(Some("user-token")).await.unwrap(),
            vec!["wet_eigen".to_string()]
        );
        let cache = corpus.changed_cache.read().await;
        let cached = cache.as_ref().unwrap();
        assert_eq!(cached.law_ids, vec!["wet_eigen".to_string()]);
        assert!(cached.computed_at > expired);
    }

    #[tokio::test]
    async fn failed_private_request_refresh_serves_stale_instead_of_erroring() {
        // Op een privé-bron ververst een gekoppelde beller de verlopen entry
        // op het request-pad. Loopt die Compare-call stuk, dan geldt dezelfde
        // degradeer-naar-stale-houding als in de achtergrond-leg: geen 502 die
        // "Bewerkt" laat verdwijnen, en de TTL-klok wordt opnieuw opgespannen
        // zodat niet elk volgend verzoek tegen een haperende upstream aanloopt.
        let corpus = private_own_corpus_with(true);
        let expired = Instant::now() - Duration::from_secs(3600);
        *corpus.changed_cache.write().await = Some(ChangedLawsCache {
            computed_at: expired,
            law_ids: vec!["wet_oud".to_string()],
        });

        assert_eq!(
            corpus.changed_law_ids(Some("user-token")).await.unwrap(),
            vec!["wet_oud".to_string()]
        );
        let cache = corpus.changed_cache.read().await;
        let cached = cache.as_ref().unwrap();
        assert_eq!(cached.law_ids, vec!["wet_oud".to_string()]);
        assert!(cached.computed_at > expired, "TTL-klok opnieuw opgespannen");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_private_refreshes_share_one_compare_call() {
        // De achtergrond-leg heeft `changed_refresh_in_flight`; de synchrone
        // leg (privé-bron, alleen het token van de beller komt binnen) heeft
        // de poort. Zonder die poort doet elk gekoppeld lid dat de
        // bibliotheek binnen hetzelfde verlopen-TTL-venster laadt zijn eigen
        // Compare-call tegen een gedeeld, gelimiteerd token.
        //
        // De semafoor maakt de uitkomst interleaving-onafhankelijk: wie de
        // poort ook als eerste pakt, hij kan hem niet loslaten voordat de
        // test een permit uitdeelt, dus de ander staat gegarandeerd te
        // wachten en vindt daarna een verse cache-entry.
        let hold = Arc::new(Semaphore::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let corpus = private_own_corpus_full(false, calls.clone(), Some(hold.clone()));

        let a = corpus.clone();
        let b = corpus.clone();
        let first = tokio::spawn(async move { a.changed_law_ids(Some("user-token")).await });
        let second = tokio::spawn(async move { b.changed_law_ids(Some("user-token")).await });

        // Eén permit: precies genoeg voor de winnaar van de poort. Zou de
        // verliezer alsnog bij de backend uitkomen, dan liep deze test vast
        // in plaats van stilletjes een tweede Compare-call te slikken.
        hold.add_permits(1);

        // De permit komt bij het droppen van de guard weer vrij, dus valt de
        // poort weg dan komt de verliezer alsnog bij de backend uit en telt
        // de assertie hieronder er twee — de test faalt dan op de telling,
        // niet op deze timeout. De timeout is puur een vangnet tegen een
        // echte deadlock, zodat een fout in de poort nooit als een hangende
        // suite eindigt.
        let (ra, rb) = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(first, second)
        })
        .await
        .expect(
            "beide bellers horen af te ronden; een tweede Compare-call parkeert op de semafoor",
        );
        assert_eq!(ra.unwrap().unwrap(), vec!["wet_eigen".to_string()]);
        assert_eq!(rb.unwrap().unwrap(), vec!["wet_eigen".to_string()]);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "gelijktijdige bellers horen één Compare-call te delen"
        );
    }

    #[tokio::test]
    async fn failed_private_first_compute_still_errors() {
        // Zonder iets in de cache is er niets om op terug te vallen: de fout
        // hoort dan wél door te komen (de handler maakt er een 502 van), in
        // plaats van stil als "niets bewerkt" te worden gepresenteerd.
        let corpus = private_own_corpus_with(true);
        assert!(corpus.changed_law_ids(Some("user-token")).await.is_err());
        assert!(corpus.changed_cache.read().await.is_none());
    }

    #[tokio::test]
    async fn record_changed_law_folds_a_save_into_the_cached_diff() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_a", "own");
        let corpus = Arc::new(test_corpus(
            map,
            HashMap::from([(
                "own".to_string(),
                backend_entry(StubBackend::with_files(&[])),
            )]),
        ));

        // No cache entry yet: recording is a no-op (the next request
        // computes the real diff, which already includes the save).
        corpus.record_changed_law("wet_a").await;
        assert!(corpus.changed_cache.read().await.is_none());

        // With a cached diff the saved law is appended once, sorted, and
        // the TTL clock is untouched.
        let computed_at = Instant::now() - Duration::from_secs(30);
        *corpus.changed_cache.write().await = Some(ChangedLawsCache {
            computed_at,
            law_ids: vec!["wet_b".to_string()],
        });
        corpus.record_changed_law("wet_a").await;
        corpus.record_changed_law("wet_a").await;
        let cache = corpus.changed_cache.read().await;
        let cached = cache.as_ref().unwrap();
        assert_eq!(
            cached.law_ids,
            vec!["wet_a".to_string(), "wet_b".to_string()]
        );
        assert_eq!(cached.computed_at, computed_at);
    }

    #[tokio::test]
    async fn sidecar_cache_round_trips_and_resets_per_snapshot() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_a", "own");
        let corpus = Arc::new(test_corpus(
            map,
            HashMap::from([(
                "own".to_string(),
                backend_entry(StubBackend::with_files(&[])),
            )]),
        ));

        // Miss, then positive and *negative* results both cache.
        assert!(corpus.cached_sidecar("ann:wet_a").await.is_none());
        corpus
            .store_sidecar("ann:wet_a".to_string(), Some("notes: []\n".to_string()))
            .await;
        corpus
            .store_sidecar("scn:wet_a/x.feature".to_string(), None)
            .await;
        assert_eq!(
            corpus.cached_sidecar("ann:wet_a").await,
            Some(Some("notes: []\n".to_string()))
        );
        assert_eq!(
            corpus.cached_sidecar("scn:wet_a/x.feature").await,
            Some(None)
        );

        // Invalidation drops a single key.
        corpus.invalidate_sidecar("ann:wet_a").await;
        assert!(corpus.cached_sidecar("ann:wet_a").await.is_none());

        // Scenario listings follow the same shape.
        corpus
            .store_scenario_list_read(
                "wet_a".to_string(),
                vec![ScenarioListEntry {
                    filename: "x.feature".to_string(),
                    target_law_ids: vec!["wet_a".to_string()],
                }],
                corpus.scenario_list_write_generation(),
            )
            .await;
        assert_eq!(
            corpus
                .cached_scenario_list("wet_a")
                .await
                .unwrap()
                .first()
                .unwrap()
                .filename,
            "x.feature"
        );

        // Saves upsert the cached listing in place (sorted), deletes
        // remove the one entry — never a drop-and-rebuild, which would
        // race GitHub's eventually-consistent directory view.
        corpus
            .record_scenario_saved("wet_a", "a.feature", vec!["wet_a".to_string()])
            .await;
        corpus
            .record_scenario_saved("wet_a", "x.feature", vec!["wet_b".to_string()])
            .await;
        let listing = corpus.cached_scenario_list("wet_a").await.unwrap();
        assert_eq!(
            listing
                .iter()
                .map(|e| e.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["a.feature", "x.feature"]
        );
        assert_eq!(listing[1].target_law_ids, vec!["wet_b".to_string()]);
        corpus.record_scenario_deleted("wet_a", "x.feature").await;
        let listing = corpus.cached_scenario_list("wet_a").await.unwrap();
        assert_eq!(
            listing
                .iter()
                .map(|e| e.filename.as_str())
                .collect::<Vec<_>>(),
            vec!["a.feature"]
        );
        // Without a cached listing both records are no-ops: nothing to
        // update, the next listing rebuilds from the branch.
        corpus
            .record_scenario_saved("wet_other", "y.feature", vec![])
            .await;
        assert!(corpus.cached_scenario_list("wet_other").await.is_none());

        // A snapshot swap starts with empty sidecar caches: upstream
        // sidecar edits become visible within one TTL window, like law
        // bodies.
        corpus
            .store_sidecar("scn:wet_a/y.feature".to_string(), None)
            .await;
        let mut next_map = SourceMap::new("2026-06-01");
        metadata_entry(&mut next_map, "wet_a", "own");
        let next = next_snapshot(&corpus, next_map).await;
        assert!(next.cached_sidecar("scn:wet_a/y.feature").await.is_none());
    }

    #[tokio::test]
    async fn stale_read_store_is_dropped_after_a_write_bumps_the_generation() {
        let mut map = SourceMap::new("2026-06-01");
        metadata_entry(&mut map, "wet_a", "own");
        let corpus = Arc::new(test_corpus(
            map,
            HashMap::from([(
                "own".to_string(),
                backend_entry(StubBackend::with_files(&[])),
            )]),
        ));

        // A GET-path read starts (generation captured), then a save lands
        // before the read completes: the read's store must be dropped so
        // the pre-save bytes cannot mask the save (they'd otherwise serve
        // — with a pre-save ETag — until the snapshot swap).
        let gen_before = corpus.sidecar_write_generation();
        corpus
            .store_sidecar("scn:wet_a/x.feature".to_string(), Some("saved".to_string()))
            .await;
        corpus
            .store_sidecar_read(
                "scn:wet_a/x.feature".to_string(),
                Some("pre-save".to_string()),
                gen_before,
            )
            .await;
        assert_eq!(
            corpus.cached_sidecar("scn:wet_a/x.feature").await,
            Some(Some("saved".to_string()))
        );

        // Same guard for the listing cache: a listing computed before a
        // delete's in-place update must not overwrite it (the deleted
        // file would resurrect).
        corpus
            .store_scenario_list_read(
                "wet_a".to_string(),
                vec![ScenarioListEntry {
                    filename: "deleted.feature".to_string(),
                    target_law_ids: vec![],
                }],
                corpus.scenario_list_write_generation(),
            )
            .await;
        let gen_before = corpus.scenario_list_write_generation();
        corpus
            .record_scenario_deleted("wet_a", "deleted.feature")
            .await;
        corpus
            .store_scenario_list_read(
                "wet_a".to_string(),
                vec![ScenarioListEntry {
                    filename: "deleted.feature".to_string(),
                    target_law_ids: vec![],
                }],
                gen_before,
            )
            .await;
        assert!(corpus
            .cached_scenario_list("wet_a")
            .await
            .unwrap()
            .is_empty());

        // An undisturbed read (no write in between) stores normally.
        let gen_before = corpus.sidecar_write_generation();
        corpus
            .store_sidecar_read("ann:wet_a".to_string(), None, gen_before)
            .await;
        assert_eq!(corpus.cached_sidecar("ann:wet_a").await, Some(None));

        // The two cache families have independent generations: a scenario
        // save/delete (listing counter) must not drop a concurrent
        // sidecar read, and an annotation save (sidecar counter) must not
        // drop a concurrent listing rebuild.
        let gen_before = corpus.sidecar_write_generation();
        corpus
            .record_scenario_saved("wet_a", "b.feature", vec![])
            .await;
        corpus
            .store_sidecar_read(
                "ann:wet_b".to_string(),
                Some("notes: []\n".to_string()),
                gen_before,
            )
            .await;
        assert_eq!(
            corpus.cached_sidecar("ann:wet_b").await,
            Some(Some("notes: []\n".to_string()))
        );
        let gen_before = corpus.scenario_list_write_generation();
        corpus
            .store_sidecar("ann:wet_b".to_string(), Some("notes: [x]\n".to_string()))
            .await;
        corpus
            .store_scenario_list_read("wet_b".to_string(), Vec::new(), gen_before)
            .await;
        assert!(corpus
            .cached_scenario_list("wet_b")
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn bounded_cache_remove_purges_the_order_entry() {
        let mut cache: BoundedCache<String> = BoundedCache::default();
        cache.insert("victim".to_string(), "v1".to_string());
        cache.remove("victim");
        cache.insert("victim".to_string(), "v2".to_string());
        // Fill to capacity; evictions must pop genuinely-oldest keys, not
        // a stale order duplicate of the re-inserted key (which would
        // evict the live value while its newer order entry survives).
        for i in 0..(BODY_CACHE_MAX_ENTRIES - 1) {
            cache.insert(format!("filler-{i}"), "x".to_string());
        }
        assert_eq!(cache.get("victim"), Some(&"v2".to_string()));
        // One past capacity: the oldest key ("victim", re-inserted first)
        // is evicted exactly once — no double-eviction from a stale
        // duplicate.
        cache.insert("overflow".to_string(), "x".to_string());
        assert!(cache.get("victim").is_none());
        assert_eq!(cache.get("filler-0"), Some(&"x".to_string()));
    }
}
