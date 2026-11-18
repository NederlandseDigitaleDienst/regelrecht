//! API-only [`RepoBackend`] implementation against the GitHub REST API.
//!
//! No local git clone, no `/tmp` working tree. Reads go straight through
//! the Contents API; writes buffer in memory and flush as one Contents
//! API PUT/DELETE per file in [`persist`]. The branch is created lazily
//! in [`ensure_ready`] when it doesn't yet exist on the remote
//! ("traject branch on first activation").
//!
//! ## Atomicity
//!
//! The Contents API commits each PUT/DELETE separately. All current
//! editor flows ([`save_law`], [`save_scenario`], [`delete_scenario`],
//! [`save_annotations`]) write exactly one file per persist, so they are
//! effectively atomic. A future multi-file save would need the Git Data
//! API (blob → tree → commit → ref update) to land both files in a
//! single commit; the implementation here would surface partial-failure
//! state.
//!
//! ## Optimistic concurrency
//!
//! Each pending write carries the blob SHA that was current when the
//! caller last read the file (or `None` if it was never read — e.g. a
//! brand-new file). On the PUT, GitHub returns 409 if the SHA is stale.
//! `persist` then re-reads the SHA and retries **once**; a second 409 is
//! surfaced as [`CorpusError::Conflict`] for the caller to deal with.
//! For `save_annotations`'s append-only flow this is safe because dedup
//! happens against the freshly-read base before re-writing.
//!
//! ### Known gap: a branch minted by someone else
//!
//! A read that lands before the traject branch exists is told "absent",
//! and the branch is then seeded from base, so the file may hold base
//! content the read never saw. `persist` refuses to adopt such a file
//! (`may_adopt_existing` in [`try_put`]) — but only when *this* backend
//! instance minted the branch. A second replica or session minting it
//! between this instance's read and its persist leaves the same window
//! open, and nothing here notices.
//!
//! Closing it takes a compare-and-set against the branch tip, which the
//! Contents API does not offer: its `sha` parameter guards the blob, not
//! the ref. That means moving writes to the Git Data API (blob → tree →
//! commit → `PATCH /git/refs` with the expected old sha), the same rework
//! the multi-file atomicity gap above needs. Until then the gap is real
//! and unguarded; it is narrow (the branch is minted on first activation,
//! by whoever gets there first) and it fails loudly on the far more
//! common single-instance path.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use tokio::sync::Mutex;

use regelrecht_github::{Committer, GithubClient, GithubError};

use crate::backend::{
    FileEntry, PersistOutcome, RecursiveFileEntry, RepoBackend, WriteContext, normalized_subpath,
    validate_relative_path,
};
use crate::error::{CorpusError, Result};
use crate::models::GitHubSource;
use crate::timing;

/// Pending change buffered between `write_file`/`delete_file` and
/// `persist`. `base_sha` is `Some` when the caller read the file first
/// (handlers that do read-modify-write, like `save_annotations`); for
/// blind writes/deletes the backend resolves the SHA lazily at persist.
#[derive(Debug, Clone)]
struct PendingWrite {
    op: PendingOp,
    base_sha: Option<String>,
    /// Whether a read of this path in this backend answered "not there"
    /// before the write was buffered. Only such a write can be misled by
    /// a branch that gets minted from base in between — see `persist`.
    /// A blind write (`save_law`) never read, so it keeps its
    /// write-what-I-say semantics.
    read_saw_absence: bool,
}

#[derive(Debug, Clone)]
enum PendingOp {
    Upsert(String),
    Delete,
    /// No write: refuse the persist if the path turned up on a branch this
    /// persist created, after a read here saw it absent (see
    /// [`RepoBackend::expect_still_absent`]).
    AssertAbsent,
}

/// Mutable state guarded by the backend's mutex. The shared `GithubClient`
/// lives here alongside the SHA cache + pending buffer so a single guard
/// covers a whole read/persist cycle; the client itself uses interior
/// mutability (all its methods take `&self`).
struct Inner {
    client: GithubClient,
    /// Map from in-repo path (the path the Contents API addresses, with
    /// any `sub_path` prefix) → most recently observed blob SHA. Keyed by
    /// in-repo path rather than source-relative path so source-relative
    /// and repository-root-relative access share one cache and one
    /// pending buffer without colliding.
    /// Populated by `read_file`. On persist: entries for paths that were
    /// written are refreshed with the post-commit SHA; entries for paths
    /// that were deleted are removed. Stale entries for paths neither
    /// written nor deleted may linger — the next write's 409/retry path
    /// covers that, so it stays correct.
    sha_cache: HashMap<String, String>,
    /// Paths whose most recent read in this backend answered "not there".
    /// Feeds [`PendingWrite::read_saw_absence`]; a read that does find the
    /// file removes its entry again. Keyed by in-repo path, like
    /// `sha_cache`.
    absent_reads: std::collections::HashSet<String>,
    /// Buffered writes/deletes by in-repo path, in insertion order.
    ///
    /// All three collections are keyed by strings built only by
    /// `repo_path_str` / `api_path`, so a key is always a validated,
    /// forward-slash path inside the repository.
    pending: Vec<(String, PendingWrite)>,
    /// Whether the target branch is known to exist. Set by a successful
    /// `ensure_ready` (rest-token bootstrap) or by the lazy bootstrap in
    /// `persist`. A token-less backend skips branch creation at
    /// `ensure_ready`, so the first user-token write mints the branch
    /// itself — this flag keeps that to one round-trip per backend.
    branch_ready: bool,
}

pub struct GitHubApiBackend {
    owner: String,
    repo: String,
    branch: String,
    /// Branch to seed the target branch from when it doesn't exist yet
    /// (default for the editor traject flow: the writable repo's
    /// default branch — `main` for the regelrecht-corpus repo).
    base_branch: Option<String>,
    /// Path prefix inside the repo (same role as `repo_subpath` on
    /// `GitBackend`). Source-relative paths are joined under this.
    sub_path: Option<String>,
    /// OAuth/PAT token for the API. `None` makes the backend read-only
    /// **at rest** (`is_writable` = false): writes can then still land via
    /// a per-call `WriteContext::token_override` (the acting editor user's
    /// own GitHub token) — `persist` refuses only when *neither* token is
    /// present.
    token: Option<String>,
    inner: Mutex<Inner>,
}

