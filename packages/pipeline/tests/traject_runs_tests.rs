// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{Value, json};
use uuid::Uuid;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::{JobStatus, JobType};
use regelrecht_pipeline::tasks::{self, NewTask, TaskType};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::traject_runs::{
    LawFilter, StopOutcome, list_traject_runs, stop_traject_enrich_run,
};

const TRAJECT_REF: &str = "zorgtoeslag-abcd1234";

/// The traject the runs belong to: its refs end in the first eight hex
/// characters of this id.
fn traject_id() -> Uuid {
    Uuid::parse_str("abcd1234-0000-4000-8000-000000000000").unwrap()
}

async fn seed_account(db: &TestDb, sub: &str, name: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) VALUES ($1, $1 || '@example.org', $2) RETURNING id",
    )
    .bind(sub)
    .bind(name)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    id
}

async fn create_run(
    db: &TestDb,
    job_type: JobType,
    law_id: &str,
    traject_ref: Option<&str>,
    payload: Value,
) -> Uuid {
    let mut req = CreateJobRequest::new(job_type, law_id).with_payload(payload);
    if let Some(r) = traject_ref {
        req = req.with_traject_ref(r);
    }
    job_queue::create_job(&db.pool, req).await.unwrap().id
}

// The read query is under test, not the queue's transitions, so the fixtures
// set the end state directly.
async fn set_state(
    db: &TestDb,
    job_id: Uuid,
    status: &str,
    result: Option<Value>,
    progress: Option<Value>,
) {
    sqlx::query(
        "UPDATE jobs SET status = $2::job_status, result = $3, progress = $4, \
                started_at = now() - interval '5 minutes', \
                completed_at = CASE WHEN $2 IN ('completed', 'failed') THEN now() END \
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(status)
    .bind(result)
    .bind(progress)
    .execute(&db.pool)
    .await
    .unwrap();
}

/// The runs one traject collected, plus three runs of other scopes.
struct SeededRuns {
    alice: Uuid,
    bob: Uuid,
    harvest: Uuid,
    enrich: Uuid,
    failed: Uuid,
    running: Uuid,
}

async fn set_task_statuses(db: &TestDb, job_id: Uuid, assignee: Uuid, statuses: &[&str]) {
    for (i, status) in statuses.iter().enumerate() {
        let task = tasks::create_task(
            &db.pool,
            NewTask {
                task_type: TaskType::JobReview,
                assignee_account_id: Some(assignee),
                traject_id: None,
                job_id: Some(job_id),
                title: format!("Artikel {}", i + 1),
                payload: Some(json!({ "article": (i + 1).to_string() })),
            },
        )
        .await
        .unwrap();
        sqlx::query("UPDATE tasks SET status = $2::task_status WHERE id = $1")
            .bind(task.id)
            .bind(status)
            .execute(&db.pool)
            .await
            .unwrap();
    }
}

async fn seed_runs(db: &TestDb) -> SeededRuns {
    let alice = seed_account(db, "sub-a", "Alice Voorbeeld").await;
    let bob = seed_account(db, "sub-b", "Bob Voorbeeld").await;

    // A harvest requested before the traject was renamed: same id part, old slug.
    let harvest = create_run(
        db,
        JobType::TrajectHarvest,
        "BWBR0018451",
        Some("oude-naam-abcd1234"),
        json!({ "law_name": "Wet op de zorgtoeslag", "requested_by": alice, "deliver": "task" }),
    )
    .await;
    set_state(db, harvest, "completed", None, None).await;

    let enrich = create_run(
        db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        json!({ "requested_by": bob, "deliver": "task", "provider": "requested-provider" }),
    )
    .await;
    let result = json!({
        "law_id": "wet_op_de_zorgtoeslag",
        "provider": "ran-provider",
        "articles_total": 12,
        "articles_with_machine_readable": 9,
        "coverage_score": 0.75,
        "markings": [{ "large": "not shipped" }]
    });
    set_state(
        db,
        enrich,
        "completed",
        Some(result),
        Some(json!({ "phase": "stale" })),
    )
    .await;
    set_task_statuses(
        db,
        enrich,
        bob,
        &["open", "approved", "approved", "rejected"],
    )
    .await;
    // A failure task on the same job is no part of its review.
    tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobFailed,
            assignee_account_id: Some(bob),
            traject_id: None,
            job_id: Some(enrich),
            title: "Verrijking mislukt".into(),
            payload: None,
        },
    )
    .await
    .unwrap();

    let failed = create_run(
        db,
        JobType::DocumentConvert,
        "doc:zorgtoeslag-abcd1234/nota.md",
        Some(TRAJECT_REF),
        json!({ "target_path": "nota.md", "requested_by": alice, "deliver": "task" }),
    )
    .await;
    let error = json!({ "error": "conversie mislukt" });
    set_state(db, failed, "failed", Some(error), None).await;

    let running = create_run(
        db,
        JobType::Enrich,
        "zorgverzekeringswet",
        Some(TRAJECT_REF),
        json!({ "requested_by": alice, "deliver": "task" }),
    )
    .await;
    let progress = json!({ "phase": "modelleren" });
    set_state(db, running, "processing", None, Some(progress)).await;

    // Not this traject's: another traject, a corpus-wide run, a corpus harvest.
    create_run(db, JobType::Enrich, "x", Some("ander-ffff0000"), json!({})).await;
    create_run(db, JobType::Enrich, "x", None, json!({})).await;
    create_run(db, JobType::Harvest, "x", Some(TRAJECT_REF), json!({})).await;

    SeededRuns {
        alice,
        bob,
        harvest,
        enrich,
        failed,
        running,
    }
}

