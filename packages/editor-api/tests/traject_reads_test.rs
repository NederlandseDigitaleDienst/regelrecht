//! Integration tests for the traject-aware read path
//! (`corpus_handlers::get_corpus_law` and friends) and the optimistic
//! concurrency (`ETag`/`If-Match`) on law and scenario saves.
//!
//! These tests exist to pin the cross-traject read isolation introduced
//! alongside `GitHubApiBackend`, and the 412 semantics that stop two
//! traject members from silently overwriting each other's law edits.
//! Each test uses local writable-own sources so the runs are hermetic —
//! no GitHub, no network — and the handlers are invoked directly with
//! inline `axum` extractors, mirroring the pattern in
//! `save_annotations_test.rs` and `trajects_test.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    app_state, empty_state, insert_traject, local_traject, seed_account, session_for,
    state_with_user_token_mode, traject_ref,
};

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{
    get_corpus_law, get_traject_corpus_law, get_traject_scenario, list_traject_corpus_laws,
    list_traject_scenarios, save_law, save_scenario,
};
use regelrecht_editor_api::github_oauth::GithubOAuth;
use regelrecht_editor_api::state::AppState;
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_op_de_zorgtoeslag";

/// Write a minimal but schema-valid law file under `corpus_dir/wet/{law_id}/2025-01-01.yaml`
/// with a single distinguishing `name:` so we can verify which copy a
/// read served back.
fn write_law(corpus_dir: &std::path::Path, name_marker: &str) {
    let law_dir = corpus_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: {name_marker}\n"),
    )
    .unwrap();
}

/// Write a scenario file next to the law (the path `save_scenario`
/// writes to / `get_traject_scenario` reads from).
fn write_scenario(corpus_dir: &std::path::Path, filename: &str, content: &str) {
    let dir = corpus_dir.join("wet").join(LAW_ID).join("scenarios");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(filename), content).unwrap();
}

fn if_match_headers(etag: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::IF_MATCH,
        HeaderValue::from_str(etag).unwrap(),
    );
    headers
}

