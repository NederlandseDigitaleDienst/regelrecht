//! The runs of one traject: every harvest, enrichment and conversion that was
//! requested in it, running or finished, with what came of it. Backs the
//! "Verwerkingen" overview in the editor.
//!
//! The task list only shows what still runs (see
//! [`crate::tasks::list_running_task_jobs_for_account`]) and what is left to
//! do; once a run is done, the only trace of it was a task. This is the
//! history: who started what, how long it took, and the outcome, which for an
//! enrichment includes how its review went (the tasks it created, by status).
//!
//! A run belongs to a traject through `jobs.traject_ref`. That ref carries the
//! traject's name as a slug, frozen at the moment of the request, so a renamed
//! traject would lose its older runs on an exact match. Only the last eight
//! hex characters identify the traject: the first eight of its id
//! (`traject_ref` in editor-api's trajects.rs). The query matches on those,
//! taken from the traject id the caller already resolved.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;
use crate::models::{JobStatus, JobType};

/// How many runs one request returns at most, newest first.
pub const MAX_RUNS: i64 = 200;

/// One run in the traject's history, as the editor shows it.
///
/// A flat read model rather than an enum per job type: it mirrors one row of
/// the query below, and the fields that only some job types fill are `None`
/// for the others. Which are filled is documented per field.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct TrajectRun {
    pub job_id: Uuid,
    pub job_type: JobType,
    /// The law the run is about. A conversion carries a synthetic key here
    /// (`doc:` or `lawdoc:`) and a traject harvest the BWB id; read
    /// `target_path` and `law_name` for those.
    pub law_id: String,
    /// The file of a conversion: the target of a document conversion, the
    /// uploaded file name of a law conversion.
    pub target_path: Option<String>,
    /// The display name a traject harvest was requested under.
    pub law_name: Option<String>,
    pub status: JobStatus,
    pub attempts: i32,
    pub max_attempts: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// The name of the member who requested the run; `None` for a run without
    /// a requester or whose account is gone.
    pub requested_by_name: Option<String>,
    /// The last error. Set on a failed run, and on a run that failed once and
    /// waits for its next attempt.
    pub error: Option<String>,
    /// The LLM provider of an enrichment: the one it ran on when it finished,
    /// otherwise the one it was requested with.
    pub provider: Option<String>,
    /// Enrichment only: articles in the law, and articles with a
    /// `machine_readable` section afterwards (pre-existing ones included).
    pub articles_total: Option<i64>,
    pub articles_with_machine_readable: Option<i64>,
    /// Enrichment only: the share of previously unenriched articles that got
    /// a `machine_readable` section. Coverage, not correctness.
    pub coverage_score: Option<f64>,
    /// Document conversion only: the route the conversion took
    /// (deterministic or with a language model).
    pub converted_with: Option<String>,
    /// The worker's progress note, only while the run is processing: on a
    /// stopped run it describes a moment that has passed.
    pub progress: Option<serde_json::Value>,
    /// The review tasks this run created, by status, whoever they are
    /// assigned to.
    pub tasks_open: i64,
    pub tasks_approved: i64,
    pub tasks_rejected: i64,
    pub tasks_dismissed: i64,
    /// The open review tasks assigned to the member asking. A review task goes
    /// to whoever requested the run, and the task list shows a member only
    /// their own, so this is what the viewer can actually pick up.
    pub tasks_open_mine: i64,
}

/// The id part of a traject's refs: the first eight hex characters of its id.
fn traject_key(traject_id: Uuid) -> String {
    let simple = traject_id.simple().to_string();
    simple.chars().take(8).collect()
}

/// The runs requested in the traject `traject_id`, newest first, at most
/// [`MAX_RUNS`]. Every member sees every run of the traject; the caller checks
/// membership. `viewer` is the member asking, for `tasks_open_mine`.
///
/// # Errors
///
/// Returns an error when the query fails.
pub async fn list_traject_runs(
    pool: &PgPool,
    traject_id: Uuid,
    viewer: Uuid,
) -> Result<Vec<TrajectRun>> {
    let runs = sqlx::query_as::<_, TrajectRun>(
        "SELECT j.id AS job_id, j.job_type, j.law_id, \
                COALESCE(j.payload->>'target_path', j.payload->>'filename') AS target_path, \
                j.payload->>'law_name' AS law_name, \
                j.status, j.attempts, j.max_attempts, \
                j.created_at, j.started_at, j.completed_at, \
                a.name AS requested_by_name, \
                j.result->>'error' AS error, \
                COALESCE(j.result->>'provider', j.payload->>'provider') AS provider, \
                (j.result->>'articles_total')::bigint AS articles_total, \
                (j.result->>'articles_with_machine_readable')::bigint AS articles_with_machine_readable, \
                (j.result->>'coverage_score')::float8 AS coverage_score, \
                j.result->>'converted_with' AS converted_with, \
                CASE WHEN j.status = 'processing' THEN j.progress END AS progress, \
                COALESCE(t.open, 0) AS tasks_open, \
                COALESCE(t.approved, 0) AS tasks_approved, \
                COALESCE(t.rejected, 0) AS tasks_rejected, \
                COALESCE(t.dismissed, 0) AS tasks_dismissed, \
                COALESCE(t.open_mine, 0) AS tasks_open_mine \
         FROM jobs j \
         LEFT JOIN accounts a ON a.id::text = j.payload->>'requested_by' \
         LEFT JOIN LATERAL ( \
             SELECT count(*) FILTER (WHERE status = 'open') AS open, \
                    count(*) FILTER (WHERE status = 'approved') AS approved, \
                    count(*) FILTER (WHERE status = 'rejected') AS rejected, \
                    count(*) FILTER (WHERE status = 'dismissed') AS dismissed, \
                    count(*) FILTER (WHERE status = 'open' AND assignee_account_id = $3) AS open_mine \
             FROM tasks WHERE tasks.job_id = j.id AND tasks.task_type = 'job_review' \
         ) t ON true \
         WHERE j.traject_ref IS NOT NULL \
           AND right(j.traject_ref, 9) = '-' || $1 \
           AND j.job_type IN ('enrich', 'document_convert', 'law_convert', 'traject_harvest') \
         ORDER BY j.created_at DESC, j.id \
         LIMIT $2",
    )
    .bind(traject_key(traject_id))
    .bind(MAX_RUNS)
    .bind(viewer)
    .fetch_all(pool)
    .await?;
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traject_key_is_the_first_eight_hex_characters_of_the_id() {
        let id = Uuid::parse_str("ABCD1234-0000-4000-8000-000000000000").unwrap();
        assert_eq!(traject_key(id), "abcd1234");
    }
}
