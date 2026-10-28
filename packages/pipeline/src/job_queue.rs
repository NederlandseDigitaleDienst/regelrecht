use std::time::Duration;

use sqlx::postgres::types::PgInterval;
use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::models::{Job, JobStatus, JobType, Priority};

/// Base delay before the first retry of a failed job. Effective retry latency
/// is this backoff plus up to the worker's max poll interval (the poll loop
/// only discovers due jobs on its next tick) — by design, not a bug.
pub const RETRY_BACKOFF_BASE: Duration = Duration::from_secs(30);

/// Maximum delay between retries of a failed job.
pub const RETRY_BACKOFF_CAP: Duration = Duration::from_secs(15 * 60);

/// Exponential backoff for retry `attempts`: `30s * 2^(attempts-1)`, capped
/// at 15 minutes. `attempts` is the number of attempts already spent (>= 1
/// when a job fails); values below 1 are treated as 1. The exponent is
/// clamped at 30 so the multiplication cannot overflow before the cap
/// applies.
///
/// `fail_job` applies the same formula in SQL (with [`RETRY_BACKOFF_BASE`] and
/// [`RETRY_BACKOFF_CAP`] bound as parameters, and the same exponent clamp) —
/// keep the two in sync.
pub fn retry_backoff(attempts: i32) -> Duration {
    let exponent = attempts.saturating_sub(1).clamp(0, 30) as u32;
    RETRY_BACKOFF_BASE
        .saturating_mul(2u32.saturating_pow(exponent))
        .min(RETRY_BACKOFF_CAP)
}

/// Convert a `Duration` to a Postgres interval bind parameter.
fn to_pg_interval(duration: Duration) -> Result<PgInterval> {
    PgInterval::try_from(duration)
        .map_err(|_| PipelineError::InvalidInput(format!("invalid interval: {duration:?}")))
}

/// Een door de reaper geraakte job. `status` is de NIEUWE status: `Pending`
/// wanneer er nog pogingen over waren (her-poging), `Failed` bij terminaal.
/// De payload rijdt mee zodat de aanroeper voor terminaal gefaalde
/// taak-flow-jobs alsnog een `job_failed`-taak kan aanmaken — zonder die
/// nazorg verdwijnt zo'n job stil uit de "Bezig"-lijst (zie
/// `tasks::notify_reaped_task_jobs`). `priority` rijdt mee voor de
/// enrich-nazorg (`worker::handle_reaped_enrich_jobs`), die een retry-job met
/// dezelfde prioriteit inplant.
#[derive(Debug, sqlx::FromRow)]
pub struct ReapedJob {
    pub id: Uuid,
    pub law_id: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub priority: i32,
    pub payload: Option<serde_json::Value>,
}

pub struct CreateJobRequest {
    pub job_type: JobType,
    pub law_id: String,
    /// Owning traject ref for traject-scoped jobs; `None` for corpus-wide
    /// harvest/enrich jobs.
    pub traject_ref: Option<String>,
    pub priority: Priority,
    pub payload: Option<serde_json::Value>,
    pub max_attempts: i32,
    /// Delay before the job becomes claimable (sets `scheduled_at` to
    /// `now() + delay`). `None` means claimable immediately.
    pub initial_delay: Option<Duration>,
}

impl CreateJobRequest {
    pub fn new(job_type: JobType, law_id: impl Into<String>) -> Self {
        Self {
            job_type,
            law_id: law_id.into(),
            traject_ref: None,
            priority: Priority::default(),
            payload: None,
            max_attempts: 3,
            initial_delay: None,
        }
    }

    /// Associate the job with an owning traject.
    pub fn with_traject_ref(mut self, traject_ref: impl Into<String>) -> Self {
        self.traject_ref = Some(traject_ref.into());
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = Some(delay);
        self
    }
}

/// Create a new job in the queue.
#[tracing::instrument(skip(executor, req), fields(job_type = ?req.job_type, law_id = %req.law_id, priority = req.priority.value()))]
pub async fn create_job<'e, E>(executor: E, req: CreateJobRequest) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let initial_delay = req.initial_delay.map(to_pg_interval).transpose()?;
    let job = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (job_type, law_id, traject_ref, priority, payload, max_attempts, scheduled_at)
        VALUES ($1, $2, $3, $4, $5, $6, now() + $7::interval)
        RETURNING *
        "#,
    )
    .bind(req.job_type)
    .bind(&req.law_id)
    .bind(&req.traject_ref)
    .bind(req.priority.value())
    .bind(&req.payload)
    .bind(req.max_attempts)
    .bind(initial_delay)
    .fetch_one(executor)
    .await?;

    tracing::info!(job_id = %job.id, "job created");
    Ok(job)
}

