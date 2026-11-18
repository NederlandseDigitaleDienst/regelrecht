//! Canonical "request a harvest" entry point.
//!
//! Both HTTP layers that accept harvest requests (the editor-facing pipeline
//! API and the admin API) used to own divergent copies of this logic, with
//! different dedup, exhausted-law and bookkeeping semantics. This module is
//! the single implementation: advisory lock, dedup against active jobs, the
//! `harvest_exhausted` check, date validation, and law-entry bookkeeping
//! (upsert + status + job link) — all in one transaction, with every error
//! propagated instead of discarded.
//!
//! The worker's follow-up path (re-harvesting laws referenced by a completed
//! harvest) deliberately stays on
//! [`crate::job_queue::create_harvest_job_if_not_exists`]: it runs in a hot
//! loop over many laws and intentionally does no per-law status bookkeeping.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::harvest::HarvestPayload;
use crate::job_queue::{self, CreateJobRequest};
use crate::law_status;
use crate::models::{Job, JobType, LawStatusValue, Priority};

/// Options for a harvest request. `Default` gives the standard pipeline
/// priority (50) and no date/name/slug.
#[derive(Debug, Default)]
pub struct HarvestRequestOptions {
    /// Job priority (higher = processed first).
    pub priority: Priority,
    /// Requested consolidation date (`YYYY-MM-DD`). `None` means the
    /// harvester resolves the latest consolidation itself.
    pub date: Option<String>,
    /// Human-readable law name to store on the law entry, if known.
    pub law_name: Option<String>,
    /// Law slug to store on the law entry, if known.
    pub slug: Option<String>,
}

/// Outcome of a harvest request. Designed so each HTTP layer can map it onto
/// its existing response shape (created / already queued / exhausted /
/// invalid date) without re-implementing any of the decision logic.
#[derive(Debug)]
pub enum HarvestRequestOutcome {
    /// A new harvest job was created and linked to the law entry.
    Created(Job),
    /// A pending or processing harvest job already exists for this law.
    AlreadyQueued { existing_job_id: Uuid },
    /// The law is `harvest_exhausted`; it must be reset before re-queueing.
    Exhausted,
    /// The requested date failed validation (format, impossible, or future).
    InvalidDate { reason: String },
}

