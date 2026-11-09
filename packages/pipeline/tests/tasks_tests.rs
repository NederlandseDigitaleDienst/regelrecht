// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use regelrecht_pipeline::document_convert::{
    ConversionRoute, ConvertedDocument, DocumentConvertPayload,
};
use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::JobType;
use regelrecht_pipeline::tasks::{self, BlobKind, NewTask, TaskStatus, TaskType};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::worker::{
    fail_enrich_task_job, fail_enrich_task_job_with_retry, finish_document_convert_task_job,
    finish_enrich_task_job,
};

/// Maak een account + traject om FK's te vullen. tasks.assignee_account_id
/// verwijst naar accounts(id); trajects(created_by) idem.
async fn seed_account_and_traject(db: &TestDb) -> (uuid::Uuid, uuid::Uuid) {
    let (account_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) \
         VALUES ('sub-test', 'tester@example.org', 'Test Persoon') RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let (traject_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, created_by) VALUES ('Testtraject', $1) RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    (account_id, traject_id)
}

#[tokio::test]
async fn test_create_and_list_open_tasks() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_wet"))
        .await
        .unwrap();

    let task = tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobReview,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: Some(job.id),
            title: "Verrijking beoordelen: test_wet".into(),
            payload: Some(json!({"law_id": "test_wet"})),
        },
    )
    .await
    .unwrap();
    assert_eq!(task.status, TaskStatus::Open);

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, task.id);
    assert_eq!(open[0].title, "Verrijking beoordelen: test_wet");

    assert_eq!(
        tasks::count_open_tasks_for_account(&db.pool, account_id)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        tasks::count_open_tasks_for_account(&db.pool, uuid::Uuid::new_v4())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn test_resolve_task_only_by_assignee_and_only_once() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let task = tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobFailed,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: None,
            title: "Conversie mislukt".into(),
            payload: None,
        },
    )
    .await
    .unwrap();

    // Detail-gating: vreemd account ziet de taak niet, assignee wel.
    let stranger_view = tasks::get_task_for_account(&db.pool, task.id, uuid::Uuid::new_v4())
        .await
        .unwrap();
    assert!(stranger_view.is_none());
    let own_view = tasks::get_task_for_account(&db.pool, task.id, account_id)
        .await
        .unwrap();
    assert_eq!(own_view.expect("assignee ziet eigen taak").id, task.id);

    // Vreemd account mag niet resolven → None.
    let stranger = uuid::Uuid::new_v4();
    let denied = tasks::resolve_task(&db.pool, task.id, stranger, TaskStatus::Dismissed)
        .await
        .unwrap();
    assert!(denied.is_none());

    // Assignee wel.
    let resolved = tasks::resolve_task(&db.pool, task.id, account_id, TaskStatus::Dismissed)
        .await
        .unwrap()
        .expect("assignee mag resolven");
    assert_eq!(resolved.status, TaskStatus::Dismissed);
    assert_eq!(resolved.resolved_by, Some(account_id));
    assert!(resolved.resolved_at.is_some());

    // Tweede keer resolven → None (status is niet meer 'open').
    let again = tasks::resolve_task(&db.pool, task.id, account_id, TaskStatus::Approved)
        .await
        .unwrap();
    assert!(again.is_none());

    // Resolve naar 'open' is geen afhandeling en wordt geweigerd.
    let task2 = tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobFailed,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: None,
            title: "Nog een taak".into(),
            payload: None,
        },
    )
    .await
    .unwrap();
    let reopened = tasks::resolve_task(&db.pool, task2.id, account_id, TaskStatus::Open)
        .await
        .unwrap();
    assert!(reopened.is_none());
}

