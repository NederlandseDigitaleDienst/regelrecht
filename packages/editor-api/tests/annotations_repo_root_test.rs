//! Notes on a GitHub-backed traject repo whose source has a subpath
//! (`gh_path`): RFC-018 keeps the sidecar at
//! `annotations/{law_id}/annotations.yaml` at the **repository root**, not
//! under the subpath. Notes an earlier editor version saved under the
//! subpath stay readable, and the next save builds on them but writes the
//! merged result to the repository root.
//!
//! GitHub is played by wiremock; `GITHUB_API_BASE` (the test seam in
//! `GithubClient::new`) points every client at it. Everything runs in ONE
//! test so that process-wide env var cannot race between parallel tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    add_central_seed, insert_traject, seed_account_record, session_for, state_with_user_token_mode,
    traject_ref, TEST_EMAIL, TEST_NAME,
};

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use base64::Engine;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_corpus::annotation_schema::{append_notes_to_sidecar, AppendOutcome};
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{get_traject_annotations, save_annotations};
use regelrecht_editor_api::github_oauth::{self, GithubOAuth};
use regelrecht_editor_api::state::AppState;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_voorbeeld_notities";
const OWN_REPO: &str = "example-org/regelrecht-corpus-example";
const OWN_BRANCH: &str = "traject-voorbeeld";
const OWN_SUBPATH: &str = "regulation/nl";
/// Same schema the handler stamps on a fresh sidecar.
const SCHEMA_URL: &str = "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.3/annotation-schema.json";

fn root_sidecar() -> String {
    format!("/repos/{OWN_REPO}/contents/annotations/{LAW_ID}/annotations.yaml")
}

fn legacy_sidecar() -> String {
    format!("/repos/{OWN_REPO}/contents/{OWN_SUBPATH}/annotations/{LAW_ID}/annotations.yaml")
}

/// A GitHub writable-own source with a subpath (no service token, so every
/// call rides the user's token) plus a local read seed that holds the law.
async fn seeded_traject(pool: &PgPool, owner_id: Uuid, seed_dir: &std::path::Path) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type,
          gh_owner, gh_repo, gh_branch, gh_base_branch, gh_path,
          priority, auth_ref, is_writable_own)
         VALUES ($1, 'traject-own-test', 'Eigen repo', 'github'::corpus_source_type,
                 'example-org', 'regelrecht-corpus-example', $2, 'main', $3,
                 0, 'example-unset-token-ref', TRUE)",
    )
    .bind(traject_id)
    .bind(OWN_BRANCH)
    .bind(OWN_SUBPATH)
    .execute(pool)
    .await
    .unwrap();
    add_central_seed(pool, traject_id, seed_dir).await;
    traject_id
}

fn note(exact: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "Annotation",
        "motivation": "commenting",
        "creator": "tester",
        "target": {
            "source": format!("regelrecht://{LAW_ID}"),
            "selector": { "type": "TextQuoteSelector", "exact": exact }
        },
        "body": { "type": "TextualBody", "value": "een toelichting", "purpose": "commenting" }
    })
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

/// Branch exists, repo readable, sidecar only at the legacy location.
async fn mount_repo(server: &MockServer, legacy_text: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWN_REPO}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "full_name": OWN_REPO,
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWN_REPO}/git/ref/heads/{OWN_BRANCH}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "ref": format!("refs/heads/{OWN_BRANCH}"),
            "object": { "sha": "branch-sha" },
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(root_sidecar()))
        .respond_with(ResponseTemplate::new(404))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(legacy_sidecar()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "annotations.yaml",
            "path": format!("{OWN_SUBPATH}/annotations/{LAW_ID}/annotations.yaml"),
            "sha": "legacy-sha",
            "type": "file",
            "content": b64(legacy_text),
            "encoding": "base64",
        })))
        .mount(server)
        .await;
    Mock::given(method("PUT"))
        .and(path(root_sidecar()))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "content": { "sha": "root-v1" },
        })))
        .expect(1)
        .mount(server)
        .await;
}

async fn read_notes(
    state: &AppState,
    account: &AccountRecord,
    tref: &str,
    headers: &HeaderMap,
) -> String {
    let (status, _, body) = get_traject_annotations(
        State(state.clone()),
        session_for(&account.person_sub).await,
        Extension(account.clone()),
        Path((tref.to_string(), LAW_ID.to_string())),
        headers.clone(),
    )
    .await
    .expect("annotations read must succeed");
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
async fn subpath_traject_reads_legacy_notes_and_saves_at_the_repo_root() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());

    let db = TestDb::new().await;
    let seed = tempfile::tempdir().unwrap();
    let law_dir = seed.path().join("wet").join(LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Voorbeeldwet\n"),
    )
    .unwrap();

    let oauth = GithubOAuth::for_tests(true);
    let state = state_with_user_token_mode(db.pool.clone(), oauth.clone());
    let account = seed_account_record(&db.pool, TEST_EMAIL, TEST_NAME).await;
    let tref = traject_ref(seeded_traject(&db.pool, account.id, seed.path()).await);

    let AppendOutcome::Write(legacy_text) =
        append_notes_to_sidecar(None, &[note("oude notitie")], SCHEMA_URL).unwrap()
    else {
        panic!("a fresh sidecar is always a write");
    };
    mount_repo(&server, &legacy_text).await;

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        axum::http::HeaderValue::from_str(&github_oauth::seal_token_cookie_for_tests(
            &oauth,
            account.id,
            "user-token",
        ))
        .unwrap(),
    );

    // Only the legacy file exists: the read serves it.
    assert_eq!(
        read_notes(&state, &account, &tref, &headers).await,
        legacy_text
    );

    // A save appends to the legacy notes and writes the result at the root.
    let saved = save_annotations(
        State(state.clone()),
        Extension(account.clone()),
        session_for(&account.person_sub).await,
        Path((tref.clone(), LAW_ID.to_string())),
        headers.clone(),
        serde_json::to_string(&[note("nieuwe notitie")]).unwrap(),
    )
    .await
    .expect("save must succeed");
    assert!(!saved.0.no_change, "the new note must be written");

    let requests = server.received_requests().await.unwrap();
    let writes: Vec<_> = requests
        .iter()
        .filter(|r| matches!(r.method.as_str(), "PUT" | "DELETE"))
        .collect();
    assert_eq!(writes.len(), 1, "exactly one write, at the root");
    assert_eq!(writes[0].method.as_str(), "PUT");
    assert_eq!(writes[0].url.path(), root_sidecar());
    let body: serde_json::Value = serde_json::from_slice(&writes[0].body).unwrap();
    assert_eq!(body["branch"], OWN_BRANCH);
    assert!(
        body.get("sha").is_none(),
        "the root write is a create, not an update of the legacy file: {body}"
    );
    let written = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(body["content"].as_str().unwrap().replace('\n', ""))
            .unwrap(),
    )
    .unwrap();
    assert!(
        written.starts_with(&legacy_text),
        "the legacy notes are kept verbatim as the append base:\n{written}"
    );
    assert!(written.contains("nieuwe notitie"));

    // The next read returns the merged document. It comes from the traject
    // sidecar cache the save filled (read-your-writes), not from GitHub;
    // the corpus tests cover the backend read of the root file.
    assert_eq!(read_notes(&state, &account, &tref, &headers).await, written);

    std::env::remove_var("GITHUB_API_BASE");
}
