//! Unit tests for `GitHubApiBackend`. Uses a wiremock server in place of
//! `api.github.com` so the tests are hermetic and the backend's HTTP
//! interactions can be asserted precisely (which queries fire, which
//! payloads land at which endpoints).

#![cfg(feature = "github")]
#![allow(clippy::unwrap_used)]

use std::path::Path;

use base64::Engine;
use regelrecht_corpus::annotation_sidecar::{LegacyFallback, read_sidecar, write_sidecar};
use regelrecht_corpus::backend::{RepoBackend, WriteContext};
use regelrecht_corpus::github_api_backend::GitHubApiBackend;
use regelrecht_corpus::models::GitHubSource;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

fn github_source(owner: &str, repo: &str, branch: &str, sub: Option<&str>) -> GitHubSource {
    GitHubSource {
        owner: owner.to_string(),
        repo: repo.to_string(),
        branch: branch.to_string(),
        path: sub.map(|s| s.to_string()),
        git_ref: None,
    }
}

fn backend(server: &MockServer) -> GitHubApiBackend {
    let src = github_source("acme", "corpus", "traject/abc", None);
    GitHubApiBackend::new(
        &src,
        Some("main".to_string()),
        Some("test-token".to_string()),
    )
    .unwrap()
    .with_api_base(server.uri())
}

/// A 404 only counts as absence once the repo has proven readable —
/// GitHub answers a repo you cannot see with the same body as a missing
/// path. Mount the repo lookup that settles it (asked once per
/// repo+token, then remembered).
async fn mount_readable_repo(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "full_name": "acme/corpus"
        })))
        .mount(server)
        .await;
}

fn ctx() -> WriteContext {
    WriteContext::new("test commit".to_string(), None)
}

