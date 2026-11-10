//! Contents API: read a file (raw or with its sha), list a directory, and
//! upsert / delete a file. Writes carry optimistic-concurrency semantics
//! (409 → [`GithubError::Conflict`]) and permission semantics (genuine 403 →
//! [`GithubError::WriteDenied`], rate-limit 403 stays generic).
//!
//! ## Absence versus failure
//!
//! The reads here answer "not there" as `Ok(None)` / an empty listing, and
//! callers act on that: a missing file is created, a missing directory has
//! no colliding names in it. So a *failure* dressed up as absence is not a
//! degraded read — it overwrites work.
//!
//! GitHub answers three different things with 404: a path that isn't in
//! the ref, a ref that doesn't exist, and a repo this credential cannot
//! see. The first two are absence — there is no content at that path
//! either way, and a traject branch that does not exist yet is a supported
//! state ("branch on first activation"). The third is a failure, and it is
//! indistinguishable from the other two by body: a repo you cannot see
//! answers the same `{"message":"Not Found"}` as a missing file, precisely
//! so it doesn't confirm the repo exists.
//!
//! So a 404 is only reported as absence once the **repository** has proven
//! readable with this credential — one repo lookup per (repo, token),
//! remembered after that. See `confirm_absence`. That a *ref* exists is
//! not this layer's question: the backends settle it in `ensure_ready`,
//! which is where a mistyped corpus branch has to fail loudly.
//!
//! ## Conditional GETs
//!
//! All three reads carry `If-None-Match` from the client's response cache,
//! which stores each ETag together with the body it belongs to. A 304 is
//! answered from that body, so it saves rate-limit quota (304s don't count)
//! without ever turning into an empty answer. A 304 arriving without a
//! cached body — the server ignoring the absence of our header — is retried
//! unconditionally rather than reported as "gone".

use base64::Engine;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue, IF_NONE_MATCH};
use serde::{Deserialize, Serialize};

use crate::client::{CacheKind, CachedPayload, GithubClient};
use crate::error::{GithubError, Result};

/// Commit identity for Contents / Git Data API writes. Both `committer` and
/// `author` accept this shape; callers set them to the same value so the human
/// editor shows on both sides of the commit when authenticating with a shared
/// service token. When authenticating with the acting user's own GitHub token,
/// callers pass `None`: the Contents API then defaults author and committer to
/// the authenticated user, attributing the commit to their real account.
#[derive(Debug, Clone, Serialize)]
pub struct Committer {
    pub name: String,
    pub email: String,
}

/// Single entry from a Contents API directory listing. Only the fields the
/// backend needs; GitHub returns more (url, html_url, size, …).
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    /// `"file"` or `"dir"` (GitHub also reports `"submodule"` / `"symlink"`).
    pub entry_type: String,
}

/// Raw Contents API response for a single path — used for file reads
/// (`type == "file"`) and, as a JSON array, for directory listings.
#[derive(Debug, Deserialize)]
struct ContentsItem {
    name: String,
    path: String,
    sha: String,
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PutResponse {
    content: PutContent,
}
#[derive(Debug, Deserialize)]
struct PutContent {
    sha: String,
}

/// What a repo lookup said about a credential's access. The distinction
/// matters for how long the answer is worth remembering: absence and
/// readability are settled, a refusal is something the user can go and
/// change in the GitHub UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepoAccess {
    /// The repo answered — this credential can read it.
    Readable,
    /// 404: no repo at this address for anyone this credential is.
    NoSuchRepo,
    /// 401/403: the repo may well exist, this credential may not have it.
    Denied,
}

/// What a conditional GET produced: a cache hit, a body to parse, or a
/// 404 that is genuinely about the path.
enum ConditionalOutcome {
    /// HTTP 304 — the cached payload is current and is the answer.
    NotModified(CachedPayload),
    /// A fresh 2xx body, with the key its payload should be stored under.
    Fresh {
        response: reqwest::Response,
        cache_key: String,
    },
    /// HTTP 404 attributed to the path: the thing is not there.
    Absent,
}

