use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::client::CorpusClient;
// Only the clone-based factory branch builds a config from scratch; with
// the `github` feature that branch is compiled out.
#[cfg_attr(feature = "github", allow(unused_imports))]
use crate::config::CorpusConfig;
use crate::error::{CorpusError, Result};
use crate::models::{Source, SourceType};

/// Identifies the human editor behind a write — surfaced as a
/// `Co-Authored-By` trailer on the commit and named in the PR body.
///
/// Optional because non-editor write paths (harvester, enrich worker) have
/// no human in the loop. The git author/committer stays the service
/// identity in all cases — this is a *credit* line, not an authentication
/// claim.
#[derive(Debug, Clone)]
pub struct EditorUser {
    /// Display name for the trailer. Should be a real human-readable name
    /// when available; falls back to `email`'s local-part otherwise.
    pub name: String,
    /// Email used in the trailer. GitHub matches this to a user account if
    /// it's registered as a verified email on someone's profile.
    pub email: String,
}

/// Metadata for a write operation (used as commit message for git backends).
pub struct WriteContext {
    pub message: String,
    /// Human editor to credit on the resulting commit / PR. `None` for
    /// non-editor paths (harvester etc.) which keep the existing
    /// service-only attribution.
    pub author: Option<EditorUser>,
    /// Per-call GitHub credential that **supersedes** the backend's baked-in
    /// token for this write only. Set by the editor when the acting user has
    /// linked their own GitHub account (user-OAuth spike): the write then
    /// authenticates *as the user*, so GitHub — not the editor — enforces
    /// whether they may push to the repo. `None` keeps the pre-existing
    /// behaviour (use the backend's configured token), so every non-editor
    /// and non-GitHub call site is unaffected.
    pub token_override: Option<String>,
}

impl WriteContext {
    /// Construct a context with no per-call credential override — the common
    /// case for internal/service call sites that keep using the backend's
    /// configured token. Editor handlers that thread a per-user token build
    /// the struct literally instead.
    pub fn new(message: String, author: Option<EditorUser>) -> Self {
        Self {
            message,
            author,
            token_override: None,
        }
    }
}

/// A file entry returned by list operations.
pub struct FileEntry {
    /// Filename only (not a full path), e.g. "eligibility.feature".
    pub name: String,
}

/// Entry returned by recursive list operations. Carries the path
/// relative to the directory the list call was rooted at, so callers
/// can rebuild the source-relative path without re-tracking the
/// starting point.
#[derive(Debug, Clone)]
pub struct RecursiveFileEntry {
    /// Path relative to the listing root, using forward slashes
    /// regardless of platform. Example: when listing `documents/abc`
    /// and a file lives at `documents/abc/mvt/concept.md`, the
    /// returned value is `mvt/concept.md`.
    pub relative_path: String,
}

/// Identifies a PR opened or updated as part of a [`RepoBackend::persist`]
/// call. Returned via [`PersistOutcome`] so the editor-api can surface the
/// URL to the frontend after a save.
///
/// Lives on `backend` (not on the `regelrecht-github` crate's own `PrInfo`)
/// so the type is available even when the `github` feature is disabled — it
/// keeps the [`PersistOutcome`] shape uniform across builds. `SessionGitBackend`
/// converts the crate's `PrInfo` into this one.
#[derive(Debug, Clone)]
pub struct PrInfo {
    /// PR number on the source repo. Used to construct the title in
    /// subsequent updates and to build the GitHub URL.
    pub number: u64,
    /// User-facing HTML URL of the PR.
    pub html_url: String,
}

/// Result of a [`RepoBackend::persist`] call.
///
/// Most backends return [`PersistOutcome::default()`] (`pr: None`). The
/// session-mode `GitBackend` populates `pr` with the PR it opened or
/// updated; the editor-api forwards this to the save-response JSON so the
/// frontend can render a "Bekijk op GitHub" link.
#[derive(Debug, Default)]
pub struct PersistOutcome {
    pub pr: Option<PrInfo>,
}

/// Abstraction over different corpus storage backends.
///
/// All paths are relative to the source root directory, except for the
/// `*_repo_file*` methods, which take paths relative to the repository root
/// (see [`RepoBackend::repo_subpath`]). The backend resolves them to
/// absolute paths internally.
#[async_trait]
pub trait RepoBackend: Send + Sync {
    /// Read a file's contents. Returns `None` if the file does not exist.
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>>;

    /// [`read_file`], authenticating with `token_override` when this
    /// backend supports per-call tokens (see
    /// [`RepoBackend::supports_token_override`]) — the read analogue of
    /// [`WriteContext::token_override`]. `None` (or a backend that never
    /// authenticates reads per call) falls back to [`read_file`]'s
    /// behaviour, so the default implementation just delegates.
    ///
    /// This exists for the traject flow where the writable-own source has
    /// no configured service token (fail-closed against shipping the
    /// central token to a user-chosen repo): reads on that source can then
    /// ride the acting editor user's own GitHub token per request, exactly
    /// like `persist` does for writes.
    ///
    /// [`read_file`]: RepoBackend::read_file
    async fn read_file_with_token(
        &self,
        relative_path: &Path,
        token_override: Option<&str>,
    ) -> Result<Option<String>> {
        let _ = token_override;
        self.read_file(relative_path).await
    }

    /// Write a file's contents, creating parent directories as needed.
    ///
    /// For git backends this writes to the local checkout without committing.
    /// Call [`persist`] afterwards to commit and push.
    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()>;

    /// Delete a file. Returns `Ok(())` even if the file did not exist.
    async fn delete_file(&self, relative_path: &Path) -> Result<()>;

    /// Where the source root sits inside its repository (e.g.
    /// `regulation/nl`), or `None` when the source root *is* the
    /// repository root. Backends without a repository notion (a plain
    /// local directory) keep the default `None`.
    ///
    /// A backend that returns `Some` here must also override
    /// [`read_repo_file_with_token`] and [`write_repo_file`]: their
    /// defaults treat the source root as the repository root.
    ///
    /// [`read_repo_file_with_token`]: RepoBackend::read_repo_file_with_token
    /// [`write_repo_file`]: RepoBackend::write_repo_file
    fn repo_subpath(&self) -> Option<&str> {
        None
    }

    /// [`read_file_with_token`] for a path relative to the **repository
    /// root** instead of the source root, for the few files a source
    /// repository keeps outside its regulation subtree (RFC-018's
    /// `annotations/` sidecars). The same sandbox applies: absolute paths
    /// and `..` components are rejected.
    ///
    /// [`read_file_with_token`]: RepoBackend::read_file_with_token
    async fn read_repo_file_with_token(
        &self,
        repo_path: &Path,
        token_override: Option<&str>,
    ) -> Result<Option<String>> {
        self.read_file_with_token(repo_path, token_override).await
    }

    /// [`write_file`] for a repository-root-relative path (see
    /// [`read_repo_file_with_token`]). The write is staged exactly like a
    /// source-relative one, so the next [`persist`] commits it together
    /// with the other pending writes, on the same branch.
    ///
    /// [`write_file`]: RepoBackend::write_file
    /// [`read_repo_file_with_token`]: RepoBackend::read_repo_file_with_token
    /// [`persist`]: RepoBackend::persist
    async fn write_repo_file(&self, repo_path: &Path, content: &str) -> Result<()> {
        self.write_file(repo_path, content).await
    }

    /// Stage a check, run by the next [`persist`] before the writes staged
    /// after it, that a source-relative path an earlier read in this
    /// backend found absent is still absent.
    ///
    /// For a write whose content was derived from a read of a *different*
    /// path than the one it writes: the notes sidecar is written at the
    /// repository root but may have been built on the legacy copy under the
    /// subpath. When `persist` has to create the branch from base, a read
    /// made before that could not see base content, so a legacy file that
    /// appears with the branch means the write was prepared against the
    /// wrong base; `persist` then refuses with [`CorpusError::Conflict`],
    /// the same answer it gives for the written path itself.
    ///
    /// The default is a no-op: only a backend that creates its branch
    /// lazily at persist (the GitHub API backend) has this window.
    ///
    /// [`persist`]: RepoBackend::persist
    async fn expect_still_absent(&self, relative_path: &Path) -> Result<()> {
        let _ = relative_path;
        Ok(())
    }

    /// List files in a directory, optionally filtered by extension (without dot).
    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>>;

    /// [`list_files`] with a per-call token override — same contract as
    /// [`RepoBackend::read_file_with_token`].
    ///
    /// [`list_files`]: RepoBackend::list_files
    async fn list_files_with_token(
        &self,
        dir: &Path,
        extension: Option<&str>,
        token_override: Option<&str>,
    ) -> Result<Vec<FileEntry>> {
        let _ = token_override;
        self.list_files(dir, extension).await
    }

