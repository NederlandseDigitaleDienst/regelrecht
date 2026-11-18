use pretty_assertions::assert_eq;

use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::law_status;
use regelrecht_pipeline::models::{JobType, LawStatusValue};
use regelrecht_pipeline::test_utils::TestDb;

#[tokio::test]
async fn test_upsert_law() {
    let db = TestDb::new().await;

    let entry = law_status::upsert_law(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        Some("Zorgtoeslagwet"),
        None,
    )
    .await
    .unwrap();

    assert_eq!(entry.law_id, "wet_op_de_zorgtoeslag");
    assert_eq!(entry.law_name, Some("Zorgtoeslagwet".to_string()));
    assert_eq!(entry.status, LawStatusValue::Unknown);
    assert!(entry.coverage_score.is_none());

    let updated = law_status::upsert_law(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        Some("Zorgtoeslagwet v2"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(updated.law_name, Some("Zorgtoeslagwet v2".to_string()));
}

#[tokio::test]
async fn test_upsert_law_without_name() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, "test_law", Some("Test Law"), None)
        .await
        .unwrap();

    let entry = law_status::upsert_law(&db.pool, "test_law", None, None)
        .await
        .unwrap();
    assert_eq!(entry.law_name, Some("Test Law".to_string()));
}

#[tokio::test]
async fn test_upsert_law_slug_preserved() {
    let db = TestDb::new().await;

    let entry = law_status::upsert_law(&db.pool, "slug_law", Some("Slug Law"), Some("slug_law"))
        .await
        .unwrap();
    assert_eq!(entry.slug, Some("slug_law".to_string()));

    // Upserting with None slug should preserve the original
    let entry = law_status::upsert_law(&db.pool, "slug_law", None, None)
        .await
        .unwrap();
    assert_eq!(entry.slug, Some("slug_law".to_string()));

    // Upserting with a new slug should update it
    let entry = law_status::upsert_law(&db.pool, "slug_law", None, Some("new_slug"))
        .await
        .unwrap();
    assert_eq!(entry.slug, Some("new_slug".to_string()));
}

#[tokio::test]
async fn test_update_status() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, "test_law", None, None)
        .await
        .unwrap();

    let entry = law_status::update_status(&db.pool, "test_law", LawStatusValue::Queued)
        .await
        .unwrap();
    assert_eq!(entry.status, LawStatusValue::Queued);

    let entry = law_status::update_status(&db.pool, "test_law", LawStatusValue::Harvesting)
        .await
        .unwrap();
    assert_eq!(entry.status, LawStatusValue::Harvesting);
}

#[tokio::test]
async fn test_update_status_not_found() {
    let db = TestDb::new().await;

    let result = law_status::update_status(&db.pool, "nonexistent", LawStatusValue::Queued).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_set_job_links() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, "test_law", None, None)
        .await
        .unwrap();

    let job = job_queue::create_job(
        &db.pool,
        CreateJobRequest::new(JobType::Harvest, "test_law"),
    )
    .await
    .unwrap();

    let entry = law_status::set_harvest_job(&db.pool, "test_law", job.id)
        .await
        .unwrap();
    assert_eq!(entry.harvest_job_id, Some(job.id));

    let enrich_job =
        job_queue::create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, "test_law"))
            .await
            .unwrap();

    let entry = law_status::set_enrich_job(&db.pool, "test_law", enrich_job.id)
        .await
        .unwrap();
    assert_eq!(entry.enrich_job_id, Some(enrich_job.id));
}

#[tokio::test]
async fn test_set_coverage_score() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, "test_law", None, None)
        .await
        .unwrap();

    let entry = law_status::set_coverage_score(&db.pool, "test_law", 0.85)
        .await
        .unwrap();
    assert_eq!(entry.coverage_score, Some(0.85));
}

#[tokio::test]
async fn test_set_coverage_score_validation() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, "test_law", None, None)
        .await
        .unwrap();

    assert!(
        law_status::set_coverage_score(&db.pool, "test_law", 1.5)
            .await
            .is_err()
    );
    assert!(
        law_status::set_coverage_score(&db.pool, "test_law", -0.1)
            .await
            .is_err()
    );

    assert!(
        law_status::set_coverage_score(&db.pool, "test_law", f64::NAN)
            .await
            .is_err()
    );
    assert!(
        law_status::set_coverage_score(&db.pool, "test_law", f64::INFINITY)
            .await
            .is_err()
    );

    assert!(
        law_status::set_coverage_score(&db.pool, "test_law", 0.0)
            .await
            .is_ok()
    );
    assert!(
        law_status::set_coverage_score(&db.pool, "test_law", 1.0)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_get_law() {
    let db = TestDb::new().await;

    law_status::upsert_law(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        Some("Zorgtoeslagwet"),
        None,
    )
    .await
    .unwrap();

    let entry = law_status::get_law(&db.pool, "wet_op_de_zorgtoeslag")
        .await
        .unwrap();
    assert_eq!(entry.law_id, "wet_op_de_zorgtoeslag");
}

#[tokio::test]
async fn test_get_law_not_found() {
    let db = TestDb::new().await;

    let result = law_status::get_law(&db.pool, "nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_laws() {
    let db = TestDb::new().await;

    law_status::upsert_law(&db.pool, "law_a", Some("Law A"), None)
        .await
        .unwrap();
    law_status::upsert_law(&db.pool, "law_b", Some("Law B"), None)
        .await
        .unwrap();

    law_status::update_status(&db.pool, "law_b", LawStatusValue::Harvested)
        .await
        .unwrap();

    let all = law_status::list_laws(&db.pool, None).await.unwrap();
    assert_eq!(all.len(), 2);

    let unknown = law_status::list_laws(&db.pool, Some(LawStatusValue::Unknown))
        .await
        .unwrap();
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].law_id, "law_a");

    let harvested = law_status::list_laws(&db.pool, Some(LawStatusValue::Harvested))
        .await
        .unwrap();
    assert_eq!(harvested.len(), 1);
    assert_eq!(harvested[0].law_id, "law_b");
}

