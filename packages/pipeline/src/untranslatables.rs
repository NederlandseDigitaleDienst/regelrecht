//! Persistence for RFC-012 "untranslatables" captured during enrichment.
//!
//! The enrichment agent flags legal constructs it cannot express with the
//! engine's current operation set in the law YAML (`machine_readable.
//! untranslatables`). [`crate::enrich`] collects those into
//! [`CapturedUntranslatable`]s (DB-free); the worker calls
//! [`replace_untranslatables`] on successful completion to mirror them into the
//! `untranslatables` table so they surface in the harvester UI.

use uuid::Uuid;

use crate::enrich::CapturedUntranslatable;
use crate::error::Result;

/// Delete-and-replace all untranslatables for `(law_id, provider)` with the
/// freshly captured set from an enrich run.
///
/// Idempotent per `(law_id, provider)`: re-running enrichment for the same
/// provider fully refreshes that provider's rows without touching another
/// provider's. Takes a single connection so the delete + inserts stay atomic
/// with the caller's transaction (`complete_job`).
#[tracing::instrument(skip(conn, entries), fields(count = entries.len()))]
pub async fn replace_untranslatables(
    conn: &mut sqlx::PgConnection,
    law_id: &str,
    provider: &str,
    enrich_job_id: Uuid,
    entries: &[CapturedUntranslatable],
) -> Result<()> {
    sqlx::query("DELETE FROM untranslatables WHERE law_id = $1 AND provider = $2")
        .bind(law_id)
        .bind(provider)
        .execute(&mut *conn)
        .await?;

    for entry in entries {
        sqlx::query(
            r#"
            INSERT INTO untranslatables
                (law_id, enrich_job_id, provider, article, construct,
                 reason, suggestion, legal_text_excerpt, accepted)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(law_id)
        .bind(enrich_job_id)
        .bind(provider)
        .bind(&entry.article)
        .bind(&entry.construct)
        .bind(&entry.reason)
        .bind(&entry.suggestion)
        .bind(&entry.legal_text_excerpt)
        .bind(entry.accepted)
        .execute(&mut *conn)
        .await?;
    }

    tracing::info!(
        law_id,
        provider,
        count = entries.len(),
        "untranslatables replaced"
    );
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "test-utils")]
mod tests {
    use super::*;
    use crate::job_queue::{CreateJobRequest, create_job};
    use crate::law_status::upsert_law;
    use crate::models::{JobType, Untranslatable};
    use crate::test_utils::TestDb;

    fn entry(construct: &str, accepted: bool) -> CapturedUntranslatable {
        CapturedUntranslatable {
            article: "1".to_string(),
            construct: construct.to_string(),
            reason: format!("cannot express {construct}"),
            suggestion: None,
            legal_text_excerpt: None,
            accepted,
        }
    }

    async fn seed_enrich_job(db: &TestDb, law_id: &str) -> Uuid {
        upsert_law(&db.pool, law_id, Some("Test Law"), None)
            .await
            .unwrap();
        create_job(&db.pool, CreateJobRequest::new(JobType::Enrich, law_id))
            .await
            .unwrap()
            .id
    }

    async fn fetch_all(db: &TestDb) -> Vec<Untranslatable> {
        sqlx::query_as::<_, Untranslatable>("SELECT * FROM untranslatables ORDER BY construct")
            .fetch_all(&db.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn replace_inserts_captured_entries() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        replace_untranslatables(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("rounding", false), entry("table_lookup", true)],
        )
        .await
        .unwrap();

        let rows = fetch_all(&db).await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].construct, "rounding");
        assert_eq!(rows[0].law_id, "test_law");
        assert_eq!(rows[0].provider, "opencode");
        assert_eq!(rows[0].enrich_job_id, job_id);
        assert!(!rows[0].accepted);
        assert!(rows[1].accepted);
    }

    #[tokio::test]
    async fn replace_is_idempotent_per_law_and_provider() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        // First run flags two constructs.
        replace_untranslatables(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("rounding", false), entry("table_lookup", false)],
        )
        .await
        .unwrap();

        // Second run of the same provider flags only one — the old set must be
        // fully replaced, not appended (no duplicates, no stale rows).
        replace_untranslatables(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("date_diff", false)],
        )
        .await
        .unwrap();

        let rows = fetch_all(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].construct, "date_diff");
    }

    #[tokio::test]
    async fn replace_leaves_other_providers_untouched() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        replace_untranslatables(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("a", false)],
        )
        .await
        .unwrap();
        replace_untranslatables(
            &mut conn,
            "test_law",
            "claude",
            job_id,
            &[entry("b", false)],
        )
        .await
        .unwrap();

        // Re-running opencode must not clear the claude rows.
        replace_untranslatables(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("c", false)],
        )
        .await
        .unwrap();

        let rows = fetch_all(&db).await;
        assert_eq!(rows.len(), 2);
        let providers: Vec<_> = rows.iter().map(|r| r.provider.as_str()).collect();
        assert!(providers.contains(&"claude"));
        assert!(providers.contains(&"opencode"));
    }
}
