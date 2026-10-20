//! Integration tests for the traject HTTP handlers.
//!
//! Each test spins up an isolated Postgres container via
//! `regelrecht_pipeline::test_utils::TestDb`, so the migrations under
//! `packages/pipeline/migrations/` are exercised end-to-end — including
//! `0014_trajects.sql`. Handlers are invoked directly (with `axum`
//! extractors constructed inline) rather than through a `Router`, which
//! keeps the tests focused on the handler logic and skips the cookie /
//! middleware plumbing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{empty_state, seed_account_record};

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};
use regelrecht_editor_api::state::AppState;
use regelrecht_editor_api::trajects::{
    self, AddMemberRequest, CreateTrajectRequest, MoveRepoRequest, UpdateMemberRequest,
    UpdateTrajectRequest,
};

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::JobType;
use regelrecht_pipeline::tasks::{self, BlobKind, NewTask, Task, TaskStatus, TaskType};
use regelrecht_pipeline::test_utils::TestDb;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_req(name: &str) -> CreateTrajectRequest {
    CreateTrajectRequest {
        name: name.to_string(),
        description: String::new(),
        scope: String::new(),
        repo_owner: None,
        repo_name: None,
        base_branch: None,
        repo_path: None,
    }
}

/// Create a traject owned by `owner` and return its id.
async fn create_traject(state: &AppState, owner: &AccountRecord, name: &str) -> Uuid {
    let (status, Json(summary)) = trajects::create(
        State(state.clone()),
        Extension(owner.clone()),
        axum::http::HeaderMap::new(),
        Json(create_req(name)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    summary.id
}

/// Test helper that calls `trajects::add_member` and returns either the
/// resulting `(StatusCode::OK, "active" | "pending")` on success or the
/// error status on failure. Tests that care about the response body call
/// `trajects::add_member` directly.
async fn add_member(
    state: &AppState,
    owner: &AccountRecord,
    traject_id: Uuid,
    invitee_email: &str,
    role: &str,
) -> Result<&'static str, StatusCode> {
    trajects::add_member(
        State(state.clone()),
        Extension(owner.clone()),
        Path(traject_id),
        Json(AddMemberRequest {
            email: invitee_email.to_string(),
            role: role.to_string(),
        }),
    )
    .await
    .map(|Json(body)| body.status)
}

// ---------------------------------------------------------------------------
// `create`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_inserts_traject_with_creator_as_owner_and_writable_own_source() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;

    let (status, Json(summary)) = trajects::create(
        State(state.clone()),
        Extension(alice.clone()),
        axum::http::HeaderMap::new(),
        Json(create_req("Tarief")),
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(summary.name, "Tarief");
    assert_eq!(summary.status, "bezig");
    assert_eq!(summary.role, "owner");

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(summary.id)
    .bind(alice.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "owner");

    let (priority, is_writable_own, gh_branch): (i32, bool, String) = sqlx::query_as(
        "SELECT priority, is_writable_own, gh_branch
         FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own = TRUE",
    )
    .bind(summary.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(priority, 0, "writable_own should beat the seeded sources");
    assert!(is_writable_own);
    assert!(
        gh_branch.starts_with("traject/tarief-"),
        "branch should be slugified, got {gh_branch}"
    );
}

#[tokio::test]
async fn create_rejects_empty_name() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;

    let mut req = create_req("");
    req.name = "   ".to_string();
    let (status, _msg) = trajects::create(
        State(state),
        Extension(alice),
        axum::http::HeaderMap::new(),
        Json(req),
    )
    .await
    .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn normal_traject_is_writable_with_github_own() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let owner = seed_account_record(&db.pool, "owner2@example.com", "Owner2").await;

    let id = create_traject(&state, &owner, "Gewoon traject").await;

    let src_type: String = sqlx::query_scalar(
        "SELECT source_type::text FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own = TRUE",
    )
    .bind(id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(src_type, "github");
}

/// Create-request with user-supplied repo coords — the variant whose
/// preflight resolves a per-user GitHub token.
fn custom_repo_req(name: &str) -> CreateTrajectRequest {
    CreateTrajectRequest {
        name: name.to_string(),
        description: String::new(),
        scope: String::new(),
        repo_owner: Some("example-org".to_string()),
        repo_name: Some("regelrecht-corpus-example".to_string()),
        base_branch: Some("main".to_string()),
        repo_path: None,
    }
}

#[tokio::test]
async fn create_custom_repo_requires_linked_github_when_enforced() {
    // With user-token enforcement on and no linked GitHub account, the
    // create-traject preflight must refuse with 428 — before any GitHub
    // round-trip (no mock server needed) and before any DB row is written.
    let db = TestDb::new().await;
    let mut state = empty_state(db.pool.clone());
    state.config = Arc::new(AppConfig {
        oidc: None,
        base_url: None,
        github_oauth: Some(GithubOAuth::for_tests(true)),
        task_enrich_provider: "claude".to_string(),
    });
    let owner = seed_account_record(&db.pool, "owner-enforced@example.com", "Owner").await;

    let err = trajects::create(
        State(state.clone()),
        Extension(owner.clone()),
        axum::http::HeaderMap::new(),
        Json(custom_repo_req("Repo traject")),
    )
    .await
    .expect_err("create with enforcement on and no linked token must refuse");
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trajects")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "a refused create must not leave a traject behind");
}

#[tokio::test]
async fn create_custom_repo_preflights_with_the_linked_user_token() {
    // The linked branch: the repo preflight must authenticate as the acting
    // user (their sealed-cookie token), not the operator token. Matching on
    // the Authorization header pins that — an operator-token request would
    // not match the mocks and the preflight would fail on the 404.
    //
    // It also mints the traject branch as part of create (see
    // `create_custom_repo_mints_the_traject_branch` for the dedicated
    // regression assertion); the branch-bootstrap mocks below are needed
    // for the create to succeed at all now that branch creation is eager.
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let db = TestDb::new().await;
    let server = MockServer::start().await;

    let mut oauth = GithubOAuth::for_tests(true);
    oauth.api_base = server.uri();
    let mut state = empty_state(db.pool.clone());
    state.config = Arc::new(AppConfig {
        oidc: None,
        base_url: None,
        github_oauth: Some(oauth.clone()),
        task_enrich_provider: "claude".to_string(),
    });
    let owner = seed_account_record(&db.pool, "owner-linked@example.com", "Owner").await;

    Mock::given(method("GET"))
        .and(path("/repos/example-org/regelrecht-corpus-example"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "main",
            "private": true,
            "permissions": { "push": true, "pull": true },
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/branches/main",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "main" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    mount_branch_bootstrap_mocks(&server).await;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            owner.id,
            "user-token",
        ))
        .unwrap(),
    );

    let (status, Json(summary)) = trajects::create(
        State(state.clone()),
        Extension(owner.clone()),
        headers,
        Json(custom_repo_req("Repo traject")),
    )
    .await
    .expect("create with a linked token must pass the preflight");
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(summary.name, "Repo traject");
}

/// Mount the three GitHub Refs API mocks that `ensure_branch` hits when the
/// traject branch does not yet exist: `branch_exists` (404 → "create it"),
/// resolve the base ref's sha, then POST the new ref. The traject branch name
/// carries a per-create random 8-hex suffix, so the branch paths are matched
/// by regex rather than a literal.
async fn mount_branch_bootstrap_mocks(server: &wiremock::MockServer) {
    use wiremock::matchers::{header, method, path, path_regex};
    use wiremock::{Mock, ResponseTemplate};

    // 1) branch_exists on the traject branch → 404 (does not exist yet).
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/repos/example-org/regelrecht-corpus-example/git/ref/heads/traject/.+$",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found",
        })))
        .mount(server)
        .await;
    // 2) resolve the base branch ref → its tip sha.
    Mock::given(method("GET"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/git/ref/heads/main",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/heads/main",
            "object": { "sha": "abc123def456", "type": "commit" },
        })))
        .mount(server)
        .await;
    // 3) create the traject ref from that sha.
    Mock::given(method("POST"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/git/refs",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "ref": "refs/heads/traject/repo-traject",
            "object": { "sha": "abc123def456", "type": "commit" },
        })))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn create_custom_repo_mints_the_traject_branch() {
    // Regression for the "fresh traject is dead-on-arrival" bug: in
    // user-token write mode nothing minted the traject branch at create
    // time, so the very first index scan 404'd on a non-existent branch and
    // every corpus endpoint 502'd — a deadlock, because the write UI that
    // would lazily mint the branch was itself behind that 502.
    //
    // `create` must now POST a new ref for `traject/{slug}-{short}` off the
    // base branch during the preflight. The `.expect(1)` on the POST-refs
    // mock (in `mount_branch_bootstrap_mocks`) is what fails on current main,
    // where no such request is ever made.
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let db = TestDb::new().await;
    let server = MockServer::start().await;

    let mut oauth = GithubOAuth::for_tests(true);
    oauth.api_base = server.uri();
    let mut state = empty_state(db.pool.clone());
    state.config = Arc::new(AppConfig {
        oidc: None,
        base_url: None,
        github_oauth: Some(oauth.clone()),
        task_enrich_provider: "claude".to_string(),
    });
    let owner = seed_account_record(&db.pool, "owner-branch@example.com", "Owner").await;

    Mock::given(method("GET"))
        .and(path("/repos/example-org/regelrecht-corpus-example"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "main",
            "private": true,
            "permissions": { "push": true, "pull": true },
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/branches/main",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "main" })),
        )
        .mount(&server)
        .await;
    mount_branch_bootstrap_mocks(&server).await;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            owner.id,
            "user-token",
        ))
        .unwrap(),
    );

    let (status, Json(summary)) = trajects::create(
        State(state.clone()),
        Extension(owner.clone()),
        headers,
        Json(custom_repo_req("Repo traject")),
    )
    .await
    .expect("create with a linked token must mint the branch and succeed");
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(summary.name, "Repo traject");

    // The traject row exists, and its writable-own source carries the
    // branch we just minted on the repo.
    let branch: String = sqlx::query_scalar(
        "SELECT gh_branch FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own = TRUE",
    )
    .bind(summary.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(
        branch.starts_with("traject/repo-traject-"),
        "writable-own source should carry the minted traject branch, got {branch}"
    );

    // The load-bearing assertion: the ref actually POSTed to GitHub is the
    // SAME branch the writable-own source stores. `gh_branch` is what the
    // index scan reads (`effective_ref()`), so minting *a* branch is not
    // enough — mint `traject/x` while persisting `traject/y` and the traject
    // is still dead-on-arrival, now with a stray branch alongside it. Reading
    // the request body pins the two together; a `.expect(1)` on the mock
    // alone would not catch that drift.
    let requests = server
        .received_requests()
        .await
        .expect("mock server records requests");
    let created_refs: Vec<String> = requests
        .iter()
        .filter(|r| r.method == wiremock::http::Method::POST && r.url.path().ends_with("/git/refs"))
        .map(|r| {
            let body: serde_json::Value =
                serde_json::from_slice(&r.body).expect("ref-create body is JSON");
            body["ref"].as_str().unwrap_or_default().to_string()
        })
        .collect();
    assert_eq!(
        created_refs,
        vec![format!("refs/heads/{branch}")],
        "create must POST exactly the ref it persists as gh_branch"
    );
}

