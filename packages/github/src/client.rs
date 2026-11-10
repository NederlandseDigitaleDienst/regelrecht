//! The `GithubClient` service: one shared `reqwest::Client`, one header
//! builder, one base-url mechanism, and the ETag + rate-limit state that the
//! stateful read paths need.
//!
//! State (ETag cache, last-seen rate-limit remaining) lives behind a
//! `std::sync::Mutex` so every method takes `&self` (interior mutability).
//! The lock is only ever taken to read/replace a small `HashMap` entry or an
//! `Option<u32>` — never held across a `.await` (clippy's `await_holding_lock`
//! guards this), so it can't stall the async runtime.
//!
//! ## Conditional GETs
//!
//! Two shapes of ETag state live here:
//!
//! * [`cached_etag`](GithubClient::cached_etag) / [`store_etag`](GithubClient::store_etag)
//!   — an ETag with **no body**, for the Trees read, whose caller keeps the
//!   previously loaded data itself and only needs to be told "unchanged"
//!   (`Ok(None)` on 304).
//! * [`cached_response`](GithubClient::cached_response) /
//!   [`store_response`](GithubClient::store_response) — an ETag **together
//!   with the payload it was observed with**, for the Contents reads, whose
//!   callers need the content back on a 304. Storing the two as one unit is
//!   what makes the 304 path safe: there is never an ETag whose body was
//!   dropped, so a 304 can never degrade into "empty" or "does not exist".
//!
//! Both caches are keyed per (url, token identity), so one principal's
//! entries do not answer another principal's reads. The token itself is
//! never stored, only a non-cryptographic digest of it — see
//! [`cache_key`](GithubClient::cache_key) for what that does and does not
//! guarantee.
//!
//! The response cache never serves content without asking GitHub first: it
//! only turns a 200 into a 304, which costs no rate-limit quota. It can
//! therefore not go stale — a changed file changes its ETag and comes back
//! as a full 200. Its size is capped ([`RESPONSE_CACHE_BUDGET_BYTES`]) so a
//! corpus-wide read sequence cannot grow it without bound; the least
//! recently used entries are evicted first.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};

use crate::contents::DirectoryEntry;
use crate::error::{GithubError, Result};

/// Memory budget for the conditional-GET response cache. Entries are
/// evicted least-recently-used first once the summed payload size passes
/// this.
///
/// The budget is **per client**, and a client is per backend: every corpus
/// source of every open traject has one. The ceiling is therefore
/// multiplied by however many of those are live, which is what keeps this
/// number small. 8 MiB still holds a few hundred law bodies (or the
/// implements index, at ~1,5 MB, alongside them) — the paths an editor
/// session re-reads across snapshot rebuilds, which is where the
/// rate-limit saving is.
pub const RESPONSE_CACHE_BUDGET_BYTES: usize = 8 * 1024 * 1024;

/// Entry cap for the two small key→flag/ETag maps. Both are keyed per
/// `(url or repo, token identity)`, and the token identity is what makes
/// the key space grow: a long-lived editor process reads on behalf of
/// every user that opens a traject, so each new user opens a new set of
/// keys for the same handful of URLs. The entries themselves are tiny
/// (an ETag, a bool), so the cap is about the key space, not the payload —
/// hence a count rather than the byte budget the response cache uses.
/// Oldest-inserted entries are dropped first; both maps are caches whose
/// miss path is a plain request.
pub const KEY_CACHE_MAX_ENTRIES: usize = 4096;

/// A `String`-keyed map with a hard entry cap, oldest insertion evicted
/// first. Re-inserting a key updates its value and leaves its place in the
/// eviction order alone — an entry that keeps being refreshed still ages
/// out, which for these two caches costs one request.
struct BoundedMap<V> {
    entries: HashMap<String, V>,
    order: VecDeque<String>,
    cap: usize,
}

impl<V> Default for BoundedMap<V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            cap: KEY_CACHE_MAX_ENTRIES,
        }
    }
}

