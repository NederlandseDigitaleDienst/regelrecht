// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::json;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::{JobStatus, JobType, Priority};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::PipelineError;

#[tokio::test]
async fn test_create_and_get_job() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    assert_eq!(job.job_type, JobType::Harvest);
    assert_eq!(job.law_id, "BWBR0001840");
    assert_eq!(job.status, JobStatus::Pending);
    assert_eq!(job.priority, 50);
    assert_eq!(job.attempts, 0);
    assert_eq!(job.max_attempts, 3);

    let fetched = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(fetched.id, job.id);
}

#[tokio::test]
async fn test_create_job_with_payload() {
    let db = TestDb::new().await;

    let payload = json!({"url": "https://wetten.overheid.nl/BWBR0001840"});
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_payload(payload.clone());
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    assert_eq!(job.payload, Some(payload));
}

#[tokio::test]
async fn test_max_attempts_clamped_to_minimum_1() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(0);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    assert_eq!(job.max_attempts, 1);

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001841").with_max_attempts(-5);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    assert_eq!(job.max_attempts, 1);
}

#[tokio::test]
async fn test_claim_job_priority_ordering() {
    let db = TestDb::new().await;

    let low = CreateJobRequest::new(JobType::Harvest, "low").with_priority(Priority::new(10));
    let high = CreateJobRequest::new(JobType::Harvest, "high").with_priority(Priority::new(90));
    let medium = CreateJobRequest::new(JobType::Harvest, "medium").with_priority(Priority::new(50));

    job_queue::create_job(&db.pool, low).await.unwrap();
    job_queue::create_job(&db.pool, high).await.unwrap();
    job_queue::create_job(&db.pool, medium).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "high");
    assert_eq!(claimed.status, JobStatus::Processing);
    assert_eq!(claimed.attempts, 1);

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "medium");

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "low");

    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(none.is_none());
}

#[tokio::test]
async fn test_claim_job_by_type() {
    let db = TestDb::new().await;

    let harvest = CreateJobRequest::new(JobType::Harvest, "law1");
    let enrich = CreateJobRequest::new(JobType::Enrich, "law2");

    job_queue::create_job(&db.pool, harvest).await.unwrap();
    job_queue::create_job(&db.pool, enrich).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.law_id, "law2");
    assert_eq!(claimed.job_type, JobType::Enrich);

    let none = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap();
    assert!(none.is_none());

    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.law_id, "law1");
}

#[tokio::test]
async fn test_complete_job() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.id, job.id);

    let result = json!({"articles": 15});
    let completed = job_queue::complete_job(&db.pool, job.id, Some(result.clone()))
        .await
        .unwrap();
    assert_eq!(completed.status, JobStatus::Completed);
    assert_eq!(completed.result, Some(result));
    assert!(completed.completed_at.is_some());
}

#[tokio::test]
async fn test_complete_job_not_processing() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let result = job_queue::complete_job(&db.pool, job.id, None).await;
    assert!(matches!(result, Err(PipelineError::JobNotProcessing(_))));
}

/// Backdate a job's scheduled_at so it becomes claimable immediately
/// (simulates the retry backoff having elapsed).
async fn make_due(pool: &sqlx::PgPool, job_id: uuid::Uuid) {
    sqlx::query("UPDATE jobs SET scheduled_at = now() - interval '1 second' WHERE id = $1")
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_fail_job_with_retry() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(3);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 1);
    let failed = job_queue::fail_job(&db.pool, job.id, Some(json!({"error": "timeout"})))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Pending);

    // The retry backoff makes the job not immediately claimable.
    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(none.is_none(), "job must not be claimable during backoff");
    make_due(&db.pool, job.id).await;

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 2);
    let failed = job_queue::fail_job(&db.pool, job.id, Some(json!({"error": "timeout again"})))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Pending);
    make_due(&db.pool, job.id).await;

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 3);
    let failed = job_queue::fail_job(&db.pool, job.id, Some(json!({"error": "gave up"})))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(failed.completed_at.is_some());
    assert!(
        failed.scheduled_at.is_none(),
        "permanently failed job must not carry a retry schedule"
    );

    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(none.is_none());
}