/// `persist` bootstraps the traject branch lazily when `ensure_ready` never
/// ran (these tests construct the backend directly, so `branch_ready` is
/// false): mount the ref-GET reporting the branch as already existing so a
/// persisting test needs no create-branch mocks.
async fn mount_branch_exists(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": "refs/heads/traject/abc",
            "object": { "sha": "branch-sha" },
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn read_file_returns_content_and_caches_sha_for_later_write() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // Read returns content + sha.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml",
            "path": "laws/x.yaml",
            "sha": "sha-v1",
            "type": "file",
            "content": b64("hello\n"),
            "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    // PUT must reuse the cached sha.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(
            json!({ "sha": "sha-v1", "branch": "traject/abc" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sha-v2" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    let got = b.read_file(Path::new("laws/x.yaml")).await.unwrap();
    assert_eq!(got.as_deref(), Some("hello\n"));

    b.write_file(Path::new("laws/x.yaml"), "goodbye\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn persist_with_token_override_authenticates_as_the_user() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // The PUT must carry the *override* token — the acting user's own
    // credential — not the backend's baked-in "test-token". Matching on the
    // Authorization header pins the precedence in `persist`
    // (`ctx.token_override` before `self.token`); if that regressed, this
    // mock would not match and persist would fail on the resulting 404.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "als de gebruiker\n")
        .await
        .unwrap();
    b.persist(&WriteContext {
        message: "user-authored commit".to_string(),
        author: None,
        token_override: Some("user-token".to_string()),
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn persist_without_override_keeps_the_backend_token() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // The counterpart: no override → the configured backend token, exactly
    // the pre-spike behaviour every non-editor call site relies on.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "als de service\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();
}

/// A traject save commits to the traject branch and opens no pull request,
/// with either kind of token. The operator docs rest on this: a
/// fine-grained PAT for a traject repository needs `Contents` and
/// `Metadata`, not `Pull requests` (docs/src/content/docs/operations/
/// private-repo-trajects.md). If `persist` ever starts touching the pulls
/// API, that page and the PAT instructions have to change with it.
#[tokio::test]
async fn persist_opens_no_pull_request_with_either_token() {
    for token_override in [None, Some("user-token".to_string())] {
        let server = MockServer::start().await;
        mount_branch_exists(&server).await;

        Mock::given(method("PUT"))
            .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "content": { "sha": "sha-new" }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let b = backend(&server);
        b.write_file(Path::new("laws/x.yaml"), "inhoud\n")
            .await
            .unwrap();
        let outcome = b
            .persist(&WriteContext {
                message: "save".to_string(),
                author: None,
                token_override: token_override.clone(),
            })
            .await
            .unwrap();

        assert!(
            outcome.pr.is_none(),
            "persist returned a PR (token_override = {token_override:?})"
        );
        let touched_pulls: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|r| r.url.path().to_string())
            .filter(|p| p.contains("/pulls"))
            .collect();
        assert!(
            touched_pulls.is_empty(),
            "persist called the pulls API: {touched_pulls:?} (token_override = {token_override:?})"
        );
    }
}

#[tokio::test]
async fn write_without_prior_read_resolves_sha_lazily_at_persist() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // First PUT (no sha) returns 422 — file already exists; backend must
    // then GET sha and re-PUT.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "branch": "traject/abc" })))
        .respond_with(
            ResponseTemplate::new(422).set_body_string("{\"message\":\"sha was not supplied\"}"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // SHA-resolve GET.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml",
            "path": "laws/x.yaml",
            "sha": "sha-existing",
            "type": "file",
            "content": b64("old"),
            "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Retry PUT with the resolved sha succeeds.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-existing" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "new").await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn put_409_refreshes_sha_and_retries_once() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // The caller read the file once, caching sha-old.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "laws/x.yaml",
            "sha": "sha-old", "type": "file",
            "content": b64("v1"), "encoding": "base64",
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // First PUT with sha-old → 409.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-old" })))
        .respond_with(ResponseTemplate::new(409).set_body_string("conflict"))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Backend re-fetches sha → sha-fresh.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "laws/x.yaml",
            "sha": "sha-fresh", "type": "file",
            "content": b64("v1.5"), "encoding": "base64",
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Retry PUT with sha-fresh → 200.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-fresh" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sha-new" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.read_file(Path::new("laws/x.yaml")).await.unwrap();
    b.write_file(Path::new("laws/x.yaml"), "v2").await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn put_403_maps_to_write_denied() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // GitHub refuses the write outright — e.g. the authenticating identity
    // has no push access, or an org's OAuth App access restrictions block
    // the token. Must surface as `WriteDenied` (with the response text for
    // logging), not as a generic `Git` error, and without any sha-refresh
    // retry (`expect(1)` pins that).
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_string("{\"message\":\"Resource not accessible by integration\"}"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "geweigerd\n")
        .await
        .unwrap();
    let err = b
        .persist(&ctx())
        .await
        .expect_err("a GitHub 403 on PUT must fail the persist");
    match err {
        regelrecht_corpus::error::CorpusError::WriteDenied(msg) => {
            assert!(
                msg.contains("Resource not accessible"),
                "the GitHub response text must ride along for logging, got: {msg}"
            );
        }
        other => panic!("expected WriteDenied, got: {other:?}"),
    }
}

#[tokio::test]
async fn put_403_rate_limit_stays_generic_git_error() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // GitHub answers rate-limit exhaustion ALSO with a 403 (primary limit:
    // `x-ratelimit-remaining: 0`). That is transient, not a permissions
    // refusal — it must NOT become `WriteDenied` (which the editor-api
    // translates into a permanent-sounding "geen schrijftoegang" message)
    // but stay on the generic `Git` path.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/laws/x.yaml"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .set_body_string("{\"message\":\"API rate limit exceeded\"}"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.write_file(Path::new("laws/x.yaml"), "gelimiteerd\n")
        .await
        .unwrap();
    let err = b
        .persist(&ctx())
        .await
        .expect_err("a rate-limited 403 on PUT must still fail the persist");
    assert!(
        matches!(err, regelrecht_corpus::error::CorpusError::Git(_)),
        "a rate-limit 403 must stay a generic Git error, got: {err:?}"
    );
}

#[tokio::test]
async fn delete_403_maps_to_write_denied() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // Sha-resolve GET for the blind delete.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "y.yaml", "path": "laws/y.yaml",
            "sha": "sha-d1", "type": "file",
            "content": b64("doomed"), "encoding": "base64",
        })))
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.delete_file(Path::new("laws/y.yaml")).await.unwrap();
    let err = b
        .persist(&ctx())
        .await
        .expect_err("a GitHub 403 on DELETE must fail the persist");
    assert!(
        matches!(err, regelrecht_corpus::error::CorpusError::WriteDenied(_)),
        "expected WriteDenied, got: {err:?}"
    );
}