impl GitHubApiBackend {
    /// A backend for `github`, reading and writing through `client`.
    pub fn new(
        github: &GitHubSource,
        base_branch: Option<String>,
        token: Option<String>,
        client: GithubClient,
    ) -> Self {
        Self {
            owner: github.owner.clone(),
            repo: github.repo.clone(),
            branch: github.effective_ref().to_string(),
            base_branch,
            sub_path: github.path.clone(),
            token,
            inner: Mutex::new(Inner {
                client,
                sha_cache: HashMap::new(),
                absent_reads: std::collections::HashSet::new(),
                pending: Vec::new(),
                branch_ready: false,
            }),
        }
    }

    fn full_repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Resolve a source-relative path to the in-repo path the GitHub API
    /// expects (with `sub_path` prefix). Forward slashes always — GitHub
    /// is OS-agnostic.
    fn api_path(&self, relative: &Path) -> Result<String> {
        let rel = repo_path_str(relative)?;
        Ok(match normalized_subpath(self.sub_path.as_deref()) {
            Some(sub) => format!("{sub}/{rel}"),
            None => rel,
        })
    }

    /// Inverse of [`api_path`]: strip the `sub_path` prefix from an in-repo
    /// path to recover the source-relative path used by `SourceMap` /
    /// `LoadedLaw`. Returns `None` for paths outside `sub_path` (files that
    /// aren't part of this source's corpus subtree — e.g. repo-root config
    /// when the corpus lives under `regulation/nl`).
    fn to_source_relative(&self, in_repo_path: &str) -> Option<String> {
        match normalized_subpath(self.sub_path.as_deref()) {
            Some(sub) => in_repo_path
                .strip_prefix(&format!("{sub}/"))
                .map(str::to_string),
            None => Some(in_repo_path.to_string()),
        }
    }

    /// One Contents API GET of an in-repo path, recording the blob SHA (or
    /// the absence) for a later write of the same path.
    async fn read_in_repo(
        &self,
        api_path: String,
        token_override: Option<&str>,
    ) -> Result<Option<String>> {
        let mut inner = self.inner.lock().await;
        // One Contents API GET — the If-Match precondition read on the save
        // path lands here, so it feeds the `gh_get` Server-Timing phase.
        let outcome = timing::measure(
            "gh_get",
            inner.client.fetch_file_with_sha(
                &self.full_repo(),
                &self.branch,
                &api_path,
                token_override.or(self.token.as_deref()),
            ),
        )
        .await?;
        match outcome {
            Some((content, sha)) => {
                inner.absent_reads.remove(&api_path);
                inner.sha_cache.insert(api_path, sha);
                Ok(Some(content))
            }
            None => {
                // Remove any stale SHA from a previous existence — a
                // later write will (correctly) be treated as a create.
                inner.sha_cache.remove(&api_path);
                inner.absent_reads.insert(api_path);
                Ok(None)
            }
        }
    }

    /// Buffer an operation on an in-repo path for the next `persist`,
    /// carrying what this backend's last read of that path observed.
    async fn stage(&self, api_path: String, op: PendingOp) {
        let mut inner = self.inner.lock().await;
        let base_sha = inner.sha_cache.get(&api_path).cloned();
        let read_saw_absence = inner.absent_reads.contains(&api_path);
        inner.pending.push((
            api_path,
            PendingWrite {
                op,
                base_sha,
                read_saw_absence,
            },
        ));
    }