/// Count enrich jobs for `provider` started within the current local clock-hour
/// bucket (Europe/Amsterdam), and return that bucket's local hour-of-day
/// (`0..=23`) so the caller can pick the day vs night cap.
///
/// Used to enforce a per-provider hourly run cap (see `ENRICH_HOURLY_LIMIT` +
/// `ENRICH_NIGHT_MULTIPLIER`), which protects a personal Claude subscription
/// token from running the whole corpus in one go. Reads the durable `jobs` table
/// so the cap holds across worker restarts/redeploys.
///
/// Timezone is resolved by Postgres (`AT TIME ZONE 'Europe/Amsterdam'`), which is
/// DST-correct; no chrono-tz needed. Served by the existing partial index
/// `0022_enrich_daily_count_index` on `((payload->>'provider'), started_at)`:
/// the hour bound lives in the `WHERE` clause, so the scan is bounded to the
/// current hour bucket (a `COUNT(*) FILTER (...)` would not push into the index).
///
/// Counts every job that actually ran this hour, in ANY status; only never-run
/// `pending` jobs (`started_at IS NULL`) are excluded. Relies on every enqueue
/// path setting `payload.provider`.
///
/// Not transactional with the caller's claim: with N concurrent workers the cap
/// may be exceeded by up to N near the hour boundary. That's acceptable for a
/// spend guard.
pub async fn count_enrich_jobs_started_this_hour<'e, E>(
    executor: E,
    provider: &str,
) -> Result<(i64, i32)>
where
    E: sqlx::PgExecutor<'e>,
{
    let row = sqlx::query_as::<_, (i64, i32)>(
        r#"
        SELECT
            COUNT(*) AS ran_this_hour,
            EXTRACT(HOUR FROM now() AT TIME ZONE 'Europe/Amsterdam')::int AS local_hour
        FROM jobs
        WHERE job_type = 'enrich'
          AND started_at IS NOT NULL
          AND payload->>'provider' = $1
          AND started_at >= date_trunc('hour', now() AT TIME ZONE 'Europe/Amsterdam')
                            AT TIME ZONE 'Europe/Amsterdam'
        "#,
    )
    .bind(provider)
    .fetch_one(executor)
    .await?;

    Ok(row)
}

/// Claim the highest-priority pending job using FOR UPDATE SKIP LOCKED.
/// Jobs scheduled in the future (`scheduled_at > now()`, set by the retry
/// backoff) are skipped until they become due.
/// Returns None if no jobs are available.
#[tracing::instrument(skip(executor))]
pub async fn claim_job<'e, E>(executor: E, job_type: Option<JobType>) -> Result<Option<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    // The single query handles both cases via a NULL-tolerant filter on $1:
    // NULL matches every pending job, a non-NULL value filters to that
    // job_type. Replaces the previous match-on-Option two-branch SQL.
    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = 'processing', started_at = now(), heartbeat_at = now(), attempts = attempts + 1
        WHERE id = (
            SELECT id FROM jobs
            WHERE status = 'pending'
              AND ($1::job_type IS NULL OR job_type = $1)
              AND (scheduled_at IS NULL OR scheduled_at <= now())
            ORDER BY priority DESC, created_at ASC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        RETURNING *
        "#,
    )
    .bind(job_type)
    .fetch_optional(executor)
    .await?;

    if let Some(ref j) = job {
        tracing::info!(job_id = %j.id, law_id = %j.law_id, attempt = j.attempts, "job claimed");
    }
    Ok(job)
}

/// Mark a job as completed with an optional result payload.
#[tracing::instrument(skip(executor, result))]
pub async fn complete_job<'e, E>(
    executor: E,
    job_id: Uuid,
    result: Option<serde_json::Value>,
) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = 'completed', completed_at = now(), result = $2
        WHERE id = $1 AND status = 'processing'
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(&result)
    .fetch_optional(executor)
    .await?
    .ok_or(PipelineError::JobNotProcessing(job_id))?;

    tracing::info!(job_id = %job.id, law_id = %job.law_id, "job completed");
    Ok(job)
}