#[tokio::test]
async fn delete_path_resolves_sha_then_deletes() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    // No prior read → backend must GET sha first.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "y.yaml", "path": "laws/y.yaml",
            "sha": "sha-d1", "type": "file",
            "content": b64("doomed"), "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/repos/acme/corpus/contents/laws/y.yaml"))
        .and(body_partial_json(json!({ "sha": "sha-d1" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.delete_file(Path::new("laws/y.yaml")).await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn delete_already_gone_is_no_op() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;
    mount_readable_repo(&server).await;

    // GET 404 → file already gone → persist swallows the delete.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/laws/gone.yaml"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    b.delete_file(Path::new("laws/gone.yaml")).await.unwrap();
    // No DELETE mock — if backend tried to call it, wiremock would 404
    // again and we'd see a failure. The point is that it doesn't.
    b.persist(&ctx()).await.unwrap();
}

#[tokio::test]
async fn ensure_ready_creates_branch_when_missing() {
    let server = MockServer::start().await;

    // branch_exists check → 404.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(404).set_body_string("missing"))
        .expect(1)
        .mount(&server)
        .await;

    // Resolve base ref.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": "abc123" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // POST new ref.
    Mock::given(method("POST"))
        .and(path("/repos/acme/corpus/git/refs"))
        .and(body_partial_json(json!({
            "ref": "refs/heads/traject/abc",
            "sha": "abc123",
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let mut b = backend(&server);
    b.ensure_ready().await.unwrap();
}

#[tokio::test]
async fn ensure_ready_no_op_when_branch_exists() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": { "sha": "abc123" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // No POST mock — if create_branch is called, the test fails.
    let mut b = backend(&server);
    b.ensure_ready().await.unwrap();
}

#[tokio::test]
async fn list_files_filters_to_extension() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/scenarios"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "name": "a.feature", "path": "scenarios/a.feature", "sha": "s1", "type": "file" },
            { "name": "b.feature", "path": "scenarios/b.feature", "sha": "s2", "type": "file" },
            { "name": "notes.md",  "path": "scenarios/notes.md",  "sha": "s3", "type": "file" },
            { "name": "subdir",    "path": "scenarios/subdir",    "sha": "s4", "type": "dir"  },
        ])))
        .mount(&server)
        .await;

    let b = backend(&server);
    let entries = b
        .list_files(Path::new("scenarios"), Some("feature"))
        .await
        .unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a.feature", "b.feature"]);
}

#[tokio::test]
async fn list_files_missing_directory_returns_empty() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/scenarios"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let b = backend(&server);
    let entries = b
        .list_files(Path::new("scenarios"), Some("feature"))
        .await
        .unwrap();
    assert!(entries.is_empty());
}

#[tokio::test]
async fn sub_path_prefixes_api_calls() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/regulation/nl/wet/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "regulation/nl/wet/x.yaml",
            "sha": "sub-sha", "type": "file",
            "content": b64("c"), "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let src = github_source("acme", "corpus", "traject/abc", Some("regulation/nl"));
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), Some("t".to_string()))
        .unwrap()
        .with_api_base(server.uri());
    let got = b.read_file(Path::new("wet/x.yaml")).await.unwrap();
    assert_eq!(got.as_deref(), Some("c"));
}

