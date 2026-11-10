//! DB-tests voor de law-convert-keten: upload → basis-wet-YAML → geketende
//! taak-flow-enrich-job → law_create-review-taak. Patroon tasks_tests.rs.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::law_convert::{
    GeneratedLaw, LawConvertPayload, ValidatedLawMeta, finish_law_convert_job,
};
use regelrecht_pipeline::models::{JobStatus, JobType};
use regelrecht_pipeline::tasks::{self, BlobKind};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::worker::finish_enrich_task_job;
use regelrecht_shared::RegulatoryLayer;

/// Maak een account + traject om FK's te vullen (patroon tasks_tests.rs).
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

fn convert_payload(
    upload_id: uuid::Uuid,
    traject_id: uuid::Uuid,
    account_id: uuid::Uuid,
) -> LawConvertPayload {
    LawConvertPayload {
        upload_id,
        traject_id,
        traject_ref: "testtraject-abcd1234".to_string(),
        filename: "beleid.pdf".to_string(),
        provider: Some("claude".to_string()),
        requested_by: Some(account_id),
        deliver: Some("task".to_string()),
    }
}

fn generated_law() -> GeneratedLaw {
    GeneratedLaw {
        yaml: "$id: werkinstructie_toetsing\narticles: []\n".to_string(),
        meta: ValidatedLawMeta {
            law_id: "werkinstructie_toetsing".to_string(),
            regulatory_layer: RegulatoryLayer::Uitvoeringsbeleid,
            valid_from: chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        },
    }
}

async fn create_and_claim_convert_job(
    db: &TestDb,
    payload: &LawConvertPayload,
) -> regelrecht_pipeline::models::Job {
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::LawConvert,
            "lawdoc:testtraject-abcd1234/beleid.pdf",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(payload).unwrap())
        .with_max_attempts(1),
    )
    .await
    .unwrap();
    // Claim zodat complete_job ('processing' vereist) slaagt.
    job_queue::claim_job(&db.pool, Some(JobType::LawConvert))
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn test_finish_law_convert_job_chains_enrich_with_input_blob() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let payload = convert_payload(uuid::Uuid::new_v4(), traject_id, account_id);
    let job = create_and_claim_convert_job(&db, &payload).await;

    finish_law_convert_job(&db.pool, &job, &payload, &generated_law())
        .await
        .unwrap();

    // Convert-job completed.
    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Completed);

    // Geketende enrich-job met taak-flow-payload en new_law-marker.
    let enrich = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("geketende enrich-job aanwezig");
    assert_eq!(enrich.law_id, "werkinstructie_toetsing");
    // `traject_ref` is een kolom (geen veld op `Job`); direct uit de rij lezen.
    let (row_ref,): (Option<String>,) =
        sqlx::query_as("SELECT traject_ref FROM jobs WHERE id = $1")
            .bind(enrich.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(row_ref.as_deref(), Some("testtraject-abcd1234"));
    let enrich_payload = enrich.payload.clone().unwrap();
    assert_eq!(enrich_payload["deliver"], "task");
    assert_eq!(enrich_payload["new_law"], true);
    assert_eq!(
        enrich_payload["requested_by"],
        json!(account_id.to_string())
    );

    // De basis-YAML rijdt mee als input-blob op het synthetische pad.
    let inputs = tasks::load_blobs(&db.pool, enrich.id, BlobKind::Input)
        .await
        .unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].path, "laws/werkinstructie_toetsing/law.yaml");
    assert_eq!(
        inputs[0].content,
        "$id: werkinstructie_toetsing\narticles: []\n"
    );
}

#[tokio::test]
async fn test_finish_law_convert_job_conflicts_on_running_enrich() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;

    // Al een actieve enrich voor dezelfde (slug, provider, traject).
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "werkinstructie_toetsing")
            .with_traject_ref("testtraject-abcd1234")
            .with_payload(json!({
                "law_id": "werkinstructie_toetsing",
                "yaml_path": "laws/werkinstructie_toetsing/law.yaml",
                "provider": "claude",
            })),
    )
    .await
    .unwrap();

    let payload = convert_payload(uuid::Uuid::new_v4(), traject_id, account_id);
    let job = create_and_claim_convert_job(&db, &payload).await;

    let err = finish_law_convert_job(&db.pool, &job, &payload, &generated_law())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("loopt al een verrijking"));

    // Rollback: de convert-job is NIET completed (de aanroeper faalt hem
    // daarna terminaal met een job_failed-taak).
    let still = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(still.status, JobStatus::Processing);
}

#[tokio::test]
async fn test_finish_enrich_task_job_new_law_creates_law_create_task() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, "werkinstructie_toetsing").with_payload(json!({
            "law_id": "werkinstructie_toetsing",
            "yaml_path": "laws/werkinstructie_toetsing/law.yaml",
            "provider": "claude",
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
    let law_abs = dir.path().join("laws/werkinstructie_toetsing/law.yaml");
    tokio::fs::create_dir_all(law_abs.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&law_abs, "verrijkt: ja").await.unwrap();

    finish_enrich_task_job(&db.pool, &job, dir.path(), &[law_abs], true, None)
        .await
        .unwrap();

    let open = tasks::list_open_tasks_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].task_type, "job_review");
    assert_eq!(
        open[0].title,
        "Nieuw regelwerk beoordelen: werkinstructie_toetsing"
    );
    let task_payload = open[0].payload.as_ref().unwrap();
    assert_eq!(task_payload["kind"], "law_create");
    assert_eq!(task_payload["law_id"], "werkinstructie_toetsing");
    assert_eq!(task_payload["traject_ref"], "testtraject-abcd1234");
}

#[tokio::test]
async fn test_cleanup_orphaned_uploads_spares_pending_law_convert_upload() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;

    // Upload-rij, kunstmatig oud zodat de 15-min-grace niet beschermt.
    let (upload_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO document_uploads (traject_ref, filename, content_type, bytes, created_at) \
         VALUES ('testtraject-abcd1234', 'beleid.pdf', 'application/pdf', $1, \
                 now() - interval '1 hour') RETURNING id",
    )
    .bind(b"pdf-bytes".to_vec())
    .fetch_one(&db.pool)
    .await
    .unwrap();

    let payload = convert_payload(upload_id, traject_id, account_id);
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::LawConvert,
            "lawdoc:testtraject-abcd1234/beleid.pdf",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&payload).unwrap())
        .with_max_attempts(1),
    )
    .await
    .unwrap();

    // Een wachtende law_convert-job houdt zijn upload vast.
    regelrecht_pipeline::document_convert::cleanup_orphaned_uploads(&db.pool)
        .await
        .unwrap();
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM document_uploads WHERE id = $1")
        .bind(upload_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "upload van een actieve law_convert-job blijft staan"
    );
}

#[tokio::test]
async fn test_running_task_jobs_include_law_convert_with_filename() {
    let db = TestDb::new().await;
    let (account_id, traject_id) = seed_account_and_traject(&db).await;
    let payload = convert_payload(uuid::Uuid::new_v4(), traject_id, account_id);
    job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(
            JobType::LawConvert,
            "lawdoc:testtraject-abcd1234/beleid.pdf",
        )
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&payload).unwrap())
        .with_max_attempts(1),
    )
    .await
    .unwrap();

    let running = tasks::list_running_task_jobs_for_account(&db.pool, account_id)
        .await
        .unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].job_type, JobType::LawConvert);
    // Weergavenaam: de bestandsnaam (COALESCE op payload.filename).
    assert_eq!(running[0].target_path.as_deref(), Some("beleid.pdf"));
}