/// Helper: call the traject-scoped law GET and return `(etag, body)`.
async fn read_law(state: AppState, session: Session, tref: &str) -> (String, String) {
    let (status, headers, body) = get_traject_corpus_law(
        State(state),
        Extension(test_account()),
        session,
        Path((tref.to_string(), LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect("get_traject_corpus_law must succeed");
    assert_eq!(status, StatusCode::OK);
    let etag = headers
        .iter()
        .find(|(name, _)| name == axum::http::header::ETAG)
        .map(|(_, value)| value.clone())
        .expect("law GET must carry an ETag header");
    (etag, body)
}

/// A throwaway account for direct handler calls. These tests either build
/// `AppConfig` with `github_oauth: None` (user-OAuth disabled) or write to
/// a local backend that never demands a user token, so the write path
/// never resolves a token for `account.id` — any value is fine.
fn test_account() -> AccountRecord {
    AccountRecord {
        id: Uuid::new_v4(),
        person_sub: "test-sub".to_string(),
        email: "test@example.gov".to_string(),
        name: "Test User".to_string(),
    }
}

/// Helper: call `save_law` and return the response (status + new ETag).
async fn save_law_with(
    state: AppState,
    session: Session,
    tref: &str,
    headers: HeaderMap,
    body: String,
) -> Result<(StatusCode, Option<String>), (StatusCode, String)> {
    let response = save_law(
        State(state),
        Extension(test_account()),
        session,
        Path((tref.to_string(), LAW_ID.to_string())),
        headers,
        body,
    )
    .await?;
    let etag = response
        .headers()
        .get(axum::http::header::ETAG)
        .map(|v| v.to_str().unwrap().to_string());
    Ok((response.status(), etag))
}

/// Helper: call the traject-scoped scenario GET and return `(etag, body)`.
async fn read_scenario(
    state: AppState,
    session: Session,
    tref: &str,
    filename: &str,
) -> (String, String) {
    let (status, headers, body) = get_traject_scenario(
        State(state),
        Extension(test_account()),
        session,
        Path((tref.to_string(), LAW_ID.to_string(), filename.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect("scenario GET must succeed");
    assert_eq!(status, StatusCode::OK);
    let etag = headers
        .iter()
        .find(|(name, _)| name == axum::http::header::ETAG)
        .map(|(_, value)| value.clone())
        .expect("scenario GET must carry an ETag header");
    (etag, body)
}

/// Helper: call `save_scenario` and return the response (status + new ETag).
async fn save_scenario_with(
    state: AppState,
    session: Session,
    tref: &str,
    filename: &str,
    headers: HeaderMap,
    body: &str,
) -> Result<(StatusCode, Option<String>), (StatusCode, String)> {
    let response = save_scenario(
        State(state),
        Extension(test_account()),
        session,
        Path((tref.to_string(), LAW_ID.to_string(), filename.to_string())),
        headers,
        body.to_string(),
    )
    .await?;
    let etag = response
        .headers()
        .get(axum::http::header::ETAG)
        .map(|v| v.to_str().unwrap().to_string());
    Ok((response.status(), etag))
}

// ---------------------------------------------------------------------------
// Read-isolation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_corpus_law_serves_the_trajects_content() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    // Two trajects, each owning a different corpus copy with the same
    // law id but distinguishable `name:` field.
    let corpus_a = tempfile::tempdir().unwrap();
    write_law(corpus_a.path(), "TRAJECT_A");
    let traject_a = local_traject(&db.pool, owner, corpus_a.path()).await;

    let corpus_b = tempfile::tempdir().unwrap();
    write_law(corpus_b.path(), "TRAJECT_B");
    let traject_b = local_traject(&db.pool, owner, corpus_b.path()).await;

    // Reading through traject A's URL returns A's copy.
    let (_etag_a, body_a) = read_law(
        state.clone(),
        session_for(&sub).await,
        &traject_ref(traject_a),
    )
    .await;
    assert!(
        body_a.contains("TRAJECT_A"),
        "expected A's content, got: {body_a}"
    );
    assert!(!body_a.contains("TRAJECT_B"));

    // Reading through traject B's URL returns B's copy.
    let (_etag_b, body_b) = read_law(
        state.clone(),
        session_for(&sub).await,
        &traject_ref(traject_b),
    )
    .await;
    assert!(
        body_b.contains("TRAJECT_B"),
        "expected B's content, got: {body_b}"
    );
    assert!(!body_b.contains("TRAJECT_A"));
}

#[tokio::test]
async fn save_then_read_returns_the_just_saved_content() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "ORIGINAL");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let tref = traject_ref(traject_id);

    // Verify the pre-save read returns the on-disk content.
    let (_etag, before) = read_law(state.clone(), session_for(&sub).await, &tref).await;
    assert!(before.contains("ORIGINAL"));

    // Save a new body via the actual handler — WITHOUT `If-Match`, so
    // this also pins the backward-compatible permissive save (older
    // clients that never send the precondition keep working).
    let new_body = format!("$id: {LAW_ID}\nname: UPDATED\n");
    let (status, _etag) = save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        HeaderMap::new(),
        new_body,
    )
    .await
    .expect("save without If-Match should succeed");
    assert_eq!(status, StatusCode::OK);

    // The next read must reflect the save — overlay populated by
    // `save_law` short-circuits the source_map snapshot.
    let (_etag, after) = read_law(state, session_for(&sub).await, &tref).await;
    assert!(
        after.contains("UPDATED"),
        "expected the saved content; got: {after}"
    );
    assert!(!after.contains("ORIGINAL"));
}

#[tokio::test]
async fn ttl_refresh_picks_up_upstream_laws_and_reconciles_saves() {
    use axum::extract::Query;
    use regelrecht_corpus::dto::PaginationParams;

    let db = TestDb::new().await;
    let mut state = empty_state(db.pool.clone());
    // Zero TTL: every request rolls the index snapshot over — the
    // production refresh behaviour, accelerated so the test doesn't wait
    // out the real TTL.
    state.trajects = Arc::new(TrajectCorpusCache::with_index_ttl(
        std::time::Duration::ZERO,
    ));
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "ORIGINAL");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let tref = traject_ref(traject_id);

    // First read builds the traject corpus.
    let (_etag, before) = read_law(state.clone(), session_for(&sub).await, &tref).await;
    assert!(before.contains("ORIGINAL"));

    // Save through the editor — the overlay records the new body.
    let saved_body = format!("$id: {LAW_ID}\nname: SAVED\n");
    let (status, _etag) = save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        HeaderMap::new(),
        saved_body,
    )
    .await
    .expect("save must succeed");
    assert_eq!(status, StatusCode::OK);

    // A fresh local save survives a refresh: the branch still holds
    // exactly the saved body, so the reconcile pass keeps the overlay
    // entry and reads keep returning the save (read-your-writes).
    let (_etag, after_save) = read_law(state.clone(), session_for(&sub).await, &tref).await;
    assert!(
        after_save.contains("SAVED"),
        "a fresh save must survive the TTL refresh; got: {after_save}"
    );

    // Upstream changes behind the snapshot's back: a brand-new law lands
    // in the source (think: merged on the central corpus, a re-harvest)
    // and the just-saved law's file is overwritten by an external writer
    // (another replica, a direct push to the branch).
    let other_dir = corpus.path().join("wet").join("andere_wet");
    std::fs::create_dir_all(&other_dir).unwrap();
    std::fs::write(
        other_dir.join("2025-01-01.yaml"),
        "$id: andere_wet\nname: Nieuw\n",
    )
    .unwrap();
    write_law(corpus.path(), "EXTERNAL");

    // A next request refreshes the snapshot (TTL expired) and the new law
    // is indexed without a traject delete/recreate or process restart.
    //
    // The refresh is stale-while-revalidate: a request past the TTL serves
    // the stale snapshot and kicks the re-enumeration off in a *background*
    // task (see `spawn_background_refresh`), so no single request is
    // guaranteed to observe the post-write snapshot — in particular, a
    // refresh spawned by an earlier request may have enumerated the sources
    // just *before* the writes above landed. Poll (bounded) until a request
    // is served the refreshed snapshot; asserting on one shot made this
    // test scheduling-dependent and flaky.
    let mut indexed = false;
    for _ in 0..50 {
        let Json(entries) = list_traject_corpus_laws(
            State(state.clone()),
            Extension(test_account()),
            session_for(&sub).await,
            Path(tref.clone()),
            Query(PaginationParams {
                offset: 0,
                limit: None,
                q: None,
                ids: None,
                source: None,
                enriched: false,
            }),
            HeaderMap::new(),
        )
        .await
        .expect("law list must succeed");
        if entries.iter().any(|e| e.law_id == "andere_wet") {
            indexed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        indexed,
        "refreshed snapshot must index the upstream-added law"
    );

    // …and the externally overwritten law CONVERGES to the branch
    // content: the refresh's reconcile pass sees the branch no longer
    // matches the overlaid save and drops the entry, so this process
    // stops pinning its own stale save (which would otherwise turn every
    // cross-replica edit into a permanent 412 loop — the user could
    // never fetch the conflicting content their If-Match fails against).
    // Same stale-while-revalidate caveat as above: poll, don't one-shot.
    let mut after = String::new();
    for _ in 0..50 {
        let (_etag, body) = read_law(state.clone(), session_for(&sub).await, &tref).await;
        after = body;
        if after.contains("EXTERNAL") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        after.contains("EXTERNAL"),
        "expected reads to converge to the externally written content; got: {after}"
    );
    assert!(!after.contains("SAVED"));
}

#[tokio::test]
async fn global_get_404s_when_law_not_in_global_corpus() {
    // No database: the global read never touches the pool.
    let state = app_state(None, None);

    // The global `state.corpus` is `CorpusState::empty()`, so the
    // non-traject GET should 404.
    let err = get_corpus_law(State(state), Path(LAW_ID.to_string()))
        .await
        .expect_err("law is not in the global corpus; expect 404");

    assert_eq!(err.0, StatusCode::NOT_FOUND, "{}", err.1);
}

#[tokio::test]
async fn revoked_membership_is_403_on_traject_read() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "TRAJECT_CONTENT");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;

    // Yank the membership AFTER the account got the traject URL — the
    // per-request membership re-check must refuse the read.
    sqlx::query("DELETE FROM traject_members WHERE traject_id = $1")
        .bind(traject_id)
        .execute(&db.pool)
        .await
        .unwrap();

    let err = get_traject_corpus_law(
        State(state),
        Extension(test_account()),
        session_for(&sub).await,
        Path((traject_ref(traject_id), LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect_err("revoked membership must refuse the traject read");
    assert_eq!(err.0, StatusCode::FORBIDDEN, "{}", err.1);
}

// ---------------------------------------------------------------------------
// Write-routing tests
// ---------------------------------------------------------------------------

/// Provision a traject with TWO local sources — a `seed` source at low
/// priority that carries the existing law file, and a `writable_own`
/// at priority 0 that starts empty. The shape mirrors a real federated
/// deploy where the seed is read-only (baked into the container or
/// pulled from a central repo) and the writable_own is the traject's
/// own branch on the writable upstream. Returns the traject id.
async fn seeded_traject(
    pool: &PgPool,
    owner_id: Uuid,
    seed_dir: &std::path::Path,
    writable_own_dir: &std::path::Path,
) -> Uuid {
    let traject_id = insert_traject(pool, owner_id).await;

    // Seed source — read-only logically (we treat it as such; the
    // LocalBackend's write_path is what makes the test crisp).
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'seed', 'Seed', 'local'::corpus_source_type, $2,
                 5, '[]'::jsonb, FALSE)",
    )
    .bind(traject_id)
    .bind(seed_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();

    // Writable-own source at priority 0 (highest priority). Starts
    // empty; first save lands here.
    sqlx::query(
        "INSERT INTO traject_corpus_sources
         (traject_id, source_id, name, source_type, local_path,
          priority, scopes, is_writable_own)
         VALUES ($1, 'writable_own', 'Writable Own',
                 'local'::corpus_source_type, $2,
                 0, '[]'::jsonb, TRUE)",
    )
    .bind(traject_id)
    .bind(writable_own_dir.to_string_lossy().to_string())
    .execute(pool)
    .await
    .unwrap();

    traject_id
}

#[tokio::test]
async fn save_on_seed_loaded_law_lands_on_writable_own_backend() {
    // Regression test for the routing bug where a law loaded from a
    // seed source (e.g. the baked-in `local` corpus on the editor
    // container) had its save fall back to the seed's own backend
    // instead of being routed to the writable_own's branch. Symptom:
    // save returned 200, read-your-writes worked via the overlay, but
    // no commit landed on the traject branch. The fix routes every
    // non-writable_own source to the writable_own at build time, not
    // just the source matching `auth_ref`.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let seed = tempfile::tempdir().unwrap();
    let writable_own = tempfile::tempdir().unwrap();
    // Seed has the law; writable_own starts empty.
    write_law(seed.path(), "SEED_VERSION");
    let traject_id = seeded_traject(&db.pool, owner, seed.path(), writable_own.path()).await;
    let tref = traject_ref(traject_id);

    // Sanity check: pre-save, the writable_own dir does NOT have the
    // law file. The read falls through priority order and lands on
    // the seed copy.
    let writable_own_law_path = writable_own
        .path()
        .join("wet")
        .join(LAW_ID)
        .join("2025-01-01.yaml");
    assert!(!writable_own_law_path.exists());

    // The first edit of a federated law sends the ETag the GET served
    // (computed from the SEED copy — the file doesn't exist on the
    // writable_own yet). The If-Match check must fall back to the
    // law's own source and pass, NOT 412 on the absent branch file.
    let (etag, _body) = read_law(state.clone(), session_for(&sub).await, &tref).await;
    let new_body = format!("$id: {LAW_ID}\nname: SAVED_TO_WRITABLE_OWN\n");
    let (status, _new_etag) = save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        if_match_headers(&etag),
        new_body.clone(),
    )
    .await
    .expect("first If-Match save of a federated law should succeed");
    assert_eq!(status, StatusCode::OK);

    // The file must now exist on the writable_own's backend root.
    // Before the fix the save silently landed on the seed source
    // (because write_target_for_source only fired for sources whose
    // id matched the writable_own's auth_ref) and this assertion
    // failed.
    let written = std::fs::read_to_string(&writable_own_law_path)
        .expect("save must land on the writable_own backend, not the seed");
    assert!(
        written.contains("SAVED_TO_WRITABLE_OWN"),
        "writable_own file must contain the saved body; got: {written}"
    );

    // The seed file is left untouched — saves never reach the seed's
    // backend.
    let seed_law_path = seed.path().join("wet").join(LAW_ID).join("2025-01-01.yaml");
    let seed_text = std::fs::read_to_string(&seed_law_path).unwrap();
    assert!(seed_text.contains("SEED_VERSION"));
    assert!(!seed_text.contains("SAVED_TO_WRITABLE_OWN"));
}