#[tokio::test]
async fn changed_files_maps_compare_to_source_relative_paths() {
    let server = MockServer::start().await;

    // Compare base...head returns two changed law files plus one file
    // outside the source's sub_path (repo-root config) that must be dropped.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/compare/main...traject/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "files": [
                { "filename": "regulation/nl/wet/a/2025-01-01.yaml" },
                { "filename": "regulation/nl/wet/b/2025-01-01.yaml" },
                { "filename": "README.md" },
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let src = github_source("acme", "corpus", "traject/abc", Some("regulation/nl"));
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), Some("t".to_string()))
        .unwrap()
        .with_api_base(server.uri());

    let mut changed = b.changed_files().await.unwrap();
    changed.sort();
    assert_eq!(
        changed,
        vec![
            "wet/a/2025-01-01.yaml".to_string(),
            "wet/b/2025-01-01.yaml".to_string(),
        ],
        "sub_path prefix must be stripped and out-of-subtree files dropped"
    );
}

#[tokio::test]
async fn changed_files_missing_branch_returns_empty() {
    let server = MockServer::start().await;

    // No traject branch yet (nothing saved) → Compare API 404 → empty list.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/compare/main...traject/abc"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "message": "Not Found"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    assert!(b.changed_files().await.unwrap().is_empty());
}

#[tokio::test]
async fn changed_files_without_token_is_empty_and_makes_no_request() {
    let server = MockServer::start().await;
    // No mounted mock: any HTTP call would 404 from wiremock. A read-only
    // backend (no token) must short-circuit before hitting the network.
    let src = github_source("acme", "corpus", "traject/abc", None);
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), None)
        .unwrap()
        .with_api_base(server.uri());
    assert!(b.changed_files().await.unwrap().is_empty());
}

/// The collision check on document upload derives a free filename from the
/// recursive listing of the traject's documents folder. An empty answer
/// means "no name is taken", and the write that follows is unconditional —
/// so a listing that failed must never come back empty. A repo this
/// credential cannot read is exactly that case, and GitHub dresses it up
/// as the same 404 a missing folder gets.
#[tokio::test]
async fn recursive_listing_on_an_unreadable_repo_fails_instead_of_reading_empty() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/documents"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message":"Not Found"}"#))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message":"Not Found"}"#))
        .mount(&server)
        .await;

    let err = backend(&server)
        .list_files_recursive(Path::new("documents"), None)
        .await
        .expect_err("an unreadable repo must not list as an empty folder");
    assert!(
        err.to_string().contains("not readable"),
        "the cause must survive: {err}"
    );
}

/// The same listing for a folder that simply has no documents yet stays an
/// empty list — a fresh traject must not error its way out of its first
/// upload.
#[tokio::test]
async fn recursive_listing_of_an_empty_folder_is_still_empty() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/documents"))
        .respond_with(
            ResponseTemplate::new(404).set_body_string(r#"{"message":"Not Found","status":"404"}"#),
        )
        .mount(&server)
        .await;

    let entries = backend(&server)
        .list_files_recursive(Path::new("documents"), None)
        .await
        .unwrap();
    assert!(entries.is_empty());
}

/// A save reads the law first (the If-Match precondition). If a failing
/// read reported "no such file", the save would commit as a *create* over
/// whatever is on the branch — so an unreadable repo errors here too.
#[tokio::test]
async fn read_on_an_unreadable_repo_fails_instead_of_reporting_no_such_file() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/wet/x.yaml"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message":"Not Found"}"#))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus"))
        .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message":"Not Found"}"#))
        .mount(&server)
        .await;

    let err = backend(&server)
        .read_file(Path::new("wet/x.yaml"))
        .await
        .expect_err("an unreadable repo must not read as a missing file");
    assert!(
        err.to_string().contains("not readable"),
        "the cause must survive: {err}"
    );
}

