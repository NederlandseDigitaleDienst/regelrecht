//! Shared setup for the editor-api integration tests: the `AppState` the
//! handlers run against, a seeded account, a traject with its owner, and a
//! session carrying a verified editor identity.
//!
//! Every test binary compiles this module on its own and uses a subset of it,
//! hence the `dead_code` allowance.

#![allow(dead_code, clippy::unwrap_used)]

use std::path::Path;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;
use uuid::Uuid;

use regelrecht_auth::handlers::{
    SESSION_KEY_EMAIL, SESSION_KEY_EMAIL_VERIFIED, SESSION_KEY_NAME, SESSION_KEY_SUB,
};
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::github_oauth::GithubOAuth;
use regelrecht_editor_api::state::{AppState, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

/// The name every seeded account and verified session carries.
pub const TEST_NAME: &str = "Test User";
/// The email `session_for` puts in the session.
pub const TEST_EMAIL: &str = "alice@test.local";

/// An `AppState` with an empty global corpus and no OIDC, pipeline or
/// harvest backends. `oauth` configures GitHub OAuth; its
/// `require_user_token` decides whether user-token mode is on.
pub fn app_state(pool: Option<PgPool>, oauth: Option<GithubOAuth>) -> AppState {
    AppState {
        corpus: Arc::new(RwLock::new(CorpusState::empty())),
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
            github_oauth: oauth,
            task_enrich_provider: "claude".to_string(),
        }),
        // regelrecht_auth::http_client(), not reqwest::Client::new(): editor-api
        // builds reqwest with `rustls-tls-webpki-roots-no-provider`, so rustls
        // picks no backend on its own. A test has no `main` that installs the
        // provider, so this helper does, or the first client panics with
        // "No provider set".
        http_client: regelrecht_auth::http_client(),
        pool,
        pipeline_api_url: None,
        harvest_admin_url: None,
        reload_lock: Arc::new(Mutex::new(())),
        integrity: Default::default(),
        trajects: Arc::new(TrajectCorpusCache::new()),
    }
}

/// `app_state` on a database, without GitHub OAuth.
pub fn empty_state(pool: PgPool) -> AppState {
    app_state(Some(pool), None)
}

/// `app_state` on a database, with GitHub OAuth configured. Pass
/// `GithubOAuth::for_tests(true)` for user-token mode.
pub fn state_with_user_token_mode(pool: PgPool, oauth: GithubOAuth) -> AppState {
    app_state(Some(pool), Some(oauth))
}

/// Insert an account named [`TEST_NAME`] with subject `sub-{email}`.
/// Returns its id and subject.
pub async fn seed_account(pool: &PgPool, email: &str) -> (Uuid, String) {
    let record = seed_account_record(pool, email, TEST_NAME).await;
    (record.id, record.person_sub)
}

/// Insert an account with subject `sub-{email}` and return it as the
/// handlers see it.
pub async fn seed_account_record(pool: &PgPool, email: &str, name: &str) -> AccountRecord {
    let sub = format!("sub-{email}");
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&sub)
    .bind(email)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();
    AccountRecord {
        id,
        person_sub: sub,
        email: email.to_string(),
        name: name.to_string(),
    }
}

/// Create a traject named `Test` with `owner_id` as its `owner`, so the
/// membership re-check on reads and writes passes. It has no corpus
/// sources yet. Returns the traject id.
pub async fn insert_traject(pool: &PgPool, owner_id: Uuid) -> Uuid {
    let (traject_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, description, scope, created_by)
         VALUES ('Test', '', '', $1) RETURNING id",
    )
    .bind(owner_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO traject_members (traject_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(traject_id)
    .bind(owner_id)
    .execute(pool)
    .await
    .unwrap();
    traject_id
}

/// `insert_traject` with one local writable-own source pointing at
/// `corpus_dir`. The source has no auth_ref: it writes back through its own
/// backend. Returns the traject id.
pub async fn local_traject(pool: &PgPool, owner_id: Uuid, corpus_dir: &Path) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'local', 'Local', 'local'::corpus_source_type, $2,
                 0, '[]'::jsonb, TRUE)",
    )
    .bind(traject_id)
    .bind(corpus_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();
    traject_id
}

/// Add a read-only local source at priority 2 that plays the central
/// corpus next to a traject's writable-own repo.
pub async fn add_central_seed(pool: &PgPool, traject_id: Uuid, seed_dir: &Path) {
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'central-seed', 'Centrale Corpus', 'local'::corpus_source_type, $2,
                 2, '[]'::jsonb, FALSE)",
    )
    .bind(traject_id)
    .bind(seed_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();
}

/// URL form of a traject reference, via the production helper. Every
/// traject [`insert_traject`] creates is named `Test`.
pub fn traject_ref(traject_id: Uuid) -> String {
    regelrecht_editor_api::trajects::traject_ref("Test", traject_id)
}

/// A session with a verified editor identity: the save handlers require
/// name, email and `email_verified=true` for commit attribution.
pub async fn verified_session(sub: &str, email: &str) -> Session {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    session.insert(SESSION_KEY_SUB, sub).await.unwrap();
    session.insert(SESSION_KEY_NAME, TEST_NAME).await.unwrap();
    session.insert(SESSION_KEY_EMAIL, email).await.unwrap();
    session
        .insert(SESSION_KEY_EMAIL_VERIFIED, true)
        .await
        .unwrap();
    session
}

/// `verified_session` with [`TEST_EMAIL`].
pub async fn session_for(sub: &str) -> Session {
    verified_session(sub, TEST_EMAIL).await
}

/// A GitHub Contents API answer for one file, base64-encoded as GitHub sends
/// it, for a wiremock stub of `/repos/{repo}/contents/{path}`.
pub fn contents_file_json(name: &str, api_path: &str, body: &str) -> serde_json::Value {
    use base64::Engine as _;
    serde_json::json!({
        "name": name,
        "path": api_path,
        "sha": format!("sha-{name}"),
        "type": "file",
        "content": base64::engine::general_purpose::STANDARD.encode(body),
        "encoding": "base64",
    })
}
