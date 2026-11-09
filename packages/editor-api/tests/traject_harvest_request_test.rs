//! Integration tests for `task_requests::request_traject_harvest`: de
//! traject-scoped harvest-aanvraag (taken-mechanisme). Pint vast dat een
//! aanvraag een `traject_harvest`-job enqueue't met het juiste
//! taak-flow-payload (deliver=task, requested_by, prioriteit 80), dat een
//! tweede aanvraag voor dezelfde wet dedupliceert (409) en dat een ongeldig
//! BWB-id met 400 wordt geweigerd.
//!
//! Zelfde hermetische opzet als `promote_law_test.rs` / `traject_reads_test.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{empty_state, local_traject, seed_account, session_for, traject_ref};

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::state::AppState;
use regelrecht_editor_api::task_requests::{request_traject_harvest, TrajectHarvestRequest};

use regelrecht_pipeline::test_utils::TestDb;

const BWB_ID: &str = "BWBR0002399";

fn account(account_id: Uuid) -> AccountRecord {
    AccountRecord {
        id: account_id,
        person_sub: "test-sub".to_string(),
        email: "test@example.gov".to_string(),
        name: "Test User".to_string(),
    }
}

async fn request(
    state: AppState,
    sub: &str,
    account_id: Uuid,
    tref: &str,
    bwb_id: &str,
    law_name: Option<&str>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (status, _body) = request_traject_harvest(
        State(state),
        session_for(sub).await,
        Extension(account(account_id)),
        Path(tref.to_string()),
        axum::Json(TrajectHarvestRequest {
            bwb_id: bwb_id.to_string(),
            law_name: law_name.map(String::from),
        }),
    )
    .await?;
    Ok(status)
}

#[tokio::test]
async fn harvest_request_enqueues_a_task_flow_job() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let status = request(
        state,
        &sub,
        owner,
        &tref,
        // lowercase input wordt genormaliseerd naar de canonieke vorm.
        &BWB_ID.to_lowercase(),
        Some("Voorbeeldwet"),
    )
    .await
    .expect("harvest request must be accepted");
    assert_eq!(status, StatusCode::ACCEPTED);

    let (job_type, law_id, job_tref, priority, payload): (
        String,
        String,
        Option<String>,
        i32,
        serde_json::Value,
    ) = sqlx::query_as("SELECT job_type::text, law_id, traject_ref, priority, payload FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(job_type, "traject_harvest");
    assert_eq!(law_id, BWB_ID);
    assert_eq!(job_tref.as_deref(), Some(tref.as_str()));
    assert_eq!(
        priority, 80,
        "een mens wacht erop: boven de bulk-prioriteit"
    );
    assert_eq!(payload["bwb_id"], BWB_ID);
    assert_eq!(payload["deliver"], "task");
    assert_eq!(payload["law_name"], "Voorbeeldwet");
    assert_eq!(payload["requested_by"], owner.to_string());
    assert_eq!(payload["traject_id"], traject_id.to_string());
}

#[tokio::test]
async fn duplicate_harvest_request_for_same_law_and_traject_conflicts() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let first = request(state.clone(), &sub, owner, &tref, BWB_ID, None)
        .await
        .expect("first request must be accepted");
    assert_eq!(first, StatusCode::ACCEPTED);

    let err = request(state, &sub, owner, &tref, BWB_ID, None)
        .await
        .expect_err("second request must conflict");
    assert_eq!(err.0, StatusCode::CONFLICT);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "dedup: er blijft één actieve job");
}

#[tokio::test]
async fn invalid_bwb_id_is_rejected_with_400() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
    let corpus_dir = tempfile::tempdir().unwrap();
    let traject_id = local_traject(&db.pool, owner, corpus_dir.path()).await;
    let tref = traject_ref(traject_id);

    let err = request(state, &sub, owner, &tref, "wet_op_de_zorgtoeslag", None)
        .await
        .expect_err("slug is geen BWB-id");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