/// The counterpart: a traject branch that has not been created yet is not
/// a failure. The promote flow reads the target law before writing it, and
/// that read happens before the first persist mints the branch.
#[tokio::test]
async fn read_on_a_branch_that_does_not_exist_yet_is_a_plain_miss() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/wet/x.yaml"))
        .respond_with(ResponseTemplate::new(404).set_body_string(
            r#"{"message":"No commit found for the ref traject/abc","status":"404"}"#,
        ))
        .mount(&server)
        .await;

    assert!(
        backend(&server)
            .read_file(Path::new("wet/x.yaml"))
            .await
            .unwrap()
            .is_none()
    );
}

/// A read against a branch that does not exist answers "absent", and the
/// persist that follows creates that branch **from base** — so the file the
/// caller thought was absent may well be sitting there, carried over from
/// base. Adopting it (PUT without sha → 422 → refetch → overwrite) replaces
/// content the caller never read: an append-only notes sidecar would come
/// back holding only the newly added note.
///
/// The write is therefore create-only on a freshly minted branch, and a
/// collision surfaces as a conflict the caller can redo.
#[tokio::test]
async fn a_write_after_creating_the_branch_does_not_overwrite_base_content() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;

    // The branch does not exist yet…
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(404).set_body_string("missing"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": "refs/heads/main",
            "object": { "sha": "base-sha" },
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/corpus/git/refs"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "ref": "refs/heads/traject/abc",
        })))
        .mount(&server)
        .await;
    // …so the sidecar read before the write finds nothing.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/notes/x.yaml"))
        .respond_with(ResponseTemplate::new(404).set_body_string(
            r#"{"message":"No commit found for the ref traject/abc","status":"404"}"#,
        ))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    // Every later read of that path — the sha-refetch the 422 handler
    // would do — finds the base sidecar with the notes already on it.
    // That is what makes this test bite: without the fix the refetch
    // succeeds and the second PUT replaces these notes with only the new.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/notes/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "notes/x.yaml",
            "sha": "base-file-sha", "type": "file",
            "content": b64("notes: [old]\n"), "encoding": "base64",
        })))
        .mount(&server)
        .await;
    // But after the branch is minted from base, the file IS there: GitHub
    // refuses a create-shaped PUT with the 422 that means "supply a sha".
    // Exactly one PUT may go out — the point is that no second one follows
    // carrying a refetched sha, because that one replaces the base notes.
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/notes/x.yaml"))
        .respond_with(
            ResponseTemplate::new(422)
                .set_body_string(r#"{"message":"Invalid request.\n\n\"sha\" wasn't supplied."}"#),
        )
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    assert!(
        b.read_file(Path::new("notes/x.yaml"))
            .await
            .unwrap()
            .is_none(),
        "the branch does not exist yet, so the sidecar reads as absent"
    );
    b.write_file(Path::new("notes/x.yaml"), "notes: [new]\n")
        .await
        .unwrap();

    let err = b
        .persist(&ctx())
        .await
        .expect_err("base content must not be silently replaced");
    assert!(
        matches!(err, regelrecht_corpus::error::CorpusError::Conflict(_)),
        "a caller can redo a conflict; it cannot undo a silent overwrite: {err}"
    );
    // The message reaches an editor and the logs, so it has to read as a
    // sentence — a line-wrapped literal leaves a run of spaces in the middle.
    let message = err.to_string();
    assert!(
        !message.contains("  "),
        "the message must not carry a run of spaces: {message}"
    );
    assert!(
        message.contains("did not exist yet"),
        "the message must say why the write was refused: {message}"
    );
}

#[tokio::test]
async fn changed_files_with_token_compares_as_the_user_without_a_service_token() {
    let server = MockServer::start().await;

    // A traject on a private, user-supplied repo: the server holds no token
    // for it at all (`self.token` is None). The override must therefore both
    // *unblock* the Compare call and *authenticate* it — matching on the
    // Authorization header pins that it is the user's credential going out.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/compare/main...traject/abc"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "files": [{ "filename": "wet/a/2025-01-01.yaml" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let src = github_source("acme", "corpus", "traject/abc", None);
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), None)
        .unwrap()
        .with_api_base(server.uri());

    let changed = b
        .changed_files_with_token(Some("user-token"))
        .await
        .unwrap();
    assert_eq!(changed, vec!["wet/a/2025-01-01.yaml".to_string()]);
}

