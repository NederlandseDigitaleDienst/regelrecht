//! Pre-flight checks for a GitHub repository before a traject commits to
//! using it as a write target.
//!
//! Two questions to answer:
//! 1. Does the supplied token actually have push access to this repo?
//! 2. Does the configured `base_branch` exist? (Branched off on first persist.)
//!
//! Distinct from the generic [`GithubError`](crate::GithubError) surface: the
//! editor maps each [`RepoAccessError`] variant onto a specific HTTP status and
//! Dutch operator message, so the pre-flight keeps its own disjoint enum.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;

use crate::client::GithubClient;

/// Per-request cap for the two pre-flight calls, overriding the shared client's
/// 60s default.
///
/// This is the one GitHub path a user waits on synchronously (creating a
/// traject blocks on it), so it keeps the tighter budget its dedicated client
/// had before the consolidation: two sequential calls stay bounded at ~60s
/// worst case rather than ~120s. Every other caller of the shared client runs
/// in a background job where the longer default is the better trade.
const PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(30);

/// Distinct failure modes for repo-access validation. The editor-api caller
/// maps each variant onto a specific HTTP status / user-facing message — keep
/// them disjoint so the matching is exhaustive.
#[derive(Debug)]
pub enum RepoAccessError {
    /// 401 on either call. Token rejected by GitHub (revoked, expired, wrong).
    Unauthorized,
    /// 404 on `GET /repos/{owner}/{repo}`. Repo really absent, or private and
    /// invisible to the token — GitHub returns 404 for both to avoid leaking.
    RepoNotFound,
    /// 404 on `GET /repos/{owner}/{repo}/branches/{base}`. Repo reachable but
    /// the named base branch is wrong.
    BranchNotFound,
    /// Repo reads fine, but the token's identity has `permissions.push == false`.
    NoPushAccess,
    /// Transport-level failure (DNS, TLS, timeout, connection reset).
    Transport(String),
    /// Anything else we can't classify (5xx, surprise 4xx, JSON parse, or a
    /// malformed token). Carries a short blurb for diagnostics — never a token.
    Other(String),
}

impl std::fmt::Display for RepoAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "token is not authorised for this repo"),
            Self::RepoNotFound => write!(f, "repository not found or token cannot see it"),
            Self::BranchNotFound => write!(f, "base branch does not exist on the repo"),
            Self::NoPushAccess => write!(f, "token has no push access to the repo"),
            Self::Transport(msg) => write!(f, "transport error talking to GitHub: {msg}"),
            Self::Other(msg) => write!(f, "unexpected GitHub response: {msg}"),
        }
    }
}

impl std::error::Error for RepoAccessError {}

/// Information gleaned from a successful validation. The caller might surface
/// `default_branch` (a hint when the operator typed the wrong base) or use
/// `is_private` for frontend hints.
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub default_branch: String,
    pub is_private: bool,
}

/// Minimal subset of the `/repos/{owner}/{repo}` response we care about.
#[derive(Debug, Deserialize)]
struct RepoResponse {
    default_branch: String,
    #[serde(default)]
    private: bool,
    permissions: Option<RepoPermissions>,
}

#[derive(Debug, Deserialize)]
struct RepoPermissions {
    #[serde(default)]
    push: bool,
}

impl GithubClient {
    /// Pre-flight check before letting a traject use this repo as its write
    /// target. Two API calls in sequence: repo lookup (also returns the
    /// permission bits) then branch existence. The client's configured base
    /// URL and shared HTTP client are used, so a test points the whole client
    /// at a wiremock server.
    pub async fn validate_repo_access(
        &self,
        owner: &str,
        repo: &str,
        base_branch: &str,
        token: &str,
    ) -> Result<RepoInfo, RepoAccessError> {
        let info = self.check_repo(owner, repo, token).await?;
        self.check_branch(owner, repo, base_branch, token).await?;
        Ok(info)
    }