#[tokio::test]
async fn test_fail_job_terminal_fails_immediately_without_retry() {
    let db = TestDb::new().await;

    // max_attempts = 3, but a terminal failure must not reschedule even on the
    // first attempt (deterministic content failures should not burn retries).
    let req = CreateJobRequest::new(JobType::Enrich, "BWBR0001840").with_max_attempts(3);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 1, "still on the first attempt");

    let failed = job_queue::fail_job_terminal(
        &db.pool,
        job.id,
        Some(json!({"error": "LLM produced no machine_readable sections"})),
    )
    .await
    .unwrap();

    assert_eq!(
        failed.status,
        JobStatus::Failed,
        "terminal failure must mark Failed even with attempts < max_attempts"
    );
    assert!(failed.completed_at.is_some());
    assert!(
        failed.scheduled_at.is_none(),
        "terminally failed job must not carry a retry schedule"
    );

    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(
        none.is_none(),
        "terminally failed job must not be reclaimed"
    );
}

#[tokio::test]
async fn test_fail_job_terminal_requires_processing_state() {
    let db = TestDb::new().await;

    // A job that was never claimed is 'pending', not 'processing' — terminal
    // fail must reject it (mirrors fail_job's guard).
    let req = CreateJobRequest::new(JobType::Enrich, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    let err = job_queue::fail_job_terminal(&db.pool, job.id, None)
        .await
        .unwrap_err();
    assert!(matches!(err, PipelineError::JobNotProcessing(_)));
}

#[tokio::test]
async fn test_fail_job_sets_exponential_backoff_schedule() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(5);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    assert!(
        job.scheduled_at.is_none(),
        "new job is claimable immediately"
    );

    // First failure: backoff = 30s * 2^0 = 30s.
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    let failed = job_queue::fail_job(&db.pool, job.id, None).await.unwrap();
    let now = chrono::Utc::now();
    let scheduled = failed.scheduled_at.expect("retrying job has scheduled_at");
    let delay = (scheduled - now).num_seconds();
    assert!(
        (20..=40).contains(&delay),
        "first retry should be ~30s out, got {delay}s"
    );

    // Second failure: backoff = 30s * 2^1 = 60s.
    make_due(&db.pool, job.id).await;
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    let failed = job_queue::fail_job(&db.pool, job.id, None).await.unwrap();
    let now = chrono::Utc::now();
    let delay = (failed.scheduled_at.unwrap() - now).num_seconds();
    assert!(
        (50..=70).contains(&delay),
        "second retry should be ~60s out, got {delay}s"
    );

    // SQL formula must match the Rust reference implementation.
    assert_eq!(job_queue::retry_backoff(1).as_secs(), 30);
    assert_eq!(job_queue::retry_backoff(2).as_secs(), 60);
}

#[tokio::test]
async fn test_claim_job_skips_future_scheduled_and_claims_due() {
    let db = TestDb::new().await;

    let future = CreateJobRequest::new(JobType::Harvest, "future")
        .with_priority(Priority::new(90))
        .with_initial_delay(Duration::from_secs(3600));
    let due = CreateJobRequest::new(JobType::Harvest, "due").with_priority(Priority::new(10));
    let future_job = job_queue::create_job(&db.pool, future).await.unwrap();
    job_queue::create_job(&db.pool, due).await.unwrap();

    assert!(
        future_job.scheduled_at.is_some(),
        "initial delay must set scheduled_at"
    );

    // Despite its higher priority, the future-scheduled job is skipped.
    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "due");

    let none = job_queue::claim_job(&db.pool, None).await.unwrap();
    assert!(none.is_none(), "future job must not be claimable yet");

    // Once due, the job becomes claimable.
    make_due(&db.pool, future_job.id).await;
    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.law_id, "future");
}

#[tokio::test]
async fn test_list_jobs() {
    let db = TestDb::new().await;

    let req1 = CreateJobRequest::new(JobType::Harvest, "law1");
    let req2 = CreateJobRequest::new(JobType::Enrich, "law2");
    job_queue::create_job(&db.pool, req1).await.unwrap();
    job_queue::create_job(&db.pool, req2).await.unwrap();

    let all = job_queue::list_jobs(&db.pool, None).await.unwrap();
    assert_eq!(all.len(), 2);

    job_queue::claim_job(&db.pool, None).await.unwrap();

    let pending = job_queue::list_jobs(&db.pool, Some(JobStatus::Pending))
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);

    let processing = job_queue::list_jobs(&db.pool, Some(JobStatus::Processing))
        .await
        .unwrap();
    assert_eq!(processing.len(), 1);
}

