//! DB-integration tests for the chunked-enrichment completion contract:
//! `worker::complete_enrich_success_tx` must complete the job and create the
//! continuation job in ONE transaction, respecting the partial unique index
//! `idx_unique_active_enrich_job`, and drive the `enriching` → `enriched`
//! status machine without ever stranding a law in `enriching` without an
//! active/pending job.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use regelrecht_pipeline::enrich::{EnrichPayload, EnrichResult, Pass};
use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::models::{Job, JobStatus, JobType, LawStatusValue, Priority};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::{law_status, worker};

const LAW_ID: &str = "BWBR0099999";
const YAML_PATH: &str = "regulation/nl/wet/test_wet/2025-01-01.yaml";

fn payload(provider: &str) -> EnrichPayload {
    EnrichPayload {
        law_id: LAW_ID.into(),
        yaml_path: YAML_PATH.into(),
        provider: Some(provider.into()),
        depth: Some(1),
        requested_by: None,
        deliver: None,
        traject_id: None,
        traject_ref: None,
        source_etag: None,
        new_law: None,
        chunk_articles: None,
        skip_mvt: None,
        // Niet geserialiseerd; een payload uit de wachtrij is altijd een
        // vertaalslag, en de terugkoppelpassen ontstaan binnen de run.
        pass: Pass::Translate,
        session: None,
    }
}

fn chunk_result(provider: &str, law_complete: bool, enrich_cursor: usize) -> EnrichResult {
    EnrichResult {
        law_id: LAW_ID.into(),
        yaml_path: YAML_PATH.into(),
        articles_total: 30,
        articles_with_machine_readable: enrich_cursor.min(30),
        coverage_score: 0.5,
        provider: provider.into(),
        branch: format!("enrich/{provider}"),
        related_legislation: Vec::new(),
        untranslatables: Vec::new(),
        markings: Vec::new(),
        law_complete,
        enrich_cursor,
        feedback: Vec::new(),
        usage: None,
        agent_calls: Vec::new(),
        models: Vec::new(),
        session_reuse: "off".into(),
    }
}

/// Create + claim an enrich job for `provider`, with the law in `enriching`.
async fn setup_processing_enrich_job(db: &TestDb, provider: &str) -> Job {
    law_status::upsert_law(&db.pool, LAW_ID, Some("Testwet"), None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Enriching)
        .await
        .unwrap();

    let req = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_priority(Priority::new(70))
        .with_payload(serde_json::to_value(payload(provider)).unwrap());
    let job = job_queue::create_enrich_job_if_not_exists(&db.pool, req)
        .await
        .unwrap()
        .expect("job created");
    let claimed = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("job claimable");
    assert_eq!(claimed.id, job.id);
    claimed
}

#[tokio::test]
async fn incomplete_chunk_creates_continuation_in_same_tx_and_keeps_enriching() {
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let result = chunk_result("opencode", false, 15);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(serde_json::to_value(&result).unwrap()),
    )
    .await
    .unwrap()
    .expect("continuation job created");

    // The original job is completed…
    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Completed);

    // …the law stays `enriching` (NOT enriched)…
    let law = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(law.status, LawStatusValue::Enriching);

    // …and the continuation is pending in the same scope with the same
    // provider/priority/depth and a bare payload (no chunk fields — the next
    // window is recomputed from the cursor on the branch).
    assert_eq!(continuation.status, JobStatus::Pending);
    assert_eq!(continuation.law_id, LAW_ID);
    assert_eq!(continuation.priority, job.priority);
    let cont_payload: EnrichPayload =
        serde_json::from_value(continuation.payload.clone().unwrap()).unwrap();
    assert_eq!(cont_payload.provider.as_deref(), Some("opencode"));
    assert_eq!(cont_payload.depth, Some(1));
    assert!(cont_payload.chunk_articles.is_none());
    assert!(cont_payload.skip_mvt.is_none());

    // The unique-active index still holds: a second enrich request for the
    // same law/provider/scope is refused while the continuation is pending.
    let dup = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_payload(serde_json::to_value(payload("opencode")).unwrap());
    let blocked = job_queue::create_enrich_job_if_not_exists(&db.pool, dup)
        .await
        .unwrap();
    assert!(
        blocked.is_none(),
        "active continuation must block duplicates"
    );
}

#[tokio::test]
async fn final_chunk_marks_law_enriched_without_continuation() {
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let result = chunk_result("opencode", true, 30);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(serde_json::to_value(&result).unwrap()),
    )
    .await
    .unwrap();
    assert!(continuation.is_none(), "complete law needs no continuation");

    let done = job_queue::get_job(&db.pool, job.id).await.unwrap();
    assert_eq!(done.status, JobStatus::Completed);
    let law = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(law.status, LawStatusValue::Enriched);
}