    /// Fetch the current SHA for a path on the target branch. Used by
    /// `persist` when the caller never read the file first (blind write
    /// to an existing file). Returns `Ok(None)` on 404 — the caller can
    /// then treat the PUT as a create.
    async fn fetch_sha(
        client: &GithubClient,
        repo: &str,
        branch: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<String>> {
        match client
            .fetch_file_with_sha(repo, branch, path, token)
            .await?
        {
            Some((_, sha)) => Ok(Some(sha)),
            None => Ok(None),
        }
    }

    /// Serve the corpus-wide implements map from the precomputed index at
    /// the repo root ([`implements_index`]), or `Ok(None)` when this ref
    /// has no index that provably describes it.
    ///
    /// The index is fetched with the **raw** representation: at 22k entries
    /// it is comfortably past the Contents API's 1 MiB JSON ceiling, where
    /// the JSON path returns no content at all.
    ///
    /// Freshness is verified the same way the checkout-based backend does
    /// it, one layer up: the index records the git tree sha of the subtree
    /// it scanned, and that is compared against the same subtree on this
    /// backend's branch. Equal means the index describes exactly this
    /// content — which makes the check branch-aware by construction, so a
    /// preview corpus branch that moved past its last index regeneration is
    /// caught. Anything else (no index, unparseable, rooted elsewhere,
    /// unverifiable, or simply stale) returns `None` so the caller falls
    /// back rather than serving a map that may be wrong.
    ///
    /// A read failure is an `Err`, never an empty map: the consumer uses
    /// this map as a negative cache, so "I could not read the index" and
    /// "these laws implement nothing" must not arrive as the same answer.
    ///
    /// [`implements_index`]: crate::implements_index
    async fn implements_from_index(&self) -> Result<Option<Vec<(String, Vec<String>)>>> {
        use crate::implements_index::{IMPLEMENTS_INDEX_FILENAME, ImplementsIndex};

        let repo = self.full_repo();
        let inner = self.inner.lock().await;

        let raw = inner
            .client
            .fetch_file_raw_opt(
                &repo,
                &self.branch,
                IMPLEMENTS_INDEX_FILENAME,
                self.token.as_deref(),
            )
            .await?;
        let Some(raw) = raw else {
            tracing::warn!(
                repo = %repo,
                branch = %self.branch,
                "no implements index on this corpus branch; falling back to the archive scan"
            );
            return Ok(None);
        };

        let index = match ImplementsIndex::parse(&raw) {
            Ok(index) => index,
            Err(e) => {
                tracing::warn!(
                    repo = %repo,
                    error = %e,
                    "implements index is unreadable; falling back to the archive scan"
                );
                return Ok(None);
            }
        };

        // An index rooted elsewhere says nothing about this source. Serving
        // its projection would yield an empty list, which the caller cannot
        // tell apart from an authoritative "this corpus holds no laws".
        if !index.covers(self.sub_path.as_deref()) {
            tracing::warn!(
                index_root = %index.root,
                source_root = self.sub_path.as_deref().unwrap_or("<repo root>"),
                "implements index does not cover this source's root; \
                 falling back to the archive scan"
            );
            return Ok(None);
        }

        match inner
            .client
            .subtree_sha(&repo, &self.branch, &index.root, self.token.as_deref())
            .await?
        {
            Some(remote_sha) if remote_sha == index.tree_sha => {
                let entries = index.to_source_relative(self.sub_path.as_deref());
                tracing::info!(
                    entries = entries.len(),
                    root = %index.root,
                    tree = %index.tree_sha,
                    branch = %self.branch,
                    "served implements map from the precomputed index"
                );
                Ok(Some(entries))
            }
            Some(remote_sha) => {
                tracing::warn!(
                    index_tree = %index.tree_sha,
                    remote_tree = %remote_sha,
                    root = %index.root,
                    branch = %self.branch,
                    "implements index does not match this branch's content \
                     (stale index on this corpus branch?); falling back to the archive scan"
                );
                Ok(None)
            }
            None => {
                tracing::warn!(
                    root = %index.root,
                    branch = %self.branch,
                    "implements index freshness could not be verified (root not found \
                     on this branch); falling back to the archive scan"
                );
                Ok(None)
            }
        }
    }

    /// Ensure `branch` exists on `repo`, creating it from `base_branch`
    /// when missing. Shared by `ensure_ready` (rest-token bootstrap at
    /// backend init) and `persist` (lazy bootstrap with the per-call
    /// user token, for backends that had no token at init).
    ///
    /// Also reused by the editor-api create-traject flow to mint the
    /// traject branch eagerly, while it still holds the token that just
    /// proved push access — closing the "fresh traject is
    /// dead-on-arrival because its branch does not exist yet" gap. The
    /// benign-race handling below makes eager and lazy bootstrap safe to
    /// coexist: whichever runs second sees the branch and no-ops.
    ///
    /// `repo` is the `owner/name` form; `token` is `None` only for
    /// anonymous public reads (branch creation always needs one).
    ///
    /// Returns whether the branch was **created** by this call. That
    /// matters to `persist`: a branch minted from base carries the base's
    /// files, which the reads leading up to this write did not see (they
    /// were made against a ref that did not exist and answered "absent").
    pub async fn ensure_branch(
        client: &GithubClient,
        repo: &str,
        branch: &str,
        base_branch: Option<&str>,
        token: Option<&str>,
    ) -> Result<bool> {
        let exists = client.branch_exists(repo, branch, token).await?;
        if exists {
            return Ok(false);
        }
        let base = base_branch.ok_or_else(|| {
            CorpusError::Config(format!(
                "branch '{}' does not exist on {} and no base_branch \
                 was configured to seed it from",
                branch, repo
            ))
        })?;
        // TOCTOU on lazy branch creation: between our `branch_exists`
        // returning false and this POST, another activation (different
        // backend instance, same traject) can win the race and create
        // the branch first. GitHub then 422s us with "Reference already
        // exists". Re-check `branch_exists` on any create_branch failure;
        // if the branch is present now the desired post-condition holds
        // and we treat the create as a benign no-op.
        match client.create_branch(repo, branch, base, token).await {
            Ok(()) => {
                tracing::info!(
                    repo = %repo,
                    branch = %branch,
                    base = %base,
                    "GitHubApiBackend: created traject branch from base"
                );
            }
            Err(e) => {
                let now_exists = client
                    .branch_exists(repo, branch, token)
                    .await
                    .unwrap_or(false);
                if now_exists {
                    tracing::info!(
                        repo = %repo,
                        branch = %branch,
                        "GitHubApiBackend: create_branch lost a benign race; branch already exists"
                    );
                    // Someone else minted it, so it was not created *here*
                    // — but it did appear after our reads, so the caller
                    // must treat it exactly like our own create.
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(true)
    }
}

/// Validate a relative path (no absolute paths, no `..`) and render it the
/// way the Contents API wants it: UTF-8, forward slashes.
fn repo_path_str(path: &Path) -> Result<String> {
    validate_relative_path(path)?;
    Ok(path
        .to_str()
        .ok_or_else(|| CorpusError::Config(format!("path is not valid UTF-8: {}", path.display())))?
        .replace('\\', "/"))
}

#[async_trait]
impl RepoBackend for GitHubApiBackend {
    #[tracing::instrument(name = "gh_read_file", skip_all, fields(path = %relative_path.display()))]
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        self.read_file_with_token(relative_path, None).await
    }

    // The read-side counterpart of `persist`'s `ctx.token_override`: a
    // per-call user token supersedes the backend's baked-in token for this
    // one Contents API GET, so a request-bound read on a private repo
    // without a configured service token can authenticate as the acting
    // editor user. Absent an override the configured token (or none) is
    // used — byte-identical to `read_file`.
    async fn read_file_with_token(
        &self,
        relative_path: &Path,
        token_override: Option<&str>,
    ) -> Result<Option<String>> {
        self.read_in_repo(self.api_path(relative_path)?, token_override)
            .await
    }

    fn repo_subpath(&self) -> Option<&str> {
        normalized_subpath(self.sub_path.as_deref())
    }

    // Same Contents API GET, addressed from the repository root: the path
    // is not prefixed with `sub_path`.
    async fn read_repo_file_with_token(
        &self,
        repo_path: &Path,
        token_override: Option<&str>,
    ) -> Result<Option<String>> {
        self.read_in_repo(repo_path_str(repo_path)?, token_override)
            .await
    }

    // Buffered in the same pending list as source-relative writes, so the
    // next `persist` commits it to the same branch.
    async fn write_repo_file(&self, repo_path: &Path, content: &str) -> Result<()> {
        self.stage(
            repo_path_str(repo_path)?,
            PendingOp::Upsert(content.to_string()),
        )
        .await;
        Ok(())
    }

    async fn expect_still_absent(&self, relative_path: &Path) -> Result<()> {
        self.stage(self.api_path(relative_path)?, PendingOp::AssertAbsent)
            .await;
        Ok(())
    }

    /// The corpus-wide implements map, from the precomputed index at the
    /// repo root when this branch has a verifiably matching one, and from
    /// the repo archive otherwise.
    ///
    /// The index is what makes this backend usable for a whole central
    /// corpus: it is two small requests, against a tarball of hundreds of
    /// megabytes that has to be held in memory to be extracted. The archive
    /// remains the fallback because being slow beats being wrong — a
    /// missing or stale index must not turn into a map that reads as
    /// authoritative.
    ///
    /// Archive path: one tarball download for the whole source instead of a
    /// Contents call per file. Bodies are parsed for `implements` and
    /// discarded during extraction (see [`fetch_archive_implements`]), so
    /// the *parsed* result never holds the corpus. Archive paths are
    /// repo-relative; map them back through [`to_source_relative`] (drops
    /// files outside `sub_path`) so they match the `SourceMap`.
    ///
    /// Holds the `inner` lock for the whole archive download + extraction
    /// (seconds for a large corpus), same as [`read_file`] holds it for its
    /// Contents call. A concurrent read on the same backend stalls for the
    /// duration, but in practice this only fires on the cold implements-index
    /// build, which is single-flighted anyway.
    ///
    /// [`fetch_archive_implements`]: crate::github::fetch_archive_implements
    /// [`to_source_relative`]: GitHubApiBackend::to_source_relative
    /// [`read_file`]: GitHubApiBackend::read_file
    async fn read_all_implements(&self) -> Result<Vec<(String, Vec<String>)>> {
        match self.implements_from_index().await {
            Ok(Some(entries)) => return Ok(entries),
            // Every "no usable index" case is already logged with its
            // reason inside `implements_from_index`.
            Ok(None) => {}
            Err(e) => tracing::warn!(
                repo = %self.full_repo(),
                branch = %self.branch,
                error = %e,
                "implements index could not be read; falling back to the archive scan"
            ),
        }

        let files = {
            let inner = self.inner.lock().await;
            crate::github::fetch_archive_implements(
                &inner.client,
                &self.full_repo(),
                &self.branch,
                self.token.as_deref(),
            )
            .await?
        };
        Ok(files
            .into_iter()
            .filter_map(|(repo_path, implements)| {
                let rel = self.to_source_relative(&repo_path)?;
                Some((rel, implements))
            })
            .collect())
    }

    // No token guard here: a backend without a rest-token can still
    // commit via `WriteContext::token_override` (per-user GitHub OAuth),
    // and the override only becomes visible at `persist`. Buffer now;
    // `persist` refuses with `ReadOnly` when neither token is present.
    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        self.stage(
            self.api_path(relative_path)?,
            PendingOp::Upsert(content.to_string()),
        )
        .await;
        Ok(())
    }

    // Same stance as `write_file`: token enforcement lives in `persist`.
    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        self.stage(self.api_path(relative_path)?, PendingOp::Delete)
            .await;
        Ok(())
    }

    #[tracing::instrument(name = "gh_list_files", skip_all, fields(dir = %dir.display()))]
    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        self.list_files_with_token(dir, extension, None).await
    }

