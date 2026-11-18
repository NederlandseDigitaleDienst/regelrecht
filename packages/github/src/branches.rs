//! Branch-level calls: list branches, how far two branches have diverged, and
//! a server-side merge of one branch into another.
//!
//! These carry the traject "bijwerken" flow: a traject branch is cut from a
//! working branch that keeps moving (someone, or an agent, keeps committing to
//! it in a local clone). Bringing the traject up to date is a plain merge of
//! that base into the traject branch, done by GitHub through the Merges API —
//! no clone, no rebase, no force-push, and a conflict is a clean 409 that
//! leaves both branches untouched.

use reqwest::header::{CONTENT_TYPE, HeaderValue};
use serde::Deserialize;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

/// How `head` relates to `base` (Compare API, `{base}...{head}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareStatus {
    /// Commits on `head` that `base` does not have.
    pub ahead_by: u32,
    /// Commits on `base` that `head` does not have.
    pub behind_by: u32,
    /// Paths `head` changed since the merge base (first page, max 300).
    pub files: Vec<String>,
}

/// Result of a successful [`GithubClient::merge_branch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// GitHub created a merge commit on `base`.
    Merged { sha: String },
    /// `base` already contained `head`; nothing was written.
    UpToDate,
}

#[derive(Debug, Deserialize)]
struct BranchItem {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CompareBody {
    #[serde(default)]
    ahead_by: u32,
    #[serde(default)]
    behind_by: u32,
    #[serde(default)]
    files: Vec<CompareFile>,
}

#[derive(Debug, Deserialize)]
struct CompareFile {
    filename: String,
}

#[derive(Debug, Deserialize)]
struct MergeBody {
    sha: String,
}

/// Upper bound on branch pages read (100 per page). A dossier repo with more
/// than a thousand branches is not a case we design the picker for.
const MAX_BRANCH_PAGES: u32 = 10;

impl GithubClient {
    /// Names of all branches in `repo`, in GitHub's order (alphabetical).
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "list_branches", repo = %repo))]
    pub async fn list_branches(&self, repo: &str, token: Option<&str>) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for page in 1..=MAX_BRANCH_PAGES {
            let url = format!(
                "{}/repos/{}/branches?per_page=100&page={}",
                self.api_base, repo, page
            );
            let headers = self.default_headers(token)?;
            let response = self
                .client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
            self.track_rate_limit(&response);
            let status = response.status();
            if !status.is_success() {
                let code = status.as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(GithubError::Api {
                    status: code,
                    message: format!("Branches API on {repo}: {body}"),
                });
            }
            let items: Vec<BranchItem> = response
                .json()
                .await
                .map_err(|e| GithubError::Decode(format!("failed to parse branches: {e}")))?;
            let n = items.len();
            out.extend(items.into_iter().map(|b| b.name));
            if n < 100 {
                break;
            }
        }
        Ok(out)
    }

    /// Divergence of `head` relative to `base`. `None` when either ref does
    /// not exist (404).
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "compare_status", repo = %repo))]
    pub async fn compare_status(
        &self,
        repo: &str,
        base: &str,
        head: &str,
        token: Option<&str>,
    ) -> Result<Option<CompareStatus>> {
        let url = format!(
            "{}/repos/{}/compare/{}...{}",
            self.api_base, repo, base, head
        );
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Compare API {base}...{head} on {repo}: {body}"),
            });
        }
        let parsed: CompareBody = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse compare response: {e}")))?;
        Ok(Some(CompareStatus {
            ahead_by: parsed.ahead_by,
            behind_by: parsed.behind_by,
            files: parsed.files.into_iter().map(|f| f.filename).collect(),
        }))
    }

    /// Merge `head` into `base` on GitHub (Merges API).
    ///
    /// - 201 → [`MergeOutcome::Merged`] with the new merge commit's sha
    /// - 204 → [`MergeOutcome::UpToDate`]
    /// - 409 → [`GithubError::Conflict`]: the branches touch the same lines;
    ///   nothing was written and both branches are as they were
    /// - 403 (not a rate limit) → [`GithubError::WriteDenied`]
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "POST", kind = "merge_branch", repo = %repo))]
    pub async fn merge_branch(
        &self,
        repo: &str,
        base: &str,
        head: &str,
        commit_message: &str,
        token: Option<&str>,
    ) -> Result<MergeOutcome> {
        let url = format!("{}/repos/{}/merges", self.api_base, repo);
        let body = serde_json::json!({
            "base": base,
            "head": head,
            "commit_message": commit_message,
        });
        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = self
            .client
            .post(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);
        let status = response.status();
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(MergeOutcome::UpToDate);
        }
        if status == reqwest::StatusCode::CONFLICT {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Conflict(format!(
                "merge of {head} into {base} on {repo} has conflicts: {body}"
            )));
        }
        if status == reqwest::StatusCode::FORBIDDEN && !Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::WriteDenied(format!(
                "Merges API ({head} into {base} on {repo}) returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Merges API {head} into {base} on {repo}: {body}"),
            });
        }
        let parsed: MergeBody = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse merge response: {e}")))?;
        Ok(MergeOutcome::Merged { sha: parsed.sha })
    }
}