impl<V> BoundedMap<V> {
    #[cfg(test)]
    fn with_cap(cap: usize) -> Self {
        Self {
            cap,
            ..Self::default()
        }
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: &str, value: V) {
        // Returning here is what keeps `order` free of duplicates. With a
        // duplicate in it, an eviction pops a key that is still live in
        // `entries` and removes an entry that was nowhere near the oldest.
        if self.entries.insert(key.to_string(), value).is_some() {
            return;
        }
        self.order.push_back(key.to_string());
        while self.order.len() > self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Salt for the second token digest in [`GithubClient::cache_key`], so the
/// two halves of the key's token identity are not the same function of the
/// same input.
const TOKEN_HASH_SALT: u64 = 0x9e37_79b9_7f4a_7c15;

/// Serialises the repo-readability probe for one `(repo, token identity)`
/// and holds what it found, so whoever queues behind it reads the answer
/// instead of asking again. `None` means nobody has answered yet — either
/// the first probe is still running, or it failed to answer at all and the
/// next in the queue should try.
pub(crate) type ProbeGate = Arc<tokio::sync::Mutex<Option<crate::contents::RepoAccess>>>;

/// User-Agent sent on every request, so GitHub audit logs attribute reads and
/// writes to this client uniformly (the three hand-rolled predecessors each
/// sent `regelrecht-corpus/0.1`).
pub(crate) const USER_AGENT_VALUE: &str = concat!("regelrecht-github/", env!("CARGO_PKG_VERSION"));

/// GitHub REST API version header value pinned across all calls.
pub(crate) const GITHUB_API_VERSION: &str = "2022-11-28";

/// Payload cached alongside an ETag, per read shape. Kept as one enum so a
/// cache hit can never hand a directory listing to a file read (or vice
/// versa) — the reader matches on the variant it expects and treats
/// anything else as a miss.
#[derive(Debug, Clone)]
pub(crate) enum CachedPayload {
    /// A Contents file read: decoded body plus its blob sha.
    File { content: String, sha: String },
    /// A raw Contents file read (`application/vnd.github.raw+json`), which
    /// carries no sha.
    Raw(String),
    /// A Contents directory listing.
    Directory(Vec<DirectoryEntry>),
}

/// Which shape of payload a reader expects back. A cache entry of another
/// shape is not a hit — the reader ignores it rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheKind {
    File,
    Raw,
    Directory,
}

impl CachedPayload {
    /// The shape of this payload, for matching against what a reader wants.
    pub(crate) fn kind(&self) -> CacheKind {
        match self {
            Self::File { .. } => CacheKind::File,
            Self::Raw(_) => CacheKind::Raw,
            Self::Directory(_) => CacheKind::Directory,
        }
    }

    /// Approximate heap footprint, for the cache budget. Exactness doesn't
    /// matter — the budget is a guard rail, not an accountant.
    fn size_bytes(&self) -> usize {
        match self {
            Self::File { content, sha } => content.len() + sha.len(),
            Self::Raw(content) => content.len(),
            Self::Directory(entries) => entries
                .iter()
                .map(|e| e.name.len() + e.entry_type.len() + 32)
                .sum(),
        }
    }
}

/// An ETag together with the payload that ETag was observed with. The two
/// are stored and evicted as one unit — see the module docs.
#[derive(Debug, Clone)]
pub(crate) struct CachedResponse {
    pub etag: String,
    pub payload: CachedPayload,
}

/// Mutable per-client state guarded by [`GithubClient::state`].
#[derive(Default)]
struct ClientState {
    /// ETag cache: cache key → last ETag value. Feeds `If-None-Match` on
    /// the Trees read so an unchanged tree comes back as a cheap 304.
    etag_cache: BoundedMap<String>,
    /// Conditional-GET cache for the Contents reads: cache key → ETag +
    /// the payload it belongs to.
    response_cache: HashMap<String, CachedResponse>,
    /// Cache keys in least-recently-used order (front = oldest touch), the
    /// eviction order for [`response_cache`](Self::response_cache).
    response_order: VecDeque<String>,
    /// Summed [`CachedPayload::size_bytes`] of `response_cache`.
    response_bytes: usize,
    /// Cache keys of `(repo, token identity)` pairs whose readability has
    /// been settled, and which way. A 404 only means "not there" once the
    /// repo it was asked of is known to be readable with that credential;
    /// see `GithubClient::confirm_absence`. Both answers are remembered:
    /// the negative one especially, because that is the case where every
    /// single read 404s and would otherwise re-probe forever.
    repo_readable: BoundedMap<bool>,
    /// One gate per `(repo, token identity)` for the readability probe, so
    /// callers that miss the answer at the same moment queue behind one
    /// lookup instead of each issuing their own, and read its answer
    /// instead of repeating it. Entries are dropped once nobody holds them
    /// any more.
    repo_probe_gates: HashMap<String, ProbeGate>,
    /// Most recent `x-ratelimit-remaining` seen on any response.
    rate_limit_remaining: Option<u32>,
}

impl ClientState {
    /// Drop least-recently-used entries until the cache fits `budget`.
    fn evict_to(&mut self, budget: usize) {
        while self.response_bytes > budget {
            let Some(oldest) = self.response_order.pop_front() else {
                // Order and map disagree — reset rather than spin.
                self.response_cache.clear();
                self.response_bytes = 0;
                break;
            };
            if let Some(dropped) = self.response_cache.remove(&oldest) {
                self.response_bytes = self
                    .response_bytes
                    .saturating_sub(dropped.payload.size_bytes());
            }
        }
    }
}

/// One GitHub REST client shared by every regelrecht application.
pub struct GithubClient {
    pub(crate) client: reqwest::Client,
    /// API base URL — no trailing slash; every method prefixes its own
    /// `/...` path. Production default is `https://api.github.com`.
    pub(crate) api_base: String,
    /// Byte budget for the conditional-GET response cache. A field rather
    /// than a constant so a test can shrink it and prove eviction.
    cache_budget: usize,
    state: Mutex<ClientState>,
}

impl GithubClient {
    /// Build a client pointed at `api.github.com`, or at whatever
    /// `GITHUB_API_BASE` names when that env var is set.
    ///
    /// The env override is read **once, here at construction**. It is the
    /// load-bearing test seam: the client is built deep inside backend /
    /// registry code with no config plumbing to inject a base URL, so the
    /// integration tests stand up a wiremock GitHub and point every client in
    /// the process at it via this env var. It doubles as a GitHub Enterprise
    /// seam; production deployments leave it unset.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| GithubError::Config(format!("failed to create HTTP client: {e}")))?;

