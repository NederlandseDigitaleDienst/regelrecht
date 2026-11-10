//! Persistence for schema v0.7.0 "markings" captured during enrichment.
//!
//! The enrichment agent flags constructs the format itself cannot express in
//! the law YAML (`machine_readable.markings`). [`crate::enrich`] collects those
//! into [`CapturedMarking`]s (DB-free); the worker calls [`replace_markings`]
//! on successful completion to mirror them into the `markings` table so they
//! surface in the harvester UI.
//!
//! The counterpart of [`crate::untranslatables`] for v0.7.0 and later. Both
//! exist at once on purpose: a law pinned to schema v0.5.x still carries
//! `untranslatables` and the engine still reads that channel, so dropping the
//! old table would blind the UI to every law that has not been migrated yet.
//!
//! Without this, a v0.7.0 enrich run was silently lossy. The worker calls
//! `replace_untranslatables` with `result.untranslatables`, which is empty for
//! such a law, and that function deletes unconditionally before inserting. So
//! the first v0.7.0 run on a law wiped its old rows and put nothing back, while
//! the markings it did find stayed in `jobs.result` where no view reads them.
//! An empty screen looks exactly like "no problems found".

use uuid::Uuid;

use crate::enrich::CapturedMarking;
use crate::error::Result;

/// Delete-and-replace all markings for `(law_id, provider)` with the freshly
/// captured set from an enrich run.
///
/// Idempotent per `(law_id, provider)`: re-running enrichment for the same
/// provider fully refreshes that provider's rows without touching another
/// provider's. Takes a single connection so the delete + inserts stay atomic
/// with the caller's transaction (`complete_job`).
#[tracing::instrument(skip(conn, entries), fields(count = entries.len()))]
pub async fn replace_markings(
    conn: &mut sqlx::PgConnection,
    law_id: &str,
    provider: &str,
    enrich_job_id: Uuid,
    entries: &[CapturedMarking],
) -> Result<()> {
    sqlx::query("DELETE FROM markings WHERE law_id = $1 AND provider = $2")
        .bind(law_id)
        .bind(provider)
        .execute(&mut *conn)
        .await?;

    for entry in entries {
        sqlx::query(
            r#"
            INSERT INTO markings
                (law_id, enrich_job_id, provider, article, about,
                 resolution, resolved_by, target,
                 legal_text_excerpt, accepted)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(law_id)
        .bind(enrich_job_id)
        .bind(provider)
        .bind(&entry.article)
        .bind(&entry.about)
        .bind(&entry.resolution)
        .bind(&entry.resolved_by)
        .bind(&entry.target)
        .bind(&entry.legal_text_excerpt)
        .bind(entry.accepted)
        .execute(&mut *conn)
        .await?;
    }

    tracing::info!(law_id, provider, count = entries.len(), "markings replaced");
    Ok(())
}

#[cfg(test)]
#[cfg(feature = "test-utils")]
mod tests {
    use super::*;
    use crate::job_queue::{CreateJobRequest, create_job};
    use crate::law_status::upsert_law;
    use crate::models::JobType;
    use crate::test_utils::TestDb;

    fn entry(about: &str, resolution: &str) -> CapturedMarking {
        CapturedMarking {
            article: "1".to_string(),
            about: about.to_string(),
            resolution: resolution.to_string(),
            resolved_by: Some(format!("build {about}")),
            target: vec!["hoogte".to_string()],
            legal_text_excerpt: format!("de tekst over {about}"),
            accepted: false,
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

    async fn count(db: &TestDb) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM markings")
            .fetch_one(&db.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_marking_survives_the_round_trip() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        replace_markings(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("de eerstvolgende werkdag", "operation")],
        )
        .await
        .unwrap();

        let (about, resolution, resolved_by, target): (
            String,
            String,
            Option<String>,
            Vec<String>,
        ) = sqlx::query_as("SELECT about, resolution, resolved_by, target FROM markings")
            .fetch_one(&db.pool)
            .await
            .unwrap();

        assert_eq!(about, "de eerstvolgende werkdag");
        assert_eq!(resolution, "operation");
        assert_eq!(
            resolved_by.as_deref(),
            Some("build de eerstvolgende werkdag")
        );
        assert_eq!(target, vec!["hoogte".to_string()]);
    }

    #[tokio::test]
    async fn replace_is_idempotent_per_law_and_provider() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        replace_markings(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("werkdag", "operation"), entry("gewoonte", "model")],
        )
        .await
        .unwrap();
        assert_eq!(count(&db).await, 2);

        // A second run of the same provider flagging one construct replaces the
        // set wholesale: no duplicates, no stale rows.
        replace_markings(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("gewoonte", "model")],
        )
        .await
        .unwrap();
        assert_eq!(count(&db).await, 1);
    }

    #[tokio::test]
    async fn another_provider_is_left_alone() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        replace_markings(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("a", "model")],
        )
        .await
        .unwrap();
        replace_markings(
            &mut conn,
            "test_law",
            "claude",
            job_id,
            &[entry("b", "model")],
        )
        .await
        .unwrap();

        // Re-running one provider must not touch the other's rows.
        replace_markings(&mut conn, "test_law", "opencode", job_id, &[])
            .await
            .unwrap();

        let providers: Vec<String> =
            sqlx::query_scalar("SELECT provider FROM markings ORDER BY provider")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert_eq!(providers, vec!["claude".to_string()]);
    }

    /// An empty target is a claim, not a missing value: the article stays
    /// executable. It has to survive as an empty array rather than NULL,
    /// because a reader distinguishes "nothing is blocked" from "we do not
    /// know what is blocked".
    #[tokio::test]
    async fn an_empty_target_stays_an_empty_list() {
        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        let mut e = entry("iets", "model");
        e.target = vec![];
        replace_markings(&mut conn, "test_law", "opencode", job_id, &[e])
            .await
            .unwrap();

        let target: Vec<String> = sqlx::query_scalar("SELECT target FROM markings")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(target.is_empty());
    }

    /// The bug this module exists to close. The worker used to mirror only
    /// `untranslatables`, and that function deletes unconditionally before it
    /// inserts. So enriching a law that had moved to schema v0.7.0 wiped its
    /// old rows and put nothing back: the markings it found stayed in
    /// `jobs.result`, where no view reads them, and the UI showed an empty
    /// screen that looks exactly like "no problems found".
    #[tokio::test]
    async fn a_law_moving_to_v0_7_0_keeps_its_flags_somewhere() {
        use crate::enrich::CapturedUntranslatable;
        use crate::untranslatables::replace_untranslatables;

        let db = TestDb::new().await;
        let job_id = seed_enrich_job(&db, "test_law").await;
        let mut conn = db.pool.acquire().await.unwrap();

        // While the law was on v0.5.x it yielded untranslatables.
        replace_untranslatables(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[CapturedUntranslatable {
                article: "1".to_string(),
                construct: "afronden".to_string(),
                reason: "geen ROUND".to_string(),
                suggestion: None,
                legal_text_excerpt: None,
                accepted: false,
            }],
        )
        .await
        .unwrap();

        // The law moves to v0.7.0, so the next run yields markings and no
        // untranslatables. The worker writes both channels.
        replace_untranslatables(&mut conn, "test_law", "opencode", job_id, &[])
            .await
            .unwrap();
        replace_markings(
            &mut conn,
            "test_law",
            "opencode",
            job_id,
            &[entry("afronden", "operation")],
        )
        .await
        .unwrap();

        let old: i64 = sqlx::query_scalar("SELECT count(*) FROM untranslatables")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(old, 0, "the old channel is correctly cleared");
        assert_eq!(
            count(&db).await,
            1,
            "and the flag is still visible, in the new channel"
        );
    }
}