#[tokio::test]
async fn test_blob_roundtrip_and_delete() {
    let db = TestDb::new().await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_wet"))
        .await
        .unwrap();

    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        "a: 1",
    )
    .await
    .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Result,
        "laws/test_wet/law.yaml",
        "a: 2",
    )
    .await
    .unwrap();

    let results = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "a: 2");

    tasks::delete_blobs_for_job(&db.pool, job.id).await.unwrap();
    let after = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn test_blobs_survive_until_the_last_per_article_task_is_resolved() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_a"))
        .await
        .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Result,
        "laws/wet_a.yaml",
        "a: 2",
    )
    .await
    .unwrap();
    // Eén verrijking, twee gewijzigde artikelen, dus twee taken op dezelfde
    // resultaat-blob.
    let mut task_ids = Vec::new();
    for article in ["1", "2"] {
        let task = tasks::create_task(
            &db.pool,
            NewTask {
                task_type: TaskType::JobReview,
                assignee_account_id: Some(account_id),
                traject_id: Some(traject_id),
                job_id: Some(job.id),
                title: format!("artikel {article}"),
                payload: None,
            },
        )
        .await
        .unwrap();
        task_ids.push(task.id);
    }

    tasks::resolve_task(&db.pool, task_ids[0], account_id, TaskStatus::Approved)
        .await
        .unwrap();
    let removed = tasks::delete_blobs_for_finished_job(&db.pool, job.id)
        .await
        .unwrap();
    assert_eq!(
        removed, 0,
        "de tweede taak moet zijn voorstel nog kunnen laden"
    );
    assert_eq!(
        tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
            .await
            .unwrap()
            .len(),
        1
    );

    tasks::resolve_task(&db.pool, task_ids[1], account_id, TaskStatus::Rejected)
        .await
        .unwrap();
    let removed = tasks::delete_blobs_for_finished_job(&db.pool, job.id)
        .await
        .unwrap();
    assert_eq!(removed, 1);
    assert!(tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_cleanup_orphaned_blobs_keeps_open_task_blobs() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let kept_job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_a"))
        .await
        .unwrap();
    let orphan_job =
        job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_b"))
            .await
            .unwrap();
    tasks::insert_blob(&db.pool, kept_job.id, BlobKind::Result, "p", "kept")
        .await
        .unwrap();
    tasks::insert_blob(&db.pool, orphan_job.id, BlobKind::Result, "p", "orphan")
        .await
        .unwrap();
    tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobReview,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id: Some(kept_job.id),
            title: "t".into(),
            payload: None,
        },
    )
    .await
    .unwrap();
    // Backdate beide blobs voorbij de 7-dagen grace zodat alleen het
    // open-taak-criterium onderscheidt.
    sqlx::query("UPDATE job_blobs SET created_at = now() - interval '8 days'")
        .execute(&db.pool)
        .await
        .unwrap();

    let removed = tasks::cleanup_orphaned_blobs(&db.pool).await.unwrap();
    assert_eq!(removed, 1);
    assert_eq!(
        tasks::load_blobs(&db.pool, kept_job.id, BlobKind::Result)
            .await
            .unwrap()
            .len(),
        1
    );
}