    // Same per-call token override stance as `read_file_with_token`.
    async fn list_files_with_token(
        &self,
        dir: &Path,
        extension: Option<&str>,
        token_override: Option<&str>,
    ) -> Result<Vec<FileEntry>> {
        let api_dir = self.api_path(dir)?;
        let inner = self.inner.lock().await;
        // One Contents API directory GET — feeds the `gh_list` Server-
        // Timing phase so listing round-trips (scenario listings) are
        // visible next to `gh_get` instead of vanishing into `total`.
        let entries = timing::measure(
            "gh_list",
            inner.client.list_directory(
                &self.full_repo(),
                &self.branch,
                &api_dir,
                token_override.or(self.token.as_deref()),
            ),
        )
        .await?;
        let mut out: Vec<FileEntry> = entries
            .into_iter()
            .filter(|e| e.entry_type == "file")
            .filter(|e| match extension {
                None => true,
                Some(ext) => Path::new(&e.name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == ext),
            })
            .map(|e| FileEntry { name: e.name })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn list_files_recursive(
        &self,
        dir: &Path,
        extension: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        self.list_files_recursive_with_token(dir, extension, None)
            .await
    }

    // Same per-call token override stance as `read_file_with_token`.
    async fn list_files_recursive_with_token(
        &self,
        dir: &Path,
        extension: Option<&str>,
        token_override: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        let api_root = self.api_path(dir)?;
        let inner = self.inner.lock().await;

        // Iterative DFS (LIFO `Vec` used as a stack) over Contents-API
        // directory pages. Traversal order doesn't matter — the result is
        // sorted afterwards. Each tuple is
        // (`relative path under the listing root`, `full API path`); the
        // empty prefix on the seed means the seed directory's direct
        // children appear with bare filenames in the output.
        //
        // **GitHub limit**: the Contents API caps each directory listing
        // at 1000 entries with no pagination. A `documents/<traject>/`
        // folder is extremely unlikely to hit that, but if it ever does
        // the listing truncates silently. Switching to the Git Trees API
        // (`/git/trees/{sha}?recursive=1`) is the proper fix when the
        // need arises — it returns the entire subtree in one call.
        let mut queue: Vec<(String, String)> = vec![(String::new(), api_root)];
        let mut out: Vec<RecursiveFileEntry> = Vec::new();

        while let Some((rel_prefix, api_dir)) = queue.pop() {
            let entries = inner
                .client
                .list_directory(
                    &self.full_repo(),
                    &self.branch,
                    &api_dir,
                    token_override.or(self.token.as_deref()),
                )
                .await?;
            // The Contents API caps a single directory listing at 1000
            // entries with no pagination (see the note above). Hitting the
            // cap means the listing was almost certainly truncated and some
            // documents are silently missing — surface it in logs so it is
            // diagnosable before it becomes a support incident.
            if entries.len() >= 1000 {
                tracing::warn!(
                    api_dir = %api_dir,
                    count = entries.len(),
                    "GitHub Contents API directory listing hit the 1000-entry cap; results may be truncated"
                );
            }
            for e in entries {
                let child_rel = if rel_prefix.is_empty() {
                    e.name.clone()
                } else {
                    format!("{}/{}", rel_prefix, e.name)
                };
                match e.entry_type.as_str() {
                    "file" => {
                        if let Some(ext) = extension {
                            let matches = Path::new(&e.name)
                                .extension()
                                .and_then(|s| s.to_str())
                                .is_some_and(|s| s == ext);
                            if !matches {
                                continue;
                            }
                        }
                        out.push(RecursiveFileEntry {
                            relative_path: child_rel,
                        });
                    }
                    "dir" => {
                        let child_api = if api_dir.is_empty() {
                            e.name
                        } else {
                            format!("{}/{}", api_dir.trim_end_matches('/'), e.name)
                        };
                        queue.push((child_rel, child_api));
                    }
                    _ => continue,
                }
            }
        }

        out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(out)
    }

    #[tracing::instrument(name = "gh_persist", skip_all)]
    async fn persist(&self, ctx: &WriteContext) -> Result<PersistOutcome> {
        let pending: Vec<(String, PendingWrite)> = {
            let mut inner = self.inner.lock().await;
            std::mem::take(&mut inner.pending)
        };
        if pending.is_empty() {
            return Ok(PersistOutcome::default());
        }
        // Wall-clock of the whole Contents API commit (PUT/DELETE, plus any
        // sha-refresh retry) — the `gh_put` Server-Timing phase. Best-effort,
        // same as `ensure_ready`: recorded only on the success path below, so
        // a `persist` that fails via `?` (e.g. a `try_put` error) yields a
        // response with no `gh_put` phase. `persist` is not a single future,
        // so it can't use the record-on-both-paths `timing::measure` wrapper.
        let put_start = std::time::Instant::now();

        // A per-call `token_override` (the acting editor user's own GitHub
        // OAuth token) supersedes the backend's baked-in token for this write,
        // so the commit authenticates *as the user* and GitHub enforces their
        // push rights. Absent an override we fall back to the configured token
        // — byte-identical to the pre-spike behaviour. Neither present =
        // read-only: this is where the token guard lives now that
        // `write_file`/`delete_file` buffer unconditionally (the override
        // only exists at persist time). The drained pending buffer is
        // dropped on this error, same as any other failing persist.
        let Some(token) = ctx.token_override.as_deref().or(self.token.as_deref()) else {
            return Err(CorpusError::ReadOnly(
                "GitHubApiBackend has no push token (neither configured nor per-user)".to_string(),
            ));
        };

        // With a user token the commit is left unattributed on our side:
        // the Contents API then defaults author/committer to the
        // authenticated GitHub account, which IS the acting editor —
        // overriding it with the session (Keycloak) identity would detach
        // the commit from their GitHub account. On the shared service
        // token the human is invisible behind the bot, so there we do
        // stamp the session identity, falling back to a service identity
        // when no human is attached — same shape as `GitBackend` (the
        // trailer/co-author is left empty rather than spoofed).
        let committer = if ctx.token_override.is_some() {
            None
        } else {
            Some(Committer {
                name: ctx
                    .author
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "regelrecht-editor".to_string()),
                email: ctx
                    .author
                    .as_ref()
                    .map(|a| a.email.clone())
                    .unwrap_or_else(|| "noreply@regelrecht.local".to_string()),
            })
        };
        let repo = self.full_repo();
        let mut new_shas: HashMap<String, String> = HashMap::new();

        // Take one lock guard for the whole loop so the shared-client calls
        // inside don't pay re-acquire cost per-write. The pending
        // buffer was already drained above; if any write fails we propagate
        // via `?` and the remaining entries are dropped. The Contents API
        // commits per file, so a multi-file persist (promote copies every
        // version plus the scenarios) that fails halfway leaves the files
        // before it committed on the branch — the caller sees the error
        // and the branch holds a prefix. Single-file saves, which is every
        // other handler, have nothing to recover.
        let mut inner = self.inner.lock().await;

        // Lazy branch bootstrap for the user-token write mode: a backend
        // without a rest-token skipped branch creation at `ensure_ready`
        // (it had nothing to authenticate with), so the first persist must
        // mint the traject branch itself — with the same effective token
        // that authenticates the commit.
        // Whether this persist is the one that minted the branch. A
        // branch seeded from base carries the base's files, which a read
        // made before it existed could not see: it asked a ref that was
        // not there and was told "absent". Upserting blindly over that
        // would silently replace base content — an append-only sidecar
        // (notes) would come back holding only the new entry. So a write
        // whose own read saw absence is create-only here.
        //
        // This catches the branch *we* mint, and only that one — see the
        // "known gap" section in the module docs for the window a branch
        // minted elsewhere leaves open, and what it takes to close it.
        let mut branch_just_created = false;
        if !inner.branch_ready {
            branch_just_created = Self::ensure_branch(
                &inner.client,
                &repo,
                &self.branch,
                self.base_branch.as_deref(),
                Some(token),
            )
            .await?;
            inner.branch_ready = true;
        }

        // Pending keys are already in-repo paths (see `Inner::sha_cache`).
        for (api_path, pw) in pending {
            match pw.op {
                PendingOp::Upsert(content) => {
                    let new_sha = try_put(
                        &inner.client,
                        &repo,
                        &self.branch,
                        &api_path,
                        &content,
                        pw.base_sha.as_deref(),
                        committer.as_ref(),
                        &ctx.message,
                        Some(token),
                        !(branch_just_created && pw.read_saw_absence),
                    )
                    .await?;
                    new_shas.insert(api_path, new_sha);
                }
                PendingOp::AssertAbsent => {
                    // Same window as the create-only upsert above, for a
                    // path the write was derived from rather than the one
                    // it writes. Nothing to check unless this persist
                    // minted the branch after our read saw absence.
                    if branch_just_created
                        && pw.read_saw_absence
                        && Self::fetch_sha(
                            &inner.client,
                            &repo,
                            &self.branch,
                            &api_path,
                            Some(token),
                        )
                        .await?
                        .is_some()
                    {
                        tracing::warn!(
                            repo = %repo,
                            path = %api_path,
                            "refusing a write built on a read that missed base content \
                             on a freshly created branch"
                        );
                        return Err(CorpusError::Conflict(format!(
                            "'{api_path}' already exists on the branch that was just created \
                             from base; the write was prepared against a branch that did not \
                             exist yet"
                        )));
                    }
                }
                PendingOp::Delete => {
                    let sha_for_delete = match &pw.base_sha {
                        Some(s) => s.clone(),
                        None => {
                            match Self::fetch_sha(
                                &inner.client,
                                &repo,
                                &self.branch,
                                &api_path,
                                Some(token),
                            )
                            .await?
                            {
                                Some(s) => s,
                                // Already gone: treat as a successful delete,
                                // same shape as `LocalBackend::delete_file`.
                                None => continue,
                            }
                        }
                    };
                    try_delete(
                        &inner.client,
                        &repo,
                        &self.branch,
                        &api_path,
                        &sha_for_delete,
                        committer.as_ref(),
                        &ctx.message,
                        Some(token),
                    )
                    .await?;
                    // Drop the cached SHA so a next read sees the file as
                    // gone (or rebuilt) without holding a stale value.
                    inner.sha_cache.remove(&api_path);
                }
            }
        }

        // Refresh the SHA cache so a follow-up read-modify-write cycle in
        // the same backend instance starts from the post-commit SHA.
        for (path, sha) in new_shas {
            inner.sha_cache.insert(path, sha);
        }

        timing::record("gh_put", put_start.elapsed());

        // Contents API commits straight to the configured branch — no
        // PR is opened. The trajectflow already accepted `pr: None`
        // from the previous `GitBackend` impl, so this is wire-
        // compatible with the existing save handlers.
        Ok(PersistOutcome::default())
    }

