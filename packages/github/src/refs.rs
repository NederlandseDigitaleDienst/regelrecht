//! Git Refs API: check a branch exists and create one off another's tip.

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
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

    /// The commit `git_ref` (a branch, a tag or a commit sha) points at, or
    /// `None` when `repo` has no such ref. One small call: GitHub answers
    /// with the bare sha, so it is a cheap way to tell whether anything
    /// keyed by the commit is still current.
    ///
    /// # Errors
    ///
    /// When the request fails, or GitHub answers anything but 200, 404 or
    /// 422 (its answer for a ref that does not exist).
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "commit_sha", repo = %repo))]
    pub async fn commit_sha(
        &self,
        repo: &str,
        git_ref: &str,
        token: Option<&str>,
    ) -> Result<Option<String>> {
        let url = format!("{}/repos/{}/commits/{}", self.api_base, repo, git_ref);
        let mut headers = self.default_headers(token)?;
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github.sha"),
        );
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
        {
            return Ok(None);
        }
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(GithubError::Api {
                status: status.as_u16(),
                message: format!("Commits API for {repo}@{git_ref}: {body}"),
            });
        }
        let sha = body.trim();
        if sha.is_empty() {
            return Err(GithubError::Decode(format!(
                "Commits API for {repo}@{git_ref} returned no sha"
            )));
        }
        Ok(Some(sha.to_string()))
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn answering(status: u16, body: &str) -> (MockServer, GithubClient) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/example-org/corpus/commits/traject/x"))
            .and(header("accept", "application/vnd.github.sha"))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .mount(&server)
            .await;
        let client = GithubClient::from_base(Some(&server.uri())).unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn commit_sha_returns_the_bare_sha() {
        let (_server, client) = answering(200, "f690844f5e5f2afc90dee4f2078ea49212985659\n").await;
        let sha = client
            .commit_sha("example-org/corpus", "traject/x", None)
            .await
            .unwrap();
        assert_eq!(
            sha.as_deref(),
            Some("f690844f5e5f2afc90dee4f2078ea49212985659")
        );
    }

    #[tokio::test]
    async fn commit_sha_of_a_missing_ref_is_none() {
        // GitHub answers 422 for a ref that does not exist, 404 for a repo
        // it does not show.
        for status in [404, 422] {
            let (_server, client) = answering(status, "{}").await;
            let sha = client
                .commit_sha("example-org/corpus", "traject/x", None)
                .await
                .unwrap();
            assert_eq!(sha, None, "status {status}");
        }
    }

    #[tokio::test]
    async fn commit_sha_reports_other_failures() {
        let (_server, client) = answering(500, "boom").await;
        let err = client
            .commit_sha("example-org/corpus", "traject/x", None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, GithubError::Api { status: 500, .. }),
            "{err:?}"
        );
    }
}