/// `dismiss_open_tasks_for_traject` belooft twee dingen die de aanroeper in
/// `trajects::delete` niet zelf nog eens nagaat: alléén de open taken van dít
/// traject gaan dicht, en de teruggegeven job-ids zijn ontdubbeld. Dat laatste
/// is geen detail — één verrijking levert een taak per gewijzigd artikel op
/// dezelfde job, dus zonder ontdubbeling ruimt de aanroeper diezelfde job zo
/// vaak op als er artikelen wijzigden.
#[tokio::test]
async fn test_dismiss_open_tasks_for_traject_scopes_and_dedupes() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let (other_traject_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, created_by) VALUES ('Ander traject', $1) RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_a"))
        .await
        .unwrap();
    // Twee taken op dezelfde job (per-artikel), één zonder job, één elders.
    let article_1 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    let article_2 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    let jobless = seed_open_task(&db, account_id, traject_id, None).await;
    let elsewhere = seed_open_task(&db, account_id, other_traject_id, Some(job.id)).await;
    // Een al afgehandelde taak van hetzelfde traject blijft zoals hij is.
    let resolved = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    tasks::resolve_task(&db.pool, resolved.id, account_id, TaskStatus::Approved)
        .await
        .unwrap()
        .expect("assignee mag resolven");

    let job_ids = tasks::dismiss_open_tasks_for_traject(&db.pool, traject_id)
        .await
        .unwrap();
    assert_eq!(
        job_ids,
        vec![job.id],
        "dezelfde job hoort één keer terug te komen, en de job-loze taak levert er geen"
    );

    for id in [article_1.id, article_2.id, jobless.id] {
        let (status,): (TaskStatus,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(status, TaskStatus::Dismissed);
    }
    let (status,): (TaskStatus,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
        .bind(elsewhere.id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        status,
        TaskStatus::Open,
        "een taak van een ander traject op dezelfde job blijft open"
    );
    let (status,): (TaskStatus,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
        .bind(resolved.id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(status, TaskStatus::Approved);
}

#[tokio::test]
async fn test_list_tasks_for_job_returns_every_part_of_one_enrichment() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_a"))
        .await
        .unwrap();
    let other_job =
        job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_b"))
            .await
            .unwrap();
    let article_1 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    let article_2 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    let elsewhere = seed_open_task(&db, account_id, traject_id, Some(other_job.id)).await;

    // Een afgehandeld onderdeel komt mee: wie de verrijking verwerkt moet
    // kunnen zien dat een zusje buiten hem om al dichtging.
    tasks::resolve_task(&db.pool, article_2.id, account_id, TaskStatus::Rejected)
        .await
        .unwrap()
        .expect("assignee mag resolven");

    let parts = tasks::list_tasks_for_job_and_account(&db.pool, job.id, account_id)
        .await
        .unwrap();
    let ids: Vec<uuid::Uuid> = parts.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![article_1.id, article_2.id]);
    assert!(!ids.contains(&elsewhere.id), "andere job hoort er niet bij");
    assert_eq!(parts[1].status, TaskStatus::Rejected);

    // Andermans account ziet niets, ook niet van een job die bestaat.
    assert!(
        tasks::list_tasks_for_job_and_account(&db.pool, job.id, uuid::Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn test_resolve_tasks_closes_a_whole_enrichment_and_counts_what_it_closed() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let job = job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "wet_a"))
        .await
        .unwrap();
    let article_1 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    let article_2 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;
    let article_3 = seed_open_task(&db, account_id, traject_id, Some(job.id)).await;

    let approved = tasks::resolve_tasks(
        &db.pool,
        &[article_1.id, article_2.id],
        account_id,
        TaskStatus::Approved,
    )
    .await
    .unwrap();
    assert_eq!(approved, 2);

    // Al dicht, of van een ander account: die tellen niet mee, en dat is
    // precies het signaal waarop de aanroeper de hele verwerking terugrolt.
    let again = tasks::resolve_tasks(
        &db.pool,
        &[article_1.id, article_3.id],
        account_id,
        TaskStatus::Rejected,
    )
    .await
    .unwrap();
    assert_eq!(again, 1);
    assert_eq!(
        tasks::resolve_tasks(&db.pool, &[], account_id, TaskStatus::Rejected)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        tasks::resolve_tasks(
            &db.pool,
            &[article_1.id],
            uuid::Uuid::new_v4(),
            TaskStatus::Rejected
        )
        .await
        .unwrap(),
        0
    );

    let parts = tasks::list_tasks_for_job_and_account(&db.pool, job.id, account_id)
        .await
        .unwrap();
    let statuses: Vec<TaskStatus> = parts.iter().map(|t| t.status).collect();
    assert_eq!(
        statuses,
        vec![
            TaskStatus::Approved,
            TaskStatus::Approved,
            TaskStatus::Rejected
        ]
    );
}

/// Open `job_review`-taak met de minimale velden die deze test nodig heeft.
async fn seed_open_task(
    db: &TestDb,
    account_id: uuid::Uuid,
    traject_id: uuid::Uuid,
    job_id: Option<uuid::Uuid>,
) -> tasks::Task {
    tasks::create_task(
        &db.pool,
        NewTask {
            task_type: TaskType::JobReview,
            assignee_account_id: Some(account_id),
            traject_id: Some(traject_id),
            job_id,
            title: "t".into(),
            payload: None,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn test_finish_enrich_task_job_creates_task_and_result_blobs() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "provider": "claude",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234",
            "source_etag": "\"etag-1\""
        })),
    )
    .await
    .unwrap();
    // Claim zodat complete_job ('processing' vereist) slaagt.
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();

    // Simuleer een geslaagde enrichment: een werkdirectory met output.
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "verrijkt: ja").await.unwrap();

    let next = finish_enrich_task_job(
        &db.pool,
        &job,
        dir.path(),
        std::slice::from_ref(&law_abs),
        true,
        Some(json!({"coverage_score": 1.0})),
    )
    .await
    .unwrap();
    assert!(next.is_none(), "een afgeronde wet krijgt geen vervolgjob");

    // Job completed, result-blob + taak aanwezig.
    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        done.status,
        regelrecht_pipeline::models::JobStatus::Completed
    );
    let results = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "laws/test_wet/law.yaml");
    assert_eq!(results[0].content, "verrijkt: ja");
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_review");
    assert_eq!(open[0].job_id, Some(job.id));
    // Zonder bronsnapshot valt de worker terug op één taak voor de hele wet,
    // en die draagt dan ook geen artikelnummer.
    assert!(open[0].payload.as_ref().unwrap().get("article").is_none());
}

#[tokio::test]
async fn test_finish_enrich_task_job_creates_one_task_per_changed_article() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "provider": "claude",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234",
            "source_etag": "\"etag-1\""
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();

    // De bronsnapshot zoals de worker die bij het starten wegschrijft.
    let source = "articles:\n  - number: '1'\n    text: een\n  - number: '2'\n    text: twee\n";
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        source,
    )
    .await
    .unwrap();

    // Het voorstel raakt alleen artikel 2.
    let proposal =
        "articles:\n  - number: '1'\n    text: een\n  - number: '2'\n    text: twee bis\n";
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, proposal).await.unwrap();

    finish_enrich_task_job(
        &db.pool,
        &job,
        dir.path(),
        std::slice::from_ref(&law_abs),
        true,
        None,
    )
    .await
    .unwrap();

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(
        open.len(),
        1,
        "alleen het gewijzigde artikel krijgt een taak"
    );
    assert_eq!(
        open[0].payload.as_ref().unwrap().get("article").unwrap(),
        "2"
    );
    assert_eq!(open[0].title, "Verrijking beoordelen: test_wet artikel 2");
}