    /// List all files in a directory tree, recursively, optionally filtered by
    /// extension (without dot). Returned entries carry their path relative to
    /// the listing root, separated by forward slashes regardless of platform.
    ///
    /// The default implementation degrades to a flat single-level listing. A
    /// backend that can do better — local checkouts can walk the tree on
    /// disk, the GitHub API backend can walk via the Contents API — should
    /// override this.
    async fn list_files_recursive(
        &self,
        dir: &Path,
        extension: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        // All current backends override this; the fallback only runs if a
        // future backend forgets to. Warn so the resulting truncated
        // (single-level) listing is diagnosable instead of silently wrong.
        tracing::warn!(
            dir = %dir.display(),
            "RepoBackend::list_files_recursive default used — degrading to a \
             flat, non-recursive listing; nested files will be missing. \
             Override this method on the backend for full-tree results."
        );
        let flat = self.list_files(dir, extension).await?;
        Ok(flat
            .into_iter()
            .map(|f| RecursiveFileEntry {
                relative_path: f.name,
            })
            .collect())
    }

    /// [`list_files_recursive`] with a per-call token override — same
    /// contract as [`RepoBackend::read_file_with_token`].
    ///
    /// [`list_files_recursive`]: RepoBackend::list_files_recursive
    async fn list_files_recursive_with_token(
        &self,
        dir: &Path,
        extension: Option<&str>,
        token_override: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        let _ = token_override;
        self.list_files_recursive(dir, extension).await
    }

    /// Return every YAML law's `implements` list as `(source-relative
    /// path, implements)` pairs — the input the implements/“who implements
    /// me” index is built from — reading bodies in as few requests as
    /// possible.
    ///
    /// Returns parsed `implements` lists, NOT bodies, on purpose: a
    /// corpus-wide scan must never hold every law body in memory at once
    /// (that OOMs on a large corpus). The default implementation reads one
    /// file at a time via [`list_files_recursive`] + [`read_file`], parsing
    /// and dropping each body as it goes. Backends that can do better
    /// override this: `GitBackend` reads the precomputed
    /// [`implements_index`](crate::implements_index) committed at the
    /// corpus repo root, the GitHub API backend downloads the repo archive
    /// in a single request — both still without materialising all bodies.
    ///
    /// The result must contain an entry for **every** YAML file the source
    /// holds, including files that implement nothing: callers use the map
    /// as a negative cache, and a missing entry triggers a per-law body
    /// fetch + parse.
    ///
    /// [`list_files_recursive`]: RepoBackend::list_files_recursive
    /// [`read_file`]: RepoBackend::read_file
    async fn read_all_implements(&self) -> Result<Vec<(String, Vec<String>)>> {
        scan_implements_via_listing(self).await
    }

    /// Persist pending changes.
    ///
    /// No-op for local backends. For git backends this commits dirty files
    /// and pushes to the remote. Returns a [`PersistOutcome`] describing
    /// any side-effects the caller may want to surface (e.g. the PR opened
    /// by a session-mode `GitBackend`).
    async fn persist(&self, ctx: &WriteContext) -> Result<PersistOutcome>;

    /// Whether [`RepoBackend::persist`] honors [`WriteContext::token_override`].
    ///
    /// Only a backend that authenticates each persist against GitHub can
    /// route a write through the acting user's own credential. Backends that
    /// ignore the override — local checkouts, clone-based git backends with a
    /// baked-in remote credential — must return `false`, so callers never
    /// demand a per-user token for a write that could not use one anyway.
    fn supports_token_override(&self) -> bool {
        false
    }

    /// Prepare the backend for use (validate directories, clone repos, etc.).
    async fn ensure_ready(&mut self) -> Result<()>;

    /// Whether this backend supports write operations.
    fn is_writable(&self) -> bool;

    /// Source-relative paths of files that differ between this backend's
    /// branch and its configured base branch.
    ///
    /// Only meaningful for backends that track a branch against a base
    /// (the GitHub API backend used by the traject flow). The default
    /// returns an empty list, so local- and clone-based backends — which
    /// have no "base vs head" notion in this context — simply report
    /// "nothing changed".
    async fn changed_files(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// [`changed_files`] with a per-call token override — same contract as
    /// [`RepoBackend::read_file_with_token`].
    ///
    /// The diff is the last read path that used to be service-token-only.
    /// On a private, user-supplied traject repo the server has no token of
    /// its own by design (that is the whole point of the private-token
    /// route), so without this the branch-vs-base diff — and with it the
    /// sidebar's "Bewerkt" section — could never be computed, not even
    /// after real edits.
    ///
    /// The default implementation delegates, so backends with no "base vs
    /// head" notion keep reporting "nothing changed" unchanged.
    ///
    /// [`changed_files`]: RepoBackend::changed_files
    async fn changed_files_with_token(&self, token_override: Option<&str>) -> Result<Vec<String>> {
        let _ = token_override;
        self.changed_files().await
    }
}

/// The checkout-scan fallback for [`GitBackend::read_all_implements`],
/// with the one case where scanning is not allowed to be a fallback.
///
/// A sparse working tree only materialises the configured cone, so a
/// filesystem scan over it returns a silently *incomplete* implements map
/// (missing entries read as "fetch this law per-request", or worse, as
/// "law does not exist"). Refuse loudly instead.
async fn scan_or_refuse(backend: &GitBackend) -> Result<Vec<(String, Vec<String>)>> {
    if backend.client.is_sparse() {
        return Err(CorpusError::Config(
            "implements index unavailable and this is a sparse checkout: \
             a filesystem scan would be silently incomplete"
                .to_string(),
        ));
    }
    scan_implements_via_listing(backend).await
}

/// Shared body of the default [`RepoBackend::read_all_implements`]: list
/// every `.yaml` **and** `.yml` file under the source root and parse each
/// body one at a time. Both extensions are law carriers (the corpus
/// convention is `.yaml`, but `.yml` occurs and the GitHub archive scan
/// counts it too) — a `.yaml`-only scan would silently drop those laws
/// from the negative cache.
async fn scan_implements_via_listing<B: RepoBackend + ?Sized>(
    backend: &B,
) -> Result<Vec<(String, Vec<String>)>> {
    let mut entries = Vec::new();
    for ext in ["yaml", "yml"] {
        entries.extend(
            backend
                .list_files_recursive(Path::new(""), Some(ext))
                .await?,
        );
    }
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some(content) = backend.read_file(Path::new(&entry.relative_path)).await? {
            let implements = crate::source_map::collect_law_implements(&content);
            out.push((entry.relative_path, implements));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// LocalBackend
// ---------------------------------------------------------------------------

/// Backend that reads/writes directly to the local filesystem.
///
/// When the configured `root` is on a read-only filesystem (typical for a
/// corpus baked into a container image), [`ensure_ready`] transparently
/// rehomes the backend onto an **ephemeral scratch copy** under
/// `$TMPDIR/regelrecht-editor-corpus/<host>/<source_id>` and operates on
/// that copy from then on. Edits survive for the lifetime of the process
/// but are lost on the next deployment — by design.
pub struct LocalBackend {
    /// Stable identifier of the source this backend belongs to. Used to
    /// namespace the scratch directory so multiple local sources do not
    /// collide on the same node.
    source_id: String,
    root: PathBuf,
    writable: bool,
}

impl LocalBackend {
    pub fn new(source_id: String, root: PathBuf, writable: bool) -> Self {
        Self {
            source_id,
            root,
            writable,
        }
    }

    fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        validate_relative_path(relative)?;
        Ok(self.root.join(relative))
    }
}

/// Probe whether a directory accepts writes by creating and removing a
/// short-lived sentinel file. Returns `false` on any IO error.
async fn probe_writable(dir: &Path) -> bool {
    let probe = dir.join(".write-probe");
    match tokio::fs::write(&probe, b"").await {
        Ok(()) => {
            if let Err(e) = tokio::fs::remove_file(&probe).await {
                tracing::warn!(
                    path = %probe.display(),
                    error = %e,
                    "failed to remove write probe file after success"
                );
            }
            true
        }
        Err(_) => false,
    }
}

/// Recursively copy a directory tree.
///
/// Uses [`walkdir::WalkDir`] for traversal and `std::fs` for the file ops;
/// callers should wrap in [`tokio::task::spawn_blocking`] to avoid stalling
/// the runtime on a large corpus.
fn copy_dir_recursive_sync(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(src) else {
            continue;
        };
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &target)?;
        }
    }
    Ok(())
}