impl GithubClient {
    /// One conditional Contents GET on `{repo}/contents/{path}?ref={git_ref}`:
    /// attaches `If-None-Match` when a cached payload of the expected shape
    /// exists, resolves 304 against that cache, separates absence from
    /// failure, and turns every other non-success status into an error.
    ///
    /// A 304 without a cached payload cannot be answered (there is no body
    /// to return, and reporting absence would be a lie), so the request is
    /// retried once without the conditional header. Only a server that 304s
    /// an unconditional request reaches the error at the end.
    async fn conditional_contents_get(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        accept: Option<&'static str>,
        kind: CacheKind,
        token: Option<&str>,
    ) -> Result<ConditionalOutcome> {
        let url = format!(
            "{}/repos/{}/contents/{}?ref={}",
            self.api_base, repo, path, git_ref
        );
        let cache_key = Self::cache_key(&url, accept, token);

        for attempt in 0..2 {
            // A cached entry of another shape cannot answer this read, so
            // it must not be revalidated either — revalidating it would
            // trade a full body for a 304 we cannot use.
            let cached = if attempt == 0 {
                self.cached_response(&cache_key)
                    .filter(|hit| hit.payload.kind() == kind)
            } else {
                None
            };

            let mut headers = self.default_headers(token)?;
            if let Some(accept) = accept {
                headers.insert(ACCEPT, HeaderValue::from_static(accept));
            }
            if let Some(hit) = &cached
                && let Ok(value) = HeaderValue::from_str(&hit.etag)
            {
                headers.insert(IF_NONE_MATCH, value);
            }

            let response = self
                .client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
            self.track_rate_limit(&response);
            tracing::debug!(status = %response.status(), path, "gh contents GET response");

            if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                match cached {
                    Some(hit) => return Ok(ConditionalOutcome::NotModified(hit.payload)),
                    // Unprompted 304: fall through to the unconditional retry.
                    None => continue,
                }
            }

            if response.status() == reqwest::StatusCode::NOT_FOUND {
                self.confirm_absence(repo, path, token).await?;
                return Ok(ConditionalOutcome::Absent);
            }

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(GithubError::Api {
                    status,
                    message: format!("Contents API for {path}: {body}"),
                });
            }