    // `persist` above authenticates every Contents-API call with
    // `ctx.token_override` when present — this is the one backend where a
    // per-user GitHub token actually changes who the write authenticates as.
    fn supports_token_override(&self) -> bool {
        true
    }

    #[tracing::instrument(name = "gh_ensure_ready", skip_all, fields(branch = %self.branch))]
    async fn ensure_ready(&mut self) -> Result<()> {
        // Backends without a rest-token have nothing to bootstrap here —
        // the branch either exists (reads work) or it doesn't (reads 404
        // as they'd 404 on a missing file), and we can't mint a branch
        // without a token. In the user-token write mode the first
        // `persist` bootstraps the branch instead, with the per-call
        // override token.
        if self.token.is_none() {
            return Ok(());
        }
        // Branch-check (+ lazy create) round-trips against GitHub; feeds
        // the `ensure_ready` Server-Timing phase on the cold-build path.
        let ready_start = std::time::Instant::now();
        let repo = format!("{}/{}", self.owner, self.repo);
        let branch = self.branch.clone();
        let base_branch = self.base_branch.clone();
        let token = self.token.clone();
        let inner = self.inner.get_mut();
        Self::ensure_branch(
            &inner.client,
            &repo,
            &branch,
            base_branch.as_deref(),
            token.as_deref(),
        )
        .await?;
        inner.branch_ready = true;
        timing::record("ensure_ready", ready_start.elapsed());
        Ok(())
    }

