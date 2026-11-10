//! Git Trees API: list every blob in a repo at a ref in one call, with ETag
//! caching so an unchanged tree comes back as a cheap 304.

use serde::Deserialize;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

/// One blob discovered in a repo tree: its repo-relative path plus the blob
/// sha the listing reported. The sha is the file's content identity — two
/// listings reporting the same sha are byte-identical.
#[derive(Debug, Clone)]
pub struct TreeEntryFile {
    pub path: String,
    /// GitHub always sends the blob sha; kept optional so a missing field
    /// degrades to "no content identity" rather than failing the whole parse.
    pub sha: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TreeResponse {
    /// Sha of the tree object this response describes.
    #[serde(default)]
    sha: Option<String>,
    tree: Vec<TreeEntry>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    sha: Option<String>,
}

impl GithubClient {
    /// List every **blob** (file) in `repo` at `git_ref` via the Trees API
    /// (`GET /repos/{repo}/git/trees/{ref}?recursive=1`), one call for the
    /// whole tree.
    ///
    /// Uses `If-None-Match` from the per-URL ETag cache: returns `Ok(None)`
    /// when the tree is unchanged (HTTP 304) so callers can preserve
    /// previously loaded data. Non-blob entries (trees, submodules, symlinks)
    /// are filtered out; the caller narrows further (e.g. by extension or
    /// sub-path). A truncated response (repo too large for one page) is an
    /// error rather than a silent partial list.
    pub async fn list_tree_files(
        &self,
        repo: &str,
        git_ref: &str,
        token: Option<&str>,
    ) -> Result<Option<Vec<TreeEntryFile>>> {
        let url = format!(
            "{}/repos/{}/git/trees/{}?recursive=1",
            self.api_base, repo, git_ref
        );

        let cache_key = Self::cache_key(&url, None, token);
        let mut headers = self.default_headers(token)?;
        if let Some(etag) = self.cached_etag(&cache_key)
            && let Ok(val) = reqwest::header::HeaderValue::from_str(&etag)
        {
            headers.insert(reqwest::header::IF_NONE_MATCH, val);
        }

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub Trees API request failed: {e}")))?;
        self.track_rate_limit(&response);

        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            tracing::debug!(repo = %repo, "Tree unchanged (ETag match)");
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("Trees API for {repo}@{git_ref}: {body}"),
            });
        }

        if let Some(etag) = response.headers().get("etag").and_then(|v| v.to_str().ok()) {
            self.store_etag(&cache_key, etag);
        }

        let tree: TreeResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse tree response: {e}")))?;

        if tree.truncated {
            return Err(GithubError::Api {
                status: 200,
                message: format!(
                    "Trees API response for '{repo}' was truncated — repository has too \
                     many files. Narrow the listing (a sub-path) or reduce the file count."
                ),
            });
        }

        let files = tree
            .tree
            .into_iter()
            .filter(|e| e.entry_type == "blob")
            .map(|e| TreeEntryFile {
                path: e.path,
                sha: e.sha,
            })
            .collect();
        Ok(Some(files))
    }

    /// Git tree sha of the directory `path` at `git_ref` — the API
    /// counterpart of `git rev-parse <ref>:<path>` on a checkout, and the
    /// value a precomputed corpus artefact is verified against.
    ///
    /// Walks the path one component at a time with **non-recursive** Trees
    /// calls (one call per component; `regulation` costs one), so it never
    /// pulls a whole corpus listing just to read a directory's identity.
    ///
    /// Returns `Ok(None)` when the ref or the path does not exist — the
    /// caller then knows the identity is *unknown*, which is a different
    /// thing from "matches" and must never be treated as one. Every other
    /// failure (403, 5xx, a truncated listing) is an `Err`: a directory
    /// whose sha could not be read is not a directory whose sha differs.
    pub async fn subtree_sha(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<String>> {
        let mut tree_ish = git_ref.to_string();
        let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();

        for component in &components {
            let Some(tree) = self.fetch_tree(repo, &tree_ish, token).await? else {
                return Ok(None);
            };
            let child = tree
                .tree
                .iter()
                .find(|e| e.entry_type == "tree" && e.path == *component)
                .and_then(|e| e.sha.clone());
            match child {
                Some(sha) => tree_ish = sha,
                None if tree.truncated => {
                    // The component may well exist beyond the truncation
                    // point — reporting "not found" would be a guess.
                    return Err(GithubError::Api {
                        status: 200,
                        message: format!(
                            "Trees API listing for '{repo}' was truncated while resolving \
                             '{path}'; the sha of that subtree cannot be established"
                        ),
                    });
                }
                None => return Ok(None),
            }
        }

        // Path fully walked: `tree_ish` is the target tree's sha, except
        // when `path` was empty — then it is still the ref and one more
        // call resolves the root tree.
        if components.is_empty() {
            return Ok(self
                .fetch_tree(repo, &tree_ish, token)
                .await?
                .and_then(|t| t.sha));
        }
        Ok(Some(tree_ish))
    }

    /// One non-recursive Trees call. `Ok(None)` on 404, which here covers
    /// both "no such ref" and "this credential cannot see this repo" — the
    /// two are indistinguishable in GitHub's answer, and for every caller
    /// they mean the same thing: no tree to read.
    pub(crate) async fn fetch_tree(
        &self,
        repo: &str,
        tree_ish: &str,
        token: Option<&str>,
    ) -> Result<Option<TreeResponse>> {
        let url = format!("{}/repos/{}/git/trees/{}", self.api_base, repo, tree_ish);
        let response = self
            .client
            .get(&url)
            .headers(self.default_headers(token)?)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub Trees API request failed: {e}")))?;
        self.track_rate_limit(&response);

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("Trees API for {repo}@{tree_ish}: {body}"),
            });
        }
        response
            .json::<TreeResponse>()
            .await
            .map(Some)
            .map_err(|e| GithubError::Decode(format!("failed to parse tree response: {e}")))
    }
}