            return Ok(ConditionalOutcome::Fresh {
                response,
                cache_key,
            });
        }

        Err(GithubError::Api {
            status: 304,
            message: format!(
                "Contents API for {path}: answered 304 to a request that carried no \
                 If-None-Match, so there is no body to serve"
            ),
        })
    }

    /// Establish that a 404 really is absence and not a read this
    /// credential was never allowed to make.
    ///
    /// A repo you cannot see answers `{"message":"Not Found"}` —
    /// deliberately the same body as a missing file, so the 404 alone
    /// proves nothing. One repo lookup settles it: the repo answers, so
    /// the credential can read it and the missing thing really is missing.
    /// It doesn't, and the read failed; the caller must not treat that as
    /// an empty folder or a free filename.
    ///
    /// The answer is remembered per (repo, token identity) — both ways.
    /// The positive so a corpus full of legitimately absent sidecar files
    /// pays the lookup once rather than per read; the negative because an
    /// unreadable repo 404s *every* read, and re-probing each one would
    /// double the traffic of the worst case instead of halving it.
    ///
    /// Readers that all miss the remembered answer queue behind one gate
    /// per (repo, token identity), so a burst costs one lookup rather than
    /// one each. The gate carries the answer, including the refusal that
    /// is deliberately not remembered on the client: everyone queued
    /// behind that probe right now gets it, and the gate is gone as soon
    /// as nobody holds it, so the next reader asks again. A probe that
    /// could not answer at all (rate limit, transport) settles nothing —
    /// the next in the queue tries it rather than inheriting a failure.
    ///
    /// It is a snapshot: access changed mid-process stays as first
    /// observed until the client is rebuilt, which is the price of not
    /// paying a lookup per miss. A token that gains access later comes
    /// with a different token identity, so it gets its own answer.
    async fn confirm_absence(&self, repo: &str, path: &str, token: Option<&str>) -> Result<()> {
        let key = Self::cache_key(repo, None, token);
        let access = match self.known_repo_readable(&key) {
            Some(true) => RepoAccess::Readable,
            Some(false) => RepoAccess::NoSuchRepo,
            None => {
                let gate = self.repo_probe_gate(&key);
                let mut probed = gate.lock().await;
                match (self.known_repo_readable(&key), *probed) {
                    (Some(true), _) => RepoAccess::Readable,
                    (Some(false), _) => RepoAccess::NoSuchRepo,
                    (None, Some(access)) => access,
                    (None, None) => {
                        let access = self.repo_access(repo, token).await?;
                        // Only a settled answer is worth remembering
                        // beyond this gate. "Readable" and "no such repo"
                        // don't change under us; a refusal can — an org
                        // OAuth-App restriction or SAML sign-in is
                        // authorised in the GitHub UI in seconds, with the
                        // same token — and pinning that for the client's
                        // lifetime would keep failing long after the user
                        // fixed it.
                        if !matches!(access, RepoAccess::Denied) {
                            self.remember_repo_readable(&key, access == RepoAccess::Readable);
                        }
                        *probed = Some(access);
                        access
                    }
                }
            }
        };
        if access == RepoAccess::Readable {
            return Ok(());
        }
        Err(GithubError::Api {
            status: 404,
            message: format!(
                "Contents API for {path}: 404, and {repo} is not readable with this \
                 credential ({access:?}) — the answer is unknown, not absence"
            ),
        })
    }

    /// What `GET /repos/{repo}` says about this credential's access. A
    /// rate-limit 403 or any other failure is an `Err`, because "I could
    /// not ask" is not "the answer is no".
    async fn repo_access(&self, repo: &str, token: Option<&str>) -> Result<RepoAccess> {
        let url = format!("{}/repos/{}", self.api_base, repo);
        let response = self
            .client
            .get(&url)
            .headers(self.default_headers(token)?)
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub repo lookup failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        if status.is_success() {
            return Ok(RepoAccess::Readable);
        }
        if status == reqwest::StatusCode::FORBIDDEN && Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: 403,
                message: format!("repo lookup for {repo} hit the rate limit: {body}"),
            });
        }
        match status {
            reqwest::StatusCode::NOT_FOUND => Ok(RepoAccess::NoSuchRepo),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                Ok(RepoAccess::Denied)
            }
            other => {
                let body = response.text().await.unwrap_or_default();
                Err(GithubError::Api {
                    status: other.as_u16(),
                    message: format!("repo lookup for {repo}: {body}"),
                })
            }
        }
    }

    /// Read the `ETag` header off a fresh response, if it has one.
    fn response_etag(response: &reqwest::Response) -> Option<String> {
        response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }

    /// Fetch a single file's content via the Contents API, requesting the raw
    /// representation (`application/vnd.github.raw+json`) so there is no
    /// base64 to decode — and, unlike the JSON representation, no 1 MiB
    /// ceiling. Errors on any non-success status, 404 included.
    pub async fn fetch_file_raw(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<String> {
        self.fetch_file_raw_opt(repo, git_ref, path, token)
            .await?
            .ok_or_else(|| GithubError::Api {
                status: 404,
                message: format!("Contents API for {path}: not found"),
            })
    }

    /// [`fetch_file_raw`](Self::fetch_file_raw) with absence as a value:
    /// `Ok(None)` when the file is not in the ref. A missing *ref* still
    /// errors — see the module docs.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "contents_raw", repo = %repo))]
    pub async fn fetch_file_raw_opt(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<String>> {
        let outcome = self
            .conditional_contents_get(
                repo,
                git_ref,
                path,
                Some("application/vnd.github.raw+json"),
                CacheKind::Raw,
                token,
            )
            .await?;
        let (response, cache_key) = match outcome {
            ConditionalOutcome::Absent => return Ok(None),
            ConditionalOutcome::NotModified(CachedPayload::Raw(content)) => {
                return Ok(Some(content));
            }
            // Unreachable: the conditional GET only revalidates an entry
            // whose shape matches, so a 304 always carries a raw body here.
            ConditionalOutcome::NotModified(_) => {
                return Err(GithubError::Decode(format!(
                    "cached response for {path} is not a raw file body"
                )));
            }
            ConditionalOutcome::Fresh {
                response,
                cache_key,
            } => (response, cache_key),
        };

        let etag = Self::response_etag(&response);
        let content = response
            .text()
            .await
            .map_err(|e| GithubError::Transport(format!("failed to read response body: {e}")))?;
        if let Some(etag) = etag {
            self.store_response(&cache_key, &etag, CachedPayload::Raw(content.clone()));
        }
        Ok(Some(content))
    }

    /// Fetch a file's content **plus** its blob sha (the value a later update
    /// PUT needs for optimistic concurrency). Returns `Ok(None)` on 404.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "contents", repo = %repo))]
    pub async fn fetch_file_with_sha(
        &self,
        repo: &str,
        git_ref: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<Option<(String, String)>> {
        let outcome = self
            .conditional_contents_get(repo, git_ref, path, None, CacheKind::File, token)
            .await?;
        let (response, cache_key) = match outcome {
            ConditionalOutcome::Absent => return Ok(None),
            ConditionalOutcome::NotModified(CachedPayload::File { content, sha }) => {
                return Ok(Some((content, sha)));
            }
            // Unreachable — see `fetch_file_raw_opt`.
            ConditionalOutcome::NotModified(_) => {
                return Err(GithubError::Decode(format!(
                    "cached response for {path} is not a file body"
                )));
            }
            ConditionalOutcome::Fresh {
                response,
                cache_key,
            } => (response, cache_key),
        };

        let etag = Self::response_etag(&response);
        let item: ContentsItem = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse contents response: {e}")))?;
        if item.entry_type != "file" {
            return Err(GithubError::Api {
                status: 200,
                message: format!("Path '{path}' is a {}, not a file", item.entry_type),
            });
        }
        let content = decode_contents_payload(&item)?;
        if let Some(etag) = etag {
            self.store_response(
                &cache_key,
                &etag,
                CachedPayload::File {
                    content: content.clone(),
                    sha: item.sha.clone(),
                },
            );
        }
        Ok(Some((content, item.sha)))
    }

    /// List a directory via the Contents API. Returns an empty list for a
    /// missing directory (404) — the "nothing here yet" path. Non-array
    /// responses (someone listed a file path) also yield an empty list.
    /// A 404 that is not about the path — an unknown ref, an unreadable
    /// repo — is an error, not an empty directory.
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "GET", kind = "contents_dir", repo = %repo))]
    pub async fn list_directory(
        &self,
        repo: &str,
        git_ref: &str,
        dir: &str,
        token: Option<&str>,
    ) -> Result<Vec<DirectoryEntry>> {
        let outcome = self
            .conditional_contents_get(repo, git_ref, dir, None, CacheKind::Directory, token)
            .await?;
        let (response, cache_key) = match outcome {
            ConditionalOutcome::Absent => return Ok(Vec::new()),
            ConditionalOutcome::NotModified(CachedPayload::Directory(entries)) => {
                return Ok(entries);
            }
            // Unreachable — see `fetch_file_raw_opt`.
            ConditionalOutcome::NotModified(_) => {
                return Err(GithubError::Decode(format!(
                    "cached response for {dir} is not a directory listing"
                )));
            }
            ConditionalOutcome::Fresh {
                response,
                cache_key,
            } => (response, cache_key),
        };

        let etag = Self::response_etag(&response);
        let body = response.text().await.map_err(|e| {
            GithubError::Transport(format!("failed to read directory listing: {e}"))
        })?;
        let trimmed = body.trim_start();
        if !trimmed.starts_with('[') {
            tracing::debug!(dir = %dir, "list_directory: path is not a directory");
            return Ok(Vec::new());
        }
        let items: Vec<ContentsItem> = serde_json::from_str(&body)
            .map_err(|e| GithubError::Decode(format!("failed to parse directory listing: {e}")))?;
        let entries: Vec<DirectoryEntry> = items
            .into_iter()
            .map(|i| DirectoryEntry {
                name: i.name,
                entry_type: i.entry_type,
            })
            .collect();
        if let Some(etag) = etag {
            self.store_response(&cache_key, &etag, CachedPayload::Directory(entries.clone()));
        }
        Ok(entries)
    }

    /// Upsert a file via Contents API PUT. `base_sha = None` creates a new
    /// file, `Some(sha)` updates an existing one. Returns the new blob sha so
    /// callers can chain writes without a re-read.
    ///
    /// Maps 409 → [`GithubError::Conflict`] (stale sha; caller can retry) and
    /// a genuine 403 → [`GithubError::WriteDenied`] — except rate-limit 403s,
    /// which stay on the generic [`GithubError::Api`] path.
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "PUT", kind = "contents", repo = %repo))]
    pub async fn put_file(
        &self,
        repo: &str,
        branch: &str,
        path: &str,
        content: &str,
        base_sha: Option<&str>,
        committer: Option<&Committer>,
        message: &str,
        token: Option<&str>,
    ) -> Result<String> {
        let url = format!("{}/repos/{}/contents/{}", self.api_base, repo, path);
        let mut body = serde_json::json!({
            "message": message,
            "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),
            "branch": branch,
        });
        if let Some(committer) = committer {
            body["committer"] = serde_json::json!(committer);
            body["author"] = serde_json::json!(committer);
        }
        if let Some(sha) = base_sha {
            body["sha"] = serde_json::Value::String(sha.to_string());
        }

        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self
            .client
            .put(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        tracing::debug!(status = %status, "gh contents PUT response");
        if status == reqwest::StatusCode::CONFLICT {
            return Err(GithubError::Conflict(format!(
                "Contents API PUT {path} hit a 409 (stale sha)"
            )));
        }
        if status == reqwest::StatusCode::FORBIDDEN && !Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::WriteDenied(format!(
                "Contents API PUT {path} returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Contents API PUT {path}: {body}"),
            });
        }
        let parsed: PutResponse = response
            .json()
            .await
            .map_err(|e| GithubError::Decode(format!("failed to parse PUT response: {e}")))?;
        Ok(parsed.content.sha)
    }

    /// Delete a file via Contents API DELETE. Requires the current blob sha.
    /// 404 is treated as "already gone" (idempotent). 409 → Conflict, genuine
    /// 403 → WriteDenied, same as [`put_file`](Self::put_file).
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(name = "gh_http", skip_all, fields(method = "DELETE", kind = "contents", repo = %repo))]
    pub async fn delete_file(
        &self,
        repo: &str,
        branch: &str,
        path: &str,
        sha: &str,
        committer: Option<&Committer>,
        message: &str,
        token: Option<&str>,
    ) -> Result<()> {
        let url = format!("{}/repos/{}/contents/{}", self.api_base, repo, path);
        let mut body = serde_json::json!({
            "message": message,
            "sha": sha,
            "branch": branch,
        });
        if let Some(committer) = committer {
            body["committer"] = serde_json::json!(committer);
            body["author"] = serde_json::json!(committer);
        }

        let mut headers = self.default_headers(token)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self
            .client
            .delete(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GithubError::Transport(format!("GitHub API request failed: {e}")))?;
        self.track_rate_limit(&response);

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        if status == reqwest::StatusCode::CONFLICT {
            return Err(GithubError::Conflict(format!(
                "Contents API DELETE {path} hit a 409 (stale sha)"
            )));
        }
        if status == reqwest::StatusCode::FORBIDDEN && !Self::forbidden_is_rate_limit(&response) {
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::WriteDenied(format!(
                "Contents API DELETE {path} returned 403: {body}"
            )));
        }
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(GithubError::Api {
                status: code,
                message: format!("Contents API DELETE {path}: {body}"),
            });
        }
        Ok(())
    }
}