#[tokio::test]
async fn test_reap_orphaned_jobs_resets_to_pending() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(3);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    // Claim the job so it becomes 'processing'
    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.id, job.id);
    assert_eq!(claimed.status, JobStatus::Processing);

    // Backdate started_at so reaper considers it orphaned
    sqlx::query(
        "UPDATE jobs SET started_at = now() - interval '2 hours', \
         heartbeat_at = now() - interval '2 hours' WHERE id = $1",
    )
    .bind(job.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let reaped_jobs = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(reaped_jobs.len(), 1);
    // De teruggegeven rij draagt de NIEUWE status en de payload mee.
    assert_eq!(reaped_jobs[0].id, job.id);
    assert_eq!(reaped_jobs[0].status, JobStatus::Pending);

    // Job should be back to pending (still has retries left: attempts=1, max=3)
    let reaped = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(reaped.status, JobStatus::Pending);
    assert!(reaped.completed_at.is_none());
}

#[tokio::test]
async fn test_reap_spares_long_running_job_with_fresh_heartbeat() {
    // The bug this guards against: a run that was still working after the
    // orphan timeout got reset to pending, and its result was then refused.
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Enrich, "wet_op_de_zorgtoeslag").with_max_attempts(3);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    // Started two hours ago, but the worker beat just now.
    sqlx::query("UPDATE jobs SET started_at = now() - interval '2 hours' WHERE id = $1")
        .bind(job.id)
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(job_queue::heartbeat(&db.pool, job.id).await.unwrap());

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(60))
        .await
        .unwrap();
    assert!(reaped.is_empty());

    // So the worker can still complete it.
    job_queue::complete_job(&db.pool, job.id, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_reap_takes_back_job_with_stale_heartbeat() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(3);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    // Started a while ago, and the worker went quiet.
    sqlx::query(
        "UPDATE jobs SET started_at = now() - interval '1 hour', \
         heartbeat_at = now() - interval '10 minutes' WHERE id = $1",
    )
    .bind(job.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(300))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].status, JobStatus::Pending);
}

#[tokio::test]
async fn test_heartbeat_reports_job_no_longer_processing() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    // Pending: nothing to beat for.
    assert!(!job_queue::heartbeat(&db.pool, job.id).await.unwrap());

    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert!(job_queue::heartbeat(&db.pool, job.id).await.unwrap());

    job_queue::complete_job(&db.pool, job.id, None)
        .await
        .unwrap();
    assert!(!job_queue::heartbeat(&db.pool, job.id).await.unwrap());
}

/// Seconds since the job's last heartbeat, as the database sees it.
async fn heartbeat_age_secs(db: &TestDb, id: uuid::Uuid) -> f64 {
    let (age,): (f64,) = sqlx::query_as(
        "SELECT EXTRACT(EPOCH FROM (now() - heartbeat_at))::float8 FROM jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    age
}

async fn age_heartbeat(db: &TestDb, id: uuid::Uuid) {
    sqlx::query("UPDATE jobs SET heartbeat_at = now() - interval '1 hour' WHERE id = $1")
        .bind(id)
        .execute(&db.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_job_heartbeat_beats_and_stops_on_drop() {
    let db = TestDb::new().await;
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    age_heartbeat(&db, job.id).await;
    let guard = job_queue::JobHeartbeat::start_with(
        db.pool.clone(),
        job.id,
        Duration::from_millis(50),
        Duration::from_secs(60),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        heartbeat_age_secs(&db, job.id).await < 5.0,
        "the guard beats"
    );

    drop(guard);
    tokio::time::sleep(Duration::from_millis(100)).await;
    age_heartbeat(&db, job.id).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        heartbeat_age_secs(&db, job.id).await > 3000.0,
        "a dropped guard stops"
    );
}

#[tokio::test]
async fn test_job_heartbeat_stops_at_its_ceiling() {
    // A worker that is alive but stuck must not hold its job forever.
    let db = TestDb::new().await;
    let req = CreateJobRequest::new(JobType::Enrich, "wet_op_de_zorgtoeslag");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    sqlx::query("UPDATE jobs SET started_at = now() - interval '2 hours' WHERE id = $1")
        .bind(job.id)
        .execute(&db.pool)
        .await
        .unwrap();
    age_heartbeat(&db, job.id).await;
    let _guard = job_queue::JobHeartbeat::start_with(
        db.pool.clone(),
        job.id,
        Duration::from_millis(50),
        Duration::ZERO,
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(heartbeat_age_secs(&db, job.id).await > 3000.0);

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(300))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1);
}

#[tokio::test]
async fn test_reap_uses_start_when_heartbeat_missing() {
    // A job claimed by a worker from before heartbeats has none.
    let db = TestDb::new().await;
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    sqlx::query("UPDATE jobs SET heartbeat_at = NULL WHERE id = $1")
        .bind(job.id)
        .execute(&db.pool)
        .await
        .unwrap();
    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(300))
        .await
        .unwrap();
    assert!(reaped.is_empty(), "just started");

    sqlx::query("UPDATE jobs SET started_at = now() - interval '1 hour' WHERE id = $1")
        .bind(job.id)
        .execute(&db.pool)
        .await
        .unwrap();
    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(300))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1, "started long ago and never beat");
}