#[tokio::test]
async fn chunk_loop_transitions_enriching_to_enriched_across_runs() {
    // Full loop over the queue: chunk 1 (incomplete) → continuation claimable
    // → chunk 2 (complete) → `enriched`. No moment leaves the law `enriching`
    // without an active/pending job.
    let db = TestDb::new().await;
    let job1 = setup_processing_enrich_job(&db, "claude").await;

    let result1 = chunk_result("claude", false, 15);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job1,
        &payload("claude"),
        &result1,
        Some(serde_json::to_value(&result1).unwrap()),
    )
    .await
    .unwrap()
    .expect("continuation created");
    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriching
    );

    // The continuation is the next claimable job for this law.
    let job2 = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("continuation claimable");
    assert_eq!(job2.id, continuation.id);

    let result2 = chunk_result("claude", true, 30);
    let none = worker::complete_enrich_success_tx(
        &db.pool,
        &job2,
        &payload("claude"),
        &result2,
        Some(serde_json::to_value(&result2).unwrap()),
    )
    .await
    .unwrap();
    assert!(none.is_none());

    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriched
    );
    assert_eq!(
        job_queue::get_job(&db.pool, job2.id).await.unwrap().status,
        JobStatus::Completed
    );
}

#[tokio::test]
async fn continuation_inherits_traject_scope() {
    // A traject-scoped enrich (non-task-flow) continues within the same
    // uniqueness scope of `idx_unique_active_enrich_job`.
    let db = TestDb::new().await;
    law_status::upsert_law(&db.pool, LAW_ID, Some("Testwet"), None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Enriching)
        .await
        .unwrap();

    let mut traject_payload = payload("opencode");
    traject_payload.traject_ref = Some("testtraject-abcd1234".into());
    let req = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&traject_payload).unwrap());
    job_queue::create_enrich_job_if_not_exists(&db.pool, req)
        .await
        .unwrap()
        .expect("job created");
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("claimable");

    let result = chunk_result("opencode", false, 15);
    let continuation =
        worker::complete_enrich_success_tx(&db.pool, &job, &traject_payload, &result, None)
            .await
            .unwrap()
            .expect("continuation created");
    let cont_payload: EnrichPayload =
        serde_json::from_value(continuation.payload.clone().unwrap()).unwrap();
    assert_eq!(
        cont_payload.traject_ref.as_deref(),
        Some("testtraject-abcd1234")
    );

    // Same scope is blocked; the corpus-wide scope (traject_ref NULL) is not.
    let same_scope = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_traject_ref("testtraject-abcd1234")
        .with_payload(serde_json::to_value(&traject_payload).unwrap());
    assert!(
        job_queue::create_enrich_job_if_not_exists(&db.pool, same_scope)
            .await
            .unwrap()
            .is_none()
    );
    let corpus_scope = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_payload(serde_json::to_value(payload("opencode")).unwrap());
    assert!(
        job_queue::create_enrich_job_if_not_exists(&db.pool, corpus_scope)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn continuation_conflict_with_existing_active_job_is_accepted() {
    // The Ok(None) branch of complete_enrich_success_tx: when another active
    // enrich job for the same law/provider/scope already exists, the
    // continuation insert is refused by `idx_unique_active_enrich_job` and
    // that is accepted — the existing job picks the cursor up from the branch
    // and carries the loop forward. Deterministic setup: the running job was
    // created WITHOUT an explicit provider (payload->>'provider' IS NULL, a
    // distinct index key), so a manual re-enrich WITH the provider can be
    // pending while it processes; the continuation (which always pins the
    // resolved provider) then collides with that manual job.
    let db = TestDb::new().await;
    law_status::upsert_law(&db.pool, LAW_ID, Some("Testwet"), None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, LAW_ID, LawStatusValue::Enriching)
        .await
        .unwrap();

    let mut no_provider_payload = payload("opencode");
    no_provider_payload.provider = None;
    let req = CreateJobRequest::new(JobType::Enrich, LAW_ID)
        .with_priority(Priority::new(70))
        .with_payload(serde_json::to_value(&no_provider_payload).unwrap());
    job_queue::create_enrich_job_if_not_exists(&db.pool, req)
        .await
        .unwrap()
        .expect("job created");
    let job = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("claimable");

    // Manual re-enrich with an explicit provider, while the first job runs.
    let manual = job_queue::create_enrich_job_if_not_exists(
        &db.pool,
        CreateJobRequest::new(JobType::Enrich, LAW_ID)
            .with_payload(serde_json::to_value(payload("opencode")).unwrap()),
    )
    .await
    .unwrap()
    .expect("distinct provider key admits a concurrent manual job");

    let result = chunk_result("opencode", false, 15);
    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &no_provider_payload,
        &result,
        Some(serde_json::to_value(&result).unwrap()),
    )
    .await
    .unwrap();
    assert!(
        continuation.is_none(),
        "conflicting active job must be accepted as the loop carrier"
    );

    // The completion itself still committed…
    assert_eq!(
        job_queue::get_job(&db.pool, job.id).await.unwrap().status,
        JobStatus::Completed
    );
    // …the law is still mid-loop…
    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriching
    );
    // …and the loop can proceed via the manual job.
    let next = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .expect("manual job claimable");
    assert_eq!(next.id, manual.id);
}

