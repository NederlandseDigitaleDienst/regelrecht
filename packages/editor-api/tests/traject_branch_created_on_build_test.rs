//! A traject from before branches were cut at creation gets its branch
//! when the corpus is built: the backend check creates it. The index lists
//! the sources at the same time, so it may ask for the traject's own repo
//! before that branch exists. The build then lists once more, and the
//! traject's laws are there on the first request.
//!
//! GitHub is played by wiremock; the `GithubSettings` the test puts in the
//! app state point every client at it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    TEST_EMAIL, TEST_NAME, add_central_seed, app_state_with_github, github_settings,
    insert_traject, seed_account_record, session_for, traject_ref,
};

use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use pretty_assertions::assert_eq;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_corpus::dto::PaginationParams;
use regelrecht_editor_api::corpus_handlers::list_traject_corpus_laws;

use regelrecht_pipeline::test_utils::TestDb;

const OWN_REPO: &str = "example-org/example-traject-repo";
const OWN_BRANCH: &str = "traject-voorbeeld";
const LAW_ID: &str = "wet_op_de_traject_branch";
/// The server token for the traject repo, so the backend check runs and
/// creates the missing branch.
const OWN_TOKEN: (&str, &str) = ("CORPUS_AUTH_EXAMPLE_OWN_REF_TOKEN", "own-server-token");

#[tokio::test]
async fn own_repo_listed_before_its_branch_existed_is_listed_again() {
    let server = MockServer::start().await;
    let db = TestDb::new().await;
    let seed = tempfile::tempdir().unwrap();
    let state = app_state_with_github(
        Some(db.pool.clone()),
        None,
        github_settings(&server.uri(), &[OWN_TOKEN]),
    );
    let account = seed_account_record(&db.pool, TEST_EMAIL, TEST_NAME).await;
    let traject_id = insert_traject(&db.pool, account.id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 'example-org', 'example-traject-repo', $2, 'main', NULL,
                 0, 'example-own-ref', TRUE)",
    )
    .bind(traject_id)
    .bind(OWN_BRANCH)
    .execute(&db.pool)
    .await
    .unwrap();
    add_central_seed(&db.pool, traject_id, seed.path()).await;

    // The branch is missing, so the backend check creates it from `main`.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/git/ref/heads/main")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": "refs/heads/main",
            "object": { "sha": "main-sha" },
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/repos/{OWN_REPO}/git/refs")))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;
    // The first listing comes before the branch exists: no commit for the
    // ref, and the fallback listing by ref 404s (wiremock's default). After
    // that the branch resolves.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/commits/{OWN_BRANCH}")))
        .respond_with(ResponseTemplate::new(422))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/commits/{OWN_BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_string("main-sha"))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}/git/trees/main-sha")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-sha",
            "truncated": false,
            "tree": [
                { "path": format!("wet/{LAW_ID}/2025-01-01.yaml"), "type": "blob", "sha": "blob-1" },
            ],
        })))
        .mount(&server)
        .await;

    let laws = list_traject_corpus_laws(
        State(state),
        Extension(account.clone()),
        session_for(&account.person_sub).await,
        Path(traject_ref(traject_id)),
        Query(PaginationParams {
            offset: 0,
            limit: None,
            q: None,
            ids: Some(LAW_ID.to_string()),
            source: None,
            enriched: false,
            loaded: false,
        }),
        HeaderMap::new(),
    )
    .await
    .expect("the traject's own laws are listed on the first request");
    assert_eq!(laws.0.len(), 1, "{:?}", laws.0);
    assert_eq!(laws.0[0].source_id, "traject-own-test");
}