/// Walk a local filesystem subtree and return file paths relative to
/// `abs_dir`, with forward slashes regardless of platform. Used by the
/// three filesystem-backed `RepoBackend` implementations to share one
/// recursive-listing path; the work runs inside `spawn_blocking` so it
/// does not stall the runtime on a large tree.
async fn walk_local_tree(
    abs_dir: PathBuf,
    extension: Option<String>,
) -> Result<Vec<RecursiveFileEntry>> {
    if !abs_dir.exists() {
        return Ok(Vec::new());
    }
    let abs_dir_clone = abs_dir.clone();
    let work = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<RecursiveFileEntry>> {
        let mut entries = Vec::new();
        for entry in walkdir::WalkDir::new(&abs_dir_clone)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ref ext) = extension
                && path.extension().is_none_or(|e| e != ext.as_str())
            {
                continue;
            }
            let Ok(rel) = path.strip_prefix(&abs_dir_clone) else {
                continue;
            };
            // Normalise to forward slashes — callers serialise these
            // straight into JSON URLs and the frontend expects `/`.
            let rel_str = rel
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => s.to_str().map(str::to_string),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            if rel_str.is_empty() {
                continue;
            }
            entries.push(RecursiveFileEntry {
                relative_path: rel_str,
            });
        }
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(entries)
    })
    .await
    .map_err(|e| CorpusError::Config(format!("walk_local_tree task panicked: {e}")))?
    .map_err(CorpusError::from)?;
    Ok(work)
}