#[tokio::test]
async fn lists_only_this_trajects_runs_newest_first_also_after_a_rename() {
    let db = TestDb::new().await;
    let seeded = seed_runs(&db).await;
    // The harvest was requested under the traject's old name; it still belongs.
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice, None)
        .await
        .unwrap();
    let ids: Vec<Uuid> = runs.iter().map(|r| r.job_id).collect();
    assert_eq!(
        ids,
        vec![seeded.running, seeded.failed, seeded.enrich, seeded.harvest]
    );
    let harvest = runs.iter().find(|r| r.job_id == seeded.harvest).unwrap();
    assert_eq!(harvest.law_name.as_deref(), Some("Wet op de zorgtoeslag"));
    assert!(harvest.started_at.is_some() && harvest.completed_at.is_some());
}

#[tokio::test]
async fn gives_an_enrichment_its_outcome_and_review() {
    let db = TestDb::new().await;
    let seeded = seed_runs(&db).await;
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice, None)
        .await
        .unwrap();
    let enrich = runs.iter().find(|r| r.job_id == seeded.enrich).unwrap();
    assert_eq!(enrich.requested_by_name.as_deref(), Some("Bob Voorbeeld"));
    assert_eq!(enrich.provider.as_deref(), Some("ran-provider"));
    assert_eq!(enrich.articles_total, Some(12));
    assert_eq!(enrich.articles_with_machine_readable, Some(9));
    assert_eq!(enrich.coverage_score, Some(0.75));
    // Progress describes a running job; a finished one carries none.
    assert_eq!(enrich.progress, None);
    let review = (
        enrich.tasks_open,
        enrich.tasks_approved,
        enrich.tasks_rejected,
        enrich.tasks_dismissed,
    );
    assert_eq!(review, (1, 2, 1, 0));
    // The open task is Bob's: Alice cannot pick it up from her task list.
    assert_eq!(enrich.tasks_open_mine, 0);
}

#[tokio::test]
async fn counts_the_open_tasks_of_the_member_asking() {
    let db = TestDb::new().await;
    let seeded = seed_runs(&db).await;
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.bob, None)
        .await
        .unwrap();
    let enrich = runs.iter().find(|r| r.job_id == seeded.enrich).unwrap();
    assert_eq!((enrich.tasks_open, enrich.tasks_open_mine), (1, 1));
}

#[tokio::test]
async fn gives_a_failed_run_its_error_and_a_running_one_its_progress() {
    let db = TestDb::new().await;
    let seeded = seed_runs(&db).await;
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice, None)
        .await
        .unwrap();

    let failed = runs.iter().find(|r| r.job_id == seeded.failed).unwrap();
    assert_eq!(failed.job_type, JobType::DocumentConvert);
    assert_eq!(failed.target_path.as_deref(), Some("nota.md"));
    assert_eq!(failed.error.as_deref(), Some("conversie mislukt"));
    assert_eq!(failed.requested_by_name.as_deref(), Some("Alice Voorbeeld"));

    let running = runs.iter().find(|r| r.job_id == seeded.running).unwrap();
    assert_eq!(running.status, JobStatus::Processing);
    assert_eq!(running.progress, Some(json!({ "phase": "modelleren" })));
}