// ---------------------------------------------------------------------------
// Optimistic concurrency (ETag / If-Match) on law and scenario saves
// ---------------------------------------------------------------------------

#[tokio::test]
async fn law_save_with_matching_if_match_succeeds_and_chains_etag() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "V1");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let tref = traject_ref(traject_id);

    // GET → ETag of the current content.
    let (etag, _body) = read_law(state.clone(), session_for(&sub).await, &tref).await;

    // PUT with that ETag as If-Match → success, response carries the
    // NEW etag (of the saved body) for the next save to chain on.
    let new_body = format!("$id: {LAW_ID}\nname: V2\n");
    let (status, new_etag) = save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        if_match_headers(&etag),
        new_body.clone(),
    )
    .await
    .expect("matching If-Match must succeed");
    assert_eq!(status, StatusCode::OK);
    let new_etag = new_etag.expect("save response must carry an ETag header");
    assert_ne!(new_etag, etag);

    // The new ETag matches what a fresh GET serves, so the chain holds.
    let (get_etag, _body) = read_law(state, session_for(&sub).await, &tref).await;
    assert_eq!(get_etag, new_etag);
}

#[tokio::test]
async fn law_save_with_stale_if_match_is_412() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "V1");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let tref = traject_ref(traject_id);

    // Member A loads the law…
    let (stale_etag, _body) = read_law(state.clone(), session_for(&sub).await, &tref).await;

    // …member B saves a new version in the meantime…
    let b_body = format!("$id: {LAW_ID}\nname: B_WAS_HERE\n");
    save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        HeaderMap::new(),
        b_body,
    )
    .await
    .expect("B's save should succeed");

    // …and A's save with the now-stale ETag must be refused with 412
    // instead of silently overwriting B's work (the old last-write-wins).
    let a_body = format!("$id: {LAW_ID}\nname: A_OVERWRITES\n");
    let err = save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        if_match_headers(&stale_etag),
        a_body,
    )
    .await
    .expect_err("stale If-Match must be refused");
    assert_eq!(err.0, StatusCode::PRECONDITION_FAILED, "{}", err.1);

    // B's version is still what a read returns.
    let (_etag, body) = read_law(state, session_for(&sub).await, &tref).await;
    assert!(body.contains("B_WAS_HERE"), "got: {body}");
}