/// Mark a job as failed. If attempts < max_attempts, reset to pending for
/// retry with an exponential backoff: `scheduled_at` is set to
/// `now() + 30s * 2^(attempts-1)`, capped at 15 minutes (see
/// [`retry_backoff`] — the SQL below implements the same formula), so a
/// transient outage doesn't burn all attempts within one poll interval.
#[tracing::instrument(skip(executor, error_result))]
pub async fn fail_job<'e, E>(
    executor: E,
    job_id: Uuid,
    error_result: Option<serde_json::Value>,
) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = CASE
                WHEN attempts < max_attempts THEN 'pending'::job_status
                ELSE 'failed'::job_status
            END,
            result = $2,
            completed_at = CASE
                WHEN attempts >= max_attempts THEN now()
                ELSE NULL
            END,
            scheduled_at = CASE
                WHEN attempts < max_attempts THEN
                    -- Clamp the exponent at 30 (like retry_backoff in Rust):
                    -- the product is computed BEFORE the LEAST cap, so an
                    -- unclamped exponent overflows the interval for large
                    -- attempt counts and the whole UPDATE errors.
                    now() + LEAST(
                        $3::interval * power(2, LEAST(GREATEST(attempts - 1, 0), 30)),
                        $4::interval
                    )
                ELSE NULL
            END
        WHERE id = $1 AND status = 'processing'
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(&error_result)
    .bind(to_pg_interval(RETRY_BACKOFF_BASE)?)
    .bind(to_pg_interval(RETRY_BACKOFF_CAP)?)
    .fetch_optional(executor)
    .await?
    .ok_or(PipelineError::JobNotProcessing(job_id))?;

    match job.status {
        JobStatus::Pending => {
            tracing::info!(
                job_id = %job.id,
                attempt = job.attempts,
                max = job.max_attempts,
                retry_at = ?job.scheduled_at,
                "job failed, will retry after backoff"
            );
        }
        JobStatus::Failed => {
            tracing::warn!(job_id = %job.id, attempts = job.attempts, "job permanently failed after exhausting retries");
        }
        _ => {}
    }
    Ok(job)
}

/// Mark a job as permanently failed regardless of remaining attempts.
///
/// Unlike [`fail_job`], this never reschedules for retry: it sets `failed`
/// immediately even when `attempts < max_attempts`. Use this for deterministic
/// failures that cannot succeed on retry against the same inputs, where
/// retrying only burns budget and blocks the serial queue. Examples:
/// - the enrichment LLM produced no machine_readable sections, or its output
///   failed to parse; or
/// - enrichment base drift, where the base is stale relative to the recorded
///   provenance and every retry would re-fail against the same base (and, on
///   each non-final attempt, flip-flop the law status Enriching -> Harvested
///   before finally landing on Failed).
#[tracing::instrument(skip(executor, error_result))]
pub async fn fail_job_terminal<'e, E>(
    executor: E,
    job_id: Uuid,
    error_result: Option<serde_json::Value>,
) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = 'failed'::job_status,
            result = $2,
            completed_at = now(),
            scheduled_at = NULL
        WHERE id = $1 AND status = 'processing'
        RETURNING *
        "#,
    )
    .bind(job_id)
    .bind(&error_result)
    .fetch_optional(executor)
    .await?
    .ok_or(PipelineError::JobNotProcessing(job_id))?;

    tracing::warn!(job_id = %job.id, attempts = job.attempts, "job terminally failed (non-retryable)");
    Ok(job)
}

/// How often a worker records that it still holds its job. The orphan
/// timeout must be a few multiples of this, or a single slow database round
/// trip would let the reaper take back a job that is still running.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Record that the worker holding a job is still working on it.
///
/// `claimed_at` is the `started_at` the claim returned, and serves as the
/// claim's token: a job that was reset and claimed again has a new
/// `started_at`, so a heartbeat left over from the earlier claim no longer
/// lands and cannot keep the later claim alive after its worker died.
///
/// Returns `false` when this claim no longer holds the job (it finished,
/// failed, was reaped, or was claimed again), which tells the heartbeat to
/// stop.
pub async fn heartbeat<'e, E>(
    executor: E,
    job_id: Uuid,
    claimed_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<bool>