#[tokio::test]
async fn changed_files_with_token_prefers_the_override_over_the_backend_token() {
    let server = MockServer::start().await;

    // Same precedence as `persist` and `read_file_with_token`: the per-call
    // token supersedes the configured one, so read and write can't
    // authenticate as different identities.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/compare/main...traject/abc"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "files": [] })))
        .expect(1)
        .mount(&server)
        .await;

    let b = backend(&server);
    assert!(
        b.changed_files_with_token(Some("user-token"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn changed_files_with_no_token_at_all_stays_empty() {
    let server = MockServer::start().await;
    // Neither an override nor a configured token: still a short-circuit
    // before the network (no mounted mock would answer).
    let src = github_source("acme", "corpus", "traject/abc", None);
    let b = GitHubApiBackend::new(&src, Some("main".to_string()), None)
        .unwrap()
        .with_api_base(server.uri());
    assert!(b.changed_files_with_token(None).await.unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Notes sidecar at the repository root (RFC-018)
// ---------------------------------------------------------------------------

const SIDECAR_LAW: &str = "wet_voorbeeld";
const ROOT_SIDECAR: &str = "/repos/acme/corpus/contents/annotations/wet_voorbeeld/annotations.yaml";
const LEGACY_SIDECAR: &str =
    "/repos/acme/corpus/contents/regulation/nl/annotations/wet_voorbeeld/annotations.yaml";

fn subpath_backend(server: &MockServer, sub: Option<&str>) -> GitHubApiBackend {
    let src = github_source("acme", "corpus", "traject/abc", sub);
    GitHubApiBackend::new(&src, Some("main".to_string()), Some("t".to_string()))
        .unwrap()
        .with_api_base(server.uri())
}

fn contents_file(path: &str, sha: &str, body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "name": "annotations.yaml", "path": path, "sha": sha, "type": "file",
        "content": b64(body), "encoding": "base64",
    }))
}

/// (method, path) of every request wiremock received, in order.
async fn received(server: &MockServer) -> Vec<(String, String)> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|r| (r.method.to_string(), r.url.path().to_string()))
        .collect()
}

/// With subpath `regulation/nl` the sidecar is written at the repository
/// root, in the same persist as a source-relative law write, and the next
/// read serves it from there.
#[tokio::test]
async fn sidecar_with_subpath_is_written_and_read_at_the_repo_root() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;
    mount_branch_exists(&server).await;

    // Root: absent until the save, then present. Legacy: absent.
    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(LEGACY_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(ROOT_SIDECAR))
        .and(body_partial_json(json!({ "branch": "traject/abc" })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "root-v1" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/regulation/nl/wet/x.yaml"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "law-v1" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = subpath_backend(&server, Some("regulation/nl"));
    assert_eq!(b.repo_subpath(), Some("regulation/nl"));
    assert!(
        read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Read)
            .await
            .unwrap()
            .is_none()
    );

    write_sidecar(&b, SIDECAR_LAW, "notes v1\n").await.unwrap();
    b.write_file(Path::new("wet/x.yaml"), "law\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();

    let puts: Vec<String> = received(&server)
        .await
        .into_iter()
        .filter(|(m, _)| m == "PUT")
        .map(|(_, p)| p)
        .collect();
    assert_eq!(
        puts,
        vec![
            ROOT_SIDECAR.to_string(),
            "/repos/acme/corpus/contents/regulation/nl/wet/x.yaml".to_string(),
        ]
    );

    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(contents_file(
            "annotations/wet_voorbeeld/annotations.yaml",
            "root-v1",
            "notes v1\n",
        ))
        .mount(&server)
        .await;
    assert_eq!(
        read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Read)
            .await
            .unwrap()
            .as_deref(),
        Some("notes v1\n")
    );
}

/// Notes saved by an earlier version under the subpath stay readable, and
/// a save builds on them but writes the result at the repository root,
/// leaving the legacy file alone.
#[tokio::test]
async fn sidecar_falls_back_to_the_legacy_subpath_location() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;
    mount_branch_exists(&server).await;

    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(LEGACY_SIDECAR))
        .respond_with(contents_file(
            "regulation/nl/annotations/wet_voorbeeld/annotations.yaml",
            "legacy-sha",
            "old notes\n",
        ))
        .mount(&server)
        .await;
    // The root write is a create: it must not carry the legacy file's sha.
    Mock::given(method("PUT"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "root-v1" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = subpath_backend(&server, Some("regulation/nl"));
    let base = read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Read)
        .await
        .unwrap();
    assert_eq!(base.as_deref(), Some("old notes\n"));

    write_sidecar(&b, SIDECAR_LAW, "old notes\nnew note\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let put = requests
        .iter()
        .find(|r| r.method.as_str() == "PUT")
        .expect("one PUT");
    assert_eq!(put.url.path(), ROOT_SIDECAR);
    let body: serde_json::Value = serde_json::from_slice(&put.body).unwrap();
    assert!(
        body.get("sha").is_none(),
        "root write must be a create: {body}"
    );
    assert_eq!(body["content"], b64("old notes\nnew note\n"));
    assert!(
        !requests.iter().any(|r| r.method.as_str() == "DELETE"
            || (r.method.as_str() == "PUT" && r.url.path() == LEGACY_SIDECAR)),
        "the legacy file must be left untouched"
    );
}

/// Without a subpath the repository root is the source root: one read, no
/// fallback, and the write lands where it always did.
#[tokio::test]
async fn sidecar_without_subpath_is_unchanged() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;
    mount_branch_exists(&server).await;

    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "root-v1" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    // An empty configured subpath means the same as none.
    assert_eq!(subpath_backend(&server, Some("")).repo_subpath(), None);

    let b = subpath_backend(&server, None);
    assert_eq!(b.repo_subpath(), None);
    assert!(
        read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Read)
            .await
            .unwrap()
            .is_none()
    );
    write_sidecar(&b, SIDECAR_LAW, "notes\n").await.unwrap();
    b.persist(&ctx()).await.unwrap();

    let contents: Vec<(String, String)> = received(&server)
        .await
        .into_iter()
        .filter(|(_, p)| p.contains("/contents/"))
        .collect();
    assert_eq!(
        contents,
        vec![
            ("GET".to_string(), ROOT_SIDECAR.to_string()),
            ("PUT".to_string(), ROOT_SIDECAR.to_string()),
        ]
    );
}