#[tokio::test]
async fn test_reap_ignores_heartbeat_left_from_earlier_claim() {
    // Reset, then claimed again by a worker that does not beat: the stale
    // heartbeat from the first claim must not get the fresh run reaped.
    let db = TestDb::new().await;
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    sqlx::query(
        "UPDATE jobs SET started_at = now(), heartbeat_at = now() - interval '1 hour' WHERE id = $1",
    )
    .bind(job.id)
    .execute(&db.pool)
    .await
    .unwrap();
    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(300))
        .await
        .unwrap();
    assert!(reaped.is_empty());
}

#[tokio::test]
async fn test_reap_orphaned_jobs_permanently_fails_exhausted() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_max_attempts(1);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();

    // Claim (attempts becomes 1, which equals max_attempts=1)
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    // Backdate started_at
    sqlx::query(
        "UPDATE jobs SET started_at = now() - interval '2 hours', \
         heartbeat_at = now() - interval '2 hours' WHERE id = $1",
    )
    .bind(job.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let reaped_jobs = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(reaped_jobs.len(), 1);
    assert_eq!(reaped_jobs[0].status, JobStatus::Failed);

    // Job should be permanently failed (attempts=1 >= max_attempts=1)
    let reaped = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(reaped.status, JobStatus::Failed);
    assert!(reaped.completed_at.is_some());
}

#[tokio::test]
async fn test_reap_orphaned_jobs_returns_zero_when_none_orphaned() {
    let db = TestDb::new().await;

    // No jobs at all
    let reaped_jobs = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(60))
        .await
        .unwrap();
    assert!(reaped_jobs.is_empty());

    // A processing job that is NOT yet timed out
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840");
    job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();

    let reaped_jobs = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(3600))
        .await
        .unwrap();
    assert!(reaped_jobs.is_empty());
}

#[tokio::test]
async fn test_create_harvest_job_if_not_exists_creates_new() {
    let db = TestDb::new().await;

    let payload = json!({"bwb_id": "BWBR0001840", "date": "2025-01-01"});
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840")
        .with_priority(Priority::new(30))
        .with_payload(payload);

    let result = job_queue::create_harvest_job_if_not_exists(&db.pool, req, "2025-01-01")
        .await
        .unwrap();
    assert!(result.is_some(), "should create job when none exists");

    let job = result.unwrap();
    assert_eq!(job.law_id, "BWBR0001840");
    assert_eq!(job.status, JobStatus::Pending);
}

#[tokio::test]
async fn test_create_harvest_job_if_not_exists_skips_duplicate() {
    let db = TestDb::new().await;

    let payload = json!({"bwb_id": "BWBR0001840", "date": "2025-01-01"});
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_payload(payload.clone());
    job_queue::create_job(&db.pool, req).await.unwrap();

    let req2 = CreateJobRequest::new(JobType::Harvest, "BWBR0001840")
        .with_priority(Priority::new(30))
        .with_payload(payload);
    let result = job_queue::create_harvest_job_if_not_exists(&db.pool, req2, "2025-01-01")
        .await
        .unwrap();
    assert!(result.is_none(), "should not create duplicate job");
}

#[tokio::test]
async fn test_create_harvest_job_if_not_exists_skips_dateless_job() {
    let db = TestDb::new().await;

    // Simulate admin-created job without a date (payload has no "date" key)
    let payload = json!({"bwb_id": "BWBR0001840"});
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_payload(payload);
    job_queue::create_job(&db.pool, req).await.unwrap();

    // Follow-up tries to create a dated job for the same law
    let follow_up = json!({"bwb_id": "BWBR0001840", "date": "2025-01-01"});
    let req2 = CreateJobRequest::new(JobType::Harvest, "BWBR0001840")
        .with_priority(Priority::new(30))
        .with_payload(follow_up);
    let result = job_queue::create_harvest_job_if_not_exists(&db.pool, req2, "2025-01-01")
        .await
        .unwrap();
    assert!(
        result.is_none(),
        "should skip when a dateless job already exists for the same law"
    );
}