#[tokio::test]
async fn create_fails_loudly_and_persists_nothing_when_the_branch_cannot_be_minted() {
    // Criterion: a traject that cannot get its branch must not come into
    // existence at all. The mint happens before the DB transaction opens, so
    // a failing ref-create has to surface as an error *and* leave zero rows
    // behind — no traject that looks fine in the list but 502s on every
    // corpus call. Move the mint after the INSERT and this test goes red.
    use wiremock::matchers::{header, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let db = TestDb::new().await;
    let server = MockServer::start().await;

    let mut oauth = GithubOAuth::for_tests(true);
    oauth.api_base = server.uri();
    let mut state = empty_state(db.pool.clone());
    state.config = Arc::new(AppConfig {
        oidc: None,
        base_url: None,
        github_oauth: Some(oauth.clone()),
        task_enrich_provider: "claude".to_string(),
    });
    let owner = seed_account_record(&db.pool, "owner-branchfail@example.com", "Owner").await;

    // Preflight passes: repo readable, push allowed, base branch present.
    Mock::given(method("GET"))
        .and(path("/repos/example-org/regelrecht-corpus-example"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "main",
            "private": true,
            "permissions": { "push": true, "pull": true },
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/branches/main",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "main" })),
        )
        .mount(&server)
        .await;
    // The traject branch does not exist...
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/repos/example-org/regelrecht-corpus-example/git/ref/heads/traject/.+$",
        ))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "message": "Not Found",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/git/ref/heads/main",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/heads/main",
            "object": { "sha": "abc123def456", "type": "commit" },
        })))
        .mount(&server)
        .await;
    // ...and creating it fails (GitHub hiccup / access revoked mid-flight).
    Mock::given(method("POST"))
        .and(path(
            "/repos/example-org/regelrecht-corpus-example/git/refs",
        ))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "message": "Internal Server Error",
        })))
        .mount(&server)
        .await;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            owner.id,
            "user-token",
        ))
        .unwrap(),
    );

    let err = trajects::create(
        State(state.clone()),
        Extension(owner.clone()),
        headers,
        Json(custom_repo_req("Repo traject")),
    )
    .await
    .expect_err("a failed branch-create must fail the whole create");
    assert_eq!(err.0, StatusCode::BAD_GATEWAY);
    // Dutch, names the branch, and says the traject was not created — an
    // operator must not have to read the logs to understand this.
    assert!(
        err.1.contains("traject-branch") && err.1.contains("niet aangemaakt"),
        "expected a Dutch branch-create failure message, got: {}",
        err.1
    );

    // Nothing was persisted: no traject row, so no half-broken traject.
    let trajects_left: i64 = sqlx::query_scalar("SELECT count(*) FROM trajects")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        trajects_left, 0,
        "a failed branch-create must leave no traject behind"
    );
}