where
    E: sqlx::PgExecutor<'e>,
{
    let updated = sqlx::query(
        "UPDATE jobs SET heartbeat_at = now() \
         WHERE id = $1 AND status = 'processing' AND started_at IS NOT DISTINCT FROM $2",
    )
    .bind(job_id)
    .bind(claimed_at)
    .execute(executor)
    .await?
    .rows_affected();
    Ok(updated == 1)
}

/// How long a job keeps beating past its own job timeout. The worker's
/// timeout normally ends the job well before this; the margin covers the
/// steps around the timed run (checkout, commit, push) that have no deadline
/// of their own.
pub const HEARTBEAT_GRACE: Duration = Duration::from_secs(10 * 60);

/// How long a job with this timeout keeps beating: the timeout plus
/// [`HEARTBEAT_GRACE`].
fn heartbeat_ceiling(job_timeout: Duration) -> Duration {
    job_timeout.saturating_add(HEARTBEAT_GRACE)
}

/// Keeps a claimed job's heartbeat going while it is alive, up to a ceiling.
///
/// Start one right after claiming a job and keep it in scope until the job is
/// done; dropping it stops the heartbeat. Bind it to a named variable
/// (`let _heartbeat = ...`), because `let _ = ...` drops it at once.
///
/// The heartbeat stops by itself after `job_timeout` plus [`HEARTBEAT_GRACE`].
/// A worker that is alive but stuck (a hung `git push`) would otherwise hold
/// its job forever; this way the reaper takes it back once the beats stop.
pub struct JobHeartbeat {
    handle: tokio::task::JoinHandle<()>,
}

impl JobHeartbeat {
    pub fn start(pool: sqlx::PgPool, job: &Job, job_timeout: Duration) -> Self {
        Self::start_with(
            pool,
            job,
            HEARTBEAT_INTERVAL,
            heartbeat_ceiling(job_timeout),
        )
    }

    /// [`JobHeartbeat::start`] with an explicit interval and ceiling, for tests.
    #[doc(hidden)]
    pub fn start_with(
        pool: sqlx::PgPool,
        job: &Job,
        interval: Duration,
        max_age: Duration,
    ) -> Self {
        let job_id = job.id;
        let claimed_at = job.started_at;
        let handle = tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + max_age;
            let mut tick = tokio::time::interval(interval);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // The first tick fires at once; `claim_job` has just set the
            // heartbeat, so skip it.
            tick.tick().await;
            loop {
                tick.tick().await;
                if tokio::time::Instant::now() >= deadline {
                    tracing::warn!(
                        job_id = %job_id,
                        max_age = ?max_age,
                        "job outlived its timeout; heartbeat stopped so the reaper can take it back"
                    );
                    break;
                }
                match heartbeat(&pool, job_id, claimed_at).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::debug!(job_id = %job_id, "claim no longer holds the job, heartbeat stopped");
                        break;
                    }
                    // A missed beat is not fatal: the reaper allows several.
                    Err(e) => {
                        tracing::warn!(job_id = %job_id, error = %e, "failed to record job heartbeat");
                    }
                }
            }
        });
        Self { handle }
    }
}