    fn is_writable(&self) -> bool {
        self.token.is_some()
    }

    async fn changed_files(&self) -> Result<Vec<String>> {
        self.changed_files_with_token(None).await
    }

    // Same per-call token override stance as `read_file_with_token`: a
    // request-bound user token supersedes the backend's baked-in one, so a
    // private traject repo without a configured service token can still be
    // diffed as the acting editor user.
    async fn changed_files_with_token(&self, token_override: Option<&str>) -> Result<Vec<String>> {
        // The diff is branch-against-base. Without a base to compare to,
        // or a token to read the (typically private) repo, there's nothing
        // meaningful to report — return empty rather than erroring.
        let Some(base) = self.base_branch.as_deref() else {
            return Ok(Vec::new());
        };
        let token = token_override.or(self.token.as_deref());
        if token.is_none() {
            return Ok(Vec::new());
        }
        let in_repo_paths = {
            let inner = self.inner.lock().await;
            inner
                .client
                .compare_files(&self.full_repo(), base, &self.branch, token)
                .await?
        };
        // Map in-repo paths back to source-relative paths, dropping any
        // that fall outside this source's `sub_path` subtree.
        Ok(in_repo_paths
            .into_iter()
            .filter_map(|p| self.to_source_relative(&p))
            .collect())
    }
}

/// PUT with one optimistic-concurrency retry: on 409 the SHA is refreshed
/// (the file moved between our last read and this PUT) and the put is
/// reattempted exactly once with the new SHA. A second 409 propagates so
/// the caller can decide between abort and a higher-level retry.
///
/// `may_adopt_existing` governs the *other* retry: a PUT without `sha`
/// against a file that turns out to exist. Normally that is adopted (the
/// caller wrote blind and the file was already there — `save_law` relies
/// on it). It is refused when the branch was just seeded from base,
/// because then "already there" means base content the caller never read,
/// and adopting it replaces that content instead of building on it.
///
/// The caller derives that flag from its own `ensure_branch` call, so a
/// branch minted by another instance does not set it — the module docs'
/// "known gap" section names what that leaves open.
#[allow(clippy::too_many_arguments)]
async fn try_put(
    client: &GithubClient,
    repo: &str,
    branch: &str,
    path: &str,
    content: &str,
    base_sha: Option<&str>,
    committer: Option<&Committer>,
    message: &str,
    token: Option<&str>,
    may_adopt_existing: bool,
) -> Result<String> {
    match client
        .put_file(
            repo, branch, path, content, base_sha, committer, message, token,
        )
        .await
    {
        Ok(sha) => Ok(sha),
        Err(GithubError::Conflict(_)) => {
            tracing::debug!(repo = %repo, path = %path, "PUT 409 — refreshing sha and retrying");
            let fresh = client
                .fetch_file_with_sha(repo, branch, path, token)
                .await?
                .map(|(_, sha)| sha);
            client
                .put_file(
                    repo,
                    branch,
                    path,
                    content,
                    fresh.as_deref(),
                    committer,
                    message,
                    token,
                )
                .await
                .map_err(Into::into)
        }
        Err(e) if is_unsigned_existing_file(&e) && !may_adopt_existing => {
            // The branch was minted from base during this persist, so the
            // file we are told already exists is base content that the
            // read before this write could not see. Overwriting it would
            // lose it silently; surface a conflict so the caller can redo
            // the read-modify-write against the branch that now exists.
            tracing::warn!(
                repo = %repo,
                path = %path,
                "refusing to overwrite base content on a freshly created branch"
            );
            Err(CorpusError::Conflict(format!(
                "'{path}' already exists on the branch that was just created from base; \
                 the write was prepared against a branch that did not exist yet"
            )))
        }
        Err(e) if is_unsigned_existing_file(&e) => {
            // A PUT without `sha` against an existing file returns 422
            // ("sha was not supplied"). Resolve the SHA and retry once
            // — covers `save_law` / `save_scenario` which call
            // `write_file` without a preceding `read_file`. This is the
            // extra GET→PUT round-trip a cold sha-cache pays; logged at
            // info with `gh_put_retry=422` so it can be counted in
            // `zad logs`. It stays inside the enclosing `gh_put` phase
            // (no separate header phase) so the Server-Timing breakdown
            // does not double-count this leg.
            tracing::info!(repo = %repo, path = %path, gh_put_retry = 422, "PUT 422 — fetching sha and retrying as update");
            let fresh = client
                .fetch_file_with_sha(repo, branch, path, token)
                .await?
                .map(|(_, sha)| sha);
            if fresh.is_none() {
                // 422 wasn't about an existing file after all — propagate
                // the original error so the operator can diagnose.
                return Err(e.into());
            }
            client
                .put_file(
                    repo,
                    branch,
                    path,
                    content,
                    fresh.as_deref(),
                    committer,
                    message,
                    token,
                )
                .await
                .map_err(Into::into)
        }
        Err(e) => Err(e.into()),
    }
}