    async fn check_repo(
        &self,
        owner: &str,
        repo: &str,
        token: &str,
    ) -> Result<RepoInfo, RepoAccessError> {
        let url = format!("{}/repos/{}/{}", self.api_base, owner, repo);
        let headers = self
            .default_headers(Some(token))
            .map_err(|e| RepoAccessError::Other(e.to_string()))?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .timeout(PREFLIGHT_TIMEOUT)
            .send()
            .await
            .map_err(|e| RepoAccessError::Transport(e.to_string()))?;

        match response.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED => return Err(RepoAccessError::Unauthorized),
            StatusCode::NOT_FOUND => return Err(RepoAccessError::RepoNotFound),
            // 403 from the repo lookup: token sees the repo exists but can't
            // read it (rare; usually masks as 404). Treat as Unauthorized —
            // the operator fixes the token either way.
            StatusCode::FORBIDDEN => return Err(RepoAccessError::Unauthorized),
            other => {
                let body = response.text().await.unwrap_or_default();
                return Err(RepoAccessError::Other(format!(
                    "{}: {}",
                    other,
                    truncate(&body, 200)
                )));
            }
        }

        let parsed: RepoResponse = response
            .json()
            .await
            .map_err(|e| RepoAccessError::Other(format!("parse repo response: {e}")))?;

        // No `permissions` block on the response usually means an
        // unauthenticated request — we always send a token, so missing
        // permissions here is a real "no access" signal.
        let pushable = parsed.permissions.as_ref().map(|p| p.push).unwrap_or(false);
        if !pushable {
            return Err(RepoAccessError::NoPushAccess);
        }

        Ok(RepoInfo {
            default_branch: parsed.default_branch,
            is_private: parsed.private,
        })
    }

    async fn check_branch(
        &self,
        owner: &str,
        repo: &str,
        base_branch: &str,
        token: &str,
    ) -> Result<(), RepoAccessError> {
        // Branch names can contain `/` (e.g. `feature/foo`). Spliced raw, the
        // slash collapses into the URL router and GitHub 404s. Percent-encode
        // the segment so the branch reaches the API as-typed.
        let url = format!(
            "{}/repos/{}/{}/branches/{}",
            self.api_base,
            owner,
            repo,
            percent_encode_path_segment(base_branch)
        );
        let headers = self
            .default_headers(Some(token))
            .map_err(|e| RepoAccessError::Other(e.to_string()))?;
        let response = self
            .client
            .get(&url)
            .headers(headers)
            .timeout(PREFLIGHT_TIMEOUT)
            .send()
            .await
            .map_err(|e| RepoAccessError::Transport(e.to_string()))?;

        match response.status() {
            StatusCode::OK => Ok(()),
            StatusCode::NOT_FOUND => Err(RepoAccessError::BranchNotFound),
            StatusCode::UNAUTHORIZED => Err(RepoAccessError::Unauthorized),
            // Per-branch 403 = repo resolved (passed the check above) but the
            // fine-grained PAT lacks `contents:read`. Surface as no-push since
            // the operator's fix is the same: widen the token.
            StatusCode::FORBIDDEN => Err(RepoAccessError::NoPushAccess),
            other => {
                let body = response.text().await.unwrap_or_default();
                Err(RepoAccessError::Other(format!(
                    "{}: {}",
                    other,
                    truncate(&body, 200)
                )))
            }
        }
    }
}