// ---------------------------------------------------------------------------
// `list`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_returns_only_trajects_caller_is_a_member_of() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;

    let t_alice = create_traject(&state, &alice, "Alice traject").await;
    let _t_bob = create_traject(&state, &bob, "Bob traject").await;

    let Json(list) = trajects::list(State(state.clone()), Extension(alice.clone()))
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, t_alice);
}

// ---------------------------------------------------------------------------
// `get`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_returns_403_for_non_member() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;

    let traject_id = create_traject(&state, &alice, "Alice traject").await;

    let err = trajects::get(State(state), Extension(bob), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn get_returns_detail_for_member() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;

    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let Json(detail) = trajects::get(State(state), Extension(alice.clone()), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(detail.summary.id, traject_id);
    assert_eq!(detail.summary.role, "owner");
    assert_eq!(detail.members.len(), 1);
    assert_eq!(detail.members[0].account_id, alice.id);
    // Only the writable-own source is present because the empty CorpusState
    // had nothing to seed from.
    assert_eq!(detail.sources.len(), 1);
    assert!(detail.sources[0].is_writable_own);
}

// ---------------------------------------------------------------------------
// `update`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_rejects_contributor_and_accepts_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );

    let body = UpdateTrajectRequest {
        name: Some("Renamed".to_string()),
        description: None,
        scope: None,
        status: None,
        repo_path: None,
    };

    // contributor → 403
    let err = trajects::update(
        State(state.clone()),
        Extension(bob),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: Some("Hacked".to_string()),
            description: None,
            scope: None,
            status: None,
            repo_path: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::FORBIDDEN);

    // owner → 204
    let ok = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(body),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let (name,): (String,) = sqlx::query_as("SELECT name FROM trajects WHERE id = $1")
        .bind(traject_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(name, "Renamed");
}

#[tokio::test]
async fn update_validates_status_enum() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::update(
        State(state),
        Extension(alice),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: None,
            description: None,
            scope: None,
            status: Some("klaar".to_string()),
            repo_path: None,
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// `update` — root path (`gh_path`) of the traject's own source
// ---------------------------------------------------------------------------

/// A traject whose writable-own source is a GitHub repo of the traject
/// owner's own choosing, inserted directly: going through `create` would
/// need a GitHub pre-flight (token + branch minting) that says nothing
/// about the behaviour under test. Fictional repo coordinates — this is a
/// public repository.
async fn own_repo_traject(pool: &PgPool, owner: &AccountRecord, repo_path: Option<&str>) -> Uuid {
    let (traject_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, description, scope, created_by)
         VALUES ('Eigen repo', '', '', $1) RETURNING id",
    )
    .bind(owner.id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(owner.id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 'example-org', 'regelrecht-corpus-example', 'traject/voorbeeld', 'main', $2,
                 0, 'example-org-regelrecht-corpus-example', TRUE)",
    )
    .bind(traject_id)
    .bind(repo_path)
    .execute(pool)
    .await
    .unwrap();
    traject_id
}

async fn stored_repo_path(pool: &PgPool, traject_id: Uuid) -> Option<String> {
    let (path,): (Option<String>,) = sqlx::query_as(
        "SELECT gh_path FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own",
    )
    .bind(traject_id)
    .fetch_one(pool)
    .await
    .unwrap();
    path
}

fn repo_path_req(repo_path: &str) -> UpdateTrajectRequest {
    UpdateTrajectRequest {
        name: None,
        description: None,
        scope: None,
        status: None,
        repo_path: Some(repo_path.to_string()),
    }
}

#[tokio::test]
async fn update_sets_own_repo_path_and_reports_it_back() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = own_repo_traject(&db.pool, &alice, None).await;

    let ok = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(repo_path_req("regulation/nl")),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);
    assert_eq!(
        stored_repo_path(&db.pool, traject_id).await.as_deref(),
        Some("regulation/nl")
    );

    // The detail endpoint is where the settings pane reads the value back.
    let Json(detail) = trajects::get(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
    )
    .await
    .unwrap();
    let own = detail.sources.iter().find(|s| s.is_writable_own).unwrap();
    assert_eq!(own.gh_path.as_deref(), Some("regulation/nl"));

    // Omitting the field leaves the path alone (COALESCE semantics, same
    // as the metadata fields).
    trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: Some("Andere naam".to_string()),
            description: None,
            scope: None,
            status: None,
            repo_path: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        stored_repo_path(&db.pool, traject_id).await.as_deref(),
        Some("regulation/nl"),
        "a PATCH without repo_path must not touch the path"
    );
}