#[tokio::test]
async fn falls_back_to_the_requested_provider_while_an_enrichment_runs() {
    let db = TestDb::new().await;
    create_run(
        &db,
        JobType::Enrich,
        "x",
        Some(TRAJECT_REF),
        json!({ "provider": "requested-provider" }),
    )
    .await;
    let runs = list_traject_runs(&db.pool, traject_id(), Uuid::new_v4(), None)
        .await
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].provider.as_deref(), Some("requested-provider"));
    assert_eq!(runs[0].requested_by_name, None);
}

#[tokio::test]
async fn narrows_to_one_law_with_the_harvest_that_fetched_it() {
    let db = TestDb::new().await;
    let seeded = seed_runs(&db).await;
    // The same law and BWB id in another traject: the law filter must not
    // reach past the traject match.
    let other = "ander-ffff0000";
    create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(other),
        json!({}),
    )
    .await;
    create_run(
        &db,
        JobType::TrajectHarvest,
        "BWBR0018451",
        Some(other),
        json!({}),
    )
    .await;
    let law = LawFilter {
        law_id: "wet_op_de_zorgtoeslag",
        bwb_id: Some("BWBR0018451"),
    };
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice, Some(law))
        .await
        .unwrap();
    let ids: Vec<Uuid> = runs.iter().map(|r| r.job_id).collect();
    assert_eq!(ids, vec![seeded.enrich, seeded.harvest]);

    // Without a BWB id there is no harvest to match.
    let law = LawFilter {
        law_id: "wet_op_de_zorgtoeslag",
        bwb_id: None,
    };
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice, Some(law))
        .await
        .unwrap();
    let ids: Vec<Uuid> = runs.iter().map(|r| r.job_id).collect();
    assert_eq!(ids, vec![seeded.enrich]);
}

#[tokio::test]
async fn matches_a_bwb_id_only_on_a_harvest() {
    let db = TestDb::new().await;
    // An enrichment whose law_id happens to be the BWB id is not the harvest.
    create_run(
        &db,
        JobType::Enrich,
        "BWBR0018451",
        Some(TRAJECT_REF),
        json!({}),
    )
    .await;
    let law = LawFilter {
        law_id: "wet_op_de_zorgtoeslag",
        bwb_id: Some("BWBR0018451"),
    };
    let runs = list_traject_runs(&db.pool, traject_id(), Uuid::new_v4(), Some(law))
        .await
        .unwrap();
    assert!(runs.is_empty());
}

fn task_enrich_payload() -> Value {
    json!({
        "deliver": "task",
        "law_id": "wet_op_de_zorgtoeslag",
        "provider": "claude",
        "yaml_path": "wet/wet_op_de_zorgtoeslag/2025-01-01.yaml",
        "traject_id": traject_id().to_string(),
        "traject_ref": TRAJECT_REF,
    })
}

async fn insert_input_blob(db: &TestDb, job_id: Uuid) {
    tasks::insert_blob(&db.pool, job_id, tasks::BlobKind::Input, "law.yaml", "x")
        .await
        .unwrap();
}

async fn blob_count(db: &TestDb, job_id: Uuid) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM job_blobs WHERE job_id = $1")
        .bind(job_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    n
}

#[tokio::test]
async fn stopping_a_pending_enrichment_removes_it_and_its_input() {
    let db = TestDb::new().await;
    let job = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        task_enrich_payload(),
    )
    .await;
    insert_input_blob(&db, job).await;

    let outcome = stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();
    assert_eq!(outcome, StopOutcome::Removed);
    assert!(job_queue::get_job(&db.pool, job).await.is_err());
    assert_eq!(blob_count(&db, job).await, 0);

    // Idempotent: the second request finds nothing to stop.
    let again = stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();
    assert_eq!(again, StopOutcome::Done);
}

