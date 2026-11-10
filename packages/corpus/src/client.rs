use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::config::CorpusConfig;
use crate::error::{CorpusError, Result};
use crate::timing;

/// Best-effort: write the snapshot contents back to disk after a failed
/// `checkout -B`. Used only on the error path so the working tree does not
/// look truncated to the user while their dirty content is still preserved
/// in memory by the caller. Filesystem errors are swallowed deliberately —
/// the original git error is what the caller cares about.
async fn replay_snapshot_best_effort(snapshots: &[(PathBuf, Option<Vec<u8>>)]) {
    for (path, content) in snapshots {
        match content {
            Some(bytes) => {
                if let Some(parent) = path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let _ = tokio::fs::write(path, bytes).await;
            }
            None => {
                let _ = tokio::fs::remove_file(path).await;
            }
        }
    }
}

pub struct CorpusClient {
    config: CorpusConfig,
    askpass_path: Option<PathBuf>,
}

impl CorpusClient {
    pub fn new(config: CorpusConfig) -> Self {
        Self {
            config,
            askpass_path: None,
        }
    }

    /// Write a GIT_ASKPASS helper script so the git token is passed via
    /// environment variables instead of being embedded in the clone URL
    /// (which would be visible via `/proc/[pid]/cmdline`).
    fn ensure_askpass_script(&mut self) -> Result<()> {
        if self.config.git_token().is_none() {
            return Ok(());
        }

        let script_path = self.config.askpass_script_path();
        if let Some(parent) = script_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CorpusError::Git(format!("failed to create askpass dir: {e}")))?;
        }
        std::fs::write(
            &script_path,
            "#!/bin/sh\nprintf '%s\\n' \"$REGELRECHT_GIT_TOKEN\"\n",
        )
        .map_err(|e| CorpusError::Git(format!("failed to write askpass script: {e}")))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| CorpusError::Git(format!("failed to set askpass permissions: {e}")))?;
        }

        self.askpass_path = Some(script_path);
        Ok(())
    }

    /// Ensure the corpus repo is available locally.
    ///
    /// If the repo directory doesn't exist, clones it (shallow, single branch).
    /// If it exists, fetches and resets to the remote branch.
    #[tracing::instrument(name = "corpus_ensure_repo", skip(self))]
    pub async fn ensure_repo(&mut self) -> Result<()> {
        self.ensure_askpass_script()?;

        let repo_path = &self.config.repo_path;

        // `clone` / `git_fetch` feed the Server-Timing phases of the same
        // name: the cold clone is the dominant cost of a traject's first
        // access (seconds), so it is worth isolating from the GitHub API
        // round-trips in the breakdown.
        if repo_path.join(".git").exists() {
            tracing::info!(path = %repo_path.display(), "corpus repo exists, updating");
            timing::measure("git_fetch", self.git_fetch_reset()).await?;
        } else {
            tracing::info!(path = %repo_path.display(), "cloning corpus repo");
            timing::measure("clone", self.git_clone()).await?;
        }

        Ok(())
    }

    /// Returns the local path to the corpus repo working directory.
    pub fn repo_path(&self) -> &Path {
        &self.config.repo_path
    }

    /// Whether this client has a push token configured.
    pub fn has_push_token(&self) -> bool {
        self.config.git_token().is_some()
    }

    /// Whether this checkout is a sparse (cone-mode) checkout. A sparse
    /// working tree is *silently incomplete* for any filesystem-wide scan;
    /// callers that would otherwise fall back to walking the tree use this
    /// to refuse instead of returning partial results.
    pub fn is_sparse(&self) -> bool {
        self.config.sparse_paths.is_some()
    }

    /// Git tree sha of `subtree` at the checkout's `HEAD` (`git rev-parse
    /// HEAD:<subtree>`). This is the content identity of that directory on
    /// the checked-out commit — comparable across clones, branches and the
    /// GitHub Trees API. Errors when the path does not exist in `HEAD` or
    /// the directory is not a git repo.
    pub async fn subtree_sha(&self, subtree: &str) -> Result<String> {
        let spec = format!("HEAD:{subtree}");
        let out = self.run_git_output(&["rev-parse", &spec]).await?;
        Ok(out.trim().to_string())
    }

    /// Create a local branch (no push).
    pub async fn create_local_branch(&self, branch: &str) -> Result<()> {
        self.run_git(&["checkout", "-b", branch]).await?;
        tracing::info!(branch = %branch, "created local branch");
        Ok(())
    }

    /// Stage the given paths and commit locally (no push).
    ///
    /// If there are no changes to commit (working tree is clean), this is a no-op.
    pub async fn commit_local(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        let mut add_args = vec!["add", "--"];
        let path_strings: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        for p in &path_strings {
            add_args.push(p);
        }
        self.run_git(&add_args).await?;

        let status_output = self.run_git_output(&["status", "--porcelain"]).await?;
        if status_output.trim().is_empty() {
            tracing::debug!("no changes to commit, skipping");
            return Ok(());
        }

        self.run_git(&["commit", "-m", message]).await?;
        tracing::info!(message = %message, "committed locally (no push)");
        Ok(())
    }

    /// Maximum number of rebase+push attempts before giving up.
    const MAX_PUSH_ATTEMPTS: u32 = 5;

    /// Stage the given paths, commit, and push to the remote branch.
    ///
    /// If there are no changes to commit (working tree is clean), this is a no-op.
    /// Uses a retry loop around rebase+push to handle concurrent push race
    /// conditions where multiple workers push to the same branch.
    pub async fn commit_and_push(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        self.stage_paths(paths).await?;

        // Check if there's anything to commit
        let status_output = self.run_git_output(&["status", "--porcelain"]).await?;

        if status_output.trim().is_empty() {
            tracing::debug!("no changes to commit, skipping");
            return Ok(());
        }

        // Commit
        self.run_git(&["commit", "-m", message]).await?;

        self.rebase_and_push(message).await
    }

    /// Commit and push, but treat `content_paths` as the only thing that
    /// counts as a "real change". `metadata_paths` (e.g. a `status.yaml`
    /// carrying a `last_harvested` timestamp that churns on every run) are
    /// committed *alongside* content when content changed, but never *trigger*
    /// a commit on their own.
    ///
    /// Returns `Ok(true)` if a commit was pushed (either freshly made, or a
    /// stranded local commit from an interrupted earlier attempt), `Ok(false)`
    /// if the content was unchanged and the remote already has it (in which
    /// case the metadata files are restored to HEAD so the working tree stays
    /// clean for the next run — this matters for the long-lived corpus clone
    /// the harvest worker reuses).
    pub async fn commit_and_push_content(
        &self,
        content_paths: &[PathBuf],
        metadata_paths: &[PathBuf],
        message: &str,
    ) -> Result<bool> {
        // Stage only the content so the porcelain check below reflects content
        // changes alone, not the always-churning metadata files.
        self.stage_paths(content_paths).await?;

        let content_path_strings = Self::path_strings(content_paths);
        let mut status_args = vec!["status", "--porcelain", "--"];
        status_args.extend(content_path_strings.iter().map(String::as_str));
        let status_output = self.run_git_output(&status_args).await?;

        if status_output.trim().is_empty() {
            tracing::debug!("no content changes, skipping commit");
            // Discard the churned metadata (e.g. status.yaml timestamp) so the
            // working tree matches HEAD again — critical for the long-lived
            // corpus clone, where a leftover dirty/untracked file would break
            // the next `pull --rebase`.
            self.restore_metadata(metadata_paths).await;

            // The porcelain check above compares against the LOCAL HEAD. A
            // previous attempt may have committed this exact content and then
            // been interrupted (e.g. the worker's job timeout) before its push
            // completed — the stranded commit makes the re-harvest look like
            // "no change" even though the remote never received the content.
            // If we returned `Ok(false)` here, the job would complete while
            // the next `reset --hard origin/<branch>` (or pod restart)
            // silently discards the commit. Push it instead.
            if self.local_commits_ahead_of_remote().await? > 0 {
                tracing::info!(
                    "local HEAD ahead of remote tracking branch, pushing stranded commit(s)"
                );
                // Log the stranded commit's own subject, not this retry's
                // `message` — the stranded commit is what lands on the remote.
                let stranded_subject = self
                    .run_git_output(&["log", "-1", "--format=%s"])
                    .await
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| "<unknown stranded commit>".to_string());
                self.rebase_and_push(&stranded_subject).await?;
                return Ok(true);
            }
            return Ok(false);
        }

        // Content changed — stage the metadata files alongside it and commit both.
        self.stage_paths(metadata_paths).await?;
        self.run_git(&["commit", "-m", message]).await?;
        self.rebase_and_push(message).await?;
        Ok(true)
    }

    /// Reset the given metadata paths back to HEAD: restore tracked files and
    /// remove any still-untracked ones, so the working tree is clean afterward.
    /// Best-effort — failures are logged but not propagated, since the caller
    /// is already returning the "no changes" success path.
    async fn restore_metadata(&self, metadata_paths: &[PathBuf]) {
        let path_strings = Self::path_strings(metadata_paths);
        if path_strings.is_empty() {
            return;
        }
        let pathspecs: Vec<&str> = path_strings.iter().map(String::as_str).collect();

        // Tracked metadata: restore the committed contents (drops the churn).
        let mut checkout_args = vec!["checkout", "--"];
        checkout_args.extend(pathspecs.iter().copied());
        if let Err(e) = self.run_git(&checkout_args).await {
            // Expected when a metadata file was never tracked (nothing to
            // restore); the `git clean` below removes those instead.
            tracing::debug!(error = %e, "metadata checkout had nothing to restore");
        }

        // Untracked metadata: remove it so the tree is clean for the next run.
        let mut clean_args = vec!["clean", "-fq", "--"];
        clean_args.extend(pathspecs.iter().copied());
        if let Err(e) = self.run_git(&clean_args).await {
            tracing::warn!(error = %e, "failed to clean untracked metadata; working tree may be dirty");
        }
    }

    /// Number of commits the local HEAD is ahead of the remote tracking
    /// branch (`origin/<branch>..HEAD`). Non-zero means an earlier
    /// commit-and-push was interrupted between the commit and the push.
    async fn local_commits_ahead_of_remote(&self) -> Result<u64> {
        let range = format!("origin/{}..HEAD", self.config.branch);
        let output = self
            .run_git_output(&["rev-list", "--count", &range])
            .await?;
        let count = output.trim();
        count
            .parse::<u64>()
            .map_err(|e| CorpusError::Git(format!("could not parse rev-list count {count:?}: {e}")))
    }

    /// Lossy UTF-8 path strings for use as git pathspec arguments.
    fn path_strings(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    }

    /// Stage the given paths (`git add -- <paths>`). No-op when `paths` is
    /// empty — running `git add --` with no pathspecs is a no-op on some git
    /// versions but errors on others, so guard it.
    async fn stage_paths(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let path_strings = Self::path_strings(paths);
        let mut add_args = vec!["add", "--"];
        add_args.extend(path_strings.iter().map(String::as_str));
        self.run_git(&add_args).await
    }

    /// Rebase on the remote branch then push the local commit, retrying with
    /// backoff to absorb concurrent pushes from other workers.
    async fn rebase_and_push(&self, message: &str) -> Result<()> {
        let mut last_error = None;
        for attempt in 1..=Self::MAX_PUSH_ATTEMPTS {
            // Pull --rebase to incorporate any concurrent remote changes.
            // On shallow clones (--depth 1), rebase may fail if the remote
            // advanced by many commits. The error-recovery path below
            // restores the working tree (abort rebase + hard-reset to remote)
            // and propagates the error for job-level retry. The committed
            // files remain on disk so the next attempt can re-stage them.
            if let Err(e) = self
                .run_git(&["pull", "--rebase", "origin", &self.config.branch])
                .await
            {
                tracing::warn!(attempt, error = %e, "pull --rebase failed, aborting rebase");
                let _ = self.run_git(&["rebase", "--abort"]).await;
                // Hard-reset to remote to recover from force-pushes or diverged
                // history. The harvested files are still on disk so the next
                // job-level retry can re-stage and commit them cleanly.
                let remote_ref = format!("origin/{}", self.config.branch);
                let _ = self
                    .run_git(&["fetch", "--depth", "1", "origin", &self.config.branch])
                    .await;
                let _ = self.run_git(&["reset", "--hard", &remote_ref]).await;
                return Err(e);
            }

            // Push — may fail if another worker pushed between our rebase and push.
            match self.run_git(&["push", "origin", &self.config.branch]).await {
                Ok(()) => {
                    if attempt > 1 {
                        tracing::info!(attempt, "push succeeded after retry");
                    }
                    tracing::info!(message = %message, "committed and pushed to corpus repo");
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        max = Self::MAX_PUSH_ATTEMPTS,
                        error = %e,
                        "push failed{}",
                        if attempt < Self::MAX_PUSH_ATTEMPTS {
                            ", will retry with rebase"
                        } else {
                            ", all attempts exhausted"
                        }
                    );
                    last_error = Some(e);
                }
            }

            if attempt < Self::MAX_PUSH_ATTEMPTS {
                // Exponential backoff: 500ms, 1s, 2s, 4s
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| CorpusError::Git("push failed after retries".into())))
    }

    /// Stage paths, commit, and push to a session branch (not the configured
    /// branch). Used by the editor write-back path: each editor session owns
    /// its own branch on the source repo, and `persist` calls land as
    /// fast-forward commits on that branch (which a PR then surfaces).
    ///
    /// The session branch is single-writer per (session UUID, source). On
    /// the first call it is created from `origin/{base_branch}`; on
    /// subsequent calls it picks up wherever `origin/{branch}` left off so
    /// the editor-api can restart between saves without losing progress
    /// (the branch is the source of truth, not the in-memory state).
    ///
    /// Push is **not** force: if a second writer ever races us, the second
    /// push fails with non-fast-forward and the user retries. Force-pushing
    /// would silently drop the racing commit.
    ///
    /// `co_authored_by` is `(name, email)`; when present it gets appended
    /// as a `Co-authored-by` trailer so GitHub credits the human editor on
    /// the commit even though the git author/committer stays the service
    /// identity.
    ///
    /// Returns `Ok(true)` when a commit was created and pushed to the
    /// remote, `Ok(false)` when there was nothing to commit (empty `paths`
    /// or all files matched the base content after snapshot+replay). The
    /// caller uses this to gate downstream side-effects like opening a PR
    /// — calling PR-open against a branch that was never pushed would
    /// return a 422 from GitHub.
    pub async fn commit_and_push_to_branch(
        &self,
        branch: &str,
        base_branch: &str,
        paths: &[PathBuf],
        message: &str,
        co_authored_by: Option<(&str, &str)>,
    ) -> Result<bool> {
        // Make sure we have the latest base, and the session branch tip
        // when one already exists on the remote.
        self.run_git(&["fetch", "--depth", "1", "origin", base_branch])
            .await?;
        let session_remote_exists = self.remote_branch_exists(branch).await?;
        if session_remote_exists {
            self.run_git(&["fetch", "--depth", "1", "origin", branch])
                .await?;
        }

        // Snapshot the dirty paths in memory BEFORE the checkout so we can
        // restore them on top of `start_point`. Without this snapshot,
        // `checkout -B branch <start_point>` would fail with "Your local
        // changes would be overwritten" when the session branch was deleted
        // upstream (start_point flips to origin/<base>) and the working
        // tree's dirty file content conflicts with the base. A missing
        // file in the snapshot means the caller invoked `delete_file`; we
        // record `None` and replay by removing the file post-checkout so
        // deletes survive the rebase onto `start_point`.
        let mut snapshots: Vec<(PathBuf, Option<Vec<u8>>)> = Vec::with_capacity(paths.len());
        for p in paths {
            match tokio::fs::read(p).await {
                Ok(bytes) => snapshots.push((p.clone(), Some(bytes))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    snapshots.push((p.clone(), None));
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Position on the session branch. `-B` resets-or-creates. Safe
        // because the session branch is single-writer per session UUID and
        // the editor-api process serialises persists for one session via
        // the SessionRegistry mutex; the local checkout is just a workspace
        // we keep moving onto whichever session branch is being written.
        let start_point = if session_remote_exists {
            format!("origin/{branch}")
        } else {
            format!("origin/{base_branch}")
        };
        // Remove the snapshot paths from disk so the subsequent
        // `checkout -B` can never fail on "would be overwritten". We then
        // replay the snapshot to put the user's edits back on top of
        // `start_point`. Removing the file directly (rather than `git
        // reset --hard`) avoids clobbering any *other* in-flight
        // sibling-session edits in the same working tree.
        for (path, _) in &snapshots {
            match tokio::fs::remove_file(path).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        // If `checkout -B` fails, the dirty paths are still removed from disk
        // (above). The caller's `persist` keeps the in-memory `dirty_files`
        // list intact, but a follow-up save would re-write the paths before
        // committing — so the *content* is recoverable from browser state. To
        // avoid leaving the working tree visibly missing files in the meantime,
        // best-effort replay the snapshot back to disk before propagating the
        // checkout error.
        if let Err(e) = self
            .run_git(&["checkout", "-B", branch, &start_point])
            .await
        {
            replay_snapshot_best_effort(&snapshots).await;
            return Err(e);
        }

        // Replay the snapshot: write back tracked content, remove paths
        // that were marked for deletion.
        for (path, content) in &snapshots {
            match content {
                Some(bytes) => {
                    if let Some(parent) = path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(path, bytes).await?;
                }
                None => match tokio::fs::remove_file(path).await {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                },
            }
        }

        // Stage the explicitly provided paths. Anything else dirty in the
        // working tree (left over from a previous sibling-session write)
        // stays unstaged and won't be committed.
        let mut add_args = vec!["add", "--"];
        let path_strings: Vec<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        for p in &path_strings {
            add_args.push(p);
        }
        self.run_git(&add_args).await?;

        // Skip empty commits — the user may have hit Save without changes.
        let status = self.run_git_output(&["status", "--porcelain"]).await?;
        if status.trim().is_empty() {
            tracing::debug!(branch = %branch, "no changes to commit on session branch");
            return Ok(false);
        }

        let commit_message = match co_authored_by {
            Some((name, email)) => format!("{message}\n\nCo-authored-by: {name} <{email}>"),
            None => message.to_string(),
        };
        self.run_git(&["commit", "-m", &commit_message]).await?;

        // First push of a new session branch needs `-u` to set upstream;
        // after that the remote tracking is in place. Use `-u` always for
        // safety — git is idempotent here.
        self.run_git(&["push", "-u", "origin", branch]).await?;

        tracing::info!(
            branch = %branch,
            base = %base_branch,
            "pushed editor session commit"
        );
        Ok(true)
    }

    #[tracing::instrument(name = "git_clone", skip(self), fields(branch = %self.config.branch))]
    async fn git_clone(&self) -> Result<()> {
        let url = self.config.clone_url();
        let path_str = self.config.repo_path.to_string_lossy().to_string();
        let sparse = self.config.sparse_paths.is_some();

        let mut args = vec![
            "clone",
            "--depth",
            "1",
            "--quiet",
            "--branch",
            &self.config.branch,
            "--single-branch",
        ];
        if sparse {
            // Partial clone: lazy-fetch blobs via sparse paths instead of shipping the full pack.
            args.push("--filter=blob:none");
            args.push("--no-checkout");
        }
        args.push(&url);
        args.push(&path_str);

        let output = self.git_command(&args).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Branch doesn't exist on remote — clone development and create the branch
            if stderr.contains("not found in upstream") || stderr.contains("Remote branch") {
                tracing::info!(
                    branch = %self.config.branch,
                    "branch not found on remote, creating from development"
                );
                return self.git_clone_and_create_branch().await;
            }

            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!("git clone failed: {sanitized}")));
        }

        self.configure_git_user().await?;
        self.setup_sparse_checkout().await?;
        Ok(())
    }

    /// Clone the base branch (configurable via [`CorpusConfig::base_branch`],
    /// defaulting to `"development"` for the legacy harvester flow), then
    /// create and push the target branch.
    async fn git_clone_and_create_branch(&self) -> Result<()> {
        let url = self.config.clone_url();
        let path_str = self.config.repo_path.to_string_lossy().to_string();
        let sparse = self.config.sparse_paths.is_some();
        let base = self.config.base_branch.as_deref().unwrap_or("development");

        let mut args = vec![
            "clone",
            "--depth",
            "1",
            "--quiet",
            "--branch",
            base,
            "--single-branch",
        ];
        if sparse {
            // Partial clone — see `git_clone` for the rationale.
            args.push("--filter=blob:none");
            args.push("--no-checkout");
        }
        args.push(&url);
        args.push(&path_str);

        let output = self.git_command(&args).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!(
                "git clone (base={base}) failed: {sanitized}"
            )));
        }

        self.configure_git_user().await?;
        self.setup_sparse_checkout().await?;

        // Create the target branch and push it
        self.run_git(&["checkout", "-b", &self.config.branch])
            .await?;
        self.run_git(&["push", "-u", "origin", &self.config.branch])
            .await?;

        tracing::info!(
            branch = %self.config.branch,
            base = %base,
            "created and pushed new branch"
        );
        Ok(())
    }

    /// Configure sparse-checkout if `sparse_paths` is set on the config.
    ///
    /// Uses cone mode so only the listed directory trees are materialized.
    /// No-op when `sparse_paths` is `None` (full checkout).
    async fn setup_sparse_checkout(&self) -> Result<()> {
        let paths = match self.config.sparse_paths {
            Some(ref p) if !p.is_empty() => p,
            _ => return Ok(()),
        };

        self.run_git(&["sparse-checkout", "init", "--cone"]).await?;

        let mut args = vec!["sparse-checkout", "set"];
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        args.extend(refs);
        self.run_git(&args).await?;

        // Materialize the working tree (only the sparse paths)
        self.run_git(&["checkout"]).await?;

        tracing::info!(paths = ?paths, "sparse checkout configured");
        Ok(())
    }

    /// Add directory trees to an existing sparse checkout.
    ///
    /// The enrichment checkout materialises one law, which is what keeps the
    /// agent's working directory small. The context brief then walks that
    /// directory looking for the laws this one cites, finds nothing, and tells
    /// the agent every citation is absent from the corpus — a claim that is
    /// false and that it acts on. Which laws those are is only known after the
    /// law file itself is readable, so they are added here rather than at
    /// clone time.
    ///
    /// No-op when the checkout is not sparse, or when no law resolves.
    ///
    /// The directories are resolved with `git ls-tree`, which reads the commit
    /// rather than the working tree: a sparse checkout has not materialised
    /// the very directories that have to be found, so looking on disk would
    /// find nothing by construction.
    pub async fn widen_to_laws(&self, bwb_ids: &[String]) -> Result<Vec<String>> {
        if self.config.sparse_paths.is_none() || bwb_ids.is_empty() {
            return Ok(Vec::new());
        }
        // The number lives inside the file, as `bwb_id:`, and never in the
        // path: the harvester names the directory after the law's title
        // (`to_slug`), so on the real corpus not one of the paths carries a
        // BWBR number while 32 of the files do. Matching on the path found
        // nothing, every time, and the widening it was supposed to do never
        // happened.
        //
        // `git grep` against HEAD reads the object database rather than the
        // working tree, which is what makes this work at all here: under a
        // sparse checkout the files this has to search are precisely the ones
        // not on disk. One law per directory, so the directory of the file
        // that names the number is the tree to add. A number no file declares
        // is a citation outside this corpus, which the brief already reports
        // honestly.
        let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for bwb in bwb_ids {
            let pattern = format!("bwb_id: {bwb}");
            let hits = match self
                .run_git_output(&["grep", "-l", "-F", &pattern, "HEAD"])
                .await
            {
                Ok(hits) => hits,
                // `git grep` exits 1 when nothing matches, which is an answer
                // and not a failure. A law this corpus does not carry is the
                // ordinary case.
                Err(_) => continue,
            };
            for line in hits.lines() {
                // `git grep <tree>` prefixes every path with `HEAD:`.
                let path = line.strip_prefix("HEAD:").unwrap_or(line);
                if let Some((dir, _)) = path.rsplit_once('/') {
                    dirs.insert(dir.to_string());
                }
            }
        }
        if dirs.is_empty() {
            return Ok(Vec::new());
        }

        let added: Vec<String> = dirs.into_iter().collect();
        let mut args = vec!["sparse-checkout", "add"];
        args.extend(added.iter().map(String::as_str));
        self.run_git(&args).await?;
        self.run_git(&["checkout"]).await?;
        tracing::info!(added = ?added, "sparse checkout widened to the cited laws");
        Ok(added)
    }

    /// Fetch the base branch shallowly and return the git blob SHA of `path` on
    /// it. Uses `FETCH_HEAD` (the single-branch enrich clone never creates
    /// `origin/<base>`). Blob SHA is content-addressed, so it is comparable
    /// across clones.
    pub async fn fetch_base_blob_sha(&self, base_branch: &str, path: &str) -> Result<String> {
        self.run_git(&["fetch", "--depth", "1", "origin", base_branch])
            .await?;
        let sha = self
            .run_git_output(&["rev-parse", &format!("FETCH_HEAD:{path}")])
            .await?;
        Ok(sha.trim().to_string())
    }

    /// Whether `path` is tracked in the current checkout/branch.
    pub async fn is_tracked(&self, path: &str) -> Result<bool> {
        let listed = self.run_git_output(&["ls-files", "--", path]).await?;
        Ok(!listed.trim().is_empty())
    }

    /// Check `path` out of the most recent `FETCH_HEAD` and unstage it, so it
    /// does not pollute an empty `git status --porcelain` check before the
    /// enrichment commits. Call `fetch_base_blob_sha` first so `FETCH_HEAD` is
    /// set.
    pub async fn checkout_path_from_fetch_head(&self, path: &str) -> Result<()> {
        self.run_git(&["checkout", "FETCH_HEAD", "--", path])
            .await?;
        self.run_git(&["reset", "HEAD", "--", path]).await?;
        Ok(())
    }

    async fn configure_git_user(&self) -> Result<()> {
        self.run_git(&["config", "user.name", &self.config.git_author_name])
            .await?;
        self.run_git(&["config", "user.email", &self.config.git_author_email])
            .await?;
        Ok(())
    }

    #[tracing::instrument(name = "git_fetch_reset", skip(self), fields(branch = %self.config.branch))]
    async fn git_fetch_reset(&self) -> Result<()> {
        self.run_git(&["fetch", "--depth", "1", "origin", &self.config.branch])
            .await?;

        let remote_ref = format!("origin/{}", self.config.branch);
        self.run_git(&["reset", "--hard", &remote_ref]).await?;

        Ok(())
    }

    /// Check whether a branch exists on the configured `origin` remote.
    pub async fn remote_branch_exists(&self, branch: &str) -> Result<bool> {
        let refspec = format!("refs/heads/{}", branch);
        let output = self
            .run_git_output(&["ls-remote", "origin", &refspec])
            .await?;
        Ok(!output.trim().is_empty())
    }

    /// Build a git `Command` with the shared environment and `kill_on_drop`.
    ///
    /// `kill_on_drop` matters because callers may be wrapped in a timeout
    /// (e.g. the harvest worker's job timeout): when the timed-out future is
    /// dropped, the subprocess must be killed too — otherwise a hung
    /// `git pull --rebase` keeps running and races the next job in the same
    /// checkout (index.lock contention).
    fn git_command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-c")
            .arg("core.hooksPath=")
            .args(args)
            .envs(self.git_env())
            .kill_on_drop(true);
        cmd
    }

    /// Run a git command in the repo directory and check for success.
    async fn run_git(&self, args: &[&str]) -> Result<()> {
        let output = self
            .git_command(args)
            .current_dir(&self.config.repo_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                sanitized
            )));
        }

        Ok(())
    }

    /// Run a git command and return stdout.
    async fn run_git_output(&self, args: &[&str]) -> Result<String> {
        let output = self
            .git_command(args)
            .current_dir(&self.config.repo_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let sanitized = self.sanitize_output(&stderr);
            return Err(CorpusError::Git(format!(
                "git {} failed: {}",
                args.first().unwrap_or(&""),
                sanitized
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Strip the git token from output to prevent credential leaks in logs.
    fn sanitize_output(&self, output: &str) -> String {
        match self.config.git_token() {
            Some(token) if !token.is_empty() => output.replace(token, "***"),
            _ => output.to_string(),
        }
    }

    /// Environment variables for git commands (author/committer identity,
    /// optional GIT_ASKPASS for token-based authentication, and resource
    /// limits for container environments).
    fn git_env(&self) -> Vec<(String, String)> {
        let mut env = vec![
            (
                "GIT_AUTHOR_NAME".into(),
                self.config.git_author_name.clone(),
            ),
            (
                "GIT_AUTHOR_EMAIL".into(),
                self.config.git_author_email.clone(),
            ),
            (
                "GIT_COMMITTER_NAME".into(),
                self.config.git_author_name.clone(),
            ),
            (
                "GIT_COMMITTER_EMAIL".into(),
                self.config.git_author_email.clone(),
            ),
            ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ];

        // Disable threaded index preloading (core.preloadIndex) and limit
        // index operations to a single thread (index.threads) to prevent
        // "unable to create threaded lstat: Resource temporarily unavailable"
        // errors in resource-constrained containers with low PID/thread limits.
        let git_configs: &[(&str, &str)] =
            &[("core.preloadIndex", "false"), ("index.threads", "1")];
        env.push(("GIT_CONFIG_COUNT".into(), git_configs.len().to_string()));
        for (i, (key, value)) in git_configs.iter().enumerate() {
            env.push((format!("GIT_CONFIG_KEY_{i}"), (*key).into()));
            env.push((format!("GIT_CONFIG_VALUE_{i}"), (*value).into()));
        }

        if let (Some(askpass), Some(token)) = (&self.askpass_path, self.config.git_token()) {
            env.push(("GIT_ASKPASS".into(), askpass.to_string_lossy().into()));
            env.push(("REGELRECHT_GIT_TOKEN".into(), token.into()));
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a bare git repo with one empty initial commit on `development`.
    async fn setup_bare_repo(dir: &Path) -> PathBuf {
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

        // Push an initial commit via a temp clone (use file:// for --depth support)
        let tmp_clone = dir.join("tmp-clone");
        let bare_url = format!("file://{}", bare_path.display());
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

        bare_path
    }

    /// Clone a bare repo and configure git user.
    async fn clone_with_config(bare_path: &Path, repo_path: &Path) {
        let bare_url = format!("file://{}", bare_path.display());
        Command::new("git")
            .args(["-c", "init.templateDir=", "clone", &bare_url])
            .arg(repo_path)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(repo_path)
                .output()
                .await
                .unwrap();
        }
    }

    /// git blob SHA of `content` as git would compute it (matches rev-parse of
    /// a committed/fetched blob with identical bytes).
    fn git_hash_object(content: &str) -> String {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("git")
            .args(["hash-object", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    #[test]
    fn test_sanitize_output_strips_token() {
        let config = CorpusConfig::new("https://github.com/example/repo.git", "/tmp/test")
            .with_token("ghp_secret123");
        let client = CorpusClient::new(config);

        let output = "fatal: could not read from remote https://token:ghp_secret123@github.com/example/repo.git";
        let sanitized = client.sanitize_output(output);
        assert!(!sanitized.contains("ghp_secret123"));
        assert!(sanitized.contains("***"));
    }

    #[test]
    fn test_sanitize_output_no_token() {
        let config = CorpusConfig::new("https://github.com/example/repo.git", "/tmp/test");
        let client = CorpusClient::new(config);

        let output = "fatal: repository not found";
        let sanitized = client.sanitize_output(output);
        assert_eq!(sanitized, output);
    }

    #[tokio::test]
    async fn test_ensure_repo_clones_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().join("corpus");
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        assert!(repo_path.join(".git").exists());
    }

    #[tokio::test]
    async fn test_commit_and_push_no_changes() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);

        // No changes — should be a no-op
        let result = client.commit_and_push(&[], "no changes").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_commit_and_push_with_changes() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Write a test file
        let test_file = repo_path.join("test.txt");
        tokio::fs::write(&test_file, "hello").await.unwrap();

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);
        client
            .commit_and_push(&[test_file], "add test file")
            .await
            .unwrap();

        // Verify commit was pushed by checking the bare repo
        let log = Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("add test file"));
    }

    #[tokio::test]
    async fn test_commit_and_push_content_skips_metadata_only_change() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let law = repo_path.join("law.yaml");
        let status = repo_path.join("status.yaml");
        tokio::fs::write(&law, "articles: 1\n").await.unwrap();
        tokio::fs::write(&status, "last_harvested: T1\n")
            .await
            .unwrap();

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);

        // First harvest: content is new → commit made.
        let committed = client
            .commit_and_push_content(
                std::slice::from_ref(&law),
                std::slice::from_ref(&status),
                "harvest: initial",
            )
            .await
            .unwrap();
        assert!(committed, "first harvest should commit new content");

        let count_commits = |path: PathBuf| async move {
            let out = Command::new("git")
                .args(["rev-list", "--count", "HEAD"])
                .current_dir(&path)
                .output()
                .await
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let commits_after_first = count_commits(bare_path.clone()).await;

        // Re-harvest: identical law content, only the status timestamp churns.
        tokio::fs::write(&status, "last_harvested: T2\n")
            .await
            .unwrap();
        let committed = client
            .commit_and_push_content(
                std::slice::from_ref(&law),
                std::slice::from_ref(&status),
                "harvest: no change",
            )
            .await
            .unwrap();
        assert!(!committed, "metadata-only change must not commit");

        // No new commit on the remote.
        assert_eq!(
            commits_after_first,
            count_commits(bare_path.clone()).await,
            "no new commit should be pushed for a metadata-only change"
        );
        // The churned metadata file was restored to HEAD (T1), keeping the
        // working tree clean for the next run.
        let restored = tokio::fs::read_to_string(&status).await.unwrap();
        assert_eq!(restored, "last_harvested: T1\n");

        // Real content change → commit again.
        tokio::fs::write(&law, "articles: 2\n").await.unwrap();
        tokio::fs::write(&status, "last_harvested: T3\n")
            .await
            .unwrap();
        let committed = client
            .commit_and_push_content(
                std::slice::from_ref(&law),
                std::slice::from_ref(&status),
                "harvest: real change",
            )
            .await
            .unwrap();
        assert!(committed, "content change should commit");

        let log = Command::new("git")
            .args(["log", "--oneline"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("harvest: initial"));
        assert!(log_str.contains("harvest: real change"));
        assert!(
            !log_str.contains("harvest: no change"),
            "no-change harvest must not appear in history: {log_str}"
        );
    }

    /// When law content is unchanged (tracked) but the metadata file is
    /// untracked, the no-change path must leave the working tree clean —
    /// otherwise a leftover untracked status.yaml breaks the next pull --rebase.
    #[tokio::test]
    async fn test_commit_and_push_content_cleans_untracked_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let law = repo_path.join("law.yaml");
        let status = repo_path.join("status.yaml");
        tokio::fs::write(&law, "articles: 1\n").await.unwrap();

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);

        // Commit only the law YAML — status.yaml never gets tracked.
        let committed = client
            .commit_and_push_content(std::slice::from_ref(&law), &[], "harvest: law only")
            .await
            .unwrap();
        assert!(committed);

        // Now an untracked status.yaml appears next to unchanged law content.
        tokio::fs::write(&status, "last_harvested: T1\n")
            .await
            .unwrap();
        let committed = client
            .commit_and_push_content(
                std::slice::from_ref(&law),
                std::slice::from_ref(&status),
                "harvest: untracked metadata",
            )
            .await
            .unwrap();
        assert!(!committed, "unchanged content must not commit");

        // The untracked metadata was cleaned and the tree is clean.
        assert!(
            !status.exists(),
            "untracked status.yaml should have been removed"
        );
        let porcelain = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&porcelain.stdout).trim().is_empty(),
            "working tree must be clean after no-change harvest"
        );
    }

    /// Regression: a harvest job can be interrupted (worker job timeout, pod
    /// restart) after `git commit` but before the push completes. The retry
    /// then re-harvests identical content, the porcelain check against the
    /// LOCAL HEAD (which already contains the stranded commit) is empty, and
    /// the old code returned `Ok(false)` without ever pushing — the law was
    /// marked harvested while the remote corpus lacked the commit, and the
    /// next `reset --hard origin/<branch>` discarded it permanently.
    /// `commit_and_push_content` must detect the stranded commit and push it.
    #[tokio::test]
    async fn test_commit_and_push_content_pushes_stranded_local_commit() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Simulate the interrupted first attempt: the harvested content was
        // committed locally but the push never happened (timeout fired
        // mid-push and the future was dropped).
        let law = repo_path.join("law.yaml");
        tokio::fs::write(&law, "articles: 1\n").await.unwrap();
        for args in [
            vec!["add", "--", "law.yaml"],
            vec!["commit", "-m", "harvest: interrupted before push"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&repo_path)
                .output()
                .await
                .unwrap();
        }

        // Sanity-check the precondition: the remote does NOT have the commit.
        let remote_log = Command::new("git")
            .args(["log", "--oneline", "--all"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&remote_log.stdout).contains("interrupted before push"),
            "precondition: stranded commit must not be on the remote yet"
        );

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);

        // The retry re-harvests identical content: the working tree already
        // matches the (stranded) local HEAD, only the metadata churns.
        let status = repo_path.join("status.yaml");
        tokio::fs::write(&law, "articles: 1\n").await.unwrap();
        tokio::fs::write(&status, "last_harvested: T2\n")
            .await
            .unwrap();
        let committed = client
            .commit_and_push_content(
                std::slice::from_ref(&law),
                std::slice::from_ref(&status),
                "harvest: retry",
            )
            .await
            .unwrap();
        assert!(
            committed,
            "retry must report a change when it pushes a stranded commit"
        );

        // The stranded commit is now on the remote.
        let remote_log = Command::new("git")
            .args(["log", "--oneline", "--all"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&remote_log.stdout);
        assert!(
            log_str.contains("interrupted before push"),
            "stranded local commit must have been pushed to the remote: {log_str}"
        );

        // And the working tree is clean (metadata churn restored), so the
        // next run's `pull --rebase` won't trip over a dirty file.
        let porcelain = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&porcelain.stdout).trim().is_empty(),
            "working tree must be clean after pushing the stranded commit"
        );
    }

    /// Verify that a worker whose local repo is behind the remote can
    /// still push successfully via the pull-rebase-push loop.  This
    /// exercises the same rebase path that resolves real concurrent
    /// push race conditions ("remote rejected: cannot lock ref").
    #[tokio::test]
    async fn test_commit_and_push_rebases_over_concurrent_changes() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Clone two working copies simulating two concurrent workers
        let repo_a = dir.path().join("worker-a");
        let repo_b = dir.path().join("worker-b");
        clone_with_config(&bare_path, &repo_a).await;
        clone_with_config(&bare_path, &repo_b).await;

        // Worker B pushes a commit first (simulating a concurrent push)
        let file_b = repo_b.join("from-b.txt");
        tokio::fs::write(&file_b, "from worker B").await.unwrap();
        let config_b = CorpusConfig::new(&bare_url, &repo_b);
        let client_b = CorpusClient::new(config_b);
        client_b
            .commit_and_push(&[file_b], "worker B commit")
            .await
            .unwrap();

        // Worker A now commits — its local repo is behind by one commit.
        // The pull --rebase inside commit_and_push must incorporate B's
        // changes before pushing.
        let file_a = repo_a.join("from-a.txt");
        tokio::fs::write(&file_a, "from worker A").await.unwrap();
        let config_a = CorpusConfig::new(&bare_url, &repo_a);
        let client_a = CorpusClient::new(config_a);
        client_a
            .commit_and_push(&[file_a], "worker A commit")
            .await
            .unwrap();

        // Verify both commits are on the remote
        let log = Command::new("git")
            .args(["log", "--oneline"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(
            log_str.contains("worker A commit"),
            "worker A commit not found in log: {log_str}"
        );
        assert!(
            log_str.contains("worker B commit"),
            "worker B commit not found in log: {log_str}"
        );
    }

    #[tokio::test]
    async fn test_remote_branch_exists() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);

        // `setup_bare_repo` creates the remote with `development` branch.
        assert!(client.remote_branch_exists("development").await.unwrap());
        // A branch that doesn't exist yet on the remote.
        assert!(!client.remote_branch_exists("pr574").await.unwrap());
    }

    #[tokio::test]
    async fn test_commit_local_without_push() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Create a local branch
        let config = CorpusConfig::new(&bare_url, &repo_path);
        let client = CorpusClient::new(config);
        client
            .create_local_branch("editor/test-session")
            .await
            .unwrap();

        // Write and commit locally
        let test_file = repo_path.join("local-edit.txt");
        tokio::fs::write(&test_file, "local change").await.unwrap();
        client
            .commit_local(&[test_file], "local edit")
            .await
            .unwrap();

        // Verify commit exists locally
        let log = Command::new("git")
            .args(["log", "--oneline", "-1"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let log_str = String::from_utf8_lossy(&log.stdout);
        assert!(log_str.contains("local edit"));

        // Verify it was NOT pushed to the bare repo
        let remote_log = Command::new("git")
            .args(["log", "--oneline", "--all"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let remote_str = String::from_utf8_lossy(&remote_log.stdout);
        assert!(
            !remote_str.contains("local edit"),
            "commit should not be on remote: {remote_str}"
        );

        // Verify we're on the session branch
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let branch_str = String::from_utf8_lossy(&branch.stdout);
        assert_eq!(branch_str.trim(), "editor/test-session");
    }

    /// Create a bare repo with files in multiple directories for sparse checkout testing.
    async fn setup_bare_repo_with_files(dir: &Path) -> PathBuf {
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

        let tmp_clone = dir.join("tmp-clone");
        let bare_url = format!("file://{}", bare_path.display());
        Command::new("git")
            .args(["-c", "init.templateDir=", "clone", &bare_url])
            .arg(&tmp_clone)
            .output()
            .await
            .unwrap();

        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }

        // Create files in multiple directories
        let law_a = tmp_clone.join("regulation/nl/wet/law_a");
        let law_b = tmp_clone.join("regulation/nl/wet/law_b");
        let features = tmp_clone.join("features");
        tokio::fs::create_dir_all(&law_a).await.unwrap();
        tokio::fs::create_dir_all(&law_b).await.unwrap();
        tokio::fs::create_dir_all(&features).await.unwrap();

        tokio::fs::write(law_a.join("2025-01-01.yaml"), "law_a content")
            .await
            .unwrap();
        tokio::fs::write(law_b.join("2025-01-01.yaml"), "law_b content")
            .await
            .unwrap();
        tokio::fs::write(features.join("law_a.feature"), "feature content")
            .await
            .unwrap();

        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "add test files"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }

        bare_path
    }

    #[tokio::test]
    async fn test_sparse_checkout_only_materializes_requested_paths() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo_with_files(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("sparse-corpus");

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.sparse_paths = Some(vec![
            "regulation/nl/wet/law_a".to_string(),
            "features".to_string(),
        ]);

        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // law_a should be present
        assert!(
            repo_path
                .join("regulation/nl/wet/law_a/2025-01-01.yaml")
                .exists()
        );
        // features should be present
        assert!(repo_path.join("features/law_a.feature").exists());
        // law_b should NOT be present (excluded by sparse checkout)
        assert!(
            !repo_path
                .join("regulation/nl/wet/law_b/2025-01-01.yaml")
                .exists()
        );
    }

    /// A bare repo shaped like the real corpus: the directory is named after
    /// the law's title, and the BWB number appears only inside the file.
    async fn setup_corpus_named_by_title(dir: &Path) -> PathBuf {
        let bare_path = dir.join("titled.git");
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

        let tmp_clone = dir.join("titled-clone");
        let bare_url = format!("file://{}", bare_path.display());
        Command::new("git")
            .args(["-c", "init.templateDir=", "clone", &bare_url])
            .arg(&tmp_clone)
            .output()
            .await
            .unwrap();
        for args in [
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@test.nl"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }

        for (slug, bwb) in [
            ("wet_op_de_testtoeslag", "BWBR0000111"),
            ("wet_op_het_andere", "BWBR0000222"),
        ] {
            let law_dir = tmp_clone.join("regulation/nl/wet").join(slug);
            tokio::fs::create_dir_all(&law_dir).await.unwrap();
            tokio::fs::write(
                law_dir.join("2025-01-01.yaml"),
                format!("$id: {slug}\nbwb_id: {bwb}\n"),
            )
            .await
            .unwrap();
        }

        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "corpus named by title"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp_clone)
                .output()
                .await
                .unwrap();
        }
        bare_path
    }

    /// Widening finds a law whose number appears nowhere in its path.
    ///
    /// This used to match the BWB number against `git ls-tree` output, which
    /// is a list of paths. The harvester names a directory after the law's
    /// title, so on the real corpus zero of the paths carry a BWBR number
    /// while 32 of the files declare one. The widening therefore returned an
    /// empty list on every job, and the cited law stayed outside the checkout
    /// exactly as before the feature existed.
    ///
    /// The fixture reproduces that shape: directories named `wet_op_de_*`,
    /// numbers only inside the files.
    #[tokio::test]
    async fn widening_finds_a_law_whose_number_is_only_inside_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_corpus_named_by_title(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("sparse-corpus");

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.sparse_paths = Some(vec!["regulation/nl/wet/wet_op_de_testtoeslag".to_string()]);
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        let cited = repo_path.join("regulation/nl/wet/wet_op_het_andere/2025-01-01.yaml");
        assert!(
            !cited.exists(),
            "the cited law starts outside the checkout; that is the premise"
        );

        let added = client
            .widen_to_laws(&["BWBR0000222".to_string()])
            .await
            .unwrap();

        assert_eq!(
            added,
            vec!["regulation/nl/wet/wet_op_het_andere".to_string()],
            "the directory carrying that number must be added"
        );
        assert!(
            cited.exists(),
            "and the law must now be on disk for the brief to read"
        );
    }

    /// A number this corpus does not carry widens nothing and is not an error.
    ///
    /// `git grep` exits 1 when it matches nothing, which is an answer rather
    /// than a failure: a citation to a law outside this corpus is the ordinary
    /// case, and the brief reports it honestly on its own.
    #[tokio::test]
    async fn widening_on_a_law_outside_the_corpus_adds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_corpus_named_by_title(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("sparse-corpus");

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.sparse_paths = Some(vec!["regulation/nl/wet/wet_op_de_testtoeslag".to_string()]);
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        let added = client
            .widen_to_laws(&["BWBR9999999".to_string()])
            .await
            .unwrap();
        assert!(added.is_empty(), "nothing to add, and no error");
    }

    #[tokio::test]
    async fn checkout_path_from_fetch_head_materializes_new_law() {
        // An untracked base-branch path is fetched and materialized into the
        // working tree, unstaged, with the exact base bytes. (Migrated from the
        // old "incorporates new files" checkout-from-base test.)
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // ensure_repo with a non-existent branch creates it from development
        // (mirrors production: enrichment branches are born from development)
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Push a new file to development (simulating a harvested law)
        let base_content = "new law content";
        let path = "regulation/nl/wet/new_law/2025-01-01.yaml";
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let new_law = tmp.join("regulation/nl/wet/new_law");
        tokio::fs::create_dir_all(&new_law).await.unwrap();
        tokio::fs::write(new_law.join("2025-01-01.yaml"), base_content)
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // The new law is not tracked on the enrichment branch yet.
        assert!(!client.is_tracked(path).await.unwrap());
        assert!(!repo_path.join(path).exists());

        // Fetch the base blob (sets FETCH_HEAD) and materialize the path from it.
        client
            .fetch_base_blob_sha("development", path)
            .await
            .unwrap();
        client.checkout_path_from_fetch_head(path).await.unwrap();

        // File is present with the exact base bytes.
        assert!(repo_path.join(path).exists());
        let content = tokio::fs::read_to_string(repo_path.join(path))
            .await
            .unwrap();
        assert_eq!(git_hash_object(&content), git_hash_object(base_content));

        // Left unstaged so the file appears as `??` (untracked) rather than `A`
        // (staged-new). commit_and_push uses explicit `git add -- <paths>`
        // which picks up untracked files, so the machine_readable additions
        // and new file land in the same commit.
        let status = Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=all"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let status_str = String::from_utf8_lossy(&status.stdout);
        assert!(
            !status_str.contains("A "),
            "file should not be staged: {status_str}"
        );
        assert!(
            status_str.contains("?? regulation/nl/wet/new_law/2025-01-01.yaml"),
            "file should be untracked: {status_str}"
        );
    }

    #[tokio::test]
    async fn checkout_path_from_fetch_head_materializes_new_version_alongside_old() {
        // An older version of the law is already tracked on the enrichment
        // branch; materializing a newer base-branch version adds it without
        // disturbing the old one. (Migrated from the old "fetches new version
        // when old exists" checkout-from-base test.)
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Push an old version of a law to development
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let law_dir = tmp.join("regulation/nl/wet/some_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        tokio::fs::write(law_dir.join("2024-01-01.yaml"), "old version")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest old version"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Create enrichment branch (inherits old version from development)
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();
        let old_path = "regulation/nl/wet/some_law/2024-01-01.yaml";
        assert!(client.is_tracked(old_path).await.unwrap());
        assert!(repo_path.join(old_path).exists());

        // Push a NEW version to development
        tokio::fs::write(law_dir.join("2025-01-01.yaml"), "new version")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new version"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // New version is not on the enrichment branch yet.
        let new_path = "regulation/nl/wet/some_law/2025-01-01.yaml";
        assert!(!client.is_tracked(new_path).await.unwrap());
        assert!(!repo_path.join(new_path).exists());

        // Materialize only the new-version file from the base branch.
        client
            .fetch_base_blob_sha("development", new_path)
            .await
            .unwrap();
        client
            .checkout_path_from_fetch_head(new_path)
            .await
            .unwrap();

        // New version present, old version still intact.
        assert!(repo_path.join(new_path).exists());
        assert!(repo_path.join(old_path).exists());
    }

    #[tokio::test]
    async fn fetch_base_blob_sha_reachable_on_single_branch_clone() {
        // Regression: when the enrichment branch already exists on the remote,
        // ensure_repo clones it with `--single-branch --branch enrich/<provider>`.
        // The configured fetch refspec then only matches enrich/<provider>, so
        // `git fetch origin development` does NOT update refs/remotes/origin/development.
        // fetch_base_blob_sha must still be able to reach the fetched commit —
        // which it does via FETCH_HEAD. (Migrated from the old "works with
        // preexisting enrichment branch" checkout-from-base test.)
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Pre-create enrich/test on the bare, branching from development.
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        for args in [
            vec!["checkout", "-b", "enrich/test"],
            vec!["push", "-u", "origin", "enrich/test"],
            vec!["checkout", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Now push a new law to development (not yet on enrich/test).
        let base_content = "new law content";
        let path = "regulation/nl/wet/new_law/2025-01-01.yaml";
        let law_dir = tmp.join("regulation/nl/wet/new_law");
        tokio::fs::create_dir_all(&law_dir).await.unwrap();
        tokio::fs::write(law_dir.join("2025-01-01.yaml"), base_content)
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // Clone enrich/test — this goes through git_clone (not the fallback),
        // so the fetch refspec is +refs/heads/enrich/test:refs/remotes/origin/enrich/test only.
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Sanity-check: origin/development must NOT exist as a remote-tracking ref.
        let refs = Command::new("git")
            .args([
                "for-each-ref",
                "--format=%(refname)",
                "refs/remotes/origin/",
            ])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let refs_str = String::from_utf8_lossy(&refs.stdout);
        assert!(
            !refs_str.contains("refs/remotes/origin/development"),
            "expected no origin/development tracking ref before fetch, got: {refs_str}"
        );

        // This would fail with `invalid reference: origin/development` before the
        // FETCH_HEAD approach: fetch_base_blob_sha resolves the blob via FETCH_HEAD.
        let sha = client
            .fetch_base_blob_sha("development", path)
            .await
            .unwrap();
        assert_eq!(sha, git_hash_object(base_content));
    }

    #[tokio::test]
    async fn fetch_base_blob_sha_returns_stable_content_hash() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Enrichment clone on a branch that doesn't exist yet (born from development).
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Push a law file to development with known content.
        let base_content = "new law content";
        let path = "regulation/nl/wet/new_law/2025-01-01.yaml";
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let new_law = tmp.join("regulation/nl/wet/new_law");
        tokio::fs::create_dir_all(&new_law).await.unwrap();
        tokio::fs::write(new_law.join("2025-01-01.yaml"), base_content)
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        let sha = client
            .fetch_base_blob_sha("development", path)
            .await
            .unwrap();

        // `git hash-object` of the same bytes must equal the reported blob SHA.
        let expected = git_hash_object(base_content);
        assert_eq!(sha, expected);
    }

    #[tokio::test]
    async fn is_tracked_reflects_branch_contents() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Enrichment clone born from development (without the new law).
        let repo_path = dir.path().join("enrich-clone");
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "enrich/test".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        // Push a new law to development after the enrich branch was created.
        let path = "regulation/nl/wet/new_law/2025-01-01.yaml";
        let tmp = dir.path().join("setup");
        clone_with_config(&bare_path, &tmp).await;
        let new_law = tmp.join("regulation/nl/wet/new_law");
        tokio::fs::create_dir_all(&new_law).await.unwrap();
        tokio::fs::write(new_law.join("2025-01-01.yaml"), "new law content")
            .await
            .unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "harvest new law"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&tmp)
                .output()
                .await
                .unwrap();
        }

        // The law exists on development but not on the enrich branch.
        assert!(!client.is_tracked(path).await.unwrap());

        client
            .fetch_base_blob_sha("development", path)
            .await
            .unwrap();
        client.checkout_path_from_fetch_head(path).await.unwrap();
        client.run_git(&["add", "--", path]).await.unwrap();
        assert!(client.is_tracked(path).await.unwrap());
    }

    #[tokio::test]
    async fn test_ensure_repo_creates_branch_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().join("corpus");
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());

        // Request a branch that doesn't exist — should clone development and create it
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "pr999".into();
        let mut client = CorpusClient::new(config);
        client.ensure_repo().await.unwrap();

        assert!(repo_path.join(".git").exists());

        // Verify local branch is pr999
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&repo_path)
            .output()
            .await
            .unwrap();
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(branch, "pr999");

        // Verify the branch was pushed to the bare remote
        let output = Command::new("git")
            .args(["branch"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let branches = String::from_utf8_lossy(&output.stdout);
        assert!(branches.contains("pr999"));
    }

    #[tokio::test]
    async fn commit_and_push_to_branch_creates_session_branch_with_trailer() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Configure the client against `development` (the base) — the test
        // covers the case where the session branch does NOT yet exist on
        // the remote.
        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        let edit = repo_path.join("article.md");
        tokio::fs::write(&edit, "edit 1").await.unwrap();

        client
            .commit_and_push_to_branch(
                "editor/session-test1",
                "development",
                std::slice::from_ref(&edit),
                "Update article.md",
                Some(("Anne Schuth", "anne@example.gov")),
            )
            .await
            .unwrap();

        // Session branch should exist on the bare remote
        let branches = Command::new("git")
            .args(["branch"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let out = String::from_utf8_lossy(&branches.stdout);
        assert!(
            out.contains("editor/session-test1"),
            "session branch missing from remote: {out}"
        );

        // The commit on that branch should carry the Co-authored-by trailer
        let log = Command::new("git")
            .args(["log", "editor/session-test1", "--format=%B", "-1"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let body = String::from_utf8_lossy(&log.stdout);
        assert!(
            body.contains("Co-authored-by: Anne Schuth <anne@example.gov>"),
            "missing trailer in commit body: {body}"
        );
    }

    #[tokio::test]
    async fn commit_and_push_to_branch_appends_to_existing_session_branch() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        // First commit on the session branch
        let f1 = repo_path.join("a.md");
        tokio::fs::write(&f1, "first").await.unwrap();
        client
            .commit_and_push_to_branch(
                "editor/session-multi",
                "development",
                &[f1],
                "first edit",
                None,
            )
            .await
            .unwrap();

        // Second commit on the SAME session branch
        let f2 = repo_path.join("b.md");
        tokio::fs::write(&f2, "second").await.unwrap();
        client
            .commit_and_push_to_branch(
                "editor/session-multi",
                "development",
                &[f2],
                "second edit",
                None,
            )
            .await
            .unwrap();

        // Both commits should be on the session branch on the remote
        let log = Command::new("git")
            .args(["log", "editor/session-multi", "--oneline"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let out = String::from_utf8_lossy(&log.stdout);
        assert!(out.contains("first edit"), "missing first commit: {out}");
        assert!(out.contains("second edit"), "missing second commit: {out}");
    }

    /// Regression test: when the session branch is force-deleted upstream
    /// while the working tree still carries dirty edits for files that
    /// also exist on `base_branch` with different content, the next
    /// `commit_and_push_to_branch` must NOT fail with "Your local changes
    /// would be overwritten". The snapshot-then-replay path preserves
    /// the user's edits across the rebase onto `origin/<base>`.
    #[tokio::test]
    async fn commit_and_push_to_branch_recovers_after_session_branch_deleted_upstream() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Seed development with a file that the editor will later edit.
        let seed = repo_path.join("article.md");
        tokio::fs::write(&seed, "original").await.unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "seed"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&repo_path)
                .output()
                .await
                .unwrap();
        }

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        // First save creates the session branch upstream.
        tokio::fs::write(&seed, "edit 1").await.unwrap();
        client
            .commit_and_push_to_branch(
                "editor/session-rolled",
                "development",
                std::slice::from_ref(&seed),
                "first edit",
                None,
            )
            .await
            .unwrap();

        // Simulate upstream force-deletion of the session branch.
        Command::new("git")
            .args(["branch", "-D", "editor/session-rolled"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();

        // User edits again — working tree now diverges from origin/development.
        tokio::fs::write(&seed, "edit 2").await.unwrap();

        // This previously failed with "Your local changes would be
        // overwritten" because the session branch was gone, `start_point`
        // flipped to origin/development, and the dirty working tree
        // conflicted with the base. The fix snapshots the dirty content
        // before resetting and replays it after.
        client
            .commit_and_push_to_branch(
                "editor/session-rolled",
                "development",
                std::slice::from_ref(&seed),
                "second edit",
                None,
            )
            .await
            .unwrap();

        // The recovered session branch on the remote contains the second
        // edit (no first edit, because it died with the force-deleted
        // branch).
        let log = Command::new("git")
            .args(["log", "editor/session-rolled", "--oneline"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let out = String::from_utf8_lossy(&log.stdout);
        assert!(out.contains("second edit"), "missing second edit: {out}");

        // And the file content on the recovered branch reflects the
        // user's most recent edit, not the base-branch version.
        let show = Command::new("git")
            .args(["show", "editor/session-rolled:article.md"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let content = String::from_utf8_lossy(&show.stdout);
        assert_eq!(content.trim(), "edit 2");
    }

    #[tokio::test]
    async fn commit_and_push_to_branch_no_changes_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        // Empty paths and clean tree → should not error and should not
        // create the branch on the remote.
        let pushed = client
            .commit_and_push_to_branch("editor/session-empty", "development", &[], "no-op", None)
            .await
            .unwrap();
        assert!(
            !pushed,
            "no-op call must report `pushed=false` so callers can skip PR-open"
        );

        let branches = Command::new("git")
            .args(["branch"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let out = String::from_utf8_lossy(&branches.stdout);
        assert!(
            !out.contains("editor/session-empty"),
            "no-op should not create remote branch: {out}"
        );
    }

    /// Regression for the iteration-3 finding: when `paths` is non-empty
    /// but every path's content already matches the base after
    /// snapshot+replay, no commit is created and no branch is pushed.
    /// The function must report `pushed=false` so the editor's
    /// `SessionGitBackend::persist` can skip `ensure_pr` (calling the
    /// GitHub PR-open API against a branch that was never pushed returns
    /// 422).
    #[tokio::test]
    async fn commit_and_push_to_branch_reports_no_push_when_content_matches_base() {
        let dir = tempfile::tempdir().unwrap();
        let bare_path = setup_bare_repo(dir.path()).await;
        let bare_url = format!("file://{}", bare_path.display());
        let repo_path = dir.path().join("corpus");
        clone_with_config(&bare_path, &repo_path).await;

        // Seed `development` with a file the user will later "save"
        // without actually changing.
        let seed = repo_path.join("article.md");
        tokio::fs::write(&seed, "base content").await.unwrap();
        for args in [
            vec!["add", "."],
            vec!["commit", "-m", "seed"],
            vec!["push", "origin", "development"],
        ] {
            Command::new("git")
                .args(&args)
                .current_dir(&repo_path)
                .output()
                .await
                .unwrap();
        }

        let mut config = CorpusConfig::new(&bare_url, &repo_path);
        config.branch = "development".to_string();
        let client = CorpusClient::new(config);

        // The user "edits" the file but the content is identical to
        // origin/development — the editor still passes the path through
        // `dirty_files` because `write_file` cannot cheaply distinguish.
        tokio::fs::write(&seed, "base content").await.unwrap();

        let pushed = client
            .commit_and_push_to_branch(
                "editor/session-unchanged",
                "development",
                std::slice::from_ref(&seed),
                "no-op save",
                None,
            )
            .await
            .unwrap();
        assert!(
            !pushed,
            "content-matches-base must report `pushed=false` so callers can skip PR-open"
        );

        let branches = Command::new("git")
            .args(["branch"])
            .current_dir(&bare_path)
            .output()
            .await
            .unwrap();
        let out = String::from_utf8_lossy(&branches.stdout);
        assert!(
            !out.contains("editor/session-unchanged"),
            "no-commit path must not push branch to remote: {out}"
        );
    }
}