#[tokio::test]
async fn update_stores_blank_repo_path_as_null() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = own_repo_traject(&db.pool, &alice, Some("regulation/nl")).await;

    // Whitespace-only means "back to repo root". Stored as NULL, not "",
    // so the row never carries two spellings of the same thing.
    for blank in ["", "   "] {
        trajects::update(
            State(state.clone()),
            Extension(alice.clone()),
            Path(traject_id),
            Json(repo_path_req(blank)),
        )
        .await
        .unwrap();
        assert_eq!(
            stored_repo_path(&db.pool, traject_id).await,
            None,
            "expected NULL for {blank:?}"
        );
    }
}

#[tokio::test]
async fn update_rejects_invalid_repo_path() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = own_repo_traject(&db.pool, &alice, Some("regulation/nl")).await;

    for bad in ["../etc", "/etc", "regulation/../..", "met spatie"] {
        let err = trajects::update(
            State(state.clone()),
            Extension(alice.clone()),
            Path(traject_id),
            Json(repo_path_req(bad)),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST, "expected 400 for {bad:?}");
        assert!(
            err.1.contains("relatief pad"),
            "the UI shows this verbatim, got {:?}",
            err.1
        );
    }
    assert_eq!(
        stored_repo_path(&db.pool, traject_id).await.as_deref(),
        Some("regulation/nl"),
        "a rejected path must leave the stored one untouched"
    );
}

#[tokio::test]
async fn update_repo_path_is_owner_only() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = own_repo_traject(&db.pool, &alice, None).await;
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'contributor')",
    )
    .bind(traject_id)
    .bind(bob.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let err = trajects::update(
        State(state.clone()),
        Extension(bob),
        Path(traject_id),
        Json(repo_path_req("elders")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::FORBIDDEN);
    assert_eq!(stored_repo_path(&db.pool, traject_id).await, None);
}

#[tokio::test]
async fn update_refuses_to_move_the_central_corpus_root() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    // `create_traject` takes the default writable source: the central
    // corpus, shared by every traject that skips the repo fields. Its
    // layout is not one traject's to move.
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(repo_path_req("ergens/anders")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("centrale corpus"),
        "the message must explain why, got {:?}",
        err.1
    );
    assert_eq!(
        stored_repo_path(&db.pool, traject_id).await.as_deref(),
        Some("regulation/nl"),
        "the central path stays put"
    );
}

/// A traject whose own source is a local directory: no repo, so no
/// `gh_path` to move. Only reachable through a hand-made row (every
/// source `create` writes is GitHub-backed), which is exactly why the
/// guard exists — and why it deserves a test of its own.
async fn local_own_traject(pool: &PgPool, owner: &AccountRecord) -> Uuid {
    let (traject_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, description, scope, created_by)
         VALUES ('Lokale bron', '', '', $1) RETURNING id",
    )
    .bind(owner.id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(owner.id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, is_writable_own)
         VALUES ($1, 'traject-own-local', 'Lokale bron', 'local'::corpus_source_type,
                 '/corpus/regulation/nl', 0, TRUE)",
    )
    .bind(traject_id)
    .execute(pool)
    .await
    .unwrap();
    traject_id
}

#[tokio::test]
async fn update_refuses_a_repo_path_on_a_non_github_own_source() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = local_own_traject(&db.pool, &alice).await;

    let err = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(repo_path_req("regulation/nl")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("geen GitHub-repo"),
        "the message must say why there is no path to move, got {:?}",
        err.1
    );
    assert_eq!(
        stored_repo_path(&db.pool, traject_id).await,
        None,
        "a local source keeps its NULL gh_path"
    );
}