#[tokio::test]
async fn test_transaction_atomicity() {
    let db = TestDb::new().await;

    let mut tx = db.pool.begin().await.unwrap();

    let job = job_queue::create_job(&mut *tx, CreateJobRequest::new(JobType::Harvest, "tx_law"))
        .await
        .unwrap();

    law_status::upsert_law(&mut *tx, "tx_law", Some("Transaction Law"), None)
        .await
        .unwrap();

    law_status::set_harvest_job(&mut *tx, "tx_law", job.id)
        .await
        .unwrap();

    law_status::update_status(&mut *tx, "tx_law", LawStatusValue::Harvesting)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let entry = law_status::get_law(&db.pool, "tx_law").await.unwrap();
    assert_eq!(entry.status, LawStatusValue::Harvesting);
    assert_eq!(entry.harvest_job_id, Some(job.id));
}

#[tokio::test]
async fn test_transaction_rollback() {
    let db = TestDb::new().await;

    {
        let mut tx = db.pool.begin().await.unwrap();

        job_queue::create_job(
            &mut *tx,
            CreateJobRequest::new(JobType::Harvest, "rollback_law"),
        )
        .await
        .unwrap();

        law_status::upsert_law(&mut *tx, "rollback_law", Some("Should Not Exist"), None)
            .await
            .unwrap();

        tx.rollback().await.unwrap();
    }

    let result = law_status::get_law(&db.pool, "rollback_law").await;
    assert!(result.is_err());

    let jobs = job_queue::list_jobs(&db.pool, None).await.unwrap();
    assert!(jobs.is_empty());
}

#[tokio::test]
async fn test_update_status_unless_any_protects_multiple_statuses() {
    let db = TestDb::new().await;

    // Mirrors the production protected set in api::harvest::create_harvest_job.
    let protected = &[
        LawStatusValue::Harvesting,
        LawStatusValue::Harvested,
        LawStatusValue::Enriching,
        LawStatusValue::Enriched,
    ];

    for status in protected {
        law_status::upsert_law(&db.pool, "protected_law", None, None)
            .await
            .unwrap();
        law_status::update_status(&db.pool, "protected_law", *status)
            .await
            .unwrap();

        let res = law_status::update_status_unless_any(
            &db.pool,
            "protected_law",
            protected,
            LawStatusValue::Queued,
        )
        .await
        .unwrap();
        assert!(
            res.is_none(),
            "{status:?} should be protected from downgrade"
        );
        let entry = law_status::get_law(&db.pool, "protected_law")
            .await
            .unwrap();
        assert_eq!(entry.status, *status);
    }

    // Non-protected statuses should still allow the update.
    law_status::update_status(&db.pool, "protected_law", LawStatusValue::HarvestFailed)
        .await
        .unwrap();
    let res = law_status::update_status_unless_any(
        &db.pool,
        "protected_law",
        protected,
        LawStatusValue::Queued,
    )
    .await
    .unwrap();
    assert!(res.is_some(), "HarvestFailed is not in the protected set");
    let entry = law_status::get_law(&db.pool, "protected_law")
        .await
        .unwrap();
    assert_eq!(entry.status, LawStatusValue::Queued);
}

#[tokio::test]
async fn test_mark_enrich_failed_guards_terminal_states() {
    let db = TestDb::new().await;

    // A non-terminal law is marked enrich_failed and reports one affected row.
    law_status::upsert_law(&db.pool, "failing_law", None, None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, "failing_law", LawStatusValue::Enriching)
        .await
        .unwrap();
    let rows = law_status::mark_enrich_failed(&db.pool, "failing_law")
        .await
        .unwrap();
    assert_eq!(rows, 1, "a non-terminal law is marked enrich_failed");
    assert_eq!(
        law_status::get_law(&db.pool, "failing_law")
            .await
            .unwrap()
            .status,
        LawStatusValue::EnrichFailed
    );

    // A law another provider already enriched must NOT regress: 0 rows, status kept.
    law_status::upsert_law(&db.pool, "done_law", None, None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, "done_law", LawStatusValue::Enriched)
        .await
        .unwrap();
    let rows = law_status::mark_enrich_failed(&db.pool, "done_law")
        .await
        .unwrap();
    assert_eq!(rows, 0, "an already-enriched law must not be regressed");
    assert_eq!(
        law_status::get_law(&db.pool, "done_law")
            .await
            .unwrap()
            .status,
        LawStatusValue::Enriched
    );

    // Same for an already-exhausted law.
    law_status::upsert_law(&db.pool, "exhausted_law", None, None)
        .await
        .unwrap();
    law_status::update_status(&db.pool, "exhausted_law", LawStatusValue::EnrichExhausted)
        .await
        .unwrap();
    let rows = law_status::mark_enrich_failed(&db.pool, "exhausted_law")
        .await
        .unwrap();
    assert_eq!(rows, 0, "an already-exhausted law must not be regressed");
    assert_eq!(
        law_status::get_law(&db.pool, "exhausted_law")
            .await
            .unwrap()
            .status,
        LawStatusValue::EnrichExhausted
    );
}