#[tokio::test]
async fn test_create_harvest_job_if_not_exists_ignores_completed_job() {
    let db = TestDb::new().await;

    // A previous harvest of this law ran to completion (editor-style payload
    // without a date key).
    let payload = json!({"bwb_id": "BWBR0001840"});
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0001840").with_payload(payload.clone());
    job_queue::create_job(&db.pool, req).await.unwrap();
    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    job_queue::complete_job(&db.pool, claimed.id, None)
        .await
        .unwrap();

    // A re-harvest request (editor uses an empty date) must create a new job:
    // only pending/processing jobs may block, never completed ones.
    let req2 = CreateJobRequest::new(JobType::Harvest, "BWBR0001840")
        .with_priority(Priority::new(30))
        .with_payload(payload);
    let result = job_queue::create_harvest_job_if_not_exists(&db.pool, req2, "")
        .await
        .unwrap();
    assert!(
        result.is_some(),
        "completed harvest job must not block a re-harvest"
    );
    assert_eq!(result.unwrap().status, JobStatus::Pending);
}

#[tokio::test]
async fn test_concurrent_claim_safety() {
    let db = TestDb::new().await;

    let req = CreateJobRequest::new(JobType::Harvest, "contested");
    job_queue::create_job(&db.pool, req).await.unwrap();

    let (r1, r2) = tokio::join!(
        job_queue::claim_job(&db.pool, None),
        job_queue::claim_job(&db.pool, None)
    );

    let claimed_count = [r1.unwrap(), r2.unwrap()]
        .iter()
        .filter(|j| j.is_some())
        .count();

    assert_eq!(claimed_count, 1);
}

#[tokio::test]
async fn test_count_enrich_jobs_started_this_hour() {
    let db = TestDb::new().await;

    // Two claude enrich jobs and one opencode enrich job (payload provider set,
    // as the admin enqueues them), plus a harvest job that must never count.
    for law in ["c1", "c2"] {
        let req =
            CreateJobRequest::new(JobType::Enrich, law).with_payload(json!({"provider": "claude"}));
        job_queue::create_job(&db.pool, req).await.unwrap();
    }
    let req =
        CreateJobRequest::new(JobType::Enrich, "o1").with_payload(json!({"provider": "opencode"}));
    job_queue::create_job(&db.pool, req).await.unwrap();
    job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Harvest, "h1"))
        .await
        .unwrap();
    // A pending (never claimed) claude job: started_at is NULL, so it must not count.
    let req =
        CreateJobRequest::new(JobType::Enrich, "c3").with_payload(json!({"provider": "claude"}));
    job_queue::create_job(&db.pool, req).await.unwrap();

    // Claiming sets started_at = now(). Claim the three oldest enrich jobs
    // (c1, c2, o1 by creation order) and the harvest job; c3 stays pending.
    for _ in 0..3 {
        job_queue::claim_job(&db.pool, Some(JobType::Enrich))
            .await
            .unwrap()
            .unwrap();
    }
    job_queue::claim_job(&db.pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();

    // Only the two claimed claude jobs count; pending c3, opencode, and harvest excluded.
    let (claude, local_hour) = job_queue::count_enrich_jobs_started_this_hour(&db.pool, "claude")
        .await
        .unwrap();
    assert_eq!(claude, 2, "two claude enrich jobs started this hour");
    assert!((0..=23).contains(&local_hour), "local hour is 0..=23");

    let (opencode, _) = job_queue::count_enrich_jobs_started_this_hour(&db.pool, "opencode")
        .await
        .unwrap();
    assert_eq!(opencode, 1, "provider filter is per-provider");

    // A run from a previous hour falls outside the current clock-hour bucket.
    // (2h back is always before the current bucket start, which is <=59min behind
    // now, so this assertion is not sensitive to where in the hour the test runs.)
    sqlx::query("UPDATE jobs SET started_at = now() - interval '2 hours' WHERE law_id = 'c1'")
        .execute(&db.pool)
        .await
        .unwrap();
    let (claude, _) = job_queue::count_enrich_jobs_started_this_hour(&db.pool, "claude")
        .await
        .unwrap();
    assert_eq!(claude, 1, "backdated run excluded from this hour's count");
}