/// Reject paths that are absolute or contain `..` components.
pub(crate) fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        return Err(CorpusError::Config(format!(
            "path must be relative: {}",
            path.display()
        )));
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(CorpusError::Config(format!(
                "path must not contain '..': {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// A configured repository subpath with trailing slashes trimmed, or
/// `None` when it is absent or empty (the source root is the repo root).
pub(crate) fn normalized_subpath(sub: Option<&str>) -> Option<&str> {
    sub.map(|s| s.trim_end_matches('/'))
        .filter(|s| !s.is_empty())
}

/// Resolve a path inside a git checkout: `relative` joined under
/// `subpath` (the source root) when given, under the checkout root
/// otherwise. Shared by the clone-based backends, for both their
/// source-relative and their repository-root-relative file access.
fn checkout_path(repo_root: &Path, subpath: Option<&str>, relative: &Path) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    Ok(match normalized_subpath(subpath) {
        Some(sub) => repo_root.join(sub).join(relative),
        None => repo_root.join(relative),
    })
}

/// Read a file off disk, mapping "not found" to `Ok(None)`.
async fn read_optional(abs: &Path) -> Result<Option<String>> {
    match tokio::fs::read_to_string(abs).await {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Write a file to disk, creating its parent directories first.
async fn write_creating_parents(abs: &Path, content: &str) -> Result<()> {
    if let Some(parent) = abs.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(abs, content).await?;
    Ok(())
}

#[async_trait]
impl RepoBackend for LocalBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        read_optional(&self.resolve(relative_path)?).await
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        if !self.writable {
            return Err(CorpusError::ReadOnly(
                "local source is read-only".to_string(),
            ));
        }
        write_creating_parents(&self.resolve(relative_path)?, content).await
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        if !self.writable {
            return Err(CorpusError::ReadOnly(
                "local source is read-only".to_string(),
            ));
        }
        let abs = self.resolve(relative_path)?;
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let abs = self.resolve(dir)?;
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let ft = entry.file_type().await?;
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = extension
                && path.extension().is_none_or(|e| e != ext)
            {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(FileEntry {
                    name: name.to_string(),
                });
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn list_files_recursive(
        &self,
        dir: &Path,
        extension: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        let abs = self.resolve(dir)?;
        walk_local_tree(abs, extension.map(str::to_string)).await
    }

    async fn persist(&self, _ctx: &WriteContext) -> Result<PersistOutcome> {
        // Local writes are immediate — nothing to persist.
        Ok(PersistOutcome::default())
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        if !self.root.exists() {
            return Err(CorpusError::Config(format!(
                "local source root does not exist: {}",
                self.root.display()
            )));
        }

        // Probe write access at the configured root. If it succeeds, we
        // operate in place. If it fails (typical for a corpus baked into a
        // read-only container layer), copy the corpus to an ephemeral
        // scratch directory under $TMPDIR and operate there. The copy is
        // lost on container restart — by design — but for the lifetime of
        // the process the source is fully editable and the engine sees
        // every edit because both reads and writes route through the same
        // backend.
        if self.writable && !probe_writable(&self.root).await {
            tracing::info!(
                path = %self.root.display(),
                "local source root is not writable; preparing ephemeral scratch copy"
            );

            let host_id = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
            let scratch = std::env::temp_dir()
                .join("regelrecht-editor-corpus")
                .join(host_id)
                .join(&self.source_id);

            // If a previous invocation of this process already populated
            // the scratch dir, leave it alone — that preserves any edits
            // the operator made earlier in the same container lifetime.
            // Otherwise copy the corpus tree across.
            if !scratch.exists() {
                let src = self.root.clone();
                let dst = scratch.clone();
                let copy_result =
                    tokio::task::spawn_blocking(move || copy_dir_recursive_sync(&src, &dst)).await;

                match copy_result {
                    Ok(Ok(())) => {
                        tracing::info!(
                            from = %self.root.display(),
                            to = %scratch.display(),
                            "copied read-only local source to writable scratch directory"
                        );
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            error = %e,
                            from = %self.root.display(),
                            to = %scratch.display(),
                            "failed to copy local source to scratch directory; \
                             marking backend read-only"
                        );
                        self.writable = false;
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "scratch copy task panicked; marking backend read-only"
                        );
                        self.writable = false;
                        return Ok(());
                    }
                }
            } else {
                tracing::info!(
                    path = %scratch.display(),
                    "reusing existing scratch directory from a previous run in this process"
                );
            }

            // Re-probe at the scratch location to confirm it's writable.
            if probe_writable(&scratch).await {
                self.root = scratch;
            } else {
                tracing::warn!(
                    path = %scratch.display(),
                    "scratch directory is not writable; marking backend read-only"
                );
                self.writable = false;
            }
        }

        Ok(())
    }

    fn is_writable(&self) -> bool {
        self.writable
    }
}

// ---------------------------------------------------------------------------
// GitBackend
// ---------------------------------------------------------------------------

/// Backend that reads/writes to a local git checkout and commits/pushes on persist.
///
/// When no push token is configured (`local_only` mode), edits are committed
/// to a local session branch without pushing. This is useful for playground
/// environments where users want to experiment without affecting the remote.
pub struct GitBackend {
    client: CorpusClient,
    /// Sub-path within the repo that corresponds to the source root
    /// (e.g. "regulation/nl").
    repo_subpath: Option<String>,
    /// Files written since the last persist, as absolute paths.
    dirty_files: tokio::sync::Mutex<Vec<PathBuf>>,
    /// When true, commits stay local (no push). Set when no push token is available.
    local_only: bool,
    /// Name of the local session branch (created on first persist).
    session_branch: Option<String>,
    /// Whether the session branch has been created yet.
    branched: tokio::sync::Mutex<bool>,
}

impl GitBackend {
    pub fn new(client: CorpusClient, repo_subpath: Option<String>) -> Self {
        let local_only = !client.has_push_token();
        let session_branch = if local_only {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Some(format!("editor/session-{ts}"))
        } else {
            None
        };
        Self {
            client,
            repo_subpath,
            dirty_files: tokio::sync::Mutex::new(Vec::new()),
            local_only,
            session_branch,
            branched: tokio::sync::Mutex::new(false),
        }
    }

    /// Resolve a source-relative path to an absolute path in the checkout.
    fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        checkout_path(
            self.client.repo_path(),
            self.repo_subpath.as_deref(),
            relative,
        )
    }

    /// Write to an absolute checkout path and mark it for the next persist.
    async fn write_dirty(&self, abs: PathBuf, content: &str) -> Result<()> {
        write_creating_parents(&abs, content).await?;
        self.dirty_files.lock().await.push(abs);
        Ok(())
    }
}

#[async_trait]
impl RepoBackend for GitBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        read_optional(&self.resolve(relative_path)?).await
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        self.write_dirty(self.resolve(relative_path)?, content)
            .await
    }

    fn repo_subpath(&self) -> Option<&str> {
        normalized_subpath(self.repo_subpath.as_deref())
    }

    // The checkout is the whole repository, so a repo-root path is just
    // resolved without the subpath. `dirty_files` holds absolute paths, so
    // `persist` commits it alongside the source-relative writes.
    async fn read_repo_file_with_token(
        &self,
        repo_path: &Path,
        _token_override: Option<&str>,
    ) -> Result<Option<String>> {
        read_optional(&checkout_path(self.client.repo_path(), None, repo_path)?).await
    }

    async fn write_repo_file(&self, repo_path: &Path, content: &str) -> Result<()> {
        self.write_dirty(
            checkout_path(self.client.repo_path(), None, repo_path)?,
            content,
        )
        .await
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        let abs = self.resolve(relative_path)?;
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => {
                self.dirty_files.lock().await.push(abs);
                Ok(())
            }
            // Deleting a file that doesn't exist is intentionally a no-op
            // and does NOT enqueue anything onto `dirty_files`. Callers
            // typically follow up with `persist`, which short-circuits on
            // an empty dirty set — so the overall flow stays an idempotent
            // no-op (no spurious empty commit, no push) when the target
            // was already gone.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let abs = self.resolve(dir)?;
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let ft = entry.file_type().await?;
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = extension
                && path.extension().is_none_or(|e| e != ext)
            {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(FileEntry {
                    name: name.to_string(),
                });
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn list_files_recursive(
        &self,
        dir: &Path,
        extension: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        let abs = self.resolve(dir)?;
        walk_local_tree(abs, extension.map(str::to_string)).await
    }

    /// Serve the corpus-wide implements map from the precomputed index
    /// committed at the checkout root ([`implements_index`]) instead of
    /// parsing every YAML file on disk.
    ///
    /// The index lives at the **repo root**, outside this backend's
    /// source sandbox (`resolve` prefixes every path with `repo_subpath`,
    /// e.g. `regulation/nl`), so it is read directly off the checkout
    /// root. Freshness is verified against the checkout itself: the
    /// index's recorded tree sha must equal `git rev-parse HEAD:<root>` in
    /// this clone. That makes the check branch-aware by construction — a
    /// `pr<N>` corpus branch that received commits after its index was
    /// last regenerated fails the comparison and falls back to the scan.
    ///
    /// Fallback matrix:
    /// - fresh index → serve it (an **empty but fresh** index is
    ///   authoritative: genuinely no YAML files, no fallback);
    /// - missing / unparseable / stale index, or one rooted outside this
    ///   source → full checkout scan, with a warning — **unless** the
    ///   checkout is sparse, in which case the scan would be silently
    ///   incomplete and the call errors instead.
    ///
    /// Known limit: the tree sha describes `HEAD`, not the working tree,
    /// so uncommitted local writes are invisible to the freshness check.
    /// That matches how this backend is used — the editor's central read
    /// clone never receives local writes (editor writes go through the
    /// session/API backends), and post-save reads ride the caller's
    /// overlay.
    ///
    /// [`implements_index`]: crate::implements_index
    async fn read_all_implements(&self) -> Result<Vec<(String, Vec<String>)>> {
        use crate::implements_index::{IMPLEMENTS_INDEX_FILENAME, ImplementsIndex};

        let index_path = self.client.repo_path().join(IMPLEMENTS_INDEX_FILENAME);
        let raw = match tokio::fs::read_to_string(&index_path).await {
            Ok(raw) => Some(raw),
            // Missing is a distinct state from empty: the corpus repo
            // simply doesn't publish an index (yet) on this branch.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };

        match raw {
            Some(raw) => match ImplementsIndex::parse(&raw) {
                // An index rooted elsewhere does not describe this source.
                // Projecting anyway yields an empty list, which the caller
                // reads as an authoritative "no laws" — worse than no index.
                Ok(index) if !index.covers(self.repo_subpath.as_deref()) => {
                    tracing::warn!(
                        index_root = %index.root,
                        source_root = self.repo_subpath.as_deref().unwrap_or("<repo root>"),
                        "implements index does not cover this source's root; \
                         falling back to a scan"
                    );
                }
                Ok(index) => match self.client.subtree_sha(&index.root).await {
                    Ok(checkout_sha) if checkout_sha == index.tree_sha => {
                        let entries = index.to_source_relative(self.repo_subpath.as_deref());
                        tracing::info!(
                            entries = entries.len(),
                            root = %index.root,
                            tree = %index.tree_sha,
                            "served implements map from the precomputed index"
                        );
                        return Ok(entries);
                    }
                    Ok(checkout_sha) => {
                        tracing::warn!(
                            index_tree = %index.tree_sha,
                            checkout_tree = %checkout_sha,
                            root = %index.root,
                            "implements index does not match this checkout's content \
                             (stale index on this corpus branch?); falling back to a scan"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            root = %index.root,
                            error = %e,
                            "implements index freshness could not be verified against \
                             the checkout; falling back to a scan"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        path = %index_path.display(),
                        error = %e,
                        "implements index is unreadable; falling back to a scan"
                    );
                }
            },
            None => {
                tracing::warn!(
                    path = %index_path.display(),
                    "no implements index in this corpus checkout; falling back to a scan"
                );
            }
        }

        scan_or_refuse(self).await
    }

    async fn persist(&self, ctx: &WriteContext) -> Result<PersistOutcome> {
        let paths: Vec<PathBuf> = {
            let mut dirty = self.dirty_files.lock().await;
            std::mem::take(&mut *dirty)
        };

        if paths.is_empty() {
            return Ok(PersistOutcome::default());
        }

        let result = if self.local_only {
            // Create session branch on first persist
            let branch_ok = {
                let mut branched = self.branched.lock().await;
                if !*branched {
                    let res = if let Some(branch) = &self.session_branch {
                        self.client.create_local_branch(branch).await
                    } else {
                        Ok(())
                    };
                    if res.is_ok() {
                        *branched = true;
                    }
                    res
                } else {
                    Ok(())
                }
            };
            match branch_ok {
                Ok(()) => self.client.commit_local(&paths, &ctx.message).await,
                Err(e) => Err(e),
            }
        } else {
            self.client.commit_and_push(&paths, &ctx.message).await
        };

        if let Err(e) = result {
            // Restore dirty files so the next persist attempt can retry.
            self.dirty_files.lock().await.extend(paths);
            return Err(e);
        }

        // GitBackend doesn't open PRs: it pushes to the configured branch
        // directly. Of the backends here only `SessionGitBackend` fills in
        // PR info, and the editor-api does not use that one.
        Ok(PersistOutcome::default())
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        self.client.ensure_repo().await
    }

    /// A `GitBackend` is always considered writable: even in `local_only`
    /// mode (no push token) edits are committed to a local session branch.
    /// "Writable" here means "the backend will accept `write_file` calls",
    /// not "the backend will push to a remote".
    fn is_writable(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// SessionGitBackend
// ---------------------------------------------------------------------------

/// Per-session clone-and-PR backend from the RFC-010 phase 6 write path.
///
/// **Not used by the editor-api.** The editor stopped building it when
/// trajects landed (#632): every traject source, own-repo or central, is
/// now a [`crate::github_api_backend::GitHubApiBackend`], which commits
/// straight to the traject branch through the Contents API and opens no
/// pull request. Nothing in this workspace constructs a `SessionGitBackend`
/// outside its own tests, so read the rest of this comment as a
/// description of the type, not of what a save in the editor does.
///
/// Each `(editor session, source repo)` pair gets its own
/// `SessionGitBackend`. `persist` always pushes to a per-session feature
/// branch and ensures an open PR exists against the source's configured
/// branch — never pushes straight to the source's main branch and never
/// silently degrades to local-only when no token is configured (a write
/// without a push destination would mislead the user into thinking the
/// edit reached upstream).
///
/// File ops mirror `GitBackend`: read/write/delete/list go through the
/// local checkout owned by this backend's `CorpusClient`
/// (`CorpusClient.repo_path()`). The editor-api gives **each** `(editor
/// session, source)` pair its own on-disk clone (see
/// `SessionRegistry::resolve_session_backend`), so two sessions writing
/// to the same source do not share a working tree — the per-session
/// mutex on this backend exists to serialise concurrent saves issued
/// against the *same* session.
#[cfg(feature = "github")]
pub struct SessionGitBackend {
    client: CorpusClient,
    repo_subpath: Option<String>,
    dirty_files: tokio::sync::Mutex<Vec<PathBuf>>,
    /// Shared GitHub REST client — used here for the Pulls API calls.
    github: regelrecht_github::GithubClient,
    /// GitHub coordinates of the source — used for PR API calls.
    github_owner: String,
    github_repo: String,
    /// Branch the PR targets (the source's configured branch).
    base_branch: String,
    /// Per-session feature branch this backend pushes to.
    session_branch: String,
    /// Bearer token used for both git push (already on `client`) and PR
    /// API calls. Empty token is rejected at construction time.
    pr_token: String,
    /// Memoised PR info from the most recent successful persist.
    /// Surfaced via `persist()`'s `PersistOutcome.pr` so subsequent saves
    /// in the same session can keep returning the same URL without an
    /// extra round-trip.
    last_pr: tokio::sync::Mutex<Option<PrInfo>>,
}

#[cfg(feature = "github")]
impl SessionGitBackend {
    /// Build a session backend. Returns an error when `pr_token` is empty
    /// — without a token there is no way to open the PR upstream and the
    /// editor must surface that as a 403 rather than silently dropping
    /// the edit.
    pub fn new(
        client: CorpusClient,
        repo_subpath: Option<String>,
        session_branch: String,
        base_branch: String,
        github_owner: String,
        github_repo: String,
        pr_token: String,
    ) -> Result<Self> {
        if pr_token.is_empty() {
            return Err(CorpusError::Config(
                "session GitBackend requires a non-empty PR token".to_string(),
            ));
        }
        Ok(Self {
            client,
            repo_subpath,
            dirty_files: tokio::sync::Mutex::new(Vec::new()),
            github: regelrecht_github::GithubClient::new()?,
            github_owner,
            github_repo,
            base_branch,
            session_branch,
            pr_token,
            last_pr: tokio::sync::Mutex::new(None),
        })
    }

    /// Test-only: swap the GitHub client for one pointed at a wiremock server.
    #[cfg(test)]
    pub(crate) fn with_github_client(mut self, github: regelrecht_github::GithubClient) -> Self {
        self.github = github;
        self
    }

    fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        checkout_path(
            self.client.repo_path(),
            self.repo_subpath.as_deref(),
            relative,
        )
    }

    /// Write to an absolute checkout path and mark it for the next persist.
    async fn write_dirty(&self, abs: PathBuf, content: &str) -> Result<()> {
        write_creating_parents(&abs, content).await?;
        self.dirty_files.lock().await.push(abs);
        Ok(())
    }

    /// Build the PR title and body. Recomputed on every persist so a
    /// subsequent edit can refresh the body to reflect new context (the
    /// PATCH side of `ensure_pr` carries this through).
    fn pr_title_body(&self, ctx: &WriteContext) -> (String, String) {
        let session_id = self
            .session_branch
            .strip_prefix("editor/session-")
            .unwrap_or(&self.session_branch);
        let title = format!("Editor session {session_id}");

        let mut body = String::new();
        body.push_str("Bewerkingen aangebracht via de Regelrecht-editor.\n\n");
        if let Some(author) = &ctx.author {
            // The OIDC display name and email come from an upstream IdP
            // and are not under our control. Strip control characters
            // before splicing into the Markdown body so a hostile name
            // can't break PR layout (extra newlines flipping the trailing
            // line into a heading, NUL bytes confusing GitHub's renderer,
            // etc.). Plain Markdown special characters are left alone —
            // GitHub sanitises HTML/script on its end, and stripping `*`
            // / `_` would mangle legitimate names with diacritics-adjacent
            // punctuation.
            //
            // \n at end is intentional — GitHub renders the next line below.
            body.push_str(&format!(
                "Ingediend door: {} <{}>\n",
                sanitize_pr_body_value(&author.name),
                sanitize_pr_body_value(&author.email),
            ));
        }
        body.push_str(
            "\n_Branch wordt automatisch bijgewerkt zolang dezelfde editor-sessie open blijft._\n",
        );
        (title, body)
    }
}

/// Strip ASCII / Unicode control characters from a value before splicing
/// it into the PR body Markdown or a commit-message trailer. Whitespace
/// runs are collapsed to a single space so a trailing newline can't break
/// the surrounding formatting.
///
/// `<` and `>` are also replaced with a space: the call sites use these
/// as delimiters around the email (`Name <email>` in the PR body and in
/// the `Co-authored-by` trailer), so a display name like
/// `"Foo <attacker@evil>"` would otherwise produce a malformed trailer
/// (`Co-authored-by: Foo <attacker@evil> <real@email>`).
#[cfg(feature = "github")]
fn sanitize_pr_body_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;
    for c in s.chars() {
        let is_whitespace_like = c.is_control() || c == '<' || c == '>' || c == ' ';
        if is_whitespace_like {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

#[cfg(feature = "github")]
#[async_trait]
impl RepoBackend for SessionGitBackend {
    async fn read_file(&self, relative_path: &Path) -> Result<Option<String>> {
        read_optional(&self.resolve(relative_path)?).await
    }

    async fn write_file(&self, relative_path: &Path, content: &str) -> Result<()> {
        self.write_dirty(self.resolve(relative_path)?, content)
            .await
    }

    fn repo_subpath(&self) -> Option<&str> {
        normalized_subpath(self.repo_subpath.as_deref())
    }

    // Same as `GitBackend`: the checkout is the whole repository.
    async fn read_repo_file_with_token(
        &self,
        repo_path: &Path,
        _token_override: Option<&str>,
    ) -> Result<Option<String>> {
        read_optional(&checkout_path(self.client.repo_path(), None, repo_path)?).await
    }

    async fn write_repo_file(&self, repo_path: &Path, content: &str) -> Result<()> {
        self.write_dirty(
            checkout_path(self.client.repo_path(), None, repo_path)?,
            content,
        )
        .await
    }

    async fn delete_file(&self, relative_path: &Path) -> Result<()> {
        let abs = self.resolve(relative_path)?;
        match tokio::fs::remove_file(&abs).await {
            Ok(()) => {
                self.dirty_files.lock().await.push(abs);
                Ok(())
            }
            // Idempotent: missing-file delete is a no-op and does NOT
            // enqueue a dirty entry, so a clean tree → empty persist →
            // no commit / no push (consistent with `GitBackend`).
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn list_files(&self, dir: &Path, extension: Option<&str>) -> Result<Vec<FileEntry>> {
        let abs = self.resolve(dir)?;
        let mut entries = Vec::new();

        let mut read_dir = match tokio::fs::read_dir(&abs).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = read_dir.next_entry().await? {
            let ft = entry.file_type().await?;
            if !ft.is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = extension
                && path.extension().is_none_or(|e| e != ext)
            {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                entries.push(FileEntry {
                    name: name.to_string(),
                });
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    async fn list_files_recursive(
        &self,
        dir: &Path,
        extension: Option<&str>,
    ) -> Result<Vec<RecursiveFileEntry>> {
        let abs = self.resolve(dir)?;
        walk_local_tree(abs, extension.map(str::to_string)).await
    }

    async fn persist(&self, ctx: &WriteContext) -> Result<PersistOutcome> {
        let paths: Vec<PathBuf> = {
            let mut dirty = self.dirty_files.lock().await;
            std::mem::take(&mut *dirty)
        };

        if paths.is_empty() {
            // Nothing to commit. If we already opened a PR earlier in the
            // session, return that — the editor still wants the link.
            return Ok(PersistOutcome {
                pr: self.last_pr.lock().await.clone(),
            });
        }

        // Sanitise name/email before they land in the commit message so a
        // crafted OIDC display name can't inject extra Git trailers (e.g. a
        // newline followed by `Reviewer-suggested-by: …`). Matches the
        // identical defence applied to the PR body above.
        let sanitized_author = ctx.author.as_ref().map(|a| {
            (
                sanitize_pr_body_value(&a.name),
                sanitize_pr_body_value(&a.email),
            )
        });
        let co_authored_by = sanitized_author
            .as_ref()
            .map(|(name, email)| (name.as_str(), email.as_str()));

        let pushed = match self
            .client
            .commit_and_push_to_branch(
                &self.session_branch,
                &self.base_branch,
                &paths,
                &ctx.message,
                co_authored_by,
            )
            .await
        {
            Ok(pushed) => pushed,
            Err(e) => {
                // Restore dirty files so the next save can retry. Note we
                // do NOT restore them on PR-API failure below: at that
                // point the commit + push succeeded so the data is
                // upstream; only the PR open/update failed and the next
                // persist would create a duplicate commit.
                self.dirty_files.lock().await.extend(paths);
                return Err(e);
            }
        };

        // No commit was created (paths matched base content after
        // snapshot+replay — typical when the user hits Save on a freshly
        // loaded article without typing). Skip `ensure_pr`: on a brand-new
        // session the branch was never pushed and GitHub would reject the
        // PR open with a 422. Return the previously memoised PR (if any)
        // so the editor keeps showing the existing badge.
        if !pushed {
            return Ok(PersistOutcome {
                pr: self.last_pr.lock().await.clone(),
            });
        }

        let (title, body) = self.pr_title_body(ctx);
        let gh_pr = self
            .github
            .ensure_pr(
                &self.github_owner,
                &self.github_repo,
                &self.session_branch,
                &self.base_branch,
                &title,
                &body,
                &self.pr_token,
            )
            .await?;
        // Convert the crate's PrInfo into the feature-independent
        // `backend::PrInfo` the editor-api consumes.
        let pr = PrInfo {
            number: gh_pr.number,
            html_url: gh_pr.html_url,
        };

        *self.last_pr.lock().await = Some(pr.clone());
        Ok(PersistOutcome { pr: Some(pr) })
    }

    async fn ensure_ready(&mut self) -> Result<()> {
        self.client.ensure_repo().await
    }

    fn is_writable(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create a [`RepoBackend`] for a given corpus source.
///
/// A GitHub source is served over the REST API
/// ([`GitHubApiBackend`](crate::github_api_backend::GitHubApiBackend)) — no
/// clone, no working tree, so `ensure_ready` is a single branch check
/// instead of minutes of git. The central corpus is 4,2 GB; cloning it in
/// the startup path is what pushed preview deploys past their 300 s
/// admission window, and the clone was only ever a read cache. The one
/// read that could not be served over the API — the corpus-wide implements
/// scan — now reads the precomputed
/// [`implements_index`](crate::implements_index) instead of the repo
/// archive.
///
/// The API backend is created **without a base branch**: this factory
/// serves read paths, and a source whose configured branch is missing is a
/// misconfiguration to surface, not a branch for the editor to create on
/// someone else's corpus. Branch creation stays with the traject flow,
/// which owns its branch and passes a base explicitly.
///
/// Without the `github` feature there is no REST client to build on, so
/// that build keeps the clone-based [`GitBackend`]. Its on-disk checkout
/// path is namespaced by the host identifier (`HOSTNAME` env var, falling
/// back to `"local"`) so that multiple replicas of the editor running on
/// the same node — or a pod restart racing with a previous instance — do
/// not share a working directory and corrupt each other's git state during
/// concurrent `clone`/`pull --rebase`/`push`.
pub fn create_backend(source: &Source, auth_token: Option<&str>) -> Result<Box<dyn RepoBackend>> {
    match &source.source_type {
        SourceType::Local { local } => Ok(Box::new(LocalBackend::new(
            source.id.clone(),
            local.path.clone(),
            true,
        ))),
        #[cfg(feature = "github")]
        SourceType::GitHub { github } => {
            Ok(Box::new(crate::github_api_backend::GitHubApiBackend::new(
                github,
                None,
                auth_token.map(str::to_string),
            )?))
        }
        #[cfg(not(feature = "github"))]
        SourceType::GitHub { github } => {
            let repo_url = format!("https://github.com/{}/{}.git", github.owner, github.repo);
            let host_id = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
            let repo_path = std::env::temp_dir()
                .join("corpus-editor")
                .join(&host_id)
                .join(&source.id);
            let mut config = CorpusConfig::new(&repo_url, &repo_path);
            config.branch = github.effective_ref().to_string();

            if let Some(token) = auth_token {
                config = config.with_token(token);
            }

            let client = CorpusClient::new(config);
            Ok(Box::new(GitBackend::new(client, github.path.clone())))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn local_read_write_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let path = Path::new("scenarios/test.feature");

        // File doesn't exist yet
        assert!(backend.read_file(path).await.unwrap().is_none());

        // Write
        backend.write_file(path, "Feature: Test\n").await.unwrap();

        // Read back
        let content = backend.read_file(path).await.unwrap().unwrap();
        assert_eq!(content, "Feature: Test\n");

        // Persist is a no-op
        backend
            .persist(&WriteContext {
                message: "test".to_string(),
                author: None,
                token_override: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn local_delete_file() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let path = Path::new("test.feature");
        backend.write_file(path, "content").await.unwrap();
        assert!(backend.read_file(path).await.unwrap().is_some());

        backend.delete_file(path).await.unwrap();
        assert!(backend.read_file(path).await.unwrap().is_none());

        // Deleting non-existent file is fine
        backend.delete_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn local_list_files_with_extension() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        backend
            .write_file(Path::new("scenarios/a.feature"), "a")
            .await
            .unwrap();
        backend
            .write_file(Path::new("scenarios/b.feature"), "b")
            .await
            .unwrap();
        backend
            .write_file(Path::new("scenarios/readme.md"), "r")
            .await
            .unwrap();

        let entries = backend
            .list_files(Path::new("scenarios"), Some("feature"))
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "a.feature");
        assert_eq!(entries[1].name, "b.feature");
    }

    #[tokio::test]
    async fn local_list_files_recursive_walks_subtree() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        // Spread files across nested directories so we can prove the
        // walker descends rather than stopping at the top level.
        backend
            .write_file(Path::new("documents/abc/notes.md"), "a")
            .await
            .unwrap();
        backend
            .write_file(Path::new("documents/abc/mvt/concept.md"), "b")
            .await
            .unwrap();
        backend
            .write_file(Path::new("documents/abc/mvt/draft.txt"), "c")
            .await
            .unwrap();
        backend
            .write_file(Path::new("documents/abc/skip.pdf"), "d")
            .await
            .unwrap();

        // Without an extension filter the entries are sorted by relative
        // path with forward slashes regardless of platform.
        let all = backend
            .list_files_recursive(Path::new("documents/abc"), None)
            .await
            .unwrap();
        let paths: Vec<_> = all.iter().map(|e| e.relative_path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["mvt/concept.md", "mvt/draft.txt", "notes.md", "skip.pdf"]
        );

        // With an extension filter only matching files come back.
        let md_only = backend
            .list_files_recursive(Path::new("documents/abc"), Some("md"))
            .await
            .unwrap();
        let md_paths: Vec<_> = md_only.iter().map(|e| e.relative_path.as_str()).collect();
        assert_eq!(md_paths, vec!["mvt/concept.md", "notes.md"]);
    }

    #[tokio::test]
    async fn local_list_files_recursive_missing_dir_returns_empty() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let entries = backend
            .list_files_recursive(Path::new("documents/nope"), None)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn local_list_files_missing_dir() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let entries = backend
            .list_files(Path::new("nonexistent"), None)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn local_read_only_rejects_writes() {
        let dir = TempDir::new().unwrap();
        let backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), false);

        let result = backend.write_file(Path::new("test.txt"), "content").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("read-only"));
    }

    #[tokio::test]
    async fn local_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let mut backend = LocalBackend::new("test".to_string(), dir.path().to_path_buf(), true);
        backend.ensure_ready().await.unwrap();

        let result = backend.read_file(Path::new("../etc/passwd")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(".."));

        let result = backend.read_file(Path::new("/etc/passwd")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("relative"));
    }

    #[tokio::test]
    async fn local_ensure_ready_fails_for_missing_dir() {
        let mut backend =
            LocalBackend::new("test".to_string(), PathBuf::from("/nonexistent/path"), true);
        let result = backend.ensure_ready().await;
        assert!(result.is_err());
    }

    /// Init a standalone git repo at `path` with a committed corpus tree:
    /// two laws under `regulation/nl` (one declaring `implements`) plus a
    /// non-law `.yml` under a second country subtree. Returns nothing —
    /// callers inspect via git / the backend.
    async fn setup_corpus_checkout(path: &Path) {
        use tokio::process::Command;

        tokio::fs::create_dir_all(path).await.unwrap();
        // Empty template dir: an ambient global `init.templateDir` would
        // otherwise install this repo's hooks into the fixture.
        let template = path.join(".empty-template");
        tokio::fs::create_dir_all(&template).await.unwrap();
        let template_arg = format!("--template={}", template.display());
        for args in [
            vec!["init", "--initial-branch=development", &template_arg],
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(path)
                .output()
                .await
                .unwrap();
        }
        tokio::fs::remove_dir_all(&template).await.unwrap();

        let write = |rel: &str, content: &str| {
            let abs = path.join(rel);
            std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
            std::fs::write(abs, content).unwrap();
        };
        write(
            "regulation/nl/wet/wet_a/2025-01-01.yaml",
            "$id: wet_a\narticles: []\n",
        );
        write(
            "regulation/nl/regeling/reg_b/2025-01-01.yaml",
            "$id: reg_b\narticles:\n  - number: '1'\n    machine_readable:\n      implements:\n        - law: wet_a\n          open_term: iets\n",
        );
        // `.yml` law inside the source root — the fallback scan must count
        // it (the corpus convention is `.yaml`, but `.yml` occurs).
        write(
            "regulation/nl/wet/wet_kort/2025-01-01.yml",
            "$id: wet_kort\narticles: []\n",
        );
        // Law outside the source root (different country subtree): present
        // in the repo-level index but must be dropped from the
        // source-relative result.
        write(
            "regulation/be/wet/wet_be/2025-01-01.yaml",
            "$id: wet_be\narticles: []\n",
        );
        git_commit_all(path, "seed corpus").await;
    }

    async fn git_commit_all(path: &Path, message: &str) {
        use tokio::process::Command;
        for args in [
            vec!["add", "-A"],
            vec!["commit", "--no-verify", "-m", message],
        ] {
            let out = Command::new("git")
                .args(&args)
                .current_dir(path)
                .output()
                .await
                .unwrap();
            assert!(out.status.success(), "git {args:?} failed");
        }
    }

    async fn regulation_tree_sha(path: &Path) -> String {
        let out = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD:regulation"])
            .current_dir(path)
            .output()
            .await
            .unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Generate + commit the implements index for the checkout, exactly as
    /// the `implements-indexer` binary would.
    async fn write_index(path: &Path) {
        let outcome = crate::implements_index::scan_tree(path, "regulation").unwrap();
        assert!(outcome.failures.is_empty());
        let index = crate::implements_index::ImplementsIndex {
            version: crate::implements_index::IMPLEMENTS_INDEX_VERSION,
            root: "regulation".to_string(),
            tree_sha: regulation_tree_sha(path).await,
            files: outcome.files,
        };
        std::fs::write(
            path.join(crate::implements_index::IMPLEMENTS_INDEX_FILENAME),
            index.to_json(),
        )
        .unwrap();
        git_commit_all(path, "regenerate implements index").await;
    }

    fn git_backend_on(path: &Path, sparse: bool) -> GitBackend {
        let mut config = CorpusConfig::new("https://example.invalid/corpus.git", path);
        if sparse {
            config.sparse_paths = Some(vec!["regulation/nl/wet".to_string()]);
        }
        let client = CorpusClient::new(config);
        GitBackend::new(client, Some("regulation/nl".to_string()))
    }

    /// A sidecar staged at the repository root lands in the same commit as
    /// a source-relative write, and reads fall back to the legacy copy
    /// under the subpath until the root file exists.
    #[tokio::test]
    async fn git_sidecar_lives_at_the_repo_root_with_legacy_fallback() {
        use crate::annotation_sidecar::{LegacyFallback, read_sidecar, write_sidecar};

        let dir = TempDir::new().unwrap();
        setup_corpus_checkout(dir.path()).await;
        let legacy = dir
            .path()
            .join("regulation/nl/annotations/wet_a/annotations.yaml");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, "old notes\n").unwrap();
        git_commit_all(dir.path(), "legacy sidecar").await;

        let backend = git_backend_on(dir.path(), false);
        assert_eq!(backend.repo_subpath(), Some("regulation/nl"));
        assert_eq!(
            read_sidecar(&backend, "wet_a", None, LegacyFallback::Read)
                .await
                .unwrap()
                .as_deref(),
            Some("old notes\n")
        );

        write_sidecar(&backend, "wet_a", "old notes\nnew note\n")
            .await
            .unwrap();
        backend
            .write_file(Path::new("wet/wet_a/2025-01-01.yaml"), "$id: wet_a\n")
            .await
            .unwrap();
        backend
            .persist(&WriteContext::new("save".to_string(), None))
            .await
            .unwrap();

        let out = tokio::process::Command::new("git")
            .args(["show", "--name-only", "--format=", "HEAD"])
            .current_dir(dir.path())
            .output()
            .await
            .unwrap();
        let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        files.sort();
        assert_eq!(
            files,
            vec![
                "annotations/wet_a/annotations.yaml".to_string(),
                "regulation/nl/wet/wet_a/2025-01-01.yaml".to_string(),
            ]
        );
        // Root file now wins; the legacy copy is left as it was.
        assert_eq!(
            read_sidecar(&backend, "wet_a", None, LegacyFallback::Read)
                .await
                .unwrap()
                .as_deref(),
            Some("old notes\nnew note\n")
        );
        assert_eq!(std::fs::read_to_string(&legacy).unwrap(), "old notes\n");
        assert!(
            backend
                .write_repo_file(Path::new("../escape.yaml"), "x")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn git_read_all_implements_serves_fresh_index_not_the_working_tree() {
        let dir = TempDir::new().unwrap();
        setup_corpus_checkout(dir.path()).await;
        write_index(dir.path()).await;

        // Uncommitted working-tree edit AFTER index generation: HEAD (and
        // so the index) is unchanged, and the documented semantics are
        // that a fresh index wins over the working tree. This divergence
        // is what proves the index is actually being read.
        std::fs::write(
            dir.path().join("regulation/nl/wet/wet_a/2025-01-01.yaml"),
            "$id: wet_a\narticles:\n  - number: '1'\n    machine_readable:\n      implements:\n        - law: iets_nieuws\n          open_term: x\n",
        )
        .unwrap();

        let backend = git_backend_on(dir.path(), false);
        let mut got = backend.read_all_implements().await.unwrap();
        got.sort();

        assert_eq!(
            got,
            vec![
                (
                    "regeling/reg_b/2025-01-01.yaml".to_string(),
                    vec!["wet_a".to_string()]
                ),
                // Entry present with an EMPTY list — and still the
                // committed (pre-edit) content, proving the index served.
                ("wet/wet_a/2025-01-01.yaml".to_string(), Vec::new()),
                // `.yml` law has an entry too.
                ("wet/wet_kort/2025-01-01.yml".to_string(), Vec::new()),
            ],
            "expected index-served, source-relative entries for every law \
             under regulation/nl (and wet_be dropped as outside the root)"
        );
    }

    #[tokio::test]
    async fn git_read_all_implements_stale_index_falls_back_to_scan() {
        let dir = TempDir::new().unwrap();
        setup_corpus_checkout(dir.path()).await;
        write_index(dir.path()).await;

        // A committed corpus change after the last index regeneration —
        // the pr<N>-branch scenario. The regulation tree sha moves, the
        // index's recorded sha doesn't.
        std::fs::create_dir_all(dir.path().join("regulation/nl/wet/wet_nieuw")).unwrap();
        std::fs::write(
            dir.path()
                .join("regulation/nl/wet/wet_nieuw/2026-01-01.yaml"),
            "$id: wet_nieuw\narticles: []\n",
        )
        .unwrap();
        git_commit_all(dir.path(), "add law without regenerating index").await;

        let backend = git_backend_on(dir.path(), false);
        let got = backend.read_all_implements().await.unwrap();
        let paths: Vec<&str> = got.iter().map(|(p, _)| p.as_str()).collect();

        assert!(
            paths.contains(&"wet/wet_nieuw/2026-01-01.yaml"),
            "stale index must not shadow the newer checkout content; got {paths:?}"
        );
        assert!(
            paths.contains(&"wet/wet_kort/2025-01-01.yml"),
            "fallback scan must include .yml laws; got {paths:?}"
        );
    }

    #[tokio::test]
    async fn git_read_all_implements_missing_index_scans_checkout() {
        let dir = TempDir::new().unwrap();
        setup_corpus_checkout(dir.path()).await;
        // No index written at all.

        let backend = git_backend_on(dir.path(), false);
        let mut got = backend.read_all_implements().await.unwrap();
        got.sort();

        let paths: Vec<&str> = got.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "regeling/reg_b/2025-01-01.yaml",
                "wet/wet_a/2025-01-01.yaml",
                "wet/wet_kort/2025-01-01.yml",
            ]
        );
    }

    /// An index generated for a narrower subtree than this source's root
    /// describes none of this source. Serving it would project to an empty
    /// list, which the caller cannot tell apart from "this corpus has no
    /// laws" — so the backend must ignore it and scan.
    #[tokio::test]
    async fn git_read_all_implements_ignores_an_index_that_does_not_cover_the_source() {
        let dir = TempDir::new().unwrap();
        setup_corpus_checkout(dir.path()).await;

        // Source root is `regulation/nl`; index root is `regulation/be`.
        let outcome = crate::implements_index::scan_tree(dir.path(), "regulation/be").unwrap();
        let index = crate::implements_index::ImplementsIndex {
            version: crate::implements_index::IMPLEMENTS_INDEX_VERSION,
            root: "regulation/be".to_string(),
            tree_sha: "irrelevant".to_string(),
            files: outcome.files,
        };
        std::fs::write(
            dir.path()
                .join(crate::implements_index::IMPLEMENTS_INDEX_FILENAME),
            index.to_json(),
        )
        .unwrap();
        git_commit_all(dir.path(), "commit a non-covering index").await;

        let backend = git_backend_on(dir.path(), false);
        let got = backend.read_all_implements().await.unwrap();
        assert_eq!(
            got.len(),
            3,
            "expected the full scan of regulation/nl, got {got:?}"
        );
    }

    #[tokio::test]
    async fn git_read_all_implements_refuses_scan_on_sparse_checkout() {
        let dir = TempDir::new().unwrap();
        setup_corpus_checkout(dir.path()).await;
        // No index + sparse checkout: a scan would be silently incomplete
        // (only the cone is materialised), so the call must error instead
        // of returning a partial implements map.
        let backend = git_backend_on(dir.path(), true);
        let err = backend.read_all_implements().await.unwrap_err();
        assert!(
            err.to_string().contains("sparse"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn git_local_only_commits_without_push() {
        use tokio::process::Command;

        // Set up a bare repo
        let dir = TempDir::new().unwrap();
        let bare_path = dir.path().join("bare.git");
        Command::new("git")
            .args([
                "-c",
                "init.templateDir=",
                "init",
                "--bare",
                "--initial-branch=development",
            ])
            .arg(&bare_path)
            .output()
            .await
            .unwrap();
        let bare_url = format!("file://{}", bare_path.display());

        // Push an initial commit
        let tmp_clone = dir.path().join("tmp-clone");
        Command::new("git")
            .args(["-c", "init.templateDir=", "clone", &bare_url])
            .arg(&tmp_clone)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
            vec!["commit", "--allow-empty", "-m", "init"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }

        // Create a GitBackend without a token (local_only mode)
        let repo_path = dir.path().join("editor-repo");
        let config = CorpusConfig::new(&bare_url, &repo_path);
        // No .with_token() — triggers local_only
        let client = CorpusClient::new(config);
        let mut backend = GitBackend::new(client, None);
        assert!(backend.local_only);

        backend.ensure_ready().await.unwrap();

        // Write and persist
        backend
            .write_file(Path::new("test.feature"), "Feature: Test\n")
            .await
            .unwrap();
        backend
            .persist(&WriteContext {
                message: "add test scenario".to_string(),
                author: None,
                token_override: None,
            })
            .await
            .unwrap();

        // Verify local commit exists on a session branch
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let branch_str = String::from_utf8_lossy(&branch.stdout);
        assert!(
            branch_str.trim().starts_with("editor/session-"),
            "expected session branch, got: {branch_str}"
        );

        let log = Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("add test scenario"));

        // Verify NOT pushed
        let remote_log = Command::new("git")
            .args(["log", "--oneline", "--all"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let remote_str = String::from_utf8_lossy(&remote_log.stdout);
        assert!(!remote_str.contains("add test scenario"));
    }

    /// Set up a bare repo on `development` and a checkout. Returns
    /// (bare_path, working_repo_path, bare_url).
    #[cfg(feature = "github")]
    async fn setup_bare_with_checkout(dir: &Path) -> (PathBuf, PathBuf, String) {
        use tokio::process::Command;

        let bare_path = dir.join("bare.git");
        Command::new("git")
            .args([
                "-c",
                "init.templateDir=",
                "init",
                "--bare",
                "--initial-branch=development",
            ])
            .arg(&bare_path)
            .output()
            .await
            .unwrap();

        let bare_url = format!("file://{}", bare_path.display());

        // Seed initial commit on development
        let seed = dir.join("seed");
        Command::new("git")
            .args(["-c", "init.templateDir=", "clone", &bare_url])
            .arg(&seed)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
            vec!["commit", "--allow-empty", "-m", "init"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&seed)
                .output()
                .await
                .unwrap();
        }

        // Working checkout the SessionGitBackend will use
        let work = dir.join("work");
        Command::new("git")
            .args([
                "-c",
                "init.templateDir=",
                "clone",
                "--branch",
                "development",
                &bare_url,
            ])
            .arg(&work)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "Editor Service"],
            vec!["config", "user.email", "editor@regelrecht.local"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&work)
                .output()
                .await
                .unwrap();
        }

        (bare_path, work, bare_url)
    }

    #[cfg(feature = "github")]
    #[tokio::test]
    async fn session_backend_persists_then_opens_pr() {
        use regelrecht_github::GithubClient;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let dir = TempDir::new().unwrap();
        let (bare_path, work, bare_url) = setup_bare_with_checkout(dir.path()).await;

        // Mock GitHub Pulls API.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/minbzk/test-source/pulls"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/minbzk/test-source/pulls"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "number": 12,
                "html_url": "https://github.com/minbzk/test-source/pull/12",
            })))
            .mount(&server)
            .await;

        // Build the session backend.
        let mut config = CorpusConfig::new(&bare_url, &work);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        let github = GithubClient::new().unwrap().with_base_url(server.uri());

        let backend = SessionGitBackend::new(
            client,
            None,
            "editor/session-abc123".to_string(),
            "development".to_string(),
            "minbzk".to_string(),
            "test-source".to_string(),
            "test-token".to_string(),
        )
        .unwrap()
        .with_github_client(github);

        // Edit + persist.
        backend
            .write_file(Path::new("article.md"), "edited body")
            .await
            .unwrap();

        let outcome = backend
            .persist(&WriteContext {
                message: "Update article.md".to_string(),
                author: Some(EditorUser {
                    name: "Anne Schuth".to_string(),
                    email: "anne@example.gov".to_string(),
                }),
                token_override: None,
            })
            .await
            .unwrap();

        // PR returned to caller
        let pr = outcome.pr.expect("persist should return PR info");
        assert_eq!(pr.number, 12);
        assert_eq!(pr.html_url, "https://github.com/minbzk/test-source/pull/12");

        // Session branch exists on remote with our commit + trailer
        let log = tokio::process::Command::new("git")
            .args(["log", "editor/session-abc123", "--format=%B", "-1"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&log.stdout);
        assert!(
            body.contains("Update article.md"),
            "missing message in commit: {body}"
        );
        assert!(
            body.contains("Co-authored-by: Anne Schuth <anne@example.gov>"),
            "missing co-authored-by trailer: {body}"
        );
    }

    #[cfg(feature = "github")]
    #[tokio::test]
    async fn session_backend_empty_persist_returns_cached_pr() {
        use regelrecht_github::GithubClient;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let dir = TempDir::new().unwrap();
        let (_bare, work, bare_url) = setup_bare_with_checkout(dir.path()).await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/minbzk/test-source/pulls"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/minbzk/test-source/pulls"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "number": 99,
                "html_url": "https://github.com/minbzk/test-source/pull/99",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let mut config = CorpusConfig::new(&bare_url, &work);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);
        let github = GithubClient::new().unwrap().with_base_url(server.uri());

        let backend = SessionGitBackend::new(
            client,
            None,
            "editor/session-cached".to_string(),
            "development".to_string(),
            "minbzk".to_string(),
            "test-source".to_string(),
            "test-token".to_string(),
        )
        .unwrap()
        .with_github_client(github);

        // First persist makes a real PR
        backend.write_file(Path::new("a.md"), "hi").await.unwrap();
        let first = backend
            .persist(&WriteContext {
                message: "edit".to_string(),
                author: None,
                token_override: None,
            })
            .await
            .unwrap();
        assert_eq!(first.pr.unwrap().number, 99);

        // Second persist with no dirty files returns the cached PR — no
        // new HTTP calls (the wiremock `.expect(1)` enforces this).
        let second = backend
            .persist(&WriteContext {
                message: "noop".to_string(),
                author: None,
                token_override: None,
            })
            .await
            .unwrap();
        assert_eq!(second.pr.unwrap().number, 99);
    }

    #[cfg(feature = "github")]
    #[test]
    fn sanitize_pr_body_value_strips_control_chars() {
        // Newlines, tabs, NUL bytes in an OIDC display name must not bleed
        // into the PR body Markdown — they could break layout (extra
        // headings) or confuse GitHub's renderer.
        let dirty = "Anne\nSchuth\u{0000}\tX";
        let clean = sanitize_pr_body_value(dirty);
        assert!(!clean.contains('\n'));
        assert!(!clean.contains('\t'));
        assert!(!clean.contains('\u{0000}'));
        // Display characters survive, and consecutive control chars are
        // collapsed to a single space rather than a run of spaces.
        assert_eq!(clean, "Anne Schuth X");
    }

    #[cfg(feature = "github")]
    #[test]
    fn sanitize_pr_body_value_strips_angle_brackets() {
        // `<` and `>` are reserved as the email delimiter in both the PR
        // body line ("Ingediend door: Name <email>") and in the
        // `Co-authored-by` trailer. A name like "Foo <attacker@evil>" must
        // not survive — otherwise the resulting trailer
        // "Co-authored-by: Foo <attacker@evil> <real@email>" is malformed
        // and the attacker-controlled email may end up linked to the
        // commit instead of the real author.
        let clean = sanitize_pr_body_value("Foo <attacker@evil>");
        assert!(!clean.contains('<'));
        assert!(!clean.contains('>'));
        assert_eq!(clean, "Foo attacker@evil");
    }

    #[cfg(feature = "github")]
    #[test]
    fn sanitize_pr_body_value_preserves_unicode_letters() {
        // Diacritics and non-ASCII letters must pass through unchanged.
        let s = "Renée Müller";
        assert_eq!(sanitize_pr_body_value(s), s);
    }

    #[cfg(feature = "github")]
    #[tokio::test]
    async fn session_backend_rejects_empty_token() {
        let dir = TempDir::new().unwrap();
        let (_bare, work, bare_url) = setup_bare_with_checkout(dir.path()).await;

        let mut config = CorpusConfig::new(&bare_url, &work);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        let result = SessionGitBackend::new(
            client,
            None,
            "editor/session-x".to_string(),
            "development".to_string(),
            "owner".to_string(),
            "repo".to_string(),
            String::new(),
        );
        assert!(result.is_err(), "empty token must be rejected");
    }
}