/// Request a harvest for `law_id`, creating a job plus the law-status
/// bookkeeping in a single transaction.
///
/// Semantics (the union of what the editor and admin paths used to do):
/// - the requested `date` is validated up front ([`HarvestRequestOutcome::InvalidDate`]);
/// - a transaction-scoped advisory lock on the law id serialises concurrent
///   requests (no TOCTOU between the dedup check and the insert);
/// - an existing pending/processing harvest job short-circuits to
///   [`HarvestRequestOutcome::AlreadyQueued`];
/// - a `harvest_exhausted` law is refused ([`HarvestRequestOutcome::Exhausted`]);
/// - the law entry is upserted (name/slug) and its status set to `queued`
///   unless it is currently in an in-progress state (`harvesting`/`enriching`),
///   and the new job is linked via `harvest_job_id`.
///
/// All bookkeeping failures roll back the transaction and propagate — they
/// are never silently discarded.
///
/// The caller is responsible for validating the *shape* of `law_id` (BWB vs
/// CVDR prefix, slug resolution); this function treats it as an opaque key.
#[tracing::instrument(skip(pool, opts), fields(priority = opts.priority.value(), date = opts.date.as_deref()))]
pub async fn request_harvest(
    pool: &PgPool,
    law_id: &str,
    opts: HarvestRequestOptions,
) -> Result<HarvestRequestOutcome> {
    if let Some(ref date) = opts.date
        && let Err(e) = regelrecht_harvester::validate_date(date)
    {
        tracing::debug!(law_id = %law_id, date = %date, error = %e, "rejected harvest request: invalid date");
        return Ok(HarvestRequestOutcome::InvalidDate {
            reason: e.to_string(),
        });
    }

    let mut tx = pool.begin().await?;

    // Serialize concurrent requests for the same law. This prevents the
    // TOCTOU race where two requests both see no existing job and both
    // create one. Released on commit/rollback.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(law_id)
        .execute(&mut *tx)
        .await?;

    // Dedup: an active (pending/processing) harvest job means there is
    // nothing to do.
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM jobs \
         WHERE law_id = $1 AND job_type = 'harvest' AND status IN ('pending', 'processing') \
         LIMIT 1",
    )
    .bind(law_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((existing_job_id,)) = existing {
        tracing::info!(law_id = %law_id, existing_job_id = %existing_job_id, "harvest request deduplicated: active job exists");
        return Ok(HarvestRequestOutcome::AlreadyQueued { existing_job_id });
    }

    // Refuse exhausted laws. LawNotFound is fine (a new law cannot be
    // exhausted); other errors propagate.
    match law_status::get_law(&mut *tx, law_id).await {
        Ok(law) if law.status == LawStatusValue::HarvestExhausted => {
            tracing::info!(law_id = %law_id, "harvest request refused: law is harvest_exhausted");
            return Ok(HarvestRequestOutcome::Exhausted);
        }
        Ok(_) | Err(PipelineError::LawNotFound(_)) => {}
        Err(e) => return Err(e),
    }

    // Bookkeeping: upsert the entry (name/slug) and mark it queued unless an
    // in-progress state would be overwritten. Errors roll the whole request
    // back — job creation and status must stay consistent.
    law_status::upsert_law(
        &mut *tx,
        law_id,
        opts.law_name.as_deref(),
        opts.slug.as_deref(),
    )
    .await?;
    law_status::update_status_unless_any(
        &mut *tx,
        law_id,
        &[LawStatusValue::Harvesting, LawStatusValue::Enriching],
        LawStatusValue::Queued,
    )
    .await?;

    let payload = HarvestPayload::for_law(law_id, opts.date.clone());
    let req = CreateJobRequest::new(JobType::Harvest, law_id)
        .with_priority(opts.priority)
        .with_payload(serde_json::to_value(&payload).map_err(|e| {
            PipelineError::InvalidInput(format!("failed to serialize harvest payload: {e}"))
        })?);

    let job = match job_queue::create_job(&mut *tx, req).await {
        Ok(job) => job,
        // The worker's follow-up path deliberately skips the advisory lock,
        // so its insert can land between our dedup SELECT and this one; the
        // partial unique index then rejects ours (23505). Report that as
        // AlreadyQueued — the translation `create_harvest_job_if_not_exists`
        // already applies — instead of surfacing an error.
        Err(PipelineError::Database(sqlx::Error::Database(ref db_err)))
            if db_err.code().as_deref() == Some("23505") =>
        {
            // The transaction is aborted after the constraint violation; drop
            // it (rollback) to release the advisory lock, then look up the
            // winning job outside the transaction.
            drop(tx);
            let existing: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM jobs \
                 WHERE law_id = $1 AND job_type = 'harvest' AND status IN ('pending', 'processing') \
                 LIMIT 1",
            )
            .bind(law_id)
            .fetch_optional(pool)
            .await?;
            return match existing {
                Some((existing_job_id,)) => {
                    tracing::info!(law_id = %law_id, existing_job_id = %existing_job_id, "harvest request lost insert race: active job exists");
                    Ok(HarvestRequestOutcome::AlreadyQueued { existing_job_id })
                }
                // The racing job left pending/processing in the same instant —
                // vanishingly rare; surface as a retryable error.
                None => Err(PipelineError::Worker(format!(
                    "harvest job insert for {law_id} lost a race and the winning job is already gone; retry"
                ))),
            };
        }
        Err(e) => return Err(e),
    };

    law_status::set_harvest_job(&mut *tx, law_id, job.id).await?;

    tx.commit().await?;

    tracing::info!(job_id = %job.id, law_id = %law_id, "harvest job created");
    Ok(HarvestRequestOutcome::Created(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_uses_standard_priority() {
        let opts = HarvestRequestOptions::default();
        assert_eq!(opts.priority.value(), Priority::default().value());
        assert!(opts.date.is_none());
        assert!(opts.law_name.is_none());
        assert!(opts.slug.is_none());
    }
}