#[tokio::test]
async fn reaped_terminal_enrich_job_does_not_strand_law_in_enriching() {
    // A worker crash mid-chunk orphans the continuation job in 'processing';
    // the reaper terminally fails it once its attempts are spent. Without
    // aftercare the law would sit in 'enriching' forever with no active or
    // pending job — invisible to the user now that the frontend exempts
    // 'enriching' from its poll cap. handle_reaped_enrich_jobs must mirror
    // the synchronous failure path: law → enrich_failed plus an auto-retry
    // job (fail count far below the exhausted threshold).
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    // Spend all attempts and backdate the start so the reaper sees an orphan
    // beyond its timeout with no retries left.
    sqlx::query(
        "UPDATE jobs SET attempts = max_attempts, started_at = now() - interval '2 hours', heartbeat_at = now() - interval '2 hours' WHERE id = $1",
    )
    .bind(job.id)
    .execute(&db.pool)
    .await
    .unwrap();

    let reaped = job_queue::reap_orphaned_jobs(&db.pool, std::time::Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].status, JobStatus::Failed);

    worker::handle_reaped_enrich_jobs(&db.pool, &reaped, 10).await;

    // Not stranded: terminal-ish status for the user, retry job pending.
    let law = law_status::get_law(&db.pool, LAW_ID).await.unwrap();
    assert_eq!(law.status, LawStatusValue::EnrichFailed);
    let retry = job_queue::claim_job(&db.pool, Some(JobType::Enrich))
        .await
        .unwrap();
    // The auto-retry job carries a backoff delay, so it need not be claimable
    // yet — but it must exist as a pending job.
    if retry.is_none() {
        let pending = job_queue::list_jobs(&db.pool, Some(JobStatus::Pending))
            .await
            .unwrap();
        assert!(
            pending
                .iter()
                .any(|j| j.law_id == LAW_ID && j.job_type == JobType::Enrich),
            "aftercare must schedule an auto-retry enrich job"
        );
    }
}

#[tokio::test]
async fn legacy_result_json_counts_as_complete() {
    // A `jobs.result` JSON without `law_complete` (pre-chunking) deserializes
    // as complete: replaying it through the completion path marks the law
    // `enriched` and creates no continuation.
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let legacy_json = json!({
        "law_id": LAW_ID,
        "yaml_path": YAML_PATH,
        "articles_total": 10,
        "articles_with_machine_readable": 10,
        "coverage_score": 1.0,
        "provider": "opencode",
        "branch": "enrich/opencode",
    });
    let result: EnrichResult = serde_json::from_value(legacy_json.clone()).unwrap();
    assert!(result.law_complete);

    let continuation = worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(legacy_json),
    )
    .await
    .unwrap();
    assert!(continuation.is_none());
    assert_eq!(
        law_status::get_law(&db.pool, LAW_ID).await.unwrap().status,
        LawStatusValue::Enriched
    );
}

/// The worker must mirror markings, not just untranslatables.
///
/// This goes through `complete_enrich_success_tx` rather than calling
/// `replace_markings` directly, because the bug being guarded against was in
/// the wiring: the worker mirrored one channel and silently dropped the other.
/// A test that calls the persistence function itself proves the function works
/// and says nothing about whether anyone calls it. Delete the
/// `replace_markings` call in the worker and this is the test that goes red.
#[tokio::test]
async fn completing_an_enrich_run_mirrors_its_markings() {
    let db = TestDb::new().await;
    let job = setup_processing_enrich_job(&db, "opencode").await;

    let mut result = chunk_result("opencode", true, 30);
    result.markings = vec![regelrecht_pipeline::enrich::CapturedMarking {
        article: "5".into(),
        about: "de eerstvolgende werkdag".into(),
        resolution: "operation".into(),
        resolved_by: Some("een WORKING_DAY-bewerking".into()),
        target: vec!["datum_van_betaling".into()],
        legal_text_excerpt: "De betaling geschiedt op de eerstvolgende werkdag".into(),
        accepted: false,
    }];

    worker::complete_enrich_success_tx(
        &db.pool,
        &job,
        &payload("opencode"),
        &result,
        Some(serde_json::to_value(&result).unwrap()),
    )
    .await
    .unwrap();

    let (law_id, about): (String, String) = sqlx::query_as("SELECT law_id, about FROM markings")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(law_id, LAW_ID);
    assert_eq!(about, "de eerstvolgende werkdag");
}