impl Drop for JobHeartbeat {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Reap orphaned jobs whose worker has stopped responding.
///
/// A job counts as orphaned when it is processing and the later of its last
/// heartbeat and its start is older than `timeout`, e.g. because the worker
/// crashed or was restarted. Taking the later of the two matters when a job
/// was reset and claimed again by a worker that does not beat (an old
/// version during a rolling deploy): its `heartbeat_at` is left over from
/// the earlier claim, and its fresh `started_at` is what counts. Postgres
/// `GREATEST` ignores a NULL, so a job claimed before heartbeats existed
/// falls back to its start. How long a job has been running does not matter
/// otherwise: a worker that keeps beating keeps its job, up to the ceiling
/// in [`JobHeartbeat`]. If the job still has retries left, it is reset to
/// 'pending'; otherwise it is marked 'failed'.
///
/// Returns the reaped jobs (atomically claimed by THIS caller: de UPDATE …
/// RETURNING geeft elke rij aan precies één concurrent reaper), zodat de
/// aanroeper nazorg kan doen — met name `tasks::notify_reaped_task_jobs`
/// voor terminaal gefaalde taak-flow-jobs.
#[tracing::instrument(skip(executor))]
pub async fn reap_orphaned_jobs<'e, E>(
    executor: E,
    timeout: std::time::Duration,
) -> Result<Vec<ReapedJob>>
where
    E: sqlx::PgExecutor<'e>,
{
    let timeout_interval = sqlx::postgres::types::PgInterval::try_from(timeout)
        .map_err(|_| PipelineError::InvalidInput(format!("invalid reaper timeout: {timeout:?}")))?;

    let reaped_rows = sqlx::query_as::<_, ReapedJob>(
        r#"
        WITH reaped AS (
            UPDATE jobs
            SET status = CASE
                    WHEN attempts < max_attempts THEN 'pending'::job_status
                    ELSE 'failed'::job_status
                END,
                result = jsonb_build_object('error', 'reaped: job stuck in processing'),
                completed_at = CASE
                    WHEN attempts >= max_attempts THEN now()
                    ELSE NULL
                END
            WHERE status = 'processing'
              AND GREATEST(heartbeat_at, started_at) < now() - $1::interval
            RETURNING id, law_id, job_type, status, priority, payload
        )
        SELECT id, law_id, job_type, status, priority, payload FROM reaped
        "#,
    )
    .bind(timeout_interval)
    .fetch_all(executor)
    .await?;

    if !reaped_rows.is_empty() {
        tracing::warn!(
            count = reaped_rows.len(),
            "reaped orphaned jobs stuck in processing"
        );
    }
    Ok(reaped_rows)
}

/// Create a harvest job only if no active (pending/processing) harvest job
/// exists for this law.
///
/// Used by the worker's follow-up path (re-harvesting referenced laws). HTTP
/// harvest requests go through [`crate::harvest_request::request_harvest`]
/// instead, which adds the exhausted check and law-status bookkeeping.
///
/// Completed and failed jobs never block: a law that was harvested before
/// must remain re-harvestable.
///
/// The fast path uses `INSERT ... WHERE NOT EXISTS` (filtered by date) to
/// avoid a round-trip for the common non-racing case. When two requests race
/// and both pass the subquery, the partial unique index
/// `idx_unique_active_harvest_job` (migration 0011) rejects the second with
/// a 23505 unique-violation — we translate that into `Ok(None)` so callers
/// see the same "already exists" signal regardless of which path caught it.
///
/// Returns `Some(Job)` if a new job was created, `None` if a matching job already exists.
pub async fn create_harvest_job_if_not_exists<'e, E>(
    executor: E,
    req: CreateJobRequest,
    date: &str,
) -> Result<Option<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let initial_delay = req.initial_delay.map(to_pg_interval).transpose()?;
    let result = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (job_type, law_id, priority, payload, max_attempts, scheduled_at)
        SELECT $1, $2, $3, $4, $5, now() + $7::interval
        WHERE NOT EXISTS (
            SELECT 1 FROM jobs
            WHERE job_type = 'harvest'
              AND law_id = $2
              AND (payload->>'date' = $6 OR payload->>'date' IS NULL)
              AND status IN ('pending', 'processing')
        )
        RETURNING *
        "#,
    )
    .bind(req.job_type)
    .bind(&req.law_id)
    .bind(req.priority.value())
    .bind(&req.payload)
    .bind(req.max_attempts)
    .bind(date)
    .bind(initial_delay)
    .fetch_optional(executor)
    .await;

    let job = match result {
        Ok(job) => job,
        Err(sqlx::Error::Database(ref db_err)) if db_err.code().as_deref() == Some("23505") => {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };

    if let Some(ref j) = job {
        tracing::info!(job_id = %j.id, law_id = %j.law_id, "follow-up harvest job created");
    }

    Ok(job)
}

/// Update the progress field of a running job.
///
/// Used by the enrich worker to report live phase information
/// (e.g. "mvt_research", "generating", "validating") while the LLM runs.
pub async fn update_progress<'e, E>(
    executor: E,
    job_id: Uuid,
    progress: serde_json::Value,
) -> Result<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query("UPDATE jobs SET progress = $2, updated_at = NOW() WHERE id = $1")
        .bind(job_id)
        .bind(&progress)
        .execute(executor)
        .await?;
    Ok(())
}