#[tokio::test]
async fn test_finish_enrich_task_job_mid_law_chains_next_window() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet")
            .with_traject_ref("testtraject-abcd1234")
            .with_max_attempts(3)
            .with_payload(json!({
                "law_id": "test_wet",
                "yaml_path": "laws/test_wet/law.yaml",
                "provider": "claude",
                "requested_by": account_id,
                "deliver": "task",
                "traject_id": traject_id,
                "traject_ref": "testtraject-abcd1234",
                "source_etag": "\"etag-1\""
            })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    let source = "articles:\n  - number: '1'\n    text: een\n  - number: '2'\n    text: twee\n";
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        source,
    )
    .await
    .unwrap();

    // Het eerste venster verrijkte artikel 1; de cursor staat op 1.
    let proposal =
        "articles:\n  - number: '1'\n    text: een bis\n  - number: '2'\n    text: twee\n";
    let sidecar = "enrich_cursor: 1\nenrich_cursor_path: laws/test_wet/law.yaml\n";
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    let meta_abs = dir.path().join("laws/test_wet/.enrichment.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, proposal).await.unwrap();
    tokio::fs::write(&meta_abs, sidecar).await.unwrap();

    let next = finish_enrich_task_job(
        &db.pool,
        &job,
        dir.path(),
        &[law_abs, meta_abs],
        false,
        None,
    )
    .await
    .unwrap()
    .expect("een wet die niet af is krijgt een vervolgjob");

    // Dit venster staat als review-taak klaar, los van het volgende.
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].job_id, Some(job.id));
    assert_eq!(
        open[0].payload.as_ref().unwrap().get("article").unwrap(),
        "1"
    );

    // De vervolgjob is dezelfde taak-flow-aanvraag, voor dezelfde aanvrager.
    let next = job_queue::get_job(&db.pool, next.id).await.unwrap();
    assert_eq!(next.max_attempts, 3);
    assert_eq!(next.priority, job.priority);
    let payload = next.payload.unwrap();
    assert_eq!(payload["deliver"], "task");
    assert_eq!(payload["requested_by"], json!(account_id));
    assert_eq!(payload["traject_id"], json!(traject_id));

    // Zijn input is het voorstel plus de cursor: daar bouwt het volgende
    // venster op voort.
    let mut inputs = tasks::load_blobs(&db.pool, next.id, BlobKind::Input)
        .await
        .unwrap();
    inputs.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].path, "laws/test_wet/.enrichment.yaml");
    assert_eq!(inputs[0].content, sidecar);
    assert_eq!(inputs[1].path, "laws/test_wet/law.yaml");
    assert_eq!(inputs[1].content, proposal);

    // Het tweede venster verrijkt artikel 2. Zijn diff loopt tegen het eerste
    // voorstel, dus artikel 1 komt niet opnieuw als taak terug.
    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, next.id);
    let final_proposal =
        "articles:\n  - number: '1'\n    text: een bis\n  - number: '2'\n    text: twee bis\n";
    let dir2 = tempfile::tempdir().unwrap();
    let law2 = dir2.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law2.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law2, final_proposal).await.unwrap();
    let after = finish_enrich_task_job(&db.pool, &claimed, dir2.path(), &[law2], true, None)
        .await
        .unwrap();
    assert!(after.is_none());
    let second: Vec<_> = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|t| t.job_id == Some(claimed.id))
        .collect();
    assert_eq!(second.len(), 1);
    assert_eq!(
        second[0].payload.as_ref().unwrap().get("article").unwrap(),
        "2"
    );
}

#[tokio::test]
async fn test_finish_enrich_task_job_mid_law_unreadable_proposal_fails_the_window() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234"
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    let source = "articles:\n  - number: '1'\n    text: een\n";
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        source,
    )
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "articles: [kapot")
        .await
        .unwrap();

    // Halverwege de wet geen terugval op één taak voor het geheel: dat
    // voorstel zou de eerdere vensters ongezien terugzetten.
    let result = finish_enrich_task_job(&db.pool, &job, dir.path(), &[law_abs], false, None).await;
    assert!(result.is_err());

    // Alles rolde terug: geen taak, geen vervolgjob, de input staat er nog
    // voor de herhaling.
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert!(open.is_empty());
    let inputs = tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert!(job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_finish_enrich_task_job_last_window_unreadable_proposal_fails_too() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234"
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    // Een vervolgjob: het voorstel van het vorige venster plus de cursor.
    let previous = "articles:\n  - number: '1'\n    text: een bis\n";
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        previous,
    )
    .await
    .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/.enrichment.yaml",
        "enrich_cursor: 1\n",
    )
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "articles: [kapot")
        .await
        .unwrap();

    // Ook het laatste venster van een reeks valt niet terug op één taak voor
    // de hele wet: die zou artikel 1 uit het vorige venster meenemen.
    let result = finish_enrich_task_job(&db.pool, &job, dir.path(), &[law_abs], true, None).await;
    assert!(result.is_err());
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert!(open.is_empty());
}