/// Decode a Contents-API response's content payload. The JSON path always
/// gives base64 (default `encoding: "base64"`); files >1 MiB come back without
/// `content` (encoding `"none"`), for which the Git Blob API is the route.
fn decode_contents_payload(item: &ContentsItem) -> Result<String> {
    let encoding = item.encoding.as_deref().unwrap_or("base64");
    if encoding != "base64" {
        return Err(GithubError::Decode(format!(
            "Contents API returned unsupported encoding '{encoding}' for {} \
             (large file? use the Blob API)",
            item.path
        )));
    }
    let content = item.content.as_deref().ok_or_else(|| {
        GithubError::Decode(format!(
            "Contents API returned no content for {} (possibly >1 MiB)",
            item.path
        ))
    })?;
    // The API wraps base64 at 60 chars/line — strip whitespace first.
    let cleaned: String = content
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|e| GithubError::Decode(format!("base64 decode failed for {}: {e}", item.path)))?;
    String::from_utf8(bytes)
        .map_err(|e| GithubError::Decode(format!("UTF-8 decode failed for {}: {e}", item.path)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use base64::Engine;
    use wiremock::matchers::{header, header_exists, method, path as path_matcher};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    /// GitHub's 404 body when the branch/sha in `?ref=` does not exist.
    const MISSING_REF_BODY: &str =
        r#"{"message":"No commit found for the ref traject/new","status":"404"}"#;
    /// GitHub's 404 body for a path that is simply not in the ref.
    const MISSING_PATH_BODY: &str = r#"{"message":"Not Found","status":"404"}"#;

    fn client(server: &MockServer) -> GithubClient {
        GithubClient::new().unwrap().with_base_url(server.uri())
    }

    /// The probe that turns a 404 into absence: the repo answers, so this
    /// credential can read it. `times` pins how often it may be asked —
    /// absence is confirmed once per (repo, token), not once per miss.
    async fn mount_readable_repo(server: &MockServer, times: u64) {
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "full_name": "acme/corpus"
            })))
            .expect(times)
            .mount(server)
            .await;
    }

    /// A Contents API file response body carrying `content` for `path`.
    fn file_body(path: &str, sha: &str, content: &str) -> serde_json::Value {
        serde_json::json!({
            "name": path.rsplit('/').next().unwrap_or(path),
            "path": path,
            "sha": sha,
            "type": "file",
            "encoding": "base64",
            "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),
        })
    }

    fn dir_body(names: &[(&str, &str)]) -> serde_json::Value {
        serde_json::Value::Array(
            names
                .iter()
                .map(|(name, kind)| {
                    serde_json::json!({
                        "name": name,
                        "path": name,
                        "sha": "s",
                        "type": kind,
                    })
                })
                .collect(),
        )
    }

    /// A ref that does not exist yet holds no documents, so an empty
    /// listing is the true answer — a traject branch is created on first
    /// write, and the read before it must not fail.
    #[tokio::test]
    async fn a_ref_that_does_not_exist_yet_lists_empty() {
        let server = MockServer::start().await;
        mount_readable_repo(&server, 1).await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_REF_BODY))
            .mount(&server)
            .await;

        let entries = client(&server)
            .list_directory("acme/corpus", "traject/new", "documents", None)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    /// The other side of that coin: a directory that genuinely isn't there
    /// yet is absence, and stays an empty listing — once the ref it was
    /// asked at has proven readable.
    #[tokio::test]
    async fn missing_path_still_lists_empty() {
        let server = MockServer::start().await;
        mount_readable_repo(&server, 1).await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        let entries = client(&server)
            .list_directory("acme/corpus", "main", "documents", None)
            .await
            .unwrap();
        assert!(entries.is_empty());
    }

    /// The dangerous 404: a repo this credential cannot read answers the
    /// same `{"message":"Not Found"}` as a missing path — deliberately, so
    /// it doesn't confirm the repo exists. Reading that as an empty folder
    /// hands the caller a free filename over an existing document, so it
    /// must fail instead.
    #[tokio::test]
    async fn an_unreadable_repo_does_not_read_as_an_empty_directory() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;
        // The probe 404s too: this credential cannot see the repo at all.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        let err = client(&server)
            .list_directory("acme/corpus", "main", "documents", Some("tok"))
            .await
            .expect_err("an unreadable repo must not list as an empty folder");
        assert!(
            err.to_string().contains("not readable"),
            "the message must name the real cause: {err}"
        );
    }

    /// An unreadable repo 404s every single read. Re-probing each one
    /// would double the traffic of the worst case, so the negative answer
    /// is remembered exactly like the positive one.
    #[tokio::test]
    async fn an_unreadable_repo_is_probed_once_not_once_per_read() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .expect(1)
            .mount(&server)
            .await;
        for file in ["a", "b", "c"] {
            Mock::given(method("GET"))
                .and(path_matcher(format!(
                    "/repos/acme/corpus/contents/wet/{file}.yaml"
                )))
                .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
                .mount(&server)
                .await;
        }

        let c = client(&server);
        for file in ["a", "b", "c"] {
            c.fetch_file_with_sha(
                "acme/corpus",
                "main",
                &format!("wet/{file}.yaml"),
                Some("tok"),
            )
            .await
            .expect_err("an unreadable repo never answers with absence");
        }
    }

    /// A refusal is not settled: an org OAuth-App restriction or a SAML
    /// sign-in is authorised in the GitHub UI in seconds, with the same
    /// token. Remembering it would keep the read failing long after the
    /// user fixed it, so a 403 is re-asked.
    #[tokio::test]
    async fn a_refused_repo_is_re_probed_rather_than_written_off() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(ResponseTemplate::new(403).set_body_string("must authorize the app"))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        // Second probe: access has been granted in the meantime.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "full_name": "acme/corpus"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        let c = client(&server);
        c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("tok"))
            .await
            .expect_err("while access is refused the read cannot answer");
        assert!(
            c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("tok"))
                .await
                .unwrap()
                .is_none(),
            "once access is granted the same read answers absence"
        );
    }

    /// The Contents API serves the same URL as JSON or as raw depending on
    /// `Accept`. Those two bodies must not share a cache entry: a raw body
    /// cannot answer a read that wants the blob sha with it, so the second
    /// read must go out unconditionally rather than revalidate an ETag
    /// whose payload it cannot use.
    #[tokio::test]
    async fn a_raw_read_and_a_json_read_of_one_path_do_not_share_a_cache_entry() {
        let server = MockServer::start().await;
        // Revalidating across representations is the bug under test.
        Mock::given(method("GET"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(304))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .and(header("accept", "application/vnd.github.raw+json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"raw1\"")
                    .set_body_string("$id: a\n"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"json1\"")
                    .set_body_json(file_body("wet/a.yaml", "b1", "$id: a\n")),
            )
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        assert_eq!(
            c.fetch_file_raw("acme/corpus", "main", "wet/a.yaml", None)
                .await
                .unwrap(),
            "$id: a\n"
        );
        assert_eq!(
            c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
                .await
                .unwrap(),
            Some(("$id: a\n".to_string(), "b1".to_string())),
            "the json read must not be answered from the raw entry"
        );
    }

    /// A corpus is full of legitimately absent sidecar files, so the probe
    /// may not run per miss: the repo is confirmed once and remembered.
    #[tokio::test]
    async fn absence_is_confirmed_once_per_repo_not_once_per_miss() {
        let server = MockServer::start().await;
        mount_readable_repo(&server, 1).await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/b.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        let c = client(&server);
        for p in ["wet/a.yaml", "wet/b.yaml"] {
            assert!(
                c.fetch_file_with_sha("acme/corpus", "main", p, Some("tok"))
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    /// Concurrent misses are the case the memo alone does not cover: none
    /// of them sees a remembered answer, so without coalescing they each
    /// probe. A snapshot rebuild reads many paths at once, which is exactly
    /// this shape.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_misses_share_one_readability_probe() {
        let server = MockServer::start().await;
        // Delayed so the readers genuinely overlap on the probe rather
        // than finishing one after another by luck of scheduling.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(150))
                    .set_body_json(serde_json::json!({"full_name": "acme/corpus"})),
            )
            .expect(1)
            .mount(&server)
            .await;
        for name in ["a", "b", "c", "d"] {
            Mock::given(method("GET"))
                .and(path_matcher(format!(
                    "/repos/acme/corpus/contents/wet/{name}.yaml"
                )))
                .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
                .mount(&server)
                .await;
        }

        let c = std::sync::Arc::new(client(&server));
        let reads = ["a", "b", "c", "d"].map(|name| {
            let c = std::sync::Arc::clone(&c);
            tokio::spawn(async move {
                c.fetch_file_with_sha(
                    "acme/corpus",
                    "main",
                    &format!("wet/{name}.yaml"),
                    Some("tok"),
                )
                .await
            })
        });
        for read in reads {
            assert!(read.await.unwrap().unwrap().is_none());
        }
        // `expect(1)` on the probe mock is verified when the server drops.
    }

    /// A refusal is not remembered on the client, so it is the answer the
    /// memo can never hand to the rest of a concurrent burst. The gate
    /// carries it for as long as that burst lasts — and no longer, which
    /// `a_refused_repo_is_re_probed_rather_than_written_off` pins down.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_misses_on_a_refused_repo_share_one_probe() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_delay(std::time::Duration::from_millis(150))
                    .set_body_string("must authorize the app"),
            )
            .expect(1)
            .mount(&server)
            .await;
        for name in ["a", "b", "c", "d"] {
            Mock::given(method("GET"))
                .and(path_matcher(format!(
                    "/repos/acme/corpus/contents/wet/{name}.yaml"
                )))
                .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
                .mount(&server)
                .await;
        }

        let c = std::sync::Arc::new(client(&server));
        let reads = ["a", "b", "c", "d"].map(|name| {
            let c = std::sync::Arc::clone(&c);
            tokio::spawn(async move {
                c.fetch_file_with_sha(
                    "acme/corpus",
                    "main",
                    &format!("wet/{name}.yaml"),
                    Some("tok"),
                )
                .await
            })
        });
        for read in reads {
            read.await
                .unwrap()
                .expect_err("a refused repo proves no absence, for any of them");
        }
    }

    /// A probe that cannot answer (rate limit) leaves absence unproven, so
    /// the read fails rather than guessing.
    #[tokio::test]
    async fn a_rate_limited_probe_leaves_the_read_failed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .set_body_string("API rate limit exceeded"),
            )
            .mount(&server)
            .await;

        let err = client(&server)
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("tok"))
            .await
            .expect_err("unproven absence is not absence");
        assert!(
            matches!(err, GithubError::Api { status: 403, .. }),
            "unexpected error: {err}"
        );
    }

    /// Same for a file read on a not-yet-created branch: the law is not
    /// there, which is what the promote flow asks before it writes.
    #[tokio::test]
    async fn a_read_on_a_ref_that_does_not_exist_yet_is_none() {
        let server = MockServer::start().await;
        mount_readable_repo(&server, 1).await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_REF_BODY))
            .mount(&server)
            .await;

        assert!(
            client(&server)
                .fetch_file_with_sha("acme/corpus", "traject/new", "wet/a.yaml", None)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn missing_file_is_still_none() {
        let server = MockServer::start().await;
        mount_readable_repo(&server, 1).await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        assert!(
            client(&server)
                .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
                .await
                .unwrap()
                .is_none()
        );
    }

    /// The rate-limit payoff: the second read of an unchanged file carries
    /// `If-None-Match`, GitHub answers 304 (free), and the caller still gets
    /// the body — never `None`.
    #[tokio::test]
    async fn repeat_file_read_revalidates_and_304_serves_the_cached_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"v1\"")
                    .set_body_json(file_body("wet/a.yaml", "blob1", "$id: a\n")),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .and(header("if-none-match", "\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        let first = c
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(first, Some(("$id: a\n".to_string(), "blob1".to_string())));

        let second = c
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(
            second,
            Some(("$id: a\n".to_string(), "blob1".to_string())),
            "a 304 must serve the cached body, not an empty answer"
        );
    }

    /// A 304 on a *listing* is the dangerous one: read as an empty body it
    /// becomes an empty directory, and the collision check goes blind.
    #[tokio::test]
    async fn repeat_directory_listing_304_serves_the_cached_entries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"d1\"")
                    .set_body_json(dir_body(&[("rapport.md", "file"), ("bijlagen", "dir")])),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/documents"))
            .and(header("if-none-match", "\"d1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        let first = c
            .list_directory("acme/corpus", "main", "documents", None)
            .await
            .unwrap();
        assert_eq!(first.len(), 2);

        let second = c
            .list_directory("acme/corpus", "main", "documents", None)
            .await
            .unwrap();
        let names: Vec<&str> = second.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["rapport.md", "bijlagen"],
            "a 304 must serve the cached listing, not an empty directory"
        );
    }

    /// The raw representation (used for the >1 MiB implements index) has the
    /// same contract.
    #[tokio::test]
    async fn repeat_raw_read_304_serves_the_cached_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/index.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"r1\"")
                    .set_body_string("{\"version\":1}"),
            )
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/index.json"))
            .and(header("if-none-match", "\"r1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        assert_eq!(
            c.fetch_file_raw("acme/corpus", "main", "index.json", None)
                .await
                .unwrap(),
            "{\"version\":1}"
        );
        assert_eq!(
            c.fetch_file_raw("acme/corpus", "main", "index.json", None)
                .await
                .unwrap(),
            "{\"version\":1}",
            "a 304 must serve the cached raw body"
        );
    }

    /// A 304 answered to a request that carried no `If-None-Match` leaves us
    /// with nothing to serve. Retry unconditionally rather than report the
    /// file as gone; only a server that 304s that retry too gets an error.
    #[tokio::test]
    async fn unprompted_304_is_retried_instead_of_read_as_absent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(304))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(200).set_body_json(file_body(
                "wet/a.yaml",
                "b1",
                "$id: a\n",
            )))
            .expect(1)
            .mount(&server)
            .await;

        let got = client(&server)
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(got, Some(("$id: a\n".to_string(), "b1".to_string())));
    }

    /// A server that keeps 304-ing an unconditional request has given us no
    /// body at all — that is an error, never "the file does not exist".
    #[tokio::test]
    async fn persistent_unprompted_304_errors_rather_than_reporting_absence() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(ResponseTemplate::new(304))
            .expect(2)
            .mount(&server)
            .await;

        let err = client(&server)
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .expect_err("a bodyless 304 is not an answer");
        assert!(
            matches!(err, GithubError::Api { status: 304, .. }),
            "unexpected error: {err}"
        );
    }

    /// Cache entries are per token identity: a read authenticated as another
    /// principal must not revalidate against — let alone be answered from —
    /// the first principal's entry.
    #[tokio::test]
    async fn a_second_token_does_not_reuse_the_first_tokens_cache_entry() {
        let server = MockServer::start().await;
        // Any conditional request on token-b is a bug; assert it never happens.
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .and(header("authorization", "Bearer token-b"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"v1\"")
                    .set_body_json(file_body("wet/a.yaml", "b1", "$id: a\n")),
            )
            .mount(&server)
            .await;

        let c = client(&server);
        c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("token-a"))
            .await
            .unwrap();
        c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", Some("token-b"))
            .await
            .unwrap();
    }

    /// The cache is bounded, so an entry can be evicted while its ETag
    /// would still validate. The re-read must then go out **without**
    /// `If-None-Match`: revalidating an ETag whose body was thrown away
    /// would trade a usable 200 for an unusable 304.
    #[tokio::test]
    async fn an_evicted_body_is_re_fetched_without_a_conditional_header() {
        let server = MockServer::start().await;
        // Any conditional request is the bug this test is about.
        Mock::given(method("GET"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(304))
            .expect(0)
            .mount(&server)
            .await;
        for (file, sha, etag) in [("a", "b1", "\"v1\""), ("b", "b2", "\"v2\"")] {
            let body = format!("$id: {file}\n");
            Mock::given(method("GET"))
                .and(path_matcher(format!(
                    "/repos/acme/corpus/contents/wet/{file}.yaml"
                )))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("etag", etag)
                        .set_body_json(file_body(&format!("wet/{file}.yaml"), sha, &body)),
                )
                .mount(&server)
                .await;
        }

        let mut c = client(&server);
        // Room for exactly one body ("$id: a\n" plus its sha is 9 bytes).
        c.set_cache_budget_bytes(12);

        c.fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        // Caching b evicts a — the budget fits only one.
        c.fetch_file_with_sha("acme/corpus", "main", "wet/b.yaml", None)
            .await
            .unwrap();
        let again = c
            .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
            .await
            .unwrap();
        assert_eq!(again, Some(("$id: a\n".to_string(), "b1".to_string())));
    }

    /// A body that cannot fit the budget at all is not cached, rather than
    /// inserted and immediately evicted.
    #[tokio::test]
    async fn a_body_larger_than_the_budget_is_not_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(304))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/contents/wet/a.yaml"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "\"v1\"")
                    .set_body_json(file_body("wet/a.yaml", "b1", "$id: a\n")),
            )
            .expect(2)
            .mount(&server)
            .await;

        let mut c = client(&server);
        c.set_cache_budget_bytes(4);
        for _ in 0..2 {
            let got = c
                .fetch_file_with_sha("acme/corpus", "main", "wet/a.yaml", None)
                .await
                .unwrap();
            assert_eq!(got, Some(("$id: a\n".to_string(), "b1".to_string())));
        }
    }

    /// A subtree sha is the identity a precomputed index is checked against,
    /// so "the ref/path is unknown" (`Ok(None)`) must stay distinguishable
    /// from a real sha — and cost one call per path component.
    #[tokio::test]
    async fn subtree_sha_walks_components_and_reports_an_unknown_path_as_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sha": "roottree",
                "truncated": false,
                "tree": [
                    {"path": "regulation", "type": "tree", "sha": "regtree"},
                    {"path": "README.md", "type": "blob", "sha": "readme"},
                ]
            })))
            .mount(&server)
            .await;

        let c = client(&server);
        assert_eq!(
            c.subtree_sha("acme/corpus", "main", "regulation", None)
                .await
                .unwrap(),
            Some("regtree".to_string())
        );
        assert_eq!(
            c.subtree_sha("acme/corpus", "main", "elders", None)
                .await
                .unwrap(),
            None,
            "an absent directory has no sha; that is not the same as a mismatch"
        );
        assert_eq!(
            c.subtree_sha("acme/corpus", "main", "", None)
                .await
                .unwrap(),
            Some("roottree".to_string())
        );
    }

    /// A subtree lookup that fails for any other reason (rate limit, 5xx)
    /// must error: an unreadable sha is not a differing sha.
    #[tokio::test]
    async fn subtree_sha_surfaces_a_rate_limited_lookup_as_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/main"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .set_body_string("API rate limit exceeded"),
            )
            .mount(&server)
            .await;

        let err = client(&server)
            .subtree_sha("acme/corpus", "main", "regulation", None)
            .await
            .expect_err("a 403 is not an answer");
        assert!(
            matches!(err, GithubError::Api { status: 403, .. }),
            "unexpected error: {err}"
        );
    }

    /// A missing ref has no tree at all — `Ok(None)`, so callers fall back
    /// rather than believing a sha they never got.
    #[tokio::test]
    async fn subtree_sha_of_a_missing_ref_is_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/repos/acme/corpus/git/trees/pr9999"))
            .respond_with(ResponseTemplate::new(404).set_body_string(MISSING_PATH_BODY))
            .mount(&server)
            .await;

        assert_eq!(
            client(&server)
                .subtree_sha("acme/corpus", "pr9999", "regulation", None)
                .await
                .unwrap(),
            None
        );
    }
}