#[tokio::test]
async fn scenario_save_if_match_matrix() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "V1");
    write_scenario(corpus.path(), "basis.feature", "Feature: v1\n");
    let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
    let tref = traject_ref(traject_id);

    // GET the scenario → ETag.
    let (etag, body) = read_scenario(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
    )
    .await;
    assert_eq!(body, "Feature: v1\n");

    // Matching If-Match → success.
    let (status, _etag) = save_scenario_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
        if_match_headers(&etag),
        "Feature: v2\n",
    )
    .await
    .expect("matching If-Match must succeed");
    assert_eq!(status, StatusCode::OK);

    // The same (now stale) ETag again → 412.
    let err = save_scenario_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
        if_match_headers(&etag),
        "Feature: v3-overschrijft\n",
    )
    .await
    .expect_err("stale If-Match must be refused");
    assert_eq!(err.0, StatusCode::PRECONDITION_FAILED, "{}", err.1);

    // No If-Match at all → permissive (backward compatibility, and the
    // create path for brand-new scenario files).
    let (status, _etag) = save_scenario_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "nieuw.feature",
        HeaderMap::new(),
        "Feature: nieuw\n",
    )
    .await
    .expect("save without If-Match must stay permissive");
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn scenario_saved_before_law_is_readable_and_chains_etag() {
    // Regression test for the permanent 412 loop on federated laws:
    // the scenario GET used to route via `resolve_backend_for_law`,
    // which only picks the writable backend when the *law* file exists
    // there. For a seed-loaded law whose law file was never saved in
    // the traject, a scenario save landed on the writable-own backend
    // but subsequent GETs kept serving the seed's old copy + old ETag —
    // while the save's If-Match check compared against the writable-own
    // copy. The user could never see their save and every retry 412'd.
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let seed = tempfile::tempdir().unwrap();
    let writable_own = tempfile::tempdir().unwrap();
    // Seed has the law AND an existing scenario; writable_own starts empty.
    write_law(seed.path(), "SEED_VERSION");
    write_scenario(seed.path(), "basis.feature", "Feature: seed-versie\n");
    let traject_id = seeded_traject(&db.pool, owner, seed.path(), writable_own.path()).await;
    let tref = traject_ref(traject_id);

    // Pre-save GET serves the seed copy (nothing on the branch yet).
    let (etag, body) = read_scenario(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
    )
    .await;
    assert_eq!(body, "Feature: seed-versie\n");

    // First If-Match save succeeds (check falls back to the seed copy)
    // and lands on the writable-own backend.
    let (status, saved_etag) = save_scenario_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
        if_match_headers(&etag),
        "Feature: traject-versie\n",
    )
    .await
    .expect("first If-Match save of a federated scenario must succeed");
    assert_eq!(status, StatusCode::OK);
    let saved_etag = saved_etag.expect("save response must carry an ETag header");

    // The LAW file still does not exist on the writable-own backend —
    // only the scenario does. This is the exact state that used to
    // flip the GET back to the seed.
    assert!(!writable_own
        .path()
        .join("wet")
        .join(LAW_ID)
        .join("2025-01-01.yaml")
        .exists());

    // GET now returns the just-saved content and its ETag.
    let (get_etag, body) = read_scenario(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
    )
    .await;
    assert_eq!(
        body, "Feature: traject-versie\n",
        "GET after save must serve the saved content, not the seed copy"
    );
    assert_eq!(
        get_etag, saved_etag,
        "GET must serve the ETag the save returned, or the chain breaks"
    );

    // And a follow-up If-Match save with that ETag succeeds — no 412 loop.
    let (status, _etag) = save_scenario_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
        if_match_headers(&get_etag),
        "Feature: traject-versie-2\n",
    )
    .await
    .expect("second If-Match save must succeed (no 412 loop)");
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn scenario_list_unions_write_target_and_seed() {
    // The list must mirror the per-file routing: a scenario saved on the
    // traject branch shows up even though the law file was never saved
    // there, AND seed scenarios never copied to the branch keep showing
    // up (their GET falls back to the seed the same way).
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let seed = tempfile::tempdir().unwrap();
    let writable_own = tempfile::tempdir().unwrap();
    write_law(seed.path(), "SEED_VERSION");
    write_scenario(seed.path(), "basis.feature", "Feature: seed-versie\n");
    write_scenario(seed.path(), "extra.feature", "Feature: extra\n");
    let traject_id = seeded_traject(&db.pool, owner, seed.path(), writable_own.path()).await;
    let tref = traject_ref(traject_id);

    // Save only `basis.feature` — it lands on the writable-own backend;
    // `extra.feature` stays seed-only.
    let (status, _etag) = save_scenario_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        "basis.feature",
        HeaderMap::new(),
        "Feature: traject-versie\n",
    )
    .await
    .expect("save must succeed");
    assert_eq!(status, StatusCode::OK);

    let Json(entries) = list_traject_scenarios(
        State(state),
        Extension(test_account()),
        session_for(&sub).await,
        Path((tref, LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect("list must succeed");
    let filenames: Vec<&str> = entries.iter().map(|e| e.filename.as_str()).collect();
    assert_eq!(
        filenames,
        vec!["basis.feature", "extra.feature"],
        "list must union the branch's saves with the seed's remaining scenarios"
    );
}

// ---------------------------------------------------------------------------
// User-token enforcement vs local writable-own backends
// ---------------------------------------------------------------------------

#[tokio::test]
async fn user_token_enforcement_skips_local_backed_trajects() {
    // `require_user_token` demands the acting user's own GitHub token on
    // traject writes — but only the GitHub Contents-API backend can use one.
    // A traject whose writable-own source is `local` (the supported
    // preview/local-stack configuration) writes without any token, so
    // enforcement must not 428 its saves: linking GitHub could never
    // satisfy the requirement there. This pins the requiredness decision
    // to the *resolved* backend, not the deployment switch alone.
    let db = TestDb::new().await;
    let state = state_with_user_token_mode(db.pool.clone(), GithubOAuth::for_tests(true));
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_law(corpus.path(), "VOOR_ENFORCEMENT");
    let traject = local_traject(&db.pool, owner, corpus.path()).await;
    let tref = traject_ref(traject);

    // No GitHub-link cookie on the request: with the pre-fix ordering this
    // 428'd before the write target was even resolved.
    let (status, _etag) = save_law_with(
        state.clone(),
        session_for(&sub).await,
        &tref,
        HeaderMap::new(),
        format!("$id: {LAW_ID}\nname: NA_ENFORCEMENT\n"),
    )
    .await
    .expect("save on a local-backed traject must not demand a GitHub link");
    assert_eq!(status, StatusCode::OK);

    let (status, _etag) = save_scenario_with(
        state,
        session_for(&sub).await,
        &tref,
        "basis.feature",
        HeaderMap::new(),
        "Feature: lokaal-zonder-koppeling\n",
    )
    .await
    .expect("scenario save on a local-backed traject must not demand a GitHub link");
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn feature_flag_row_drives_user_token_requirement() {
    // `write_requires_user_token` OR-s two switches: the static env-var field
    // (covered above via `for_tests(true)`) and the DB-backed
    // `github.user_oauth` feature flag — the switch an admin actually flips
    // through the "Functies" menu. Drive the requirement through that DB row:
    // the env switch stays OFF, so any 428 below can only come from the flag.
    let db = TestDb::new().await;
    let state = app_state(Some(db.pool.clone()), Some(GithubOAuth::for_tests(false)));
    let account_id = Uuid::new_v4();

    // Flag row absent (default off): pre-spike behaviour, no override demanded.
    let headers = HeaderMap::new();
    let token =
        regelrecht_editor_api::credentials::TrajectCredentials::new(&state, account_id, &headers)
            .user_write_token()
            .await
            .expect("without the flag no user token may be demanded");
    assert_eq!(token, None);

    // Flip the DB row — the same write the "Functies" toggle PUT performs.
    regelrecht_pipeline::feature_flags::upsert_flag(
        &db.pool,
        regelrecht_editor_api::feature_flags::GITHUB_USER_OAUTH,
        true,
        None,
    )
    .await
    .expect("flag upsert must succeed");

    // Flag on + no linked token cookie → 428 on the GitHub-capable write path.
    let headers = HeaderMap::new();
    let err =
        regelrecht_editor_api::credentials::TrajectCredentials::new(&state, account_id, &headers)
            .user_write_token()
            .await
            .expect_err("flag on without a linked token must refuse the write");
    assert_eq!(err.0, StatusCode::PRECONDITION_REQUIRED);
}
