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
use regelrecht_pipeline::traject_runs::list_traject_runs;

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
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice)
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
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice)
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
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.bob)
        .await
        .unwrap();
    let enrich = runs.iter().find(|r| r.job_id == seeded.enrich).unwrap();
    assert_eq!((enrich.tasks_open, enrich.tasks_open_mine), (1, 1));
}

#[tokio::test]
async fn gives_a_failed_run_its_error_and_a_running_one_its_progress() {
    let db = TestDb::new().await;
    let seeded = seed_runs(&db).await;
    let runs = list_traject_runs(&db.pool, traject_id(), seeded.alice)
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
    let runs = list_traject_runs(&db.pool, traject_id(), Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].provider.as_deref(), Some("requested-provider"));
    assert_eq!(runs[0].requested_by_name, None);
}