#[tokio::test]
async fn test_finish_enrich_task_job_single_window_unreadable_proposal_falls_back_to_whole_law() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234"
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    // De eerste (en enige) job: alleen de traject-snapshot, geen cursor.
    let source = "articles:\n  - number: '1'\n    text: een\n";
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        source,
    )
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "articles: [kapot")
        .await
        .unwrap();

    // Past de wet in één venster, dan is de terugval op één taak veilig: het
    // voorstel bevat niets van eerdere vensters.
    finish_enrich_task_job(&db.pool, &job, dir.path(), &[law_abs], true, None)
        .await
        .unwrap();
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert!(open[0].payload.as_ref().unwrap().get("article").is_none());
}

#[tokio::test]
async fn test_finish_enrich_task_job_unchanged_proposal_creates_no_task() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234"
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    let law = "articles:\n  - number: '1'\n    text: een\n";
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        law,
    )
    .await
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, law).await.unwrap();

    // Een venster dat niets veranderde levert geen taak op, ook geen taak
    // voor de hele wet.
    finish_enrich_task_job(&db.pool, &job, dir.path(), &[law_abs], true, None)
        .await
        .unwrap();
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert!(open.is_empty());
}

#[tokio::test]
async fn test_finish_enrich_task_job_new_law_mid_law_creates_no_task_yet() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet")
            .with_traject_ref("testtraject-abcd1234")
            .with_payload(json!({
                "law_id": "test_wet",
                "yaml_path": "laws/test_wet/law.yaml",
                "requested_by": account_id,
                "deliver": "task",
                "traject_id": traject_id,
                "traject_ref": "testtraject-abcd1234",
                "new_law": true
            })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let law_abs = dir.path().join("laws/test_wet/law.yaml");
    let feature_abs = dir.path().join("features/vrije naam.feature");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::create_dir_all(feature_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "articles: []\n").await.unwrap();
    tokio::fs::write(&feature_abs, "Feature: x\n")
        .await
        .unwrap();

    let next = finish_enrich_task_job(
        &db.pool,
        &job,
        dir.path(),
        &[law_abs, feature_abs],
        false,
        None,
    )
    .await
    .unwrap()
    .expect("ook een nieuwe wet loopt in vensters");

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert!(
        open.is_empty(),
        "een half aangemaakte wet is nog niet te beoordelen"
    );
    // Alleen de wet reist mee, niet de feature-file van de agent.
    let inputs = tasks::load_blobs(&db.pool, next.id, BlobKind::Input)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].path, "laws/test_wet/law.yaml");
}

#[tokio::test]
async fn test_finish_document_convert_task_job_creates_task_and_result_blob() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let payload = DocumentConvertPayload {
        upload_id: uuid::Uuid::new_v4(),
        traject_id,
        traject_ref: "testtraject-abcd1234".to_string(),
        target_path: "report.md".to_string(),
        provider: None,
        requested_by: Some(account_id),
        deliver: Some("task".to_string()),
        allow_llm: false,
    };
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::DocumentConvert,
            "doc:testtraject-abcd1234/report.md",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&payload).unwrap())
        .with_max_attempts(1),
    )
    .await
    .unwrap();
    // Claim zodat complete_job ('processing' vereist) slaagt.
    let job = job_queue::claim_job(&db.pool, Some(JobType::DocumentConvert))
        .await
        .unwrap()
        .unwrap();

    finish_document_convert_task_job(
        &db.pool,
        &job,
        &payload,
        &ConvertedDocument {
            markdown: "# Report\n\nBody.\n".to_string(),
            route: ConversionRoute::Pandoc,
        },
    )
    .await
    .unwrap();

    // Job completed, result-blob met target_path + markdown.
    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        done.status,
        regelrecht_pipeline::models::JobStatus::Completed
    );
    let results = tasks::load_blobs(&db.pool, job.id, BlobKind::Result)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "report.md");
    assert_eq!(results[0].content, "# Report\n\nBody.\n");

    // job_review-taak met kind=document/traject_ref/target_path, juiste assignee.
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_review");
    assert_eq!(open[0].job_id, Some(job.id));
    assert_eq!(open[0].traject_id, Some(traject_id));
    let task_payload = open[0].payload.as_ref().unwrap();
    assert_eq!(task_payload["kind"], "document");
    assert_eq!(task_payload["traject_ref"], "testtraject-abcd1234");
    assert_eq!(task_payload["target_path"], "report.md");
}