#[tokio::test]
async fn update_refuses_a_repo_path_without_an_own_source() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    // A traject without a writable-own row: nothing of this traject's to
    // move, so the PATCH is refused instead of silently doing nothing.
    let (traject_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, description, scope, created_by)
         VALUES ('Zonder eigen bron', '', '', $1) RETURNING id",
    )
    .bind(alice.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(alice.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let err = trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(repo_path_req("regulation/nl")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("geen eigen bron"),
        "the message must explain why, got {:?}",
        err.1
    );

    // The metadata of a traject without an own source is still editable:
    // the refusal is about the path, not about the whole PATCH.
    trajects::update(
        State(state.clone()),
        Extension(alice.clone()),
        Path(traject_id),
        Json(UpdateTrajectRequest {
            name: Some("Nieuwe naam".to_string()),
            description: None,
            scope: None,
            status: None,
            repo_path: None,
        }),
    )
    .await
    .unwrap();
}

// ---------------------------------------------------------------------------
// `move_repo`
// ---------------------------------------------------------------------------

/// A session without a login: what a non-admin request carries.
fn anonymous_session() -> tower_sessions::Session {
    tower_sessions::Session::new(
        None,
        Arc::new(tower_sessions_memory_store::MemoryStore::default()),
        None,
    )
}

/// A logged-in session holding `editor-admin`.
async fn admin_session() -> tower_sessions::Session {
    let session = anonymous_session();
    session
        .insert(regelrecht_auth::SESSION_KEY_AUTHENTICATED, true)
        .await
        .unwrap();
    session
        .insert(
            regelrecht_auth::SESSION_KEY_ROLES,
            vec!["editor-admin".to_string()],
        )
        .await
        .unwrap();
    session
}

fn move_req(owner: &str, repo: &str) -> MoveRepoRequest {
    MoveRepoRequest {
        repo_owner: owner.to_string(),
        repo_name: repo.to_string(),
    }
}

/// `(gh_owner, gh_repo, auth_ref, name)` of the writable-own source.
async fn stored_repo(pool: &PgPool, traject_id: Uuid) -> (String, String, String, String) {
    sqlx::query_as(
        "SELECT gh_owner, gh_repo, auth_ref, name FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own",
    )
    .bind(traject_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn untouched_repo() -> (String, String, String, String) {
    (
        "example-org".to_string(),
        "regelrecht-corpus-example".to_string(),
        "example-org-regelrecht-corpus-example".to_string(),
        "Eigen repo".to_string(),
    )
}

#[tokio::test]
async fn move_repo_is_refused_without_the_admin_role() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let carol = seed_account_record(&db.pool, "carol@test.local", "Carol").await;
    let traject_id = own_repo_traject(&db.pool, &alice, None).await;
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'contributor')",
    )
    .bind(traject_id)
    .bind(bob.id)
    .execute(&db.pool)
    .await
    .unwrap();

    // The owner included: which repo a traject writes to is an admin call.
    for who in [alice, bob, carol] {
        let err = trajects::move_repo(
            State(state.clone()),
            Extension(who),
            anonymous_session(),
            axum::http::HeaderMap::new(),
            Path(traject_id),
            Json(move_req("other-org", "regelrecht-corpus-example")),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }
    assert_eq!(stored_repo(&db.pool, traject_id).await, untouched_repo());
}

#[tokio::test]
async fn move_repo_lets_an_admin_in_who_is_not_a_member() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let admin = seed_account_record(&db.pool, "admin@test.local", "Admin").await;
    let traject_id = own_repo_traject(&db.pool, &alice, None).await;

    // Past the access check, the request is judged on its content: a bad
    // owner is a 400, not the 403 a non-member without the role gets.
    let err = trajects::move_repo(
        State(state.clone()),
        Extension(admin.clone()),
        admin_session().await,
        axum::http::HeaderMap::new(),
        Path(traject_id),
        Json(move_req("../etc", "regelrecht-corpus-example")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    // An admin asking for a traject that does not exist gets a 404.
    let err = trajects::move_repo(
        State(state.clone()),
        Extension(admin),
        admin_session().await,
        axum::http::HeaderMap::new(),
        Path(Uuid::new_v4()),
        Json(move_req("other-org", "regelrecht-corpus-example")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn move_repo_refuses_the_central_corpus() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::move_repo(
        State(state.clone()),
        Extension(alice),
        admin_session().await,
        axum::http::HeaderMap::new(),
        Path(traject_id),
        Json(move_req("other-org", "regelrecht-corpus-example")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("centrale corpus"),
        "the message must explain why, got {:?}",
        err.1
    );
}

#[tokio::test]
async fn move_repo_refuses_to_point_a_traject_at_the_central_repo() {
    // The central repo is shared by every traject on it. A traject moved
    // onto it would not be recognised as central, so its layout guards
    // would not apply; refuse it whatever the casing.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = own_repo_traject(&db.pool, &alice, None).await;

    for (owner, repo) in [
        ("MinBZK", "regelrecht-corpus"),
        ("minbzk", "Regelrecht-Corpus"),
    ] {
        let err = trajects::move_repo(
            State(state.clone()),
            Extension(alice.clone()),
            admin_session().await,
            axum::http::HeaderMap::new(),
            Path(traject_id),
            Json(move_req(owner, repo)),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err.0,
            StatusCode::BAD_REQUEST,
            "expected 400 for {owner}/{repo}"
        );
    }
    assert_eq!(stored_repo(&db.pool, traject_id).await, untouched_repo());
}

#[tokio::test]
async fn move_repo_without_a_token_for_the_new_repo_changes_nothing() {
    // User-token mode with no linked account: the preflight refuses before
    // any GitHub call, and the row keeps pointing at the old repo.
    let db = TestDb::new().await;
    let mut state = empty_state(db.pool.clone());
    state.config = Arc::new(AppConfig {
        oidc: None,
        base_url: None,
        github_oauth: Some(GithubOAuth::for_tests(true)),
        task_enrich_provider: "claude".to_string(),
    });
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = own_repo_traject(&db.pool, &alice, Some("regulation/nl")).await;

    let err = trajects::move_repo(
        State(state.clone()),
        Extension(alice),
        admin_session().await,
        axum::http::HeaderMap::new(),
        Path(traject_id),
        Json(move_req("other-org", "regelrecht-corpus-example")),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(stored_repo(&db.pool, traject_id).await, untouched_repo());
}

#[tokio::test]
async fn move_repo_points_the_own_source_at_the_new_repo() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let db = TestDb::new().await;
    let server = MockServer::start().await;
    let mut oauth = GithubOAuth::for_tests(true);
    oauth.api_base = server.uri();
    let mut state = empty_state(db.pool.clone());
    state.config = Arc::new(AppConfig {
        oidc: None,
        base_url: None,
        github_oauth: Some(oauth.clone()),
        task_enrich_provider: "claude".to_string(),
    });
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = own_repo_traject(&db.pool, &alice, Some("regulation/nl")).await;

    // The preflight runs against the NEW coordinates, as the acting user.
    Mock::given(method("GET"))
        .and(path("/repos/other-org/regelrecht-corpus-example"))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": "main",
            "private": true,
            "permissions": { "push": true, "pull": true },
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/repos/other-org/regelrecht-corpus-example/branches/main",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "name": "main" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    // A transferred repo keeps its branches: the traject branch is there,
    // so nothing gets minted.
    Mock::given(method("GET"))
        .and(path(
            "/repos/other-org/regelrecht-corpus-example/git/ref/heads/traject/voorbeeld",
        ))
        .and(header("authorization", "Bearer user-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/heads/traject/voorbeeld",
            "object": { "sha": "abc123def456", "type": "commit" },
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            alice.id,
            "user-token",
        ))
        .unwrap(),
    );

    let status = trajects::move_repo(
        State(state.clone()),
        Extension(alice),
        admin_session().await,
        headers,
        Path(traject_id),
        Json(move_req(" other-org ", "regelrecht-corpus-example")),
    )
    .await
    .expect("a repo the user can push to must be accepted");
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(
        stored_repo(&db.pool, traject_id).await,
        (
            "other-org".to_string(),
            "regelrecht-corpus-example".to_string(),
            "other-org-regelrecht-corpus-example".to_string(),
            "other-org/regelrecht-corpus-example".to_string(),
        ),
    );
    // Branch, base branch and path are the traject's, not the repo's.
    let (branch, base, repo_path): (String, String, Option<String>) = sqlx::query_as(
        "SELECT gh_branch, gh_base_branch, gh_path FROM traject_corpus_sources
         WHERE traject_id = $1 AND is_writable_own",
    )
    .bind(traject_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(branch, "traject/voorbeeld");
    assert_eq!(base, "main");
    assert_eq!(repo_path.as_deref(), Some("regulation/nl"));
}

// ---------------------------------------------------------------------------
// `add_member` / `update_member` / `remove_member`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_member_with_unknown_email_creates_pending_invite() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let status = add_member(
        &state,
        &alice,
        traject_id,
        "ghost@test.local",
        "contributor",
    )
    .await;
    assert_eq!(status, Ok("pending"));

    let (email, role): (String, String) =
        sqlx::query_as("SELECT email, role::text FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(email, "ghost@test.local");
    assert_eq!(role, "contributor");

    // No traject_members row was created — only the inviter.
    let (member_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_members WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(member_count, 1);
}

#[tokio::test]
async fn add_member_normalizes_email_case_for_pending_invites() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "Mixed@Case.LOCAL",
            "contributor"
        )
        .await,
        Ok("pending")
    );
    // A re-invite using a different casing of the same email must
    // collide on the (traject_id, email) primary key.
    assert_eq!(
        add_member(&state, &alice, traject_id, "mixed@case.local", "owner").await,
        Ok("pending")
    );

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT email, role::text FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1, "case variants must collide");
    assert_eq!(rows[0].0, "mixed@case.local");
    assert_eq!(rows[0].1, "owner", "re-invite updates the role");
}

#[tokio::test]
async fn add_member_upserts_role_on_conflict() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "owner").await,
        Ok("active")
    );

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "owner");
}