/// Repository-root paths get the same sandbox as source-relative ones.
#[tokio::test]
async fn repo_root_access_rejects_escapes() {
    let server = MockServer::start().await;
    let b = subpath_backend(&server, Some("regulation/nl"));
    for bad in ["../outside.yaml", "/etc/passwd", "annotations/../../x"] {
        assert!(
            b.read_repo_file_with_token(Path::new(bad), None)
                .await
                .is_err()
        );
        assert!(b.write_repo_file(Path::new(bad), "x").await.is_err());
    }
    assert!(
        read_sidecar(&b, "../wet", None, LegacyFallback::Read)
            .await
            .is_err()
    );
    assert!(write_sidecar(&b, "a/b", "x").await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}

/// The traject branch does not exist yet, so both sidecar reads are told
/// "absent" — but the branch `persist` then creates from base carries a
/// legacy notes file under the subpath. Writing a root sidecar holding only
/// the new note would hide those notes for good (the root file wins every
/// later read), so the save must come back as a conflict, with no PUT.
#[tokio::test]
async fn sidecar_save_on_a_minted_branch_refuses_when_base_has_legacy_notes() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;

    // Branch missing at read time (both sidecar reads 404 on the ref) …
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/traject/abc"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(404).set_body_string(
            r#"{"message":"No commit found for the ref traject/abc","status":"404"}"#,
        ))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(LEGACY_SIDECAR))
        .respond_with(ResponseTemplate::new(404).set_body_string(
            r#"{"message":"No commit found for the ref traject/abc","status":"404"}"#,
        ))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    // … then `persist` mints it from base, which holds the legacy file.
    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/git/ref/heads/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": "refs/heads/main",
            "object": { "sha": "base-sha" },
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/repos/acme/corpus/git/refs"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "ref": "refs/heads/traject/abc",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(LEGACY_SIDECAR))
        .respond_with(contents_file(
            "regulation/nl/annotations/wet_voorbeeld/annotations.yaml",
            "legacy-sha",
            "base notes\n",
        ))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "never" }
        })))
        .expect(0)
        .mount(&server)
        .await;

    let b = subpath_backend(&server, Some("regulation/nl"));
    assert!(
        read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Read)
            .await
            .unwrap()
            .is_none()
    );
    write_sidecar(&b, SIDECAR_LAW, "new note\n").await.unwrap();
    let err = b.persist(&ctx()).await.unwrap_err();
    assert!(
        matches!(err, regelrecht_corpus::CorpusError::Conflict(_)),
        "expected a conflict, got {err:?}"
    );
}