/// Get a job by ID.
pub async fn get_job<'e, E>(executor: E, job_id: Uuid) -> Result<Job>
where
    E: sqlx::PgExecutor<'e>,
{
    let job = sqlx::query_as::<_, Job>(r#"SELECT * FROM jobs WHERE id = $1"#)
        .bind(job_id)
        .fetch_optional(executor)
        .await?
        .ok_or(PipelineError::JobNotFound(job_id))?;

    Ok(job)
}

/// Create an enrich job if no active (pending/processing) enrich job exists
/// for this law_id + provider + traject scope.
///
/// Uses `INSERT ... ON CONFLICT DO NOTHING` against the
/// `idx_unique_active_enrich_job` partial unique index to atomically
/// prevent duplicates — no TOCTOU race regardless of isolation level. The
/// index keys on `(law_id, job_type, provider, COALESCE(traject_ref, ''))`,
/// so uniqueness is scoped per traject: a corpus-wide auto-enrich
/// (`traject_ref` NULL) and a traject-scoped enrich request (task-flow) for
/// the same law_id + provider no longer collide, and two different trajects
/// requesting the same law_id + provider don't block each other either. A
/// second active request within the *same* scope (same traject, or both
/// corpus-wide) still collides and loses with `None` (`Ok(None)`).
///
/// Returns `Some(job)` if created, `None` if a duplicate already existed.
pub async fn create_enrich_job_if_not_exists<'e, E>(
    executor: E,
    req: CreateJobRequest,
) -> Result<Option<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let initial_delay = req.initial_delay.map(to_pg_interval).transpose()?;
    let job = sqlx::query_as::<_, Job>(
        r#"
        INSERT INTO jobs (job_type, law_id, traject_ref, priority, payload, max_attempts, scheduled_at)
        VALUES ($1, $2, $3, $4, $5, $6, now() + $7::interval)
        ON CONFLICT (law_id, job_type, (payload->>'provider'), COALESCE(traject_ref, ''))
            WHERE job_type = 'enrich' AND status IN ('pending', 'processing')
        DO NOTHING
        RETURNING *
        "#,
    )
    .bind(req.job_type)
    .bind(&req.law_id)
    .bind(&req.traject_ref)
    .bind(req.priority.value())
    .bind(&req.payload)
    .bind(req.max_attempts)
    .bind(initial_delay)
    .fetch_optional(executor)
    .await?;

    if let Some(ref j) = job {
        tracing::info!(job_id = %j.id, "enrich job created");
    }
    Ok(job)
}

/// List jobs with optional status filter.
pub async fn list_jobs<'e, E>(executor: E, status: Option<JobStatus>) -> Result<Vec<Job>>
where
    E: sqlx::PgExecutor<'e>,
{
    let jobs = match status {
        Some(s) => {
            sqlx::query_as::<_, Job>(
                r#"SELECT * FROM jobs WHERE status = $1 ORDER BY priority DESC, created_at ASC"#,
            )
            .bind(s)
            .fetch_all(executor)
            .await?
        }
        None => {
            sqlx::query_as::<_, Job>(r#"SELECT * FROM jobs ORDER BY priority DESC, created_at ASC"#)
                .fetch_all(executor)
                .await?
        }
    };

    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_stops_ten_minutes_after_the_job_timeout() {
        // The pipeline docs promise this margin for the untimed steps around
        // a run (checkout, commit, push).
        assert_eq!(
            heartbeat_ceiling(Duration::from_secs(3900)),
            Duration::from_secs(3900 + 10 * 60)
        );
    }

    #[test]
    fn retry_backoff_doubles_per_attempt() {
        assert_eq!(retry_backoff(1), Duration::from_secs(30));
        assert_eq!(retry_backoff(2), Duration::from_secs(60));
        assert_eq!(retry_backoff(3), Duration::from_secs(120));
        assert_eq!(retry_backoff(4), Duration::from_secs(240));
    }

    #[test]
    fn retry_backoff_is_capped_at_fifteen_minutes() {
        assert_eq!(retry_backoff(6), Duration::from_secs(15 * 60));
        assert_eq!(retry_backoff(10), RETRY_BACKOFF_CAP);
        assert_eq!(retry_backoff(i32::MAX), RETRY_BACKOFF_CAP);
    }

    #[test]
    fn retry_backoff_treats_non_positive_attempts_as_first() {
        assert_eq!(retry_backoff(0), RETRY_BACKOFF_BASE);
        assert_eq!(retry_backoff(-5), RETRY_BACKOFF_BASE);
    }
}