#[tokio::test]
async fn test_open_document_task_target_paths_reserves_names_until_resolved() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let (other_traject_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, created_by) VALUES ('Ander traject', $1) RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    let payload = DocumentConvertPayload {
        upload_id: uuid::Uuid::new_v4(),
        traject_id,
        traject_ref: "testtraject-abcd1234".to_string(),
        target_path: "report.md".to_string(),
        provider: None,
        requested_by: Some(account_id),
        deliver: Some("task".to_string()),
        allow_llm: false,
    };
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::DocumentConvert,
            "doc:testtraject-abcd1234/report.md",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&payload).unwrap())
        .with_max_attempts(1),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::DocumentConvert))
        .await
        .unwrap()
        .unwrap();

    // Nog geen taak - lege reservering.
    assert!(tasks::open_document_task_target_paths(&db.pool, traject_id)
        .await
        .unwrap()
        .is_empty());

    // Task-delivered conversie: de job is 'completed' (dus buiten
    // `pending_target_paths`), maar de review is nog open - de naam moet nu
    // gereserveerd zijn.
    finish_document_convert_task_job(
        &db.pool,
        &job,
        &payload,
        &ConvertedDocument {
            markdown: "# Report\n\nBody.\n".to_string(),
            route: ConversionRoute::Pandoc,
        },
    )
    .await
    .unwrap();

    let reserved = tasks::open_document_task_target_paths(&db.pool, traject_id)
        .await
        .unwrap();
    assert_eq!(reserved, vec!["report.md".to_string()]);

    // Een ander traject ziet de reservering niet, ongeacht dezelfde naam.
    assert!(
        tasks::open_document_task_target_paths(&db.pool, other_traject_id)
            .await
            .unwrap()
            .is_empty()
    );

    // Na goedkeuren (of afwijzen) van de review-taak vervalt de reservering.
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    tasks::resolve_task(&db.pool, open[0].id, account_id, TaskStatus::Approved)
        .await
        .unwrap()
        .expect("assignee mag resolven");

    assert!(tasks::open_document_task_target_paths(&db.pool, traject_id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_fail_enrich_task_job_creates_failed_task_and_cleans_input() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet").with_payload(json!({
            "law_id": "test_wet",
            "yaml_path": "laws/test_wet/law.yaml",
            "requested_by": account_id,
            "deliver": "task",
            "traject_id": traject_id
        })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        "x",
    )
    .await
    .unwrap();

    fail_enrich_task_job(&db.pool, &job, "LLM produceerde niets")
        .await
        .unwrap();

    let failed = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        failed.status,
        regelrecht_pipeline::models::JobStatus::Failed
    );
    assert!(tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
        .await
        .unwrap()
        .is_empty());
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_failed");
}

#[tokio::test]
async fn test_fail_with_retry_creates_task_only_when_attempts_exhausted() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let _created = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet")
            .with_max_attempts(2)
            .with_payload(json!({
                "law_id": "test_wet",
                "yaml_path": "laws/test_wet/law.yaml",
                "requested_by": account_id,
                "deliver": "task",
                "traject_id": traject_id
            })),
    )
    .await
    .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    tasks::insert_blob(
        &db.pool,
        job.id,
        BlobKind::Input,
        "laws/test_wet/law.yaml",
        "x",
    )
    .await
    .unwrap();

    // Eerste fout: attempts over → pending met backoff, GEEN taak, blobs blijven.
    fail_enrich_task_job_with_retry(&db.pool, &job, "transiente fout")
        .await
        .unwrap();
    let after_first = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        after_first.status,
        regelrecht_pipeline::models::JobStatus::Pending
    );
    assert!(tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
            .await
            .unwrap()
            .len(),
        1
    );

    // Backoff resetten zodat de job direct claimbaar is voor de tweede poging.
    sqlx::query("UPDATE jobs SET scheduled_at = NULL WHERE id = $1")
        .bind(job.id)
        .execute(&db.pool)
        .await
        .unwrap();
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();

    // Tweede fout: attempts uitgeput → Failed + taak + blobs weg.
    fail_enrich_task_job_with_retry(&db.pool, &job, "transiente fout")
        .await
        .unwrap();
    let after_second = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(
        after_second.status,
        regelrecht_pipeline::models::JobStatus::Failed
    );
    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_failed");
    assert!(tasks::load_blobs(&db.pool, job.id, BlobKind::Input)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_list_running_task_jobs_for_account() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let (other_account_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO accounts (person_sub, email, name) \
         VALUES ('sub-test-2', 'tester2@example.org', 'Andere Persoon') RETURNING id",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    let (other_traject_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO trajects (name, created_by) VALUES ('Ander traject', $1) RETURNING id",
    )
    .bind(other_account_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();

    // Eigen taak-flow-job: deliver=task + requested_by=account_id, nog
    // pending → moet in de "Bezig"-lijst verschijnen.
    let own_job = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet")
            .with_traject_ref("testtraject-abcd1234")
            .with_payload(json!({
                "law_id": "test_wet",
                "yaml_path": "laws/test_wet/law.yaml",
                "requested_by": account_id,
                "deliver": "task",
                "traject_id": traject_id,
                "traject_ref": "testtraject-abcd1234"
            })),
    )
    .await
    .unwrap();

    // Eigen document_convert-taak-flow-job: hoort ook in de "Bezig"-lijst,
    // met target_path als weergave-handvat (law_id is een doc:-sleutel).
    let own_doc_job = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::DocumentConvert,
            "doc:testtraject-abcd1234/analyses/rapport.md",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(json!({
            "upload_id": uuid::Uuid::new_v4(),
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234",
            "target_path": "analyses/rapport.md",
            "requested_by": account_id,
            "deliver": "task"
        })),
    )
    .await
    .unwrap();

    // Corpus-brede job (geen deliver/requested_by) voor dezelfde wet - hoort
    // niet in de taak-flow-lijst thuis, ongeacht account.
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "andere_wet")
            .with_payload(json!({"law_id": "andere_wet", "yaml_path": "x.yaml"})),
    )
    .await
    .unwrap();

    // Taak-flow-job van een ander account - hoort niet in de lijst van
    // account_id thuis.
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "vreemde_wet").with_payload(json!({
            "law_id": "vreemde_wet",
            "yaml_path": "y.yaml",
            "requested_by": other_account_id,
            "deliver": "task",
            "traject_id": other_traject_id
        })),
    )
    .await
    .unwrap();

    let running = tasks::list_running_task_jobs_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(running.len(), 2);
    // Nieuwste eerst: de document_convert-job is na de enrich-job aangemaakt.
    assert_eq!(running[0].job_id, own_doc_job.id);
    assert_eq!(running[0].job_type, JobType::DocumentConvert);
    assert_eq!(
        running[0].target_path.as_deref(),
        Some("analyses/rapport.md")
    );
    assert_eq!(running[1].job_id, own_job.id);
    assert_eq!(running[1].job_type, JobType::Enrich);
    assert_eq!(running[1].law_id, "test_wet");
    assert_eq!(running[1].target_path, None);
    assert_eq!(
        running[1].traject_ref.as_deref(),
        Some("testtraject-abcd1234")
    );
    assert_eq!(
        running[1].status,
        regelrecht_pipeline::models::JobStatus::Pending
    );

    // Claim + complete de eigen enrich-job: eenmaal afgerond hoort hij niet
    // meer in de "Bezig"-lijst (hij verschijnt dan als job_review-taak
    // i.p.v.); de nog lopende document_convert-job blijft staan.
    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, own_job.id);
    job_queue::complete_job(&db.pool, own_job.id, None)
        .await
        .unwrap();

    let after = tasks::list_running_task_jobs_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].job_id, own_doc_job.id);
}