        let api_base = std::env::var("GITHUB_API_BASE")
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "https://api.github.com".to_string());

        Ok(Self {
            client,
            api_base,
            cache_budget: RESPONSE_CACHE_BUDGET_BYTES,
            state: Mutex::new(ClientState::default()),
        })
    }

    /// Shrink or grow the conditional-GET cache budget. Test seam: nothing
    /// in production changes it away from [`RESPONSE_CACHE_BUDGET_BYTES`].
    /// Takes `&mut self` so it can only be set before the client is shared.
    pub fn set_cache_budget_bytes(&mut self, budget: usize) {
        self.cache_budget = budget;
        if let Ok(mut state) = self.state.lock() {
            state.evict_to(budget);
        }
    }

    /// Override the API base URL, consuming self — for callers that build a
    /// client and immediately point it at a wiremock server (or a specific
    /// enterprise host). Trailing slashes are trimmed so callers can pass a
    /// server URI verbatim.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.set_base_url(base_url);
        self
    }

    /// In-place variant of [`with_base_url`](Self::with_base_url) for call
    /// sites that already hold the client by `&mut` (e.g. a backend reaching
    /// through its guard to repoint the client at a test server).
    pub fn set_base_url(&mut self, base_url: impl Into<String>) {
        self.api_base = base_url.into().trim_end_matches('/').to_string();
    }

    /// Most recent `x-ratelimit-remaining` value observed on a response, if
    /// any has been seen yet.
    pub fn rate_limit_remaining(&self) -> Option<u32> {
        self.state
            .lock()
            .map(|s| s.rate_limit_remaining)
            .unwrap_or(None)
    }

    /// Build the default header set every GitHub call shares (User-Agent,
    /// Accept, API version) plus the `Authorization` header when a token is
    /// given.
    ///
    /// Returns [`GithubError::InvalidToken`] when the token can't form a valid
    /// header value, rather than dropping the header and sending an
    /// unauthenticated request (which would surface as a misleading 401).
    pub(crate) fn default_headers(&self, token: Option<&str>) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(GITHUB_API_VERSION),
        );
        if let Some(token) = token {
            let auth = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| GithubError::InvalidToken(e.to_string()))?;
            headers.insert(AUTHORIZATION, auth);
        }
        Ok(headers)
    }

    /// Record `x-ratelimit-remaining` from a response and warn when it runs
    /// low. Takes the guard, mutates, drops it — never spans an await.
    pub(crate) fn track_rate_limit(&self, response: &reqwest::Response) {
        if let Some(remaining) = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            if let Ok(mut state) = self.state.lock() {
                state.rate_limit_remaining = Some(remaining);
            }
            if remaining < 100 {
                tracing::warn!(remaining, "GitHub API rate limit running low");
            }
        }
    }

    /// Cache key for a conditional GET: the URL, the representation asked
    /// for, and the identity of the token it is made with.
    ///
    /// The token is part of the key so one principal's entries do not
    /// answer another principal's reads; it is hashed, never stored. The
    /// hash is `DefaultHasher` — unkeyed, not cryptographic — so what it
    /// gives is compartmenting, not a security boundary. Two independent
    /// 64-bit digests of the token go into the key rather than one,
    /// because the answer this key guards is not always rechecked: an ETag
    /// entry is only ever revalidated against GitHub with the caller's own
    /// credential, but `repo_readable` (same key shape) is *believed*, and
    /// a token that inherited another's "readable" would read a 404 as
    /// absence. 128 bits puts that beyond arithmetic rather than beyond
    /// worry.
    ///
    /// The representation is part of it because the Contents API serves the
    /// same URL as JSON or raw depending on `Accept`, and those two bodies
    /// must not share an entry.
    pub(crate) fn cache_key(url: &str, accept: Option<&str>, token: Option<&str>) -> String {
        let representation = accept.unwrap_or("default");
        match token {
            Some(token) => {
                // Two digests of the same token, salted apart, read as one
                // 128-bit value. `DefaultHasher` is fixed-seed, so the
                // salt is the only thing making the halves independent.
                let mut low = DefaultHasher::new();
                token.hash(&mut low);
                let mut high = DefaultHasher::new();
                (TOKEN_HASH_SALT, token).hash(&mut high);
                format!(
                    "{url}#{representation}#{:016x}{:016x}",
                    high.finish(),
                    low.finish()
                )
            }
            None => format!("{url}#{representation}#anon"),
        }
    }

    /// What is known about this `(repo, token)` pair's readability, if it
    /// has been settled — see `confirm_absence` in the contents module.
    pub(crate) fn known_repo_readable(&self, key: &str) -> Option<bool> {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.repo_readable.get(key).copied())
    }

    /// The gate that serialises readability probes for one `(repo, token)`
    /// pair. Gates nobody is waiting on are dropped here rather than kept
    /// for the client's lifetime: the map is keyed by token identity too,
    /// so it would otherwise grow with every user the process serves.
    pub(crate) fn repo_probe_gate(&self, key: &str) -> ProbeGate {
        let Ok(mut state) = self.state.lock() else {
            return ProbeGate::default();
        };
        // A gate nobody holds any more takes its answer with it, this
        // key's included. That is what keeps a refusal from outliving the
        // burst it answered: the next caller to arrive alone builds a
        // fresh gate and asks GitHub again.
        state
            .repo_probe_gates
            .retain(|_, gate| Arc::strong_count(gate) > 1);
        Arc::clone(state.repo_probe_gates.entry(key.to_string()).or_default())
    }

    /// Record what a repo lookup said about a `(repo, token)` pair.
    pub(crate) fn remember_repo_readable(&self, key: &str, readable: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.repo_readable.insert(key, readable);
        }
    }

    /// Read the cached ETag for `key`, if any. Guard scope is this call only.
    pub(crate) fn cached_etag(&self, key: &str) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.etag_cache.get(key).cloned())
    }

    /// Store the ETag observed for `key`. Guard scope is this call only.
    pub(crate) fn store_etag(&self, key: &str, etag: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.etag_cache.insert(key, etag.to_string());
        }
    }

    /// Read the cached ETag **plus payload** for `key`, marking the entry
    /// as most recently used. Returns a clone: the caller holds the payload
    /// across its request, so a concurrent eviction can never leave it with
    /// an ETag whose body is gone.
    pub(crate) fn cached_response(&self, key: &str) -> Option<CachedResponse> {
        let mut state = self.state.lock().ok()?;
        let hit = state.response_cache.get(key).cloned()?;
        if let Some(pos) = state.response_order.iter().position(|k| k == key) {
            state.response_order.remove(pos);
        }
        state.response_order.push_back(key.to_string());
        Some(hit)
    }

    /// Store an ETag together with the payload it was observed with, then
    /// evict least-recently-used entries until the cache fits its budget.
    pub(crate) fn store_response(&self, key: &str, etag: &str, payload: CachedPayload) {
        let budget = self.cache_budget;
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        // The previous entry for this key goes first, and unconditionally:
        // what we just observed supersedes it, so keeping it around when
        // the new payload does not fit would leave an ETag for content
        // that has since changed. Harmless on the wire (it revalidates and
        // gets a 200) but it is dead state, and it would sit there until
        // unrelated LRU pressure happened to reach it.
        if let Some(previous) = state.response_cache.remove(key) {
            state.response_bytes = state
                .response_bytes
                .saturating_sub(previous.payload.size_bytes());
            if let Some(pos) = state.response_order.iter().position(|k| k == key) {
                state.response_order.remove(pos);
            }
        }
        // A payload larger than the whole budget would be inserted and
        // immediately evicted; skip the churn.
        let size = payload.size_bytes();
        if size > budget {
            return;
        }
        state.response_bytes += size;
        state.response_cache.insert(
            key.to_string(),
            CachedResponse {
                etag: etag.to_string(),
                payload,
            },
        );
        state.response_order.push_back(key.to_string());
        state.evict_to(budget);
    }

    /// True when a 403 response is GitHub *rate limiting* rather than a
    /// permission refusal: primary exhaustion answers 403 with
    /// `x-ratelimit-remaining: 0`, secondary limits answer 403 with a
    /// `retry-after` header. Write methods keep those on the generic
    /// [`GithubError::Api`] path instead of [`GithubError::WriteDenied`] — a
    /// "no write access" message for a transient limit would mislead.
    pub(crate) fn forbidden_is_rate_limit(response: &reqwest::Response) -> bool {
        response.headers().contains_key("retry-after")
            || response
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                == Some("0")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn set_env(key: &str, value: &str) {
        // SAFETY: std locks its own environment access, so the hazard is C code
        // reading the environment on another thread. The tests in this crate talk
        // to mock servers on 127.0.0.1 and resolve no host names, the C path that
        // would read it.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: as in `set_env`.
        unsafe { std::env::remove_var(key) };
    }

    // `GITHUB_API_BASE` mutates process-global env, which every test in this
    // binary shares, so the env-sensitive assertions live in this ONE test.
    // No other test may read or write that var — not even defensively — because
    // a `remove_var` racing this test's `set_var` makes it fail intermittently.
    // Tests that need a specific base URL use `with_base_url`/`set_base_url`.
    #[test]
    fn env_base_url_is_read_at_construction_and_trimmed() {
        // Unset: falls back to api.github.com.
        remove_env("GITHUB_API_BASE");
        let c = GithubClient::new().unwrap();
        assert_eq!(c.api_base, "https://api.github.com");

        // Set with a trailing slash: trimmed.
        set_env("GITHUB_API_BASE", "https://ghe.example.test/api/v3/");
        let c = GithubClient::new().unwrap();
        assert_eq!(c.api_base, "https://ghe.example.test/api/v3");

        // Blank / whitespace-only: ignored, back to the default.
        set_env("GITHUB_API_BASE", "   ");
        let c = GithubClient::new().unwrap();
        assert_eq!(c.api_base, "https://api.github.com");

        remove_env("GITHUB_API_BASE");
    }

    #[test]
    fn set_base_url_trims_trailing_slash() {
        let mut c = GithubClient::new().unwrap();
        c.set_base_url("http://127.0.0.1:1234/");
        assert_eq!(c.api_base, "http://127.0.0.1:1234");
        let c = c.with_base_url("http://127.0.0.1:9999///");
        assert_eq!(c.api_base, "http://127.0.0.1:9999");
    }

    #[test]
    fn default_headers_carry_the_shared_set() {
        let c = GithubClient::new().unwrap();
        let headers = c.default_headers(Some("tok")).unwrap();
        assert_eq!(headers.get(USER_AGENT).unwrap(), USER_AGENT_VALUE);
        assert_eq!(headers.get(ACCEPT).unwrap(), "application/vnd.github+json");
        assert_eq!(
            headers.get("X-GitHub-Api-Version").unwrap(),
            GITHUB_API_VERSION
        );
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer tok");

        // Without a token there is no Authorization header at all.
        let anon = c.default_headers(None).unwrap();
        assert!(anon.get(AUTHORIZATION).is_none());
    }

    #[test]
    fn malformed_token_is_invalid_token_error() {
        let c = GithubClient::new().unwrap();
        // An embedded newline can't form a valid header value.
        let err = c
            .default_headers(Some("bad\ntoken"))
            .expect_err("malformed token must error");
        assert!(matches!(err, GithubError::InvalidToken(_)));
        assert!(
            err.to_string()
                .contains("not valid in an HTTP header value"),
            "message must name the real cause: {err}"
        );
    }

    #[test]
    fn a_bounded_map_evicts_in_insertion_order_and_keeps_its_order_intact() {
        let mut map = BoundedMap::with_cap(3);
        for key in ["a", "b", "c"] {
            map.insert(key, key.to_string());
        }
        // Refreshing an existing key updates it without queueing it again.
        map.insert("a", "a2".to_string());
        assert_eq!(map.order.len(), map.entries.len());
        assert_eq!(map.get("a"), Some(&"a2".to_string()));

        map.insert("d", "d".to_string());
        assert_eq!(map.len(), 3);
        assert_eq!(map.order.len(), map.entries.len());
        assert!(map.get("a").is_none(), "the oldest insertion goes first");
        for key in ["b", "c", "d"] {
            assert!(map.get(key).is_some(), "{key} was evicted out of turn");
        }
    }

    #[test]
    fn the_key_caches_stop_growing_at_their_cap() {
        let client = GithubClient::new().unwrap();
        // One key per (url, token identity): the same URL read on behalf
        // of ever more users is exactly how this map grows in a
        // long-running editor process.
        let url = "https://api.github.test/repos/example-org/corpus-example";
        for i in 0..(KEY_CACHE_MAX_ENTRIES + 50) {
            let key = GithubClient::cache_key(url, None, Some(&format!("token-{i}")));
            client.store_etag(&key, "\"etag\"");
            client.remember_repo_readable(&key, true);
        }

        let first = GithubClient::cache_key(url, None, Some("token-0"));
        let last = GithubClient::cache_key(
            url,
            None,
            Some(&format!("token-{}", KEY_CACHE_MAX_ENTRIES + 49)),
        );
        let state = client.state.lock().unwrap();
        assert_eq!(state.etag_cache.len(), KEY_CACHE_MAX_ENTRIES);
        assert_eq!(state.repo_readable.len(), KEY_CACHE_MAX_ENTRIES);
        assert!(state.etag_cache.get(&first).is_none());
        assert!(state.repo_readable.get(&first).is_none());
        assert!(state.etag_cache.get(&last).is_some());
        assert!(state.repo_readable.get(&last).is_some());
    }

    #[test]
    fn an_over_budget_payload_drops_the_entry_it_supersedes() {
        let mut client = GithubClient::new().unwrap();
        client.set_cache_budget_bytes(64);

        client.store_response(
            "key",
            "\"old\"",
            CachedPayload::Raw("small".repeat(2).to_string()),
        );
        assert!(client.cached_response("key").is_some());

        // The same path, now too big to cache. Keeping the old ETag would
        // leave the cache pointing at content that has been superseded.
        client.store_response("key", "\"new\"", CachedPayload::Raw("x".repeat(500)));
        assert!(
            client.cached_response("key").is_none(),
            "the superseded entry must be gone, not waiting for LRU pressure"
        );
        let state = client.state.lock().unwrap();
        assert_eq!(state.response_bytes, 0);
    }

    #[tokio::test]
    async fn etag_roundtrip_sends_if_none_match_and_handles_304() {
        let server = MockServer::start().await;

        // First response carries an ETag; second request must echo it back
        // via If-None-Match and gets a 304. The first mock is capped at one
        // hit (`up_to_n_times`) so the second request — which also matches its
        // looser matcher — falls through to the header-specific 304 mock.
        Mock::given(method("GET"))
            .and(path("/probe"))
            .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"abc\""))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/probe"))
            .and(header("if-none-match", "\"abc\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let client = GithubClient::new().unwrap().with_base_url(server.uri());
        let url = format!("{}/probe", client.api_base);

        // First call: no cached etag, store the one we get back.
        assert!(client.cached_etag(&url).is_none());
        let headers = client.default_headers(None).unwrap();
        let resp = client
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        if let Some(etag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
            client.store_etag(&url, etag);
        }
        assert_eq!(client.cached_etag(&url).as_deref(), Some("\"abc\""));

        // Second call: send the cached etag; server answers 304.
        let mut headers = client.default_headers(None).unwrap();
        let etag = client.cached_etag(&url).unwrap();
        headers.insert(
            reqwest::header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).unwrap(),
        );
        let resp = client
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 304);
    }
}
