//! Git Refs API: check a branch exists and create one off another's tip.

use reqwest::header::{CONTENT_TYPE, HeaderValue};
use serde::Deserialize;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

#[derive(Debug, Deserialize)]
struct RefResponse {
    object: RefObject,
}
#[derive(Debug, Deserialize)]
struct RefObject {
    sha: String,
}

impl GithubClient {
    /// Check whether `branch` exists on `repo`. `Ok(true)` on 200, `Ok(false)`
    /// on 404, error otherwise.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "ref", repo = %repo))]
    pub async fn branch_exists(
        &self,
        repo: &str,
        branch: &str,
        token: Option<&str>,
    ) -> Result<bool> {
        let url = format!("{}/repos/{}/git/ref/heads/{}", self.api_base, repo, branch);
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
        tracing::debug!(status = %status, "gh ref GET response");
        if status.is_success() {
            return Ok(true);
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        let code = status.as_u16();
        let body = response.text().await.unwrap_or_default();
        Err(GithubError::Api {
            status: code,
            message: format!("Refs API for {repo}@{branch}: {body}"),
        })
    }

    /// Create `branch` pointing at the tip of `base_branch`. The base must
    /// exist; the target must not (GitHub 422s otherwise). A 403 on the
    /// ref-create (no push access) maps to [`GithubError::WriteDenied`].
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "POST", kind = "create_branch", repo = %repo))]
    pub async fn create_branch(
        &self,
        repo: &str,
        branch: &str,
        base_branch: &str,
        token: Option<&str>,
    ) -> Result<()> {
        // 1) resolve the base ref's sha
        let base_url = format!(
            "{}/repos/{}/git/ref/heads/{}",
            self.api_base, repo, base_branch
        );
        let headers = self.default_headers(token)?;
        let response = self
            .client
            .get(&base_url)
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
                message: format!("could not resolve base branch {repo}@{base_branch}: {body}"),
            });
        }
        let parsed: RefResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse base ref: {e}")))?;

        // 2) POST a new ref pointing at the same sha
        let post_url = format!("{}/repos/{}/git/refs", self.api_base, repo);
        let body = serde_json::json!({
            "ref": format!("refs/heads/{branch}"),
            "sha": parsed.object.sha,
        });
        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = self
            .client
            .post(&post_url)
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
                "Refs API POST (create branch {repo}@{branch}) returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Refs API POST for {repo}@{branch}: {body}"),
            });
        }
        Ok(())
    }
}