/// UTF-8-safe slice up to `max` *bytes*, walking back to the nearest char
/// boundary so a non-ASCII body doesn't panic the formatter on a multi-byte
/// split.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Percent-encode a single URL path segment per RFC 3986 §2.3: unreserved
/// chars (alphanumeric, `-._~`) pass through, everything else becomes `%XX`.
///
/// Inline rather than a crate dep: the call is tiny, the rule is fixed by the
/// RFC, and this crate carries no `percent_encoding`/`url` dependency. Also
/// safe for a query-string *value* — encoding more than strictly required
/// there is always allowed — which is what `activity` uses it for (this crate
/// builds reqwest without the feature behind `RequestBuilder::query`).
pub(crate) fn percent_encode_path_segment(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(server: &MockServer) -> GithubClient {
        GithubClient::from_base(Some(&server.uri())).unwrap()
    }

    fn ok_repo_body(default_branch: &str, push: bool, private: bool) -> serde_json::Value {
        serde_json::json!({
            "default_branch": default_branch,
            "private": private,
            "permissions": { "push": push, "pull": true, "admin": false }
        })
    }

    #[tokio::test]
    async fn happy_path_returns_info() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .and(header("Authorization", "Bearer t"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_repo_body("main", true, true)),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/branches/main"))
            .and(header("Authorization", "Bearer t"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"name":"main"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let info = client_for(&server)
            .validate_repo_access("acme", "foo", "main", "t")
            .await
            .unwrap();
        assert_eq!(info.default_branch, "main");
        assert!(info.is_private);
    }

    #[tokio::test]
    async fn slashed_branch_is_percent_encoded() {
        // `feature/foo` would collapse into the URL path if spliced raw:
        // GitHub's router would match `branch = "feature"` with a trailing
        // segment and return 404. It must go out as `branches/feature%2Ffoo`.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_repo_body("main", true, true)),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/branches/feature%2Ffoo"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"name":"feature/foo"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        // Negative-path catch-all: a regression dropping the encoding would
        // hit this route instead, and the positive mock's `.expect(1)` fails.
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/branches/feature"))
            .respond_with(ResponseTemplate::new(404))
            .expect(0)
            .mount(&server)
            .await;

        client_for(&server)
            .validate_repo_access("acme", "foo", "feature/foo", "t")
            .await
            .expect("slashed branch should resolve");
    }

    #[test]
    fn percent_encode_path_segment_examples() {
        assert_eq!(percent_encode_path_segment("main"), "main");
        assert_eq!(percent_encode_path_segment("feature/foo"), "feature%2Ffoo");
        assert_eq!(
            percent_encode_path_segment("release-1.0_rc~final"),
            "release-1.0_rc~final"
        );
        assert_eq!(percent_encode_path_segment("a b"), "a%20b");
        assert_eq!(percent_encode_path_segment("a#b"), "a%23b");
        assert_eq!(percent_encode_path_segment("a?b"), "a%3Fb");
        // Multi-byte UTF-8: encoded as raw bytes per RFC 3986. `é` = 0xC3 0xA9.
        assert_eq!(percent_encode_path_segment("é"), "%C3%A9");
    }

    #[tokio::test]
    async fn missing_repo_is_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = client_for(&server)
            .validate_repo_access("acme", "foo", "main", "t")
            .await
            .expect_err("404 should error");
        assert!(matches!(err, RepoAccessError::RepoNotFound));
    }

    #[tokio::test]
    async fn bad_token_is_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let err = client_for(&server)
            .validate_repo_access("acme", "foo", "main", "t")
            .await
            .expect_err("401 should error");
        assert!(matches!(err, RepoAccessError::Unauthorized));
    }

    #[tokio::test]
    async fn read_only_token_has_no_push_access() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_repo_body("main", false, true)),
            )
            .expect(1)
            .mount(&server)
            .await;
        // Branch call must NOT be made — fail-fast on no-push.
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/branches/main"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let err = client_for(&server)
            .validate_repo_access("acme", "foo", "main", "t")
            .await
            .expect_err("no push must error");
        assert!(matches!(err, RepoAccessError::NoPushAccess));
    }

    #[tokio::test]
    async fn missing_branch_is_branch_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(ok_repo_body("main", true, false)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo/branches/wibble"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = client_for(&server)
            .validate_repo_access("acme", "foo", "wibble", "t")
            .await
            .expect_err("missing branch should error");
        assert!(matches!(err, RepoAccessError::BranchNotFound));
    }

    #[tokio::test]
    async fn malformed_token_surfaces_as_other_not_silent_unauth() {
        // A token with bytes invalid in an HTTP header value (here an embedded
        // newline) must surface as RepoAccessError::Other naming the real
        // cause (corrupt env var), not silently drop the header and 401. The
        // failure happens before any network call — assert the server is
        // never touched.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let err = client_for(&server)
            .validate_repo_access("acme", "foo", "main", "bad\ntoken")
            .await
            .expect_err("malformed token must error");
        match err {
            RepoAccessError::Other(msg) => assert!(
                msg.contains("not valid in an HTTP header value"),
                "unexpected Other body: {msg}"
            ),
            other => panic!("expected RepoAccessError::Other, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_permissions_block_is_no_push_access() {
        // We always send a token, so a response without a permissions block
        // isn't an "unauth read shape" — reject it rather than fall through.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/foo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "default_branch": "main",
                "private": true
            })))
            .mount(&server)
            .await;

        let err = client_for(&server)
            .validate_repo_access("acme", "foo", "main", "t")
            .await
            .expect_err("missing perms should error");
        assert!(matches!(err, RepoAccessError::NoPushAccess));
    }
}