#[tokio::test]
async fn stopping_a_processing_enrichment_records_the_request_for_the_worker() {
    let db = TestDb::new().await;
    let job = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        task_enrich_payload(),
    )
    .await;
    let claimed = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    assert_eq!(claimed.id, job);

    let outcome = stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();
    assert_eq!(outcome, StopOutcome::Stopping);
    assert_eq!(
        job_queue::heartbeat(&db.pool, job, claimed.started_at)
            .await
            .unwrap(),
        job_queue::Beat::StopRequested
    );
    // Asked twice while it winds down: still stopping, the moment kept.
    let again = stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();
    assert_eq!(again, StopOutcome::Stopping);

    let cancelled = job_queue::mark_cancelled(&db.pool, job).await.unwrap();
    assert_eq!(cancelled.status, JobStatus::Cancelled);
    let after = stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();
    assert_eq!(after, StopOutcome::Done);
}

#[tokio::test]
async fn a_stop_does_not_reach_another_trajects_or_a_corpus_run() {
    let db = TestDb::new().await;
    let other = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some("ander-ffff0000"),
        task_enrich_payload(),
    )
    .await;
    let corpus = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        json!({ "provider": "claude" }),
    )
    .await;
    // Same eight hex characters, another traject: the full id tells them apart.
    let mut collision = task_enrich_payload();
    collision["traject_id"] = json!("abcd1234-ffff-4000-8000-000000000000");
    let lookalike = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        collision,
    )
    .await;

    for job in [other, corpus, lookalike] {
        let outcome = stop_traject_enrich_run(&db.pool, traject_id(), job)
            .await
            .unwrap();
        assert_eq!(outcome, StopOutcome::Done);
        assert_eq!(
            job_queue::get_job(&db.pool, job).await.unwrap().status,
            JobStatus::Pending
        );
    }
}

#[tokio::test]
async fn a_failure_after_a_stop_ends_cancelled_without_a_retry() {
    let db = TestDb::new().await;
    let job = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        task_enrich_payload(),
    )
    .await;
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();

    let failed = job_queue::fail_job(&db.pool, job, Some(json!({ "error": "killed" })))
        .await
        .unwrap();
    assert_eq!(failed.status, JobStatus::Cancelled);
    assert!(failed.scheduled_at.is_none());
    assert!(failed.completed_at.is_some());
}

#[tokio::test]
async fn the_reaper_ends_a_stopped_run_as_cancelled() {
    let db = TestDb::new().await;
    let job = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        task_enrich_payload(),
    )
    .await;
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE jobs SET started_at = now() - interval '1 hour', \
         heartbeat_at = now() - interval '10 minutes' WHERE id = $1",
    )
    .bind(job)
    .execute(&db.pool)
    .await
    .unwrap();

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, std::time::Duration::from_secs(300))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].status, JobStatus::Cancelled);
}

#[tokio::test]
async fn the_runs_list_shows_a_run_that_is_stopping() {
    let db = TestDb::new().await;
    let viewer = seed_account(&db, "viewer", "Viewer").await;
    let job = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        task_enrich_payload(),
    )
    .await;
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    stop_traject_enrich_run(&db.pool, traject_id(), job)
        .await
        .unwrap();

    let runs = list_traject_runs(&db.pool, traject_id(), viewer, None)
        .await
        .unwrap();
    let run = runs.iter().find(|r| r.job_id == job).unwrap();
    assert_eq!(run.status, JobStatus::Processing);
    assert!(run.cancel_requested_at.is_some());
}

#[tokio::test]
async fn a_stop_while_the_result_is_written_discards_it() {
    let db = TestDb::new().await;
    let job_id = create_run(
        &db,
        JobType::Enrich,
        "wet_op_de_zorgtoeslag",
        Some(TRAJECT_REF),
        task_enrich_payload(),
    )
    .await;
    insert_input_blob(&db, job_id).await;
    let job = job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    stop_traject_enrich_run(&db.pool, traject_id(), job_id)
        .await
        .unwrap();

    let workdir = tempfile::tempdir().unwrap();
    let continuation = regelrecht_pipeline::worker::finish_enrich_task_job_scoped(
        &db.pool,
        &job,
        workdir.path(),
        &[],
        false,
        None,
        Some(vec!["3".to_string()]),
    )
    .await
    .unwrap();
    assert!(continuation.is_none(), "no continuation after a stop");
    assert_eq!(
        job_queue::get_job(&db.pool, job_id).await.unwrap().status,
        JobStatus::Cancelled
    );
    assert_eq!(blob_count(&db, job_id).await, 0);
    let (tasks,): (i64,) = sqlx::query_as("SELECT count(*) FROM tasks WHERE job_id = $1")
        .bind(job_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(tasks, 0);
}
