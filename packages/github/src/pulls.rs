//! Pulls API: the three calls the editor's write-back path needs — find an
//! open PR for a session branch, open a new PR, or refresh an existing PR's
//! title/body.

use serde::{Deserialize, Serialize};

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

/// Identifies a PR opened or updated during a write-back. The corpus crate
/// converts this into its feature-independent `backend::PrInfo` before
/// surfacing it to the editor-api.
#[derive(Debug, Clone)]
pub struct PrInfo {
    /// PR number on the source repo.
    pub number: u64,
    /// User-facing HTML URL of the PR.
    pub html_url: String,
}

#[derive(Debug, Serialize)]
struct CreatePrBody<'a> {
    title: &'a str,
    body: &'a str,
    head: &'a str,
    base: &'a str,
}

#[derive(Debug, Serialize)]
struct UpdatePrBody<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(Debug, Deserialize)]
struct PrResponse {
    number: u64,
    html_url: String,
}

impl GithubClient {
    /// Idempotently ensure there is an open PR for `head` against `base`.
    ///
    /// If an open PR for `head` already exists, refresh its title/body (so
    /// later saves in the same session update the description) and return it;
    /// otherwise open a new PR.
    ///
    /// `head` is the branch name on the source repo (no `owner:` prefix — this
    /// always pushes to and PRs from the same repo). The list call builds the
    /// `head=owner:branch` query internally per the GitHub API spec.
    #[allow(clippy::too_many_arguments)]
    pub async fn ensure_pr(
        &self,
        owner: &str,
        repo: &str,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
        token: &str,
    ) -> Result<PrInfo> {
        if let Some(existing) = self.find_open_pr(owner, repo, head, token).await? {
            self.update_pr(owner, repo, existing.number, title, body, token)
                .await
        } else {
            self.create_pr(owner, repo, head, base, title, body, token)
                .await
        }
    }

    async fn find_open_pr(
        &self,
        owner: &str,
        repo: &str,
        head: &str,
        token: &str,
    ) -> Result<Option<PrResponse>> {
        let url = format!(
            "{}/repos/{}/{}/pulls?state=open&head={}:{}",
            self.api_base, owner, repo, owner, head
        );
        let response = self
            .client
            .get(&url)
            .headers(self.default_headers(Some(token))?)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("PR list request failed: {e}")))?;
        self.track_rate_limit(&response);

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("PR list for {owner}/{repo}: {body}"),
            });
        }

        let prs: Vec<PrResponse> = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse PR list: {e}")))?;
        // GitHub blocks duplicate open PRs for a head, but if one slips
        // through we take the first for determinism.
        Ok(prs.into_iter().next())
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_pr(
        &self,
        owner: &str,
        repo: &str,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
        token: &str,
    ) -> Result<PrInfo> {
        let url = format!("{}/repos/{}/{}/pulls", self.api_base, owner, repo);
        let payload = CreatePrBody {
            title,
            body,
            head,
            base,
        };
        let response = self
            .client
            .post(&url)
            .headers(self.default_headers(Some(token))?)
            .json(&payload)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("PR create request failed: {e}")))?;
        self.track_rate_limit(&response);

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("PR create for {owner}/{repo}: {body}"),
            });
        }

        let pr: PrResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse PR create response: {e}")))?;
        Ok(PrInfo {
            number: pr.number,
            html_url: pr.html_url,
        })
    }

    async fn update_pr(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        title: &str,
        body: &str,
        token: &str,
    ) -> Result<PrInfo> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.api_base, owner, repo, number
        );
        let payload = UpdatePrBody { title, body };
        let response = self
            .client
            .patch(&url)
            .headers(self.default_headers(Some(token))?)
            .json(&payload)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("PR update request failed: {e}")))?;
        self.track_rate_limit(&response);

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status,
                message: format!("PR update for {owner}/{repo}: {body}"),
            });
        }

        let pr: PrResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse PR update response: {e}")))?;
        Ok(PrInfo {
            number: pr.number,
            html_url: pr.html_url,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(server: &MockServer) -> GithubClient {
        GithubClient::from_base(Some(&server.uri())).unwrap()
    }

    #[tokio::test]
    async fn ensure_pr_creates_when_none_exists() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/org/repo/pulls"))
            .and(query_param("state", "open"))
            .and(query_param("head", "org:editor/session-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/repos/org/repo/pulls"))
            .and(header("Authorization", "Bearer test-token"))
            .and(body_partial_json(serde_json::json!({
                "head": "editor/session-abc",
                "base": "main",
                "title": "Editor session abc",
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "number": 42,
                "html_url": "https://github.com/org/repo/pull/42",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let pr = client_for(&server)
            .ensure_pr(
                "org",
                "repo",
                "editor/session-abc",
                "main",
                "Editor session abc",
                "Submitted via Regelrecht editor",
                "test-token",
            )
            .await
            .unwrap();

        assert_eq!(pr.number, 42);
        assert_eq!(pr.html_url, "https://github.com/org/repo/pull/42");
    }

    #[tokio::test]
    async fn ensure_pr_updates_existing_open_pr() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/org/repo/pulls"))
            .and(query_param("head", "org:editor/session-xyz"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "number": 7,
                    "html_url": "https://github.com/org/repo/pull/7",
                }])),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("PATCH"))
            .and(path("/repos/org/repo/pulls/7"))
            .and(body_partial_json(serde_json::json!({
                "title": "Editor session xyz (3 edits)",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "number": 7,
                "html_url": "https://github.com/org/repo/pull/7",
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Reject any POST (create) — we must NOT open a new PR.
        Mock::given(method("POST"))
            .and(path("/repos/org/repo/pulls"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;

        let pr = client_for(&server)
            .ensure_pr(
                "org",
                "repo",
                "editor/session-xyz",
                "main",
                "Editor session xyz (3 edits)",
                "updated body",
                "test-token",
            )
            .await
            .unwrap();

        assert_eq!(pr.number, 7);
    }

    #[tokio::test]
    async fn ensure_pr_propagates_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/org/repo/pulls"))
            .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
            .mount(&server)
            .await;

        let err = client_for(&server)
            .ensure_pr(
                "org",
                "repo",
                "editor/session-foo",
                "main",
                "title",
                "body",
                "no-perms-token",
            )
            .await
            .expect_err("403 must error");

        let msg = err.to_string();
        assert!(msg.contains("403"), "error should include status: {msg}");
    }
}