/// `include_failed = false` (taken-mechanisme, the editor-api's mode) ->
/// failed jobs excluded, the uploader gets a `job_failed` task instead.
/// `include_failed = true` -> pre-taken-mechanisme behaviour, failed rows
/// included with their `error`.
#[tokio::test]
async fn test_document_jobs_list_excludes_failed() {
    let db = TestDb::new().await;
    let req = CreateJobRequest::new(JobType::DocumentConvert, "doc:testtraject-abcd1234/a.md")
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(json!({
            "upload_id": uuid::Uuid::new_v4(),
            "traject_id": uuid::Uuid::new_v4(),
            "traject_ref": "testtraject-abcd1234",
            "target_path": "a.md"
        }))
        .with_max_attempts(1);
    let job = job_queue::create_job(&db.pool, req).await.unwrap();
    let claimed = job_queue::claim_job(&db.pool, Some(JobType::DocumentConvert))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, job.id);
    job_queue::fail_job_terminal(
        &db.pool,
        job.id,
        Some(json!({ "error": "conversie mislukt" })),
    )
    .await
    .unwrap();

    let views_flag_on = regelrecht_pipeline::document_convert::list_traject_document_jobs(
        &db.pool,
        "testtraject-abcd1234",
        false,
    )
    .await
    .unwrap();
    assert!(
        views_flag_on.is_empty(),
        "flag aan: failed jobs horen niet meer in de documentenlijst"
    );

    let views_flag_off = regelrecht_pipeline::document_convert::list_traject_document_jobs(
        &db.pool,
        "testtraject-abcd1234",
        true,
    )
    .await
    .unwrap();
    assert_eq!(
        views_flag_off.len(),
        1,
        "flag uit: failed jobs horen weer in de documentenlijst"
    );
    assert_eq!(
        views_flag_off[0].error.as_deref(),
        Some("conversie mislukt")
    );
}