impl GithubClient {
    /// Move `branch` forward to the tip of `to_branch` without a merge commit
    /// (`PATCH git/refs`, `force: false`). GitHub refuses with 422 when that
    /// is not a fast-forward; that surfaces as [`GithubError::Api`].
    ///
    /// The Merges API always writes a merge commit, even when `branch` has no
    /// commits of its own. Used for exactly that case, so a traject that only
    /// follows its working branch does not collect merge commits that then
    /// read as "work not yet on the working branch".
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "PATCH", kind = "fast_forward", repo = %repo))]
    pub async fn fast_forward_branch(
        &self,
        repo: &str,
        branch: &str,
        to_branch: &str,
        token: Option<&str>,
    ) -> Result<String> {
        let url = format!(
            "{}/repos/{}/git/ref/heads/{}",
            self.api_base, repo, to_branch
        );
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("could not resolve {repo}@{to_branch}: {body}"),
            });
        }
        let parsed: RefBody = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse ref: {e}")))?;
        let sha = parsed.object.sha;

        let url = format!("{}/repos/{}/git/refs/heads/{}", self.api_base, repo, branch);
        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = serde_json::json!({ "sha": sha, "force": false });
        let response = self
            .client
            .patch(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);
        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN && !Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::WriteDenied(format!(
                "Refs API PATCH ({repo}@{branch}) returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("fast-forward of {repo}@{branch} to {to_branch}: {body}"),
            });
        }
        Ok(sha)
    }
}

#[derive(Debug, Deserialize)]
struct RefBody {
    object: RefObject,
}

#[derive(Debug, Deserialize)]
struct RefObject {
    sha: String,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(server: &MockServer) -> GithubClient {
        GithubClient::new().unwrap().with_base_url(server.uri())
    }

    #[tokio::test]
    async fn merge_returns_sha_on_201() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/org/repo/merges"))
            .and(header("Authorization", "Bearer t"))
            .and(body_partial_json(serde_json::json!({
                "base": "traject/x-1234abcd",
                "head": "werk",
            })))
            .respond_with(
                ResponseTemplate::new(201).set_body_json(serde_json::json!({ "sha": "abc123" })),
            )
            .expect(1)
            .mount(&server)
            .await;
        let out = client_for(&server)
            .merge_branch("org/repo", "traject/x-1234abcd", "werk", "msg", Some("t"))
            .await
            .unwrap();
        assert_eq!(
            out,
            MergeOutcome::Merged {
                sha: "abc123".into()
            }
        );
    }

    #[tokio::test]
    async fn merge_is_up_to_date_on_204() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/org/repo/merges"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let out = client_for(&server)
            .merge_branch("org/repo", "a", "b", "msg", None)
            .await
            .unwrap();
        assert_eq!(out, MergeOutcome::UpToDate);
    }

    #[tokio::test]
    async fn merge_conflict_maps_to_conflict() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/org/repo/merges"))
            .respond_with(
                ResponseTemplate::new(409)
                    .set_body_json(serde_json::json!({ "message": "Merge conflict" })),
            )
            .mount(&server)
            .await;
        let err = client_for(&server)
            .merge_branch("org/repo", "a", "b", "msg", None)
            .await
            .unwrap_err();
        assert!(matches!(err, GithubError::Conflict(_)), "{err:?}");
    }

    #[tokio::test]
    async fn merge_permission_denied_maps_to_write_denied() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/repos/org/repo/merges"))
            .respond_with(ResponseTemplate::new(403).set_body_string("no push"))
            .mount(&server)
            .await;
        let err = client_for(&server)
            .merge_branch("org/repo", "a", "b", "msg", None)
            .await
            .unwrap_err();
        assert!(matches!(err, GithubError::WriteDenied(_)), "{err:?}");
    }

    #[tokio::test]
    async fn fast_forward_patches_ref_to_target_sha() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/git/ref/heads/werk"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "object": { "sha": "f00d" } })),
            )
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/repos/org/repo/git/refs/heads/traject/x-1234abcd"))
            .and(body_partial_json(
                serde_json::json!({ "sha": "f00d", "force": false }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;
        let sha = client_for(&server)
            .fast_forward_branch("org/repo", "traject/x-1234abcd", "werk", None)
            .await
            .unwrap();
        assert_eq!(sha, "f00d");
    }

    #[tokio::test]
    async fn compare_status_reads_counts_and_files() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/compare/werk...traject/x"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ahead_by": 2,
                "behind_by": 5,
                "files": [{ "filename": "regulation/nl/wet/a/2025-01-01.yaml" }],
            })))
            .mount(&server)
            .await;
        let st = client_for(&server)
            .compare_status("org/repo", "werk", "traject/x", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(st.ahead_by, 2);
        assert_eq!(st.behind_by, 5);
        assert_eq!(st.files, vec!["regulation/nl/wet/a/2025-01-01.yaml"]);
    }

    #[tokio::test]
    async fn compare_status_missing_ref_is_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/compare/a...b"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert!(
            client_for(&server)
                .compare_status("org/repo", "a", "b", None)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn list_branches_follows_pages() {
        let server = MockServer::start().await;
        let first: Vec<_> = (0..100)
            .map(|i| serde_json::json!({ "name": format!("b{i:03}") }))
            .collect();
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/branches"))
            .and(query_param("page", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(first))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/org/repo/branches"))
            .and(query_param("page", "2"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{ "name": "main" }])),
            )
            .mount(&server)
            .await;
        let names = client_for(&server)
            .list_branches("org/repo", None)
            .await
            .unwrap();
        assert_eq!(names.len(), 101);
        assert_eq!(names.last().map(String::as_str), Some("main"));
    }
}