/// The same flow on a branch that already exists stages the check but
/// never spends a request on it: only a just-minted branch has the window.
#[tokio::test]
async fn sidecar_absence_check_costs_nothing_on_an_existing_branch() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;
    mount_branch_exists(&server).await;

    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(LEGACY_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "sha": "root-v1" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = subpath_backend(&server, Some("regulation/nl"));
    assert!(
        read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Read)
            .await
            .unwrap()
            .is_none()
    );
    write_sidecar(&b, SIDECAR_LAW, "new note\n").await.unwrap();
    b.persist(&ctx()).await.unwrap();
}

/// Sources the editor never wrote to skip the legacy location: one GET.
#[tokio::test]
async fn sidecar_read_without_legacy_fallback_is_one_request() {
    let server = MockServer::start().await;
    mount_readable_repo(&server).await;

    Mock::given(method("GET"))
        .and(path(ROOT_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(LEGACY_SIDECAR))
        .respond_with(ResponseTemplate::new(404))
        .expect(0)
        .mount(&server)
        .await;

    let b = subpath_backend(&server, Some("regulation/nl"));
    assert!(
        read_sidecar(&b, SIDECAR_LAW, None, LegacyFallback::Skip)
            .await
            .unwrap()
            .is_none()
    );
}

/// Regression guard for the cache re-keying: on a source with a subpath a
/// read still feeds its sha to the later write of the same file.
#[tokio::test]
async fn sub_path_read_then_write_reuses_the_sha() {
    let server = MockServer::start().await;
    mount_branch_exists(&server).await;

    Mock::given(method("GET"))
        .and(path("/repos/acme/corpus/contents/regulation/nl/wet/x.yaml"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "x.yaml", "path": "regulation/nl/wet/x.yaml",
            "sha": "sub-v1", "type": "file",
            "content": b64("hello\n"), "encoding": "base64",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/repos/acme/corpus/contents/regulation/nl/wet/x.yaml"))
        .and(body_partial_json(
            json!({ "sha": "sub-v1", "branch": "traject/abc" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": { "sha": "sub-v2" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let b = subpath_backend(&server, Some("regulation/nl"));
    assert_eq!(
        b.read_file(Path::new("wet/x.yaml"))
            .await
            .unwrap()
            .as_deref(),
        Some("hello\n")
    );
    b.write_file(Path::new("wet/x.yaml"), "goodbye\n")
        .await
        .unwrap();
    b.persist(&ctx()).await.unwrap();
}
