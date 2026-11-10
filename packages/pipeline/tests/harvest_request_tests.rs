use pretty_assertions::assert_eq;

use regelrecht_pipeline::harvest_request::{
    HarvestRequestOptions, HarvestRequestOutcome, request_harvest,
};
use regelrecht_pipeline::law_status;
use regelrecht_pipeline::models::{JobStatus, JobType, LawStatusValue, Priority};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::{HarvestPayload, job_queue};

const LAW_ID: &str = "BWBR0018451";

fn opts() -> HarvestRequestOptions {
    HarvestRequestOptions::default()
}

#[tokio::test]
async fn creates_job_and_bookkeeping() {
    let db = TestDb::new().await;

    let outcome = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();

    let HarvestRequestOutcome::Created(job) = outcome else {
        panic!("expected Created, got {outcome:?}");
    };
    assert_eq!(job.law_id, LAW_ID);
    assert_eq!(job.job_type, JobType::Harvest);
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.priority, 50);

    // Payload uses the canonical HarvestPayload shape.
    let payload: HarvestPayload = serde_json::from_value(job.payload.clone().unwrap()).unwrap();
    assert_eq!(payload.bwb_id.as_deref(), Some(LAW_ID));
    assert!(payload.date.is_none());

    // Law entry was upserted, queued, and linked to the job.
    let entry = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(entry.status, LawStatusValue::Queued);
    assert_eq!(entry.harvest_job_id, Some(job.id));
}

#[tokio::test]
async fn stores_priority_date_and_slug() {
    let db = TestDb::new().await;

    let outcome = request_harvest(
        &db.pool,
        LAW_ID,
        HarvestRequestOptions {
            priority: Priority::new(80),
            date: Some("2024-01-01".to_string()),
            law_name: Some("Zorgtoeslagwet".to_string()),
            slug: Some("wet_op_de_zorgtoeslag".to_string()),
        },
    )
    .await
    .unwrap();

    let HarvestRequestOutcome::Created(job) = outcome else {
        panic!("expected Created, got {outcome:?}");
    };
    assert_eq!(job.priority, 80);

    let payload: HarvestPayload = serde_json::from_value(job.payload.clone().unwrap()).unwrap();
    assert_eq!(payload.date.as_deref(), Some("2024-01-01"));

    let entry = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(entry.law_name.as_deref(), Some("Zorgtoeslagwet"));
    assert_eq!(entry.slug.as_deref(), Some("wet_op_de_zorgtoeslag"));
}

#[tokio::test]
async fn cvdr_id_produces_cvdr_payload() {
    let db = TestDb::new().await;

    let outcome = request_harvest(&db.pool, "CVDR681386", opts())
        .await
        .unwrap();

    let HarvestRequestOutcome::Created(job) = outcome else {
        panic!("expected Created, got {outcome:?}");
    };
    let payload: HarvestPayload = serde_json::from_value(job.payload.clone().unwrap()).unwrap();
    assert!(payload.bwb_id.is_none());
    assert_eq!(payload.cvdr_id.as_deref(), Some("CVDR681386"));
}

#[tokio::test]
async fn dedups_against_active_job() {
    let db = TestDb::new().await;

    let first = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    let HarvestRequestOutcome::Created(first_job) = first else {
        panic!("expected Created, got {first:?}");
    };

    let second = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    let HarvestRequestOutcome::AlreadyQueued { existing_job_id } = second else {
        panic!("expected AlreadyQueued, got {second:?}");
    };
    assert_eq!(existing_job_id, first_job.id);
}

#[tokio::test]
async fn dedups_against_dated_job() {
    let db = TestDb::new().await;

    // A pending job WITH a date must also block an undated request — the old
    // editor path missed this (it only deduped against undated jobs).
    let dated = request_harvest(
        &db.pool,
        LAW_ID,
        HarvestRequestOptions {
            date: Some("2024-01-01".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(matches!(dated, HarvestRequestOutcome::Created(_)));

    let undated = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    assert!(matches!(
        undated,
        HarvestRequestOutcome::AlreadyQueued { .. }
    ));
}

#[tokio::test]
async fn allows_requeue_after_completion() {
    let db = TestDb::new().await;

    let first = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    let HarvestRequestOutcome::Created(first_job) = first else {
        panic!("expected Created, got {first:?}");
    };

    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, first_job.id);
    job_queue::complete_job(&db.pool, claimed.id, None)
        .await
        .unwrap();

    let second = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    assert!(matches!(second, HarvestRequestOutcome::Created(_)));
}

#[tokio::test]
async fn refuses_exhausted_law() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, LAW_ID, None, None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::HarvestExhausted)
        .await
        .unwrap();

    let outcome = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    assert!(matches!(outcome, HarvestRequestOutcome::Exhausted));

    // No job was created and the status was left untouched.
    let jobs = job_queue::list_jobs(&db.pool, None).await.unwrap();
    assert!(jobs.is_empty());
    let entry = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(entry.status, LawStatusValue::HarvestExhausted);
}

#[tokio::test]
async fn rejects_invalid_date_before_touching_db() {
    let db = TestDb::new().await;

    for bad_date in ["not-a-date", "2025-13-01", "9999-01-01"] {
        let outcome = request_harvest(
            &db.pool,
            LAW_ID,
            HarvestRequestOptions {
                date: Some(bad_date.to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(
            matches!(outcome, HarvestRequestOutcome::InvalidDate { .. }),
            "expected InvalidDate for {bad_date}, got {outcome:?}"
        );
    }

    // Nothing was written.
    let jobs = job_queue::list_jobs(&db.pool, None).await.unwrap();
    assert!(jobs.is_empty());
    assert!(law_status::get_law(&db.pool, LAW_ID).await.is_err());
}

#[tokio::test]
async fn does_not_downgrade_in_progress_status() {
    let db = TestDb::new().await;

    // A law stuck in 'harvesting' without an active job (e.g. its job was
    // deleted) can be re-queued, but the in-progress status must not be
    // overwritten by the bookkeeping.
    law_status::upsert_law(&db.pool, LAW_ID, None, None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Harvesting)
        .await
        .unwrap();

    let outcome = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    assert!(matches!(outcome, HarvestRequestOutcome::Created(_)));

    let entry = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(entry.status, LawStatusValue::Harvesting);
}

#[tokio::test]
async fn resets_completed_status_to_queued() {
    let db = TestDb::new().await;

    // Re-harvesting an already-harvested law marks it queued again — the new
    // pending job is the source of truth (admin semantics, now canonical).
    law_status::upsert_law(&db.pool, LAW_ID, None, None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Harvested)
        .await
        .unwrap();

    let outcome = request_harvest(&db.pool, LAW_ID, opts()).await.unwrap();
    assert!(matches!(outcome, HarvestRequestOutcome::Created(_)));

    let entry = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(entry.status, LawStatusValue::Queued);
}