#[tokio::test]
async fn test_traject_enrich_not_blocked_by_corpus_enrich() {
    let db = TestDb::new().await;

    // Corpus-brede enrich-job (geen traject) voor dezelfde wet + provider.
    let corpus_req = CreateJobRequest::new(JobType::Enrich, "test_wet")
        .with_payload(json!({"law_id": "test_wet", "yaml_path": "x.yaml", "provider": "claude"}));
    let corpus_job = job_queue::create_enrich_job_if_not_exists(&db.pool, corpus_req)
        .await
        .unwrap();
    assert!(corpus_job.is_some());

    // Traject-taak-aanvraag voor dezelfde wet + provider: mag NIET blokkeren.
    let traject_req = CreateJobRequest::new(JobType::Enrich, "test_wet")
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(json!({
            "law_id": "test_wet", "yaml_path": "x.yaml", "provider": "claude",
            "deliver": "task"
        }));
    let traject_job = job_queue::create_enrich_job_if_not_exists(&db.pool, traject_req)
        .await
        .unwrap();
    assert!(
        traject_job.is_some(),
        "traject-aanvraag mag niet botsen met corpus-job"
    );

    // Maar een tweede aanvraag binnen HETZELFDE traject is wél een duplicaat.
    let dup_req = CreateJobRequest::new(JobType::Enrich, "test_wet")
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(json!({
            "law_id": "test_wet", "yaml_path": "x.yaml", "provider": "claude",
            "deliver": "task"
        }));
    let dup = job_queue::create_enrich_job_if_not_exists(&db.pool, dup_req)
        .await
        .unwrap();
    assert!(
        dup.is_none(),
        "duplicaat binnen zelfde traject moet geweigerd worden"
    );

    // En een tweede corpus-brede job ook (bestaand gedrag blijft).
    let dup_corpus = CreateJobRequest::new(JobType::Enrich, "test_wet")
        .with_payload(json!({"law_id": "test_wet", "yaml_path": "x.yaml", "provider": "claude"}));
    let dup_corpus = job_queue::create_enrich_job_if_not_exists(&db.pool, dup_corpus)
        .await
        .unwrap();
    assert!(dup_corpus.is_none());
}

/// Door de reaper terminaal gefaalde taak-flow-jobs moeten alsnog een
/// job_failed-taak opleveren; her-pogingen (terug naar pending) en
/// niet-taak-flow-jobs niet.
#[tokio::test]
async fn test_notify_reaped_task_jobs_creates_failed_tasks() {
    use std::time::Duration;

    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;

    // 1. Terminale taak-flow law_convert (max_attempts=1).
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::LawConvert,
            "lawdoc:testtraject-abcd1234/beleid.pdf",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_max_attempts(1)
        .with_payload(json!({
            "upload_id": uuid::Uuid::new_v4(),
            "traject_id": traject_id,
            "traject_ref": "testtraject-abcd1234",
            "filename": "beleid.pdf",
            "requested_by": account_id,
            "deliver": "task",
        })),
    )
    .await
    .unwrap();
    // 2. Taak-flow enrich met retries over: reap zet hem terug naar pending,
    //    dus géén taak.
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "test_wet")
            .with_max_attempts(3)
            .with_payload(json!({
                "law_id": "test_wet",
                "yaml_path": "laws/test_wet/law.yaml",
                "requested_by": account_id,
                "deliver": "task",
            })),
    )
    .await
    .unwrap();
    // 3. Niet-taak-flow harvest, terminaal: reaped maar geen taak.
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000000").with_max_attempts(1),
    )
    .await
    .unwrap();

    // Claim alle drie en antidateer ze voorbij de reaper-window.
    while job_queue::claim_job(&db.pool, None)
        .await
        .unwrap()
        .is_some()
    {}
    sqlx::query("UPDATE jobs SET started_at = now() - interval '2 hours', heartbeat_at = now() - interval '2 hours'")
        .execute(&db.pool)
        .await
        .unwrap();

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 3);
    tasks::notify_reaped_task_jobs(&db.pool, &reaped)
        .await
        .unwrap();

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(
        open.len(),
        1,
        "alleen de terminale taak-flow-job krijgt een taak"
    );
    assert_eq!(open[0].task_type, "job_failed");
    assert_eq!(
        open[0].title,
        "Conversie naar regelwerk mislukt: beleid.pdf"
    );
    assert_eq!(open[0].traject_id, Some(traject_id));
    let payload = open[0].payload.as_ref().unwrap();
    assert_eq!(payload["traject_ref"], "testtraject-abcd1234");
    assert!(payload["error"].as_str().unwrap().contains("afgebroken"));
}

/// De nieuwe-wet-variant van een gereapte enrich krijgt de law_create-titel.
#[tokio::test]
async fn test_notify_reaped_new_law_enrich_gets_law_create_title() {
    use std::time::Duration;

    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "werkinstructie_toetsing")
            .with_max_attempts(1)
            .with_payload(json!({
                "law_id": "werkinstructie_toetsing",
                "yaml_path": "laws/werkinstructie_toetsing/law.yaml",
                "requested_by": account_id,
                "deliver": "task",
                "traject_id": traject_id,
                "traject_ref": "testtraject-abcd1234",
                "new_law": true,
            })),
    )
    .await
    .unwrap();
    job_queue::claim_job(&db.pool, None).await.unwrap().unwrap();
    sqlx::query("UPDATE jobs SET started_at = now() - interval '2 hours', heartbeat_at = now() - interval '2 hours'")
        .execute(&db.pool)
        .await
        .unwrap();

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, Duration::from_secs(60))
        .await
        .unwrap();
    tasks::notify_reaped_task_jobs(&db.pool, &reaped)
        .await
        .unwrap();

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(
        open[0].title,
        "Regelwerk aanmaken mislukt: werkinstructie_toetsing"
    );
}
