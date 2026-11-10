//! Repository Activity API: the audit trail GitHub keeps of ref changes
//! (pushes, force pushes, branch creations and deletions) on a repository.
//!
//! One question is asked of it, and only on an error path: *was this branch
//! ever deleted?* That is the single in-band signal separating "the branch
//! was never created" from "the branch existed and is gone" — two states
//! that look identical to the Refs API (both are a 404) but call for very
//! different answers: initialise it, versus warn that work may be lost.
//!
//! Read access to the repository is enough for this endpoint, so the same
//! token that reads the corpus can ask the question.

use serde::Deserialize;

use crate::client::GithubClient;
use crate::error::{GithubError, Result};

/// The two fields of an activity entry we care about. GitHub also returns
/// the actor, the before/after shas and a timestamp — deliberately not
/// deserialised: the actor is a person, and this crate's callers log the
/// classification, not who caused it.
#[derive(Debug, Deserialize)]
struct ActivityEntry {
    #[serde(default)]
    activity_type: String,
    /// The full ref the entry is about (`refs/heads/…`). Absent on a host
    /// that does not send it, which then matches nothing — the safe
    /// direction: a missed deletion degrades to "never created", whereas a
    /// wrongly claimed one warns about work loss that never happened.
    #[serde(rename = "ref", default)]
    git_ref: String,
}

/// The `activity_type` value that marks a branch deletion.
const BRANCH_DELETION: &str = "branch_deletion";

impl GithubClient {
    /// Whether the repository's activity log records a deletion of `branch`.
    ///
    /// `repo` is the full `owner/name`. `Ok(true)` means the branch existed
    /// at some point and was deleted; `Ok(false)` means the log holds no such
    /// event (a branch that never existed, or a deletion older than the
    /// window GitHub retains). Any non-success response is an `Err` so the
    /// caller can decide how to degrade — this is a *refinement* of a
    /// classification, never the classification itself.
    #[tracing::instrument(
        name = "gh_http",
        skip_all,
        fields(method = "GET", kind = "activity", repo = %repo)
    )]
    pub async fn branch_deletion_recorded(
        &self,
        repo: &str,
        branch: &str,
        token: Option<&str>,
    ) -> Result<bool> {
        let wanted_ref = format!("refs/heads/{branch}");
        // The branch is percent-encoded, so the `/` in a `traject/<slug>`
        // name reaches GitHub as part of the ref rather than splitting the
        // query. One entry is enough: the question is "any deletion at all".
        let url = format!(
            "{}/repos/{}/activity?ref={}&activity_type={}&per_page=1",
            self.api_base,
            repo,
            crate::repo_access::percent_encode_path_segment(&wanted_ref),
            BRANCH_DELETION,
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
        tracing::debug!(status = %status, "gh activity GET response");
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Activity API for {repo}@{branch}: {body}"),
            });
        }

        let entries: Vec<ActivityEntry> = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("parse activity response: {e}")))?;
        // Filter rather than trust the server-side `activity_type` and `ref`
        // filters: a host that ignores either parameter would otherwise turn
        // "this branch was pushed to" — or "some other branch was deleted" —
        // into "this branch was deleted".
        Ok(entries
            .iter()
            .any(|e| e.activity_type == BRANCH_DELETION && e.git_ref == wanted_ref))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(server: &MockServer) -> GithubClient {
        GithubClient::from_base(Some(&server.uri())).unwrap()
    }

    #[tokio::test]
    async fn deletion_in_the_log_is_reported() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/activity"))
            // The slash in the branch name has to survive as part of the
            // `ref` value, not split the query.
            .and(query_param("ref", "refs/heads/traject/tarief-1a2b"))
            .and(query_param("activity_type", "branch_deletion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "activity_type": "branch_deletion", "ref": "refs/heads/traject/tarief-1a2b" }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        assert!(
            client_for(&server)
                .branch_deletion_recorded("acme/foo", "traject/tarief-1a2b", Some("t"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn empty_log_means_never_deleted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/activity"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        assert!(
            !client_for(&server)
                .branch_deletion_recorded("acme/foo", "traject/tarief-1a2b", Some("t"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn other_activity_is_not_a_deletion() {
        // A host that ignores the `activity_type` filter must not turn a
        // push into a deletion.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/activity"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "activity_type": "force_push" }
            ])))
            .mount(&server)
            .await;

        assert!(
            !client_for(&server)
                .branch_deletion_recorded("acme/foo", "main", Some("t"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn a_deletion_of_another_branch_is_not_ours() {
        // Same defence as the `activity_type` filter, and this one matters
        // more: a host that ignores `ref` would answer with whatever branch
        // was deleted last, and the caller would tell a member their work
        // may be lost over someone else's branch.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/activity"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "activity_type": "branch_deletion", "ref": "refs/heads/een-andere-branch" }
            ])))
            .mount(&server)
            .await;

        assert!(
            !client_for(&server)
                .branch_deletion_recorded("acme/foo", "traject/tarief-1a2b", Some("t"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn refused_log_is_an_error_not_a_verdict() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/activity"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let err = client_for(&server)
            .branch_deletion_recorded("acme/foo", "main", Some("t"))
            .await
            .expect_err("a refused activity log must not read as 'never deleted'");
        assert!(matches!(err, GithubError::Api { status: 403, .. }));
    }
}