#[tokio::test]
async fn add_member_blocks_demoting_last_owner_via_upsert() {
    // Without this guard, add_member would be a back-door around the
    // last-owner check that update_member already enforces.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let status = add_member(&state, &alice, traject_id, &alice.email, "contributor").await;
    assert_eq!(status, Err(StatusCode::CONFLICT));
}

#[tokio::test]
async fn update_member_blocks_demoting_last_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::update_member(
        State(state),
        Extension(alice.clone()),
        Path((traject_id, alice.id)),
        Json(UpdateMemberRequest {
            role: "contributor".to_string(),
        }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

#[tokio::test]
async fn remove_member_blocks_removing_last_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::remove_member(
        State(state),
        Extension(alice.clone()),
        Path((traject_id, alice.id)),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// `delete`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_is_owner_only_and_cascades() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );

    // contributor cannot delete
    let err = trajects::delete(State(state.clone()), Extension(bob), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);

    // owner can
    let ok = trajects::delete(State(state.clone()), Extension(alice), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let (members, sources, trajects_count): (i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT COUNT(*) FROM traject_members WHERE traject_id = $1),
           (SELECT COUNT(*) FROM traject_corpus_sources WHERE traject_id = $1),
           (SELECT COUNT(*) FROM trajects WHERE id = $1)",
    )
    .bind(traject_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(members, 0, "traject_members should cascade-delete");
    assert_eq!(sources, 0, "traject_corpus_sources should cascade-delete");
    assert_eq!(trajects_count, 0);
}

/// Een verwijderd traject mag geen open taken achterlaten: `tasks.traject_id`
/// is `ON DELETE SET NULL`, dus zonder de expliciete afhandeling in `delete`
/// blijft de taak staan, hangt hij in de account-brede takenlijst en wijst zijn
/// `payload.traject_ref` naar een traject dat niet meer bestaat.
#[tokio::test]
async fn delete_dismisses_open_tasks_and_clears_their_blobs() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let doomed = create_traject(&state, &alice, "Tarief").await;
    let survivor = create_traject(&state, &alice, "Toeslag").await;

    // Eén job met twee review-taken (het per-artikel-patroon) plus een blob,
    // en een al afgehandelde taak op een eigen job.
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_wet"))
        .await
        .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Result,
        "corpus/regulation/nl/wet/test_wet/2025-01-01.yaml",
        "$id: test_wet\n",
    )
    .await
    .unwrap();
    let open_a = seed_task(&db.pool, &alice, doomed, Some(job.id), "1").await;
    let open_b = seed_task(&db.pool, &alice, doomed, Some(job.id), "2").await;

    let done_job =
        job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_wet"))
            .await
            .unwrap();
    let done = seed_task(&db.pool, &alice, doomed, Some(done_job.id), "3").await;
    let done = tasks::resolve_task(&db.pool, done.id, alice.id, TaskStatus::Approved)
        .await
        .unwrap()
        .expect("assignee mag resolven");

    // Een open taak van een ánder traject blijft er buiten.
    let elsewhere = seed_task(&db.pool, &alice, survivor, None, "1").await;

    assert_eq!(
        trajects::delete(State(state.clone()), Extension(alice.clone()), Path(doomed))
            .await
            .unwrap(),
        StatusCode::NO_CONTENT
    );

    for id in [open_a.id, open_b.id] {
        let (status, resolved_at, resolved_by): (TaskStatus, Option<i64>, Option<Uuid>) =
            task_state(&db.pool, id).await;
        assert_eq!(
            status,
            TaskStatus::Dismissed,
            "open taak moet gesloten zijn"
        );
        assert!(resolved_at.is_some(), "resolved_at moet gestempeld zijn");
        assert_eq!(
            resolved_by, None,
            "niemand heeft de taak beoordeeld; het traject verdween eronder"
        );
    }

    let (status, _, resolved_by) = task_state(&db.pool, done.id).await;
    assert_eq!(
        status,
        TaskStatus::Approved,
        "afgehandelde taak moet onaangeroerd blijven"
    );
    assert_eq!(resolved_by, Some(alice.id));

    let (status, _, _) = task_state(&db.pool, elsewhere.id).await;
    assert_eq!(
        status,
        TaskStatus::Open,
        "taak van een ander traject blijft open"
    );

    let blobs = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert!(
        blobs.is_empty(),
        "result-blobs van de gesloten taken moeten weg zijn"
    );
}

/// Maak een open `job_review`-taak voor `traject_id`, met dezelfde
/// payload-vorm als de worker die aanmaakt.
async fn seed_task(
    pool: &PgPool,
    assignee: &AccountRecord,
    traject_id: Uuid,
    job_id: Option<Uuid>,
    article: &str,
) -> Task {
    tasks::create_task(
        pool,
        NewTask {
            task_type: TaskType::JobReview,
            assignee_account_id: Some(assignee.id),
            traject_id: Some(traject_id),
            job_id,
            title: format!("Verrijking beoordelen: test_wet artikel {article}"),
            payload: Some(serde_json::json!({
                "law_id": "test_wet",
                "article": article,
            })),
        },
    )
    .await
    .unwrap()
}

/// Status + audit-velden van één taak, ongeacht assignee: de store-helpers
/// filteren op account, hier willen we de rauwe rij zien. `resolved_at` komt
/// als unix-seconde terug — de test kijkt alleen of hij gezet is.
async fn task_state(pool: &PgPool, task_id: Uuid) -> (TaskStatus, Option<i64>, Option<Uuid>) {
    let row: (
        TaskStatus,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<Uuid>,
    ) = sqlx::query_as("SELECT status, resolved_at, resolved_by FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (row.0, row.1.map(|t| t.timestamp()), row.2)
}

// ---------------------------------------------------------------------------
// `leave`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn leave_blocks_last_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    let err = trajects::leave(State(state), Extension(alice), Path(traject_id))
        .await
        .unwrap_err();
    assert_eq!(err, StatusCode::CONFLICT);
}

#[tokio::test]
async fn leave_allows_contributor() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    assert_eq!(
        add_member(&state, &alice, traject_id, &bob.email, "contributor").await,
        Ok("active")
    );

    let ok = trajects::leave(
        State(state.clone()),
        Extension(bob.clone()),
        Path(traject_id),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Pending invites + promotion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pending_invite_is_promoted_to_membership_on_login() {
    // End-to-end: owner invites an unknown email, the invitee later logs
    // in (their account is upserted), and the next time
    // `account_middleware` runs the invite turns into a real membership.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "Ghost@Test.Local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    // Simulate the invitee logging in: a new accounts row is created.
    let ghost = seed_account_record(&db.pool, "ghost@test.local", "Ghost").await;

    // Simulate `account_middleware` post-upsert step.
    regelrecht_editor_api::accounts::promote_pending_invites(&db.pool, ghost.id, &ghost.email)
        .await;

    let (invite_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invite_count, 0);

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(ghost.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "contributor");

    let Json(list) = trajects::list(State(state.clone()), Extension(ghost.clone()))
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, traject_id);
    assert_eq!(list[0].role, "contributor");
}

#[tokio::test]
async fn promotion_is_idempotent_when_user_is_already_a_member() {
    // Defence-in-depth: even if an invite somehow co-exists with a
    // membership for the same email + traject, promotion must not error
    // and the invite must be cleaned up. Bob's existing role wins.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    add_member(&state, &alice, traject_id, &bob.email, "contributor")
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO traject_invites (traject_id, email, role, invited_by)
         VALUES ($1, $2, 'owner', $3)",
    )
    .bind(traject_id)
    .bind(&bob.email)
    .bind(alice.id)
    .execute(&db.pool)
    .await
    .unwrap();

    regelrecht_editor_api::accounts::promote_pending_invites(&db.pool, bob.id, &bob.email).await;

    let (invite_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_invites WHERE email = $1")
            .bind(&bob.email)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invite_count, 0);

    let (role,): (String,) = sqlx::query_as(
        "SELECT role::text FROM traject_members WHERE traject_id = $1 AND account_id = $2",
    )
    .bind(traject_id)
    .bind(bob.id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(role, "contributor");
}

#[tokio::test]
async fn get_returns_pending_invites_alongside_members() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    let Json(detail) = trajects::get(State(state), Extension(alice), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(detail.members.len(), 1, "alice is still the only member");
    assert_eq!(detail.pending_invites.len(), 1);
    assert_eq!(detail.pending_invites[0].email, "ghost@test.local");
    assert_eq!(detail.pending_invites[0].role, "contributor");
}

#[tokio::test]
async fn remove_invite_deletes_and_is_idempotent_404() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    let ok = trajects::remove_invite(
        State(state.clone()),
        Extension(alice.clone()),
        Path((traject_id, "ghost@test.local".to_string())),
    )
    .await
    .unwrap();
    assert_eq!(ok, StatusCode::NO_CONTENT);

    let err = trajects::remove_invite(
        State(state),
        Extension(alice),
        Path((traject_id, "ghost@test.local".to_string())),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn remove_invite_requires_owner() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    add_member(&state, &alice, traject_id, &bob.email, "contributor")
        .await
        .unwrap();
    add_member(
        &state,
        &alice,
        traject_id,
        "ghost@test.local",
        "contributor",
    )
    .await
    .unwrap();

    let err = trajects::remove_invite(
        State(state),
        Extension(bob),
        Path((traject_id, "ghost@test.local".to_string())),
    )
    .await
    .unwrap_err();
    assert_eq!(err, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn add_member_active_path_cleans_up_stale_invite() {
    // Race window: invite an unknown email → ghost's account gets
    // created later (e.g. they registered with the IdP but haven't
    // hit any API yet, so promote_pending_invites hasn't run) → owner
    // re-invites. The active path must clean up the stale invite so
    // GET doesn't return ghost in both members and pending_invites.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("pending")
    );

    // Ghost's account materialises (e.g. via OIDC login) but
    // promote_pending_invites hasn't run yet.
    let _ghost = seed_account_record(&db.pool, "ghost@test.local", "Ghost").await;

    // Owner re-invites — now the active path is taken.
    assert_eq!(
        add_member(
            &state,
            &alice,
            traject_id,
            "ghost@test.local",
            "contributor"
        )
        .await,
        Ok("active")
    );

    let (invite_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM traject_invites WHERE traject_id = $1")
            .bind(traject_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(invite_count, 0, "stale invite must be cleaned up");
}

#[tokio::test]
async fn get_hides_pending_invites_from_contributors() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let bob = seed_account_record(&db.pool, "bob@test.local", "Bob").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;
    add_member(&state, &alice, traject_id, &bob.email, "contributor")
        .await
        .unwrap();
    add_member(
        &state,
        &alice,
        traject_id,
        "ghost@test.local",
        "contributor",
    )
    .await
    .unwrap();

    // Alice (owner) sees the pending invite.
    let Json(owner_view) = trajects::get(State(state.clone()), Extension(alice), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(owner_view.pending_invites.len(), 1);

    // Bob (contributor) does not.
    let Json(contributor_view) = trajects::get(State(state), Extension(bob), Path(traject_id))
        .await
        .unwrap();
    assert_eq!(contributor_view.pending_invites.len(), 0);
    assert_eq!(
        contributor_view.members.len(),
        2,
        "members list itself still visible to contributors"
    );
}

#[tokio::test]
async fn add_member_rejects_malformed_emails() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let alice = seed_account_record(&db.pool, "alice@test.local", "Alice").await;
    let traject_id = create_traject(&state, &alice, "Tarief").await;

    for bad in [
        "notanemail",
        "@nolocal.com",
        "nodomain@",
        "no-dot-in-domain@localhost",
        "two@@signs.com",
        "   ",
    ] {
        assert_eq!(
            add_member(&state, &alice, traject_id, bad, "contributor").await,
            Err(StatusCode::BAD_REQUEST),
            "expected 400 for {bad:?}",
        );
    }
}