/// DELETE with one optimistic-concurrency retry: same shape as `try_put`.
#[allow(clippy::too_many_arguments)]
async fn try_delete(
    client: &GithubClient,
    repo: &str,
    branch: &str,
    path: &str,
    sha: &str,
    committer: Option<&Committer>,
    message: &str,
    token: Option<&str>,
) -> Result<()> {
    match client
        .delete_file(repo, branch, path, sha, committer, message, token)
        .await
    {
        Ok(()) => Ok(()),
        Err(GithubError::Conflict(_)) => {
            tracing::debug!(repo = %repo, path = %path, "DELETE 409 — refreshing sha and retrying");
            let fresh = client
                .fetch_file_with_sha(repo, branch, path, token)
                .await?
                .map(|(_, sha)| sha);
            match fresh {
                Some(s) => client
                    .delete_file(repo, branch, path, &s, committer, message, token)
                    .await
                    .map_err(Into::into),
                // Race: file was deleted between our 409 and this
                // refetch. Treat as a successful delete.
                None => Ok(()),
            }
        }
        Err(e) => Err(e.into()),
    }
}

/// Best-effort detection that a PUT failed with 422 because we omitted
/// `sha` while the file already exists. The GitHub response text reliably
/// mentions `sha`, which rides along in the [`GithubError::Api`] message.
fn is_unsigned_existing_file(e: &GithubError) -> bool {
    match e {
        GithubError::Api {
            status: 422,
            message,
        } => message.contains("sha"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use wiremock::matchers::{method, path as path_matcher};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::GitHubApiBackend;
    use crate::backend::{EditorUser, RepoBackend, WriteContext};
    use crate::models::GitHubSource;

    /// Build a gzipped tar laid out like GitHub's tarball endpoint: every
    /// entry nested under a single top-level `{owner}-{repo}-{sha}/` dir.
    fn make_repo_tar_gz(top: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        for (rel, content) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, format!("{top}/{rel}"), content.as_bytes())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    /// A law body whose `machine_readable.implements` references `higher`.
    fn law_with_implements(id: &str, higher: &str) -> String {
        format!(
            "$id: {id}\narticles:\n  - number: '1'\n    machine_readable:\n      implements:\n        - law: {higher}\n"
        )
    }

    /// The `regulation/nl`-rooted source every implements test reads.
    fn nl_source() -> GitHubSource {
        GitHubSource {
            owner: "acme".to_string(),
            repo: "corpus".to_string(),
            branch: "main".to_string(),
            path: Some("regulation/nl".to_string()),
            git_ref: None,
        }
    }

    fn backend_at(server: &MockServer) -> GitHubApiBackend {
        GitHubApiBackend::new(
            &nl_source(),
            Some("main".to_string()),
            Some("tok".to_string()),
            regelrecht_github::GithubClient::from_base(Some(&server.uri())).unwrap(),
        )
    }

    /// A committed index over `regulation`, recording `tree_sha` for that
    /// subtree. One entry per file including the empty one, and one entry
    /// outside the source root that projection must drop.
    fn index_json(tree_sha: &str, root: &str) -> String {
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            "regulation/nl/wet/foo/2025-01-01.yaml".to_string(),
            vec!["wet_target".to_string()],
        );
        files.insert(
            "regulation/nl/wet/bar/2025-01-01.yaml".to_string(),
            Vec::new(),
        );
        files.insert(
            "regulation/be/wet/b/2025-01-01.yaml".to_string(),
            Vec::new(),
        );
        crate::implements_index::ImplementsIndex {
            version: crate::implements_index::IMPLEMENTS_INDEX_VERSION,
            root: root.to_string(),
            tree_sha: tree_sha.to_string(),
            files,
        }
        .to_json()
    }

    /// Serve the index file at the repo root for `main`.
    async fn mount_index(server: &MockServer, body: String) {
        Mock::given(method("GET"))
            .and(path_matcher(
                "/repos/acme/corpus/contents/implements-index.json",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(server)
            .await;
    }

    /// Serve the root tree of `main`, where `regulation` has sha `reg_sha`.
    async fn mount_root_tree(server: &MockServer, reg_sha: &str) {
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sha": "roottree",
                "truncated": false,
                "tree": [{"path": "regulation", "type": "tree", "sha": reg_sha}]
            })))
            .mount(server)
            .await;
    }

    /// A tarball mock that must not be hit.
    async fn forbid_archive(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/tarball/main"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(server)
            .await;
    }

    /// A tarball mock holding one implementing and one non-implementing law.
    async fn mount_archive(server: &MockServer, times: u64) {
        let foo = law_with_implements("foo", "wet_target");
        let tar_gz = make_repo_tar_gz(
            "acme-corpus-deadbeef",
            &[
                ("regulation/nl/wet/foo/2025-01-01.yaml", foo.as_str()),
                (
                    "regulation/nl/wet/bar/2025-01-01.yaml",
                    "$id: bar\narticles: []\n",
                ),
            ],
        );
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/tarball/main"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(tar_gz, "application/gzip"))
            .expect(times)
            .mount(server)
            .await;
    }

    /// The two source-relative entries both routes must produce.
    fn expected_entries() -> Vec<(String, Vec<String>)> {
        vec![
            ("wet/bar/2025-01-01.yaml".to_string(), Vec::<String>::new()),
            (
                "wet/foo/2025-01-01.yaml".to_string(),
                vec!["wet_target".to_string()],
            ),
        ]
    }

    /// The point of the whole exercise: a central corpus is served from the
    /// committed index in two small requests, so nothing has to pull a
    /// repo archive of hundreds of megabytes into memory to answer it.
    #[tokio::test]
    async fn read_all_implements_serves_the_index_without_touching_the_archive() {
        let server = MockServer::start().await;
        forbid_archive(&server).await;
        mount_index(&server, index_json("regtree", "regulation")).await;
        mount_root_tree(&server, "regtree").await;

        let mut got = backend_at(&server).read_all_implements().await.unwrap();
        got.sort();
        assert_eq!(got, expected_entries());
    }

    /// The index records the tree it was generated from. A corpus branch
    /// that moved on since — a preview branch that got commits after its
    /// last index build — must not be answered from it.
    #[tokio::test]
    async fn a_stale_index_is_not_served_but_falls_back_to_the_archive() {
        let server = MockServer::start().await;
        mount_archive(&server, 1).await;
        mount_index(&server, index_json("oldtree", "regulation")).await;
        mount_root_tree(&server, "regtree").await;

        let mut got = backend_at(&server).read_all_implements().await.unwrap();
        got.sort();
        assert_eq!(got, expected_entries());
    }

    /// An index scanned under a different root says nothing about this
    /// source. Projecting it anyway would yield an empty list, which the
    /// caller reads as an authoritative "this corpus holds no laws".
    #[tokio::test]
    async fn an_index_rooted_outside_the_source_is_not_served() {
        let server = MockServer::start().await;
        mount_archive(&server, 1).await;
        mount_index(&server, index_json("regtree", "andere-root")).await;
        mount_root_tree(&server, "regtree").await;

        let mut got = backend_at(&server).read_all_implements().await.unwrap();
        got.sort();
        assert_eq!(got, expected_entries());
    }

    /// A freshness check that cannot be performed is not a freshness check
    /// that passed: a rate-limited Trees call falls back instead of serving
    /// an index whose currency is unknown.
    #[tokio::test]
    async fn an_unverifiable_index_is_not_served() {
        let server = MockServer::start().await;
        mount_archive(&server, 1).await;
        mount_index(&server, index_json("regtree", "regulation")).await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/main"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .set_body_string("API rate limit exceeded"),
            )
            .mount(&server)
            .await;

        let mut got = backend_at(&server).read_all_implements().await.unwrap();
        got.sort();
        assert_eq!(got, expected_entries());
    }

    /// The map is a negative cache: a missing path means "fetch this law".
    /// So a failed read must never surface as a successful empty map — if
    /// neither route can answer, the call fails.
    #[tokio::test]
    async fn a_failed_read_is_an_error_and_never_an_empty_map() {
        let server = MockServer::start().await;
        // Index read: rate-limited, not absent.
        Mock::given(method("GET"))
            .and(path_matcher(
                "/repos/acme/corpus/contents/implements-index.json",
            ))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .set_body_string("API rate limit exceeded"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/tarball/main"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream unavailable"))
            .mount(&server)
            .await;

        let err = backend_at(&server)
            .read_all_implements()
            .await
            .expect_err("an unreadable corpus is not an empty corpus");
        assert!(
            err.to_string().contains("503"),
            "the real cause must survive: {err}"
        );
    }

    #[tokio::test]
    async fn read_all_implements_bulk_downloads_archive_and_maps_to_source_relative() {
        let server = MockServer::start().await;

        let foo = law_with_implements("foo", "wet_target");
        let tar_gz = make_repo_tar_gz(
            "acme-corpus-deadbeef",
            &[
                ("regulation/nl/wet/foo/2025-01-01.yaml", foo.as_str()),
                // No implements → empty list, but still reported.
                (
                    "regulation/nl/wet/bar/2025-01-01.yaml",
                    "$id: bar\narticles: []\n",
                ),
                // Non-YAML under the corpus subtree: dropped by the extract.
                ("regulation/nl/README.md", "docs\n"),
                // YAML outside the source sub_path: dropped by to_source_relative.
                ("tools/gen.yaml", "$id: noise\n"),
            ],
        );

        // One request to the tarball endpoint serves the whole source.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/tarball/main"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(tar_gz, "application/gzip"))
            .expect(1)
            .mount(&server)
            .await;

        let src = GitHubSource {
            owner: "acme".to_string(),
            repo: "corpus".to_string(),
            branch: "main".to_string(),
            path: Some("regulation/nl".to_string()),
            git_ref: None,
        };
        let backend = GitHubApiBackend::new(
            &src,
            Some("main".to_string()),
            Some("tok".to_string()),
            regelrecht_github::GithubClient::from_base(Some(&server.uri())).unwrap(),
        );

        let mut got = backend.read_all_implements().await.unwrap();
        got.sort();
        assert_eq!(
            got,
            vec![
                ("wet/bar/2025-01-01.yaml".to_string(), Vec::<String>::new()),
                (
                    "wet/foo/2025-01-01.yaml".to_string(),
                    vec!["wet_target".to_string()]
                ),
            ]
        );
    }

    /// Buffer one write and persist it with the given context; returns the
    /// JSON body of the resulting Contents API PUT.
    async fn persist_and_capture_put_body(ctx: WriteContext) -> serde_json::Value {
        let server = MockServer::start().await;
        // `persist` bootstraps the branch lazily when `ensure_ready` never
        // ran (this helper builds the backend directly); an existing branch
        // is a single ref GET.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/ref/heads/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ref": "refs/heads/main",
                "object": {"sha": "branch-sha"}
            })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path_matcher(
                "/repos/acme/corpus/contents/regulation/nl/wet/foo/2025-01-01.yaml",
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "content": {"sha": "newsha"}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let src = GitHubSource {
            owner: "acme".to_string(),
            repo: "corpus".to_string(),
            branch: "main".to_string(),
            path: Some("regulation/nl".to_string()),
            git_ref: None,
        };
        let backend = GitHubApiBackend::new(
            &src,
            Some("main".to_string()),
            Some("service-tok".to_string()),
            regelrecht_github::GithubClient::from_base(Some(&server.uri())).unwrap(),
        );
        backend
            .write_file(
                std::path::Path::new("wet/foo/2025-01-01.yaml"),
                "$id: foo\narticles: []\n",
            )
            .await
            .unwrap();
        backend.persist(&ctx).await.unwrap();

        let requests = server.received_requests().await.unwrap();
        let put = requests
            .iter()
            .find(|r| r.method.as_str() == "PUT")
            .expect("no PUT request captured");
        serde_json::from_slice(&put.body).unwrap()
    }

    /// With the acting user's own token the commit must stay attributed to
    /// their GitHub account: the PUT body carries no `committer`/`author`
    /// override, so the Contents API defaults both to the authenticated
    /// user instead of the (unlinkable) editor-session identity.
    #[tokio::test]
    async fn persist_with_user_token_lets_github_attribute_the_commit() {
        let ctx = WriteContext {
            message: "Update law foo".to_string(),
            author: Some(EditorUser {
                name: "Anne Schuth".to_string(),
                email: "anne@example.gov".to_string(),
            }),
            token_override: Some("user-tok".to_string()),
        };
        let body = persist_and_capture_put_body(ctx).await;
        assert!(
            body.get("committer").is_none() && body.get("author").is_none(),
            "user-token write must not override commit identity: {body}"
        );
    }

    /// On the shared service token the human is invisible behind the bot,
    /// so the session identity is stamped on the commit as before.
    #[tokio::test]
    async fn persist_with_service_token_stamps_session_identity() {
        let ctx = WriteContext::new(
            "Update law foo".to_string(),
            Some(EditorUser {
                name: "Anne Schuth".to_string(),
                email: "anne@example.gov".to_string(),
            }),
        );
        let body = persist_and_capture_put_body(ctx).await;
        for side in ["committer", "author"] {
            assert_eq!(
                body[side],
                serde_json::json!({"name": "Anne Schuth", "email": "anne@example.gov"}),
                "service-token write keeps crediting the session identity"
            );
        }
    }
}
