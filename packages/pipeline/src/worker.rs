use std::path::Path;
use std::time::Duration;

use regelrecht_corpus::{CorpusClient, CorpusConfig};
use reqwest::Client;
use sqlx::PgPool;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::WorkerConfig;
use crate::db;
use crate::document_convert::{self, DocumentConvertPayload, LlmDocumentConverter};
use crate::enrich::{
    create_enrich_corpus, enrich_branch_name, execute_enrich, progress_file_path, EnrichConfig,
    EnrichPayload, EnrichResult, RelatedLegislation,
};
use crate::error::{PipelineError, Result};
use crate::harvest::{execute_harvest, HarvestPayload, HarvestResult, MAX_HARVEST_DEPTH};
use crate::job_queue::{self, CreateJobRequest};
use crate::law_convert::{self, LawConvertPayload, LlmLawStructurer};
use crate::law_status;
use crate::models::{JobType, LawStatusValue, Priority};
use crate::traject_harvest::{self, TrajectHarvestPayload};

/// Local night window (Europe/Amsterdam) as a half-open hour-of-day range
/// `[start, end)`. Hours in this range get the multiplied enrich cap.
const NIGHT_START_HOUR: i32 = 0;
const NIGHT_END_HOUR: i32 = 8;

/// Whether the given local hour-of-day falls in the night window `[start, end)`.
fn is_night_hour(local_hour: i32) -> bool {
    (NIGHT_START_HOUR..NIGHT_END_HOUR).contains(&local_hour)
}

/// The enrich cap for a given local hour-of-day: the base hourly limit during
/// the day, times `night_multiplier` during the night window. Saturating so a
/// large multiplier can't overflow `u32`.
fn hourly_cap(base: u32, night_multiplier: u32, local_hour: i32) -> u32 {
    if is_night_hour(local_hour) {
        base.saturating_mul(night_multiplier)
    } else {
        base
    }
}

/// Outcome of attempting to process a single job, used to drive the
/// resource-exhaustion circuit breaker in the worker loops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobOutcome {
    /// A job was claimed and handled — either completed, or failed for a
    /// reason specific to that job/law (bad data, LLM error, etc.).
    Processed,
    /// No job was available to claim.
    Idle,
    /// The job failed because the container ran out of the OS resources needed
    /// to spawn processes/threads (fork() EAGAIN / OOM). This is environmental,
    /// not the job's fault, and only clears on restart. The job is failed and
    /// requeued through the normal failure path (so its law status is reset and
    /// it isn't left dangling); the loop additionally counts these and exits the
    /// process once too many happen in a row, so the orchestrator restarts the
    /// worker with a clean process table.
    ResourceExhausted,
}

/// Map a job error to the [`JobOutcome`] reported after normal failure handling:
/// `ResourceExhausted` for environmental fork()/EAGAIN/OOM faults (drives the
/// breaker), `Processed` for ordinary per-law failures.
fn outcome_for_error(err: &str) -> JobOutcome {
    if is_resource_exhaustion(err) {
        JobOutcome::ResourceExhausted
    } else {
        JobOutcome::Processed
    }
}

/// [`outcome_for_error`] voor een mislukte document-convert-job, met één
/// uitzondering die het verschil maakt.
///
/// `is_resource_exhaustion` herkent echte fork/EAGAIN/OOM-uitputting aan
/// tekstmarkers ("cannot fork", "os error 11", …) in de foutmelding. Dat werkt
/// alleen zolang die foutmelding niet door een buitenstaander te kiezen is: de
/// weigering-zonder-toestemming noemt de geüploade bestandsnaam, dus een
/// document dat `cannot fork.pdf` heet zou anders als resource-uitputting tellen
/// en — na `WORKER_MAX_CONSECUTIVE_RESOURCE_FAILURES` van zulke uploads op rij —
/// de breaker laten afgaan die de worker afsluit. Die mislukking is per
/// definitie een gewoon, afgehandeld geval: we classificeren 'm op het
/// fouttype in plaats van op de tekst.
fn document_convert_outcome(err: &PipelineError, msg: &str) -> JobOutcome {
    match err {
        PipelineError::LlmNotPermitted(_) => JobOutcome::Processed,
        _ => outcome_for_error(msg),
    }
}

/// Returns true when an error indicates the container has exhausted the OS
/// resources needed to spawn processes or threads — `fork()` returning EAGAIN
/// ("cannot fork() ... Resource temporarily unavailable"), thread-create
/// failures, or OOM. These faults are environmental: a long-running worker can
/// accumulate them (e.g. un-reaped child processes against a low pids limit)
/// and they only clear when the process restarts. Distinguishing them from
/// per-law failures lets the worker bail out for a restart instead of
/// fast-failing every queued job in a tight loop.
fn is_resource_exhaustion(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "cannot fork",
        "resource temporarily unavailable", // EAGAIN, human-readable form
        "os error 11",                      // EAGAIN
        "cannot allocate memory",
        "os error 12", // ENOMEM
        "unable to create thread",
        "event loop thread panicked",
    ];
    MARKERS.iter().any(|m| e.contains(m))
}

/// Returns true when an enrichment error is deterministic — re-running the same
/// law with the same provider reproduces it, so retrying wastes LLM budget and
/// blocks the serial queue. These are content/output faults: the LLM produced no
/// machine_readable sections at all, or its output failed to parse/validate
/// against the schema (malformed YAML, wrong types, missing fields — all
/// surfaced as `PipelineError::Yaml`, i.e. "YAML error: …").
///
/// Transient faults (timeouts, reaped/stuck jobs, corpus/git-push failures,
/// resource exhaustion, network errors) are deliberately excluded — those can
/// succeed on a later attempt and must stay retryable.
fn is_deterministic_content_failure(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    // These markers track PipelineError's `#[error(...)]` Display formats
    // ("YAML error: …" on `Yaml`, and the `Enrich` message from enrich.rs). The
    // `deterministic_markers_track_error_display_format` test constructs the real
    // errors so a format change fails loudly instead of silently regressing.
    const MARKERS: &[&str] = &[
        "no machine_readable sections", // LLM returned nothing usable
        "yaml error",                   // parse / deserialize / schema-validation failure
    ];
    MARKERS.iter().any(|m| e.contains(m))
}

/// Returns true when a [`materialize_task_workdir`] error means the task-job
/// has no input blobs to materialize — deterministic: there is nothing to
/// find on retry, so it must fail terminally. Any other error from that path
/// (DB hiccup, tempdir/IO failure) is transient and stays retryable — see
/// [`process_enrich_task_job`].
fn is_missing_input_blob_error(err: &str) -> bool {
    err.contains("heeft geen input-blob")
}

/// Interval between orphaned-job reaper runs.
const REAPER_INTERVAL: Duration = Duration::from_secs(60);

/// Spawn the orphaned-job reaper as an independent interval task.
///
/// Running the reaper inside the worker loop meant a wedged job stopped
/// reaping too, freezing the whole queue until a pod restart. As its own
/// task (with its own pool handle) it keeps resetting jobs stuck in
/// 'processing' even when the main loop is blocked on a job. The reaper
/// query is idempotent, so multiple workers running it concurrently is safe.
///
/// The task runs until the cancellation token fires (on worker shutdown).
fn spawn_reaper(
    pool: PgPool,
    orphan_timeout: Duration,
    exhausted_threshold: i32,
    cancel: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match job_queue::reap_orphaned_jobs(&pool, orphan_timeout).await {
                Ok(reaped) => {
                    // Terminaal gefaalde taak-flow-jobs krijgen alsnog een
                    // job_failed-taak; zonder dit verdwijnt de "Bezig"-regel
                    // stil en hoort de aanvrager nooit dat het misging.
                    if let Err(e) = crate::tasks::notify_reaped_task_jobs(&pool, &reaped).await {
                        tracing::warn!(error = %e, "failed to create tasks for reaped jobs");
                    }
                    // Terminaal gefaalde niet-taak-flow enrich-jobs: dezelfde
                    // status/retry/exhausted-bookkeeping als het synchrone
                    // failure-pad, anders strandt de wet in 'enriching' zonder
                    // actieve/pending job (en polt de frontend eindeloos).
                    handle_reaped_enrich_jobs(&pool, &reaped, exhausted_threshold).await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to reap orphaned jobs");
                }
            }
            // GC upload bytes orphaned when a worker died mid-conversion: the
            // generic reaper above fails such a job without running the
            // type-specific delete_upload, so sweep them here.
            match document_convert::cleanup_orphaned_uploads(&pool).await {
                Ok(n) if n > 0 => {
                    tracing::info!(removed = n, "cleaned up orphaned document uploads")
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "failed to clean up orphaned document uploads")
                }
            }
            // GC taak-flow blobs orphaned when their job never got an open
            // review task (crashed worker, dismissed/expired task path).
            match crate::tasks::cleanup_orphaned_blobs(&pool).await {
                Ok(n) if n > 0 => {
                    tracing::info!(removed = n, "cleaned up orphaned job blobs")
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "failed to clean up orphaned job blobs")
                }
            }
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(REAPER_INTERVAL) => {}
            }
        }
    })
}

/// Run the harvest worker loop.
///
/// Polls the job queue for harvest jobs and executes them.
/// Supports graceful shutdown via SIGTERM and SIGINT (ctrl+c).
/// Shutdown is checked between jobs — an in-flight job always runs to completion.
pub async fn run_harvest_worker(config: WorkerConfig) -> Result<()> {
    let pipeline_config = config.pipeline_config();
    let pool = db::create_pool(&pipeline_config).await?;
    db::ensure_schema(&pool).await?;

    // Initialize corpus client if configured
    let corpus = if let Some(ref corpus_config) = config.corpus_config {
        let mut client = CorpusClient::new(corpus_config.clone());
        client.ensure_repo().await?;
        tracing::info!(path = %corpus_config.repo_path.display(), "corpus repo ready");
        Some(client)
    } else {
        tracing::info!("corpus integration disabled (CORPUS_REPO_URL not set)");
        None
    };

    // When corpus is enabled, write output into the corpus repo checkout
    let output_dir = match &corpus {
        Some(client) => client.repo_path().to_path_buf(),
        None => config.output_dir.clone(),
    };

    let http_client = regelrecht_harvester::http::create_client().map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to create HTTP client: {e}"))
    })?;

    tracing::info!(
        output_dir = %output_dir.display(),
        output_base = %config.regulation_output_base,
        poll_interval = ?config.poll_interval,
        job_timeout = ?config.job_timeout,
        orphan_timeout = ?config.orphan_timeout,
        "starting harvest worker"
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to register SIGTERM handler: {e}"))
    })?;

    // Reap orphaned jobs on an independent task so a wedged job in the main
    // loop can't also stop the reaper (which would freeze the whole queue).
    let reaper_cancel = tokio_util::sync::CancellationToken::new();
    let reaper_handle = spawn_reaper(
        pool.clone(),
        config.orphan_timeout,
        config.exhausted_threshold,
        reaper_cancel.clone(),
    );

    let mut current_interval = std::time::Duration::ZERO; // poll immediately on startup
    let mut consecutive_resource_failures: u32 = 0;

    loop {
        // Check for shutdown signals between jobs
        tokio::select! {
            biased;

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received SIGINT, stopping worker");
                break;
            }
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, stopping worker");
                break;
            }
            _ = tokio::time::sleep(current_interval) => {
                // Ready to process next job
            }
        }

        // Traject-scoped harvests (taak-flow) gaan vóór de corpus-brede
        // queue: een mens wacht erop (prio 80-aanvraag uit de editor), en de
        // corpus-brede harvest kan lange bulkruns draaien. Spiegel van hoe de
        // enrich-worker document-/law-convert-jobs vóór de enrich-cap draait.
        match process_next_traject_harvest_job(&pool, &config, &http_client).await {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
                continue;
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "traject-harvest",
                );
                current_interval = config.poll_interval;
                continue;
            }
            Ok(JobOutcome::Idle) => { /* geen traject-harvest — door naar de gewone queue */ }
            Err(e) => {
                tracing::error!(error = %e, "error processing traject-harvest job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                continue;
            }
        }

        // Process job outside of select! — runs to completion without cancellation
        match process_next_job(&pool, &config, &output_dir, corpus.as_ref(), &http_client).await {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
            }
            Ok(JobOutcome::Idle) => {
                consecutive_resource_failures = 0;
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::info!(next_poll = ?current_interval, "no jobs available, backing off");
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "harvest",
                );
                current_interval = config.poll_interval;
            }
            Err(e) => {
                tracing::error!(error = %e, "error processing job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
            }
        }
    }

    reaper_cancel.cancel();
    if let Err(e) = reaper_handle.await {
        tracing::error!(error = %e, "reaper task panicked");
    }

    Ok(())
}

/// Increment the consecutive resource-exhaustion counter and, once it crosses
/// the configured threshold, exit the process so the orchestrator (RIG) restarts
/// the worker with a fresh process table. fork()/EAGAIN faults are environmental
/// and only clear on restart, so bailing out beats fast-failing the whole queue.
fn handle_resource_exhaustion(counter: &mut u32, threshold: u32, worker: &str) {
    *counter += 1;
    tracing::error!(
        worker,
        consecutive = *counter,
        threshold,
        "resource exhaustion (cannot fork / EAGAIN) while processing job"
    );
    if *counter >= threshold {
        tracing::error!(
            worker,
            threshold,
            "resource-exhaustion breaker tripped; exiting so the orchestrator \
             restarts the worker with a clean process table"
        );
        std::process::exit(1);
    }
}

/// Process the next available harvest job.
///
/// Returns the [`JobOutcome`]: `Processed` when a job was handled, `Idle` when
/// none was available, or `ResourceExhausted` when the job failed because the
/// container could not spawn processes/threads (fork()/EAGAIN).
async fn process_next_job(
    pool: &PgPool,
    config: &WorkerConfig,
    output_dir: &Path,
    corpus: Option<&CorpusClient>,
    http_client: &Client,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::Harvest)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };
    // Tells the reaper this job is still being worked on, until it is done.
    let _heartbeat = job_queue::JobHeartbeat::start(pool.clone(), job.id, config.job_timeout);

    tracing::info!(
        job_id = %job.id,
        law_id = %job.law_id,
        attempt = job.attempts,
        "processing harvest job"
    );

    // Parse payload — on failure, fail the job so it doesn't stay orphaned
    let payload: HarvestPayload = match &job.payload {
        Some(p) => match serde_json::from_value(p.clone()) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "invalid harvest payload");
                let error_json =
                    serde_json::json!({ "error": format!("invalid harvest payload: {e}") });
                if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                }
                return Ok(JobOutcome::Processed);
            }
        },
        None => {
            // Infer source type from law_id prefix when no payload is stored.
            let (bwb_id, cvdr_id) = if job.law_id.starts_with("CVDR") {
                (None, Some(job.law_id.clone()))
            } else {
                (Some(job.law_id.clone()), None)
            };
            HarvestPayload {
                bwb_id,
                cvdr_id,
                date: None,
                max_size_mb: None,
                depth: None,
            }
        }
    };

    if let Err(e) = law_status::upsert_law(pool, &job.law_id, None, None).await {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to upsert law entry before harvest");
    }
    if let Err(e) = law_status::update_status(pool, &job.law_id, LawStatusValue::Harvesting).await {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to harvesting");
    }

    // Bound the harvest with the job timeout (mirrors the enrich path): the
    // BWB HTTP fetches and corpus git operations (fetch/pull/push) have no
    // internal deadline, so a hung remote would otherwise stall this
    // sequential worker loop forever. On timeout the error feeds the normal
    // failure path below (retry with backoff / exhausted threshold).
    let harvest_outcome = match tokio::time::timeout(
        config.job_timeout,
        execute_harvest_job(output_dir, config, &payload, corpus, http_client),
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(_elapsed) => {
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                timeout = ?config.job_timeout,
                "harvest job timed out"
            );
            Err(PipelineError::Worker(format!(
                "harvest job timed out after {}s",
                config.job_timeout.as_secs()
            )))
        }
    };

    match harvest_outcome {
        Ok(result) => {
            tracing::info!(
                job_id = %job.id,
                law_name = %result.law_name,
                articles = result.article_count,
                warnings = result.warning_count,
                "harvest completed successfully"
            );

            let result_json = match serde_json::to_value(&result) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, job_id = %job.id, "failed to serialize harvest result");
                    None
                }
            };

            // Use a transaction so job completion and law status update are atomic.
            // Both operations must succeed — if either fails, the transaction is
            // rolled back to prevent inconsistent state (e.g. job 'completed'
            // while law status is stuck at 'harvesting').
            let mut tx = pool.begin().await?;
            job_queue::complete_job(&mut *tx, job.id, result_json).await?;
            law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::Harvested).await?;
            tx.commit().await?;

            if let Err(e) = law_status::reset_fail_count(pool, &job.law_id, JobType::Harvest).await
            {
                tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset harvest fail count after success");
            }

            // Always store slug and refresh law_name from latest harvest.
            if let Err(e) = law_status::upsert_law(
                pool,
                &job.law_id,
                Some(&result.law_name),
                Some(&result.slug),
            )
            .await
            {
                tracing::warn!(error = %e, law_id = %job.law_id, "failed to upsert law name/slug");
            }

            // When the re-harvest was byte-identical to the existing version
            // (only the harvest timestamp differed), no new version was
            // committed — skip the follow-up work that only makes sense for an
            // actual content change (re-enriching identical content wastes LLM
            // budget; referenced-law harvests were already queued on the run
            // that first produced this content).
            if !result.changed {
                tracing::info!(
                    law_id = %job.law_id,
                    "no changes detected — law content unchanged; skipped commit, enrich, and follow-up harvests"
                );
                return Ok(JobOutcome::Processed);
            }

            // Auto-create enrich jobs after successful harvest — one per provider.
            // Each provider writes to its own branch (`enrich/{provider}`)
            // so results can be compared side-by-side.
            // Uses INSERT ... ON CONFLICT DO NOTHING against the
            // idx_unique_active_enrich_job partial unique index to atomically
            // prevent duplicate enrich jobs — no TOCTOU race possible.

            // Auto-enrich is opt-in (parsed once in WorkerConfig from
            // ENRICH_AUTO_ENQUEUE). By default, harvesting a law does NOT enqueue
            // enrich jobs — enrichment is requested explicitly via the admin API
            // (POST /api/enrich-jobs). This prevents the recursive "harvest
            // everything → enrich everything" queue from filling up (and burning
            // LLM budget) for laws nobody asked to enrich.
            let auto_enrich = config.auto_enrich_enqueue;
            // Skip auto-enrich if law is exhausted for enrich
            let enrich_exhausted = match law_status::get_law(pool, &job.law_id).await {
                Ok(law) => law.status == LawStatusValue::EnrichExhausted,
                Err(e) => {
                    tracing::warn!(error = %e, law_id = %job.law_id, "failed to check enrich exhausted status, proceeding with enrich");
                    false
                }
            };
            if !auto_enrich {
                // debug, not info: with auto-enrich off by default this fires for
                // every harvested law (~22k on a full corpus harvest) — steady-state
                // noise, not an event worth surfacing at info.
                tracing::debug!(law_id = %job.law_id, "auto-enrich disabled (set ENRICH_AUTO_ENQUEUE=true to enable); not enqueuing enrich jobs");
            } else if enrich_exhausted {
                tracing::info!(law_id = %job.law_id, "skipping auto-enrich: law is enrich_exhausted");
            } else {
                for provider_name in crate::enrich::ENRICH_PROVIDERS {
                    let enrich_payload = EnrichPayload {
                        pass: Default::default(),
                        law_id: job.law_id.clone(),
                        yaml_path: result.file_path.clone(),
                        provider: Some((*provider_name).to_string()),
                        // Inherit the harvest's depth. NB: this is the shared
                        // extref-recursion counter, so a law reached via
                        // >= RELATED_HARVEST_MAX_DEPTH extref hops enriches at a
                        // depth that skips related-legislation discovery. Roots and
                        // shallow laws (the intended case) are unaffected; a
                        // dedicated related-depth counter is the follow-up.
                        depth: payload.depth,
                        requested_by: None,
                        deliver: None,
                        traject_id: None,
                        traject_ref: None,
                        source_etag: None,
                        new_law: None,
                        chunk_articles: None,
                        skip_mvt: None,
                        // Queue payload: a session never outlives its window.
                        session: None,
                    };
                    let payload_json = match serde_json::to_value(&enrich_payload) {
                        Ok(json) => json,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "failed to serialize enrich payload, skipping"
                            );
                            continue;
                        }
                    };
                    let enrich_req = CreateJobRequest::new(JobType::Enrich, &job.law_id)
                        .with_priority(auto_enrich_priority(payload.depth))
                        .with_payload(payload_json);
                    match job_queue::create_enrich_job_if_not_exists(pool, enrich_req).await {
                        Ok(Some(enrich_job)) => {
                            // Link the first created enrich job to the law entry.
                            // With dual providers only one enrich_job_id column exists,
                            // so the first provider's job wins.
                            if let Err(e) =
                                law_status::set_enrich_job(pool, &job.law_id, enrich_job.id).await
                            {
                                tracing::warn!(
                                    error = %e,
                                    law_id = %job.law_id,
                                    enrich_job_id = %enrich_job.id,
                                    "failed to link enrich job to law entry"
                                );
                            }
                            tracing::info!(
                                enrich_job_id = %enrich_job.id,
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "auto-created enrich job after harvest"
                            );
                        }
                        Ok(None) => {
                            tracing::info!(
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "skipping enrich job creation: active job already exists"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                law_id = %job.law_id,
                                provider = %provider_name,
                                "failed to auto-create enrich job (harvest still succeeded)"
                            );
                        }
                    }
                }
            }

            // Create follow-up harvest jobs for referenced laws (best-effort).
            // Respects a depth limit to prevent unbounded recursive harvesting.
            let current_depth = payload.depth.unwrap_or(0);
            if !result.referenced_bwb_ids.is_empty() && current_depth < MAX_HARVEST_DEPTH {
                let next_depth = current_depth + 1;
                let mut created = 0u32;
                for bwb_id in &result.referenced_bwb_ids {
                    // Skip harvest for exhausted laws
                    match law_status::get_law(pool, bwb_id).await {
                        Ok(law) if law.status == LawStatusValue::HarvestExhausted => {
                            tracing::info!(bwb_id = %bwb_id, "skipping follow-up harvest: law is harvest_exhausted");
                            continue;
                        }
                        _ => {}
                    }

                    // Propagate the original requested date through the chain.
                    // When None (no date specified), each law independently resolves
                    // its own latest consolidation from BWB — this ensures we always
                    // harvest the version that is valid today.
                    // Follow-up jobs from referenced_bwb_ids are always BWB laws.
                    let follow_up_payload = HarvestPayload {
                        bwb_id: Some(bwb_id.clone()),
                        cvdr_id: None,
                        date: payload.date.clone(),
                        max_size_mb: payload.max_size_mb,
                        depth: Some(next_depth),
                    };
                    let payload_json = match serde_json::to_value(&follow_up_payload) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(bwb_id = %bwb_id, error = %e, "failed to serialize follow-up payload");
                            continue;
                        }
                    };
                    let req = CreateJobRequest::new(JobType::Harvest, bwb_id.as_str())
                        .with_priority(Priority::new(30))
                        .with_payload(payload_json);
                    let dedup_date = payload.date.as_deref().unwrap_or(&result.harvest_date);
                    match job_queue::create_harvest_job_if_not_exists(pool, req, dedup_date).await {
                        Ok(Some(_)) => created += 1,
                        Ok(None) => {} // already exists, skip
                        Err(e) => tracing::warn!(
                            bwb_id = %bwb_id,
                            error = %e,
                            "failed to create follow-up harvest job"
                        ),
                    }
                }
                if created > 0 {
                    tracing::info!(
                        count = created,
                        total_refs = result.referenced_bwb_ids.len(),
                        parent_job_id = %job.id,
                        parent_law_id = %job.law_id,
                        depth = next_depth,
                        "created follow-up harvest jobs for referenced laws"
                    );
                }
            } else if !result.referenced_bwb_ids.is_empty() {
                tracing::info!(
                    depth = current_depth,
                    max_depth = MAX_HARVEST_DEPTH,
                    refs = result.referenced_bwb_ids.len(),
                    parent_job_id = %job.id,
                    parent_law_id = %job.law_id,
                    "skipping follow-up harvest jobs: max depth reached"
                );
            }

            Ok(JobOutcome::Processed)
        }
        Err(e) => {
            // The work has no consolidated text to harvest (withdrawn / not yet
            // in force / only announced). The skip reason is uniform — there is
            // no text — so the law lands in a single terminal `not_harvestable`
            // status; the precise reason and date are kept in the job result.
            // Complete (don't fail) the job so it is never retried.
            if let PipelineError::Harvester(
                regelrecht_harvester::HarvesterError::NoConsolidatedText { reason, .. },
            ) = &e
            {
                tracing::info!(
                    job_id = %job.id,
                    law_id = %job.law_id,
                    ?reason,
                    "no consolidated text to harvest; marking not_harvestable (terminal)"
                );
                let result_json = serde_json::json!({ "skipped": reason });
                let mut tx = pool.begin().await?;
                job_queue::complete_job(&mut *tx, job.id, Some(result_json)).await?;
                law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::NotHarvestable)
                    .await?;
                tx.commit().await?;
                return Ok(JobOutcome::Processed);
            }

            // Container resource exhaustion (fork()/EAGAIN) is environmental, not
            // the law's fault, but the attempt was already spent at claim time, so
            // we still run the normal failure/requeue path (which resets law
            // status). We additionally report it via the outcome so the loop's
            // breaker can exit the worker for a clean restart — the only thing
            // that actually clears the condition.
            let outcome = outcome_for_error(&e.to_string());

            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                error = %e,
                "harvest failed"
            );

            let error_json = serde_json::json!({ "error": e.to_string() });
            // Don't `?` here: a failure while recording the failure must still
            // report `outcome` so the breaker counts a resource-exhaustion fault
            // (matches how the enrich paths handle fail_job errors). For a
            // non-resource error `outcome` is `Processed`, so the loop just moves
            // on — a genuine DB outage resurfaces at the next claim_job and backs
            // off there, so this can't tight-loop.
            let failed_job = match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                Ok(j) => j,
                Err(fail_err) => {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark harvest job as failed");
                    return Ok(outcome);
                }
            };

            // Only mark law as failed when retries are exhausted
            if failed_job.status == crate::models::JobStatus::Failed {
                if let Err(status_err) =
                    law_status::update_status(pool, &job.law_id, LawStatusValue::HarvestFailed)
                        .await
                {
                    tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to harvest_failed");
                }

                // Check exhausted threshold
                match law_status::increment_fail_count(pool, &job.law_id, JobType::Harvest).await {
                    Ok(count) if count >= config.exhausted_threshold => {
                        if let Err(e) =
                            law_status::exhaust_law(pool, &job.law_id, JobType::Harvest).await
                        {
                            tracing::warn!(error = %e, law_id = %job.law_id, "failed to mark law as harvest_exhausted");
                        }
                    }
                    Ok(count) => {
                        // Not yet exhausted — queue a new harvest job so the
                        // fail_count can accumulate toward the threshold. The
                        // job starts with a backoff delay that grows with the
                        // law's fail count, so a transient BWB outage doesn't
                        // mass-exhaust laws within minutes.
                        let retry_delay = job_queue::retry_backoff(count);
                        tracing::info!(
                            law_id = %job.law_id,
                            fail_count = count,
                            threshold = config.exhausted_threshold,
                            delay = ?retry_delay,
                            "scheduling auto-retry harvest job"
                        );
                        match serde_json::to_value(&payload) {
                            Ok(payload_json) => {
                                let date = payload.date.as_deref().unwrap_or("");
                                let req = CreateJobRequest::new(JobType::Harvest, &job.law_id)
                                    .with_priority(Priority::new(job.priority))
                                    .with_payload(payload_json)
                                    .with_initial_delay(retry_delay);
                                match job_queue::create_harvest_job_if_not_exists(pool, req, date)
                                    .await
                                {
                                    Ok(Some(new_job)) => {
                                        tracing::info!(
                                            new_job_id = %new_job.id,
                                            law_id = %job.law_id,
                                            "auto-retry harvest job created"
                                        );
                                    }
                                    Ok(None) => {
                                        tracing::debug!(
                                            law_id = %job.law_id,
                                            "auto-retry harvest job skipped: active job already exists"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            law_id = %job.law_id,
                                            "failed to create auto-retry harvest job"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    law_id = %job.law_id,
                                    "failed to serialize retry payload, skipping auto-retry harvest job"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, law_id = %job.law_id, "failed to increment harvest fail count");
                    }
                }
            } else {
                // Job will be retried — reset law status to queued
                if let Err(status_err) =
                    law_status::update_status(pool, &job.law_id, LawStatusValue::Queued).await
                {
                    tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to reset status to queued for retry");
                }
            }

            Ok(outcome)
        }
    }
}

/// Run the enrich worker loop.
///
/// Polls the job queue for enrich jobs and executes them using the configured
/// LLM provider (opencode or claude). Each enrichment pushes to a separate
/// branch (`enrich/{provider}`) for review before merging.
///
/// Supports graceful shutdown via SIGTERM and SIGINT (ctrl+c).
pub async fn run_enrich_worker(config: WorkerConfig) -> Result<()> {
    let pipeline_config = config.pipeline_config();
    let pool = db::create_pool(&pipeline_config).await?;
    db::ensure_schema(&pool).await?;

    let enrich_config = EnrichConfig::from_env();

    // One shared HTTP client for related-legislation SRU resolution. Built once
    // (connection pooling) and threaded into every enrich job's follow-up hook.
    let http_client = regelrecht_harvester::http::create_client().map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to create HTTP client: {e}"))
    })?;

    // Corpus config is passed per-job so each enrichment creates its own
    // branch-specific corpus client. We still use the base repo_path as
    // fallback when corpus is not configured.
    let repo_path = config
        .corpus_config
        .as_ref()
        .map(|c| c.repo_path.clone())
        .unwrap_or_else(|| config.output_dir.clone());

    if config.corpus_config.is_some() {
        tracing::info!("corpus integration enabled, enrichments will push to separate branches");
    } else {
        tracing::info!("corpus integration disabled (CORPUS_REPO_URL not set)");
    }

    tracing::info!(
        repo_path = %repo_path.display(),
        provider = %enrich_config.provider.name(),
        poll_interval = ?config.poll_interval,
        job_timeout = ?config.job_timeout,
        orphan_timeout = ?config.orphan_timeout,
        "starting enrich worker"
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(|e| {
        crate::error::PipelineError::Worker(format!("failed to register SIGTERM handler: {e}"))
    })?;

    // Reap orphaned jobs on an independent task so a wedged job in the main
    // loop can't also stop the reaper (which would freeze the whole queue).
    let reaper_cancel = tokio_util::sync::CancellationToken::new();
    let reaper_handle = spawn_reaper(
        pool.clone(),
        config.orphan_timeout,
        config.exhausted_threshold,
        reaper_cancel.clone(),
    );

    let mut current_interval = std::time::Duration::ZERO;
    let mut consecutive_resource_failures: u32 = 0;
    // Log the "paused on hourly limit" state at info only on the first hit, then
    // debug while it persists — otherwise a paused worker (e.g. ENRICH_HOURLY_LIMIT
    // unset) emits an info line every poll interval indefinitely. Reset once a job
    // actually runs so a later pause is surfaced again.
    let mut hourly_limit_pause_logged = false;

    loop {
        tokio::select! {
            biased;

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received SIGINT, stopping enrich worker");
                break;
            }
            _ = sigterm.recv() => {
                tracing::info!("received SIGTERM, stopping enrich worker");
                break;
            }
            _ = tokio::time::sleep(current_interval) => {
                // Ready to process next job
            }
        }

        // Document-convert jobs run BEFORE the enrich hourly-cap gate: they are
        // interactive, user-initiated uploads (naturally rate-limited) and must
        // not be blocked when bulk enrichment has hit its budget cap. They still
        // share this worker's LLM CLI environment and OAuth token.
        match process_next_document_convert_job(&pool, &enrich_config, config.job_timeout).await {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
                continue;
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "document-convert",
                );
                current_interval = config.poll_interval;
                continue;
            }
            Ok(JobOutcome::Idle) => { /* no document-convert job — continue to enrich */ }
            Err(e) => {
                tracing::error!(error = %e, "error processing document-convert job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                continue;
            }
        }

        // Law-convert jobs (upload → basis-wet-YAML) run before the cap gate
        // for the same reason as document-convert: interactive uploads. The
        // enrich job they chain into DOES fall under the hourly cap.
        match process_next_law_convert_job(&pool, &enrich_config, config.job_timeout).await {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
                continue;
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "law-convert",
                );
                current_interval = config.poll_interval;
                continue;
            }
            Ok(JobOutcome::Idle) => { /* no law-convert job, continue to enrich */ }
            Err(e) => {
                tracing::error!(error = %e, "error processing law-convert job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                continue;
            }
        }

        // Enforce the per-provider hourly run cap before claiming a job. The cap
        // is multiplied during the local night window (00:00–08:00 Europe/
        // Amsterdam) by ENRICH_NIGHT_MULTIPLIER, so bulk enrichment runs mostly
        // overnight. Counted from the durable `jobs` table (not an in-memory
        // counter) so the cap holds across restarts/redeploys.
        //
        // Task-flow enrich jobs (deliver=task, e.g. "Verrijk deze wet") fall
        // under this cap on purpose: they are LLM runs on the same shared token
        // budget as bulk enrichment, so the spend guard must cover them too.
        // Their higher priority (80) means they are claimed first within the
        // budget, so an interactive request still wins the next available slot —
        // but if the cap is already exhausted it waits like any other job.
        // Up-to-an-hour interactive latency is the accepted tradeoff here
        // (follow-up: a separate budget lane for interactive requests so they
        // aren't starved by a bulk backlog). Document-convert runs *before* this
        // gate on purpose — it is not an LLM run and doesn't touch the token
        // budget, so it must not be throttled by the enrich cap.
        //
        // Fail-closed: a base limit of 0 (the default when ENRICH_HOURLY_LIMIT is
        // unset) pauses the worker without even querying.
        //
        // The cap keys on the worker's configured provider (LLM_PROVIDER), not
        // the per-job payload provider — exact for a provider-dedicated worker
        // (the intended deployment).
        let base = config.enrich_hourly_limit;
        let provider = enrich_config.provider.name();
        // `Some((cap, local_hour))` when we must pause; `None` when clear to run.
        // A base of 0 pauses with a sentinel hour of -1 (renders as window=day).
        let pause: Option<(u32, i32)> = if base == 0 {
            Some((0, -1))
        } else {
            match job_queue::count_enrich_jobs_started_this_hour(&pool, provider).await {
                Ok((ran_this_hour, local_hour)) => {
                    let cap = hourly_cap(base, config.enrich_night_multiplier, local_hour);
                    if ran_this_hour >= i64::from(cap) {
                        Some((cap, local_hour))
                    } else {
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to check hourly enrich limit, proceeding");
                    None
                }
            }
        };
        if let Some((cap, local_hour)) = pause {
            current_interval = config.max_poll_interval;
            let window = if is_night_hour(local_hour) {
                "night"
            } else {
                "day"
            };
            if !hourly_limit_pause_logged {
                tracing::info!(
                    provider,
                    cap,
                    local_hour,
                    window,
                    next_poll = ?current_interval,
                    "hourly enrich limit reached (or ENRICH_HOURLY_LIMIT unset/0), pausing until the next local hour"
                );
                hourly_limit_pause_logged = true;
            } else {
                tracing::debug!(
                    provider,
                    cap,
                    local_hour,
                    window,
                    "still paused on hourly enrich limit"
                );
            }
            continue;
        }
        // Not paused this cycle — re-arm the info-level pause log for the next pause.
        hourly_limit_pause_logged = false;

        match process_next_enrich_job(
            &pool,
            &repo_path,
            &enrich_config,
            config.corpus_config.as_ref(),
            config.job_timeout,
            config.exhausted_threshold,
            &http_client,
            config.related_harvest_max_depth,
        )
        .await
        {
            Ok(JobOutcome::Processed) => {
                consecutive_resource_failures = 0;
                current_interval = config.poll_interval;
            }
            Ok(JobOutcome::Idle) => {
                consecutive_resource_failures = 0;
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
                tracing::info!(next_poll = ?current_interval, "no enrich jobs available, backing off");
            }
            Ok(JobOutcome::ResourceExhausted) => {
                handle_resource_exhaustion(
                    &mut consecutive_resource_failures,
                    config.max_consecutive_resource_failures,
                    "enrich",
                );
                current_interval = config.poll_interval;
            }
            Err(e) => {
                tracing::error!(error = %e, "error processing enrich job");
                current_interval = (current_interval * 2)
                    .max(config.poll_interval)
                    .min(config.max_poll_interval);
            }
        }
    }

    reaper_cancel.cancel();
    if let Err(e) = reaper_handle.await {
        tracing::error!(error = %e, "reaper task panicked");
    }

    Ok(())
}

/// Base priority for related-legislation follow-up harvests. One point below
/// this per nesting level (see [`related_harvest_priority`]) so the speculative
/// related-harvest chain always yields to editor- and root-requested harvests
/// (which use higher priorities).
const RELATED_HARVEST_BASE: i32 = 40;

/// Priority for a related-legislation follow-up harvest spawned by an enrichment
/// at `enrich_depth`. Drops one point per nesting level so deeper (more
/// speculative) harvests yield to shallower ones; [`Priority::new`] clamps the
/// result into the valid `0..=100` range.
fn related_harvest_priority(enrich_depth: u32) -> Priority {
    Priority::new(RELATED_HARVEST_BASE - (enrich_depth as i32 + 1))
}

/// Priority for enrich jobs auto-created after a *recursive* (follow-up)
/// harvest. Well below the default (50) so speculative, recursively-discovered
/// enrichments always yield to directly/manually requested enrich work.
const RECURSIVE_ENRICH_PRIORITY: i32 = 10;

/// Auto-enrich priority for a harvest at the given `depth`. Root/direct harvests
/// (depth `None`/`0`) keep the default priority; recursive follow-up harvests
/// (depth `>= 1`) drop to [`RECURSIVE_ENRICH_PRIORITY`] so they are claimed only
/// after all directly/manually requested enrich jobs.
fn auto_enrich_priority(depth: Option<u32>) -> Priority {
    if depth.unwrap_or(0) == 0 {
        Priority::default()
    } else {
        Priority::new(RECURSIVE_ENRICH_PRIORITY)
    }
}

/// True when `s` is a syntactically valid BWB regulation id (`^BWBR\d{7}$`).
fn is_valid_bwb_id(s: &str) -> bool {
    s.len() == 11 && s.starts_with("BWBR") && s[4..].bytes().all(|b| b.is_ascii_digit())
}

/// Turn a law name into a corpus slug: ASCII-lowercase, every run of
/// non-alphanumeric characters collapsed to a single `_`, trimmed of leading and
/// trailing `_`. Best-effort fallback used for slug lookup when the agent didn't
/// supply an explicit `slug`.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_underscore = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_underscore && !out.is_empty() {
                out.push('_');
            }
            pending_underscore = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_underscore = true;
        }
    }
    out
}

/// Outcome of resolving a single [`RelatedLegislation`] entry to a BWB id.
enum RelatedResolution {
    /// Resolved to a concrete BWB id.
    Resolved(String),
    /// SRU search matched more than one law — a human must pick; skip for now.
    NeedsConfirmation,
    /// No candidate: unknown slug or zero SRU hits. Skip.
    Unresolved,
    /// The source could not be asked — a throttle or an outage at
    /// wetten.overheid.nl. Distinct from `Unresolved` on purpose: the law may
    /// well exist, so counting this as "not found" hides an incomplete
    /// enrichment behind a normal-looking number.
    LookupFailed,
}

/// Map an SRU search outcome onto a resolution. Only an unambiguous single hit
/// with a well-formed BWB id resolves; a malformed id must not slip into a
/// harvest payload.
fn classify_sru_result(
    results: std::result::Result<Vec<crate::api::bwb_search::BwbSearchResult>, String>,
) -> RelatedResolution {
    match results {
        Ok(results) if results.len() == 1 && is_valid_bwb_id(&results[0].bwb_id) => {
            RelatedResolution::Resolved(results[0].bwb_id.clone())
        }
        Ok(results) if results.len() > 1 => RelatedResolution::NeedsConfirmation,
        Ok(_) => RelatedResolution::Unresolved,
        Err(_) => RelatedResolution::LookupFailed,
    }
}

/// Resolve a related-legislation entry to a BWB id via the hybrid order:
/// (a) explicit valid `bwb_id`, (b) slug lookup (explicit `slug` else
/// `slugify(name)`) against `law_entries`, (c) SRU search by name accepting only
/// an unambiguous single hit. Never errors — lookup failures degrade to skips.
async fn resolve_related_bwb_id(
    pool: &PgPool,
    http_client: &Client,
    entry: &RelatedLegislation,
) -> RelatedResolution {
    // (a) explicit bwb_id
    if let Some(bwb_id) = entry.bwb_id.as_deref() {
        if is_valid_bwb_id(bwb_id) {
            return RelatedResolution::Resolved(bwb_id.to_string());
        }
    }

    // (b) slug lookup
    let slug = entry.slug.clone().unwrap_or_else(|| slugify(&entry.name));
    if !slug.is_empty() {
        match crate::api::harvest::find_bwb_id_by_slug(pool, &slug).await {
            // The slug may map to a CVDR id (local regulation); only a BWB id is
            // harvestable through the `bwb_id` follow-up path, so skip non-BWB
            // hits rather than enqueue a malformed harvest.
            Ok(Some(id)) if is_valid_bwb_id(&id) => return RelatedResolution::Resolved(id),
            Ok(Some(id)) => {
                // The slug already identified the law (it's just CVDR, not
                // BWB-harvestable here). Do NOT fall through to the name search:
                // a title match could resolve a *different* national law.
                tracing::debug!(slug = %slug, resolved = %id, "slug resolved to a non-BWB id; not harvestable via bwb_id path");
                return RelatedResolution::Unresolved;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(slug = %slug, error = %e, "slug lookup failed for related legislation");
            }
        }
    }

    // (c) SRU search by name — accept only an unambiguous single hit, and only
    // if it is a well-formed BWB id (paths a/b validate too; don't let a
    // malformed SRU id slip into a harvest payload).
    let results = crate::api::bwb_search::search_bwb_by_name(http_client, &entry.name).await;
    if let Err(e) = &results {
        tracing::warn!(name = %entry.name, error = %e, "SRU search failed for related legislation");
    }
    classify_sru_result(results)
}

/// Resolve every related-legislation entry declared by an enrichment and enqueue
/// a follow-up harvest for each resolved BWB id at `enrich_depth + 1`. Emits one
/// summary log with the total/resolved/enqueued/needs_confirmation/unresolved
/// counts. Best-effort throughout: a failure on one entry never blocks the rest,
/// and none of this can fail the already-committed enrichment.
async fn harvest_related_legislation(
    pool: &PgPool,
    http_client: &Client,
    parent_law_id: &str,
    related: &[RelatedLegislation],
    enrich_depth: u32,
) {
    if related.is_empty() {
        return;
    }

    let child_depth = enrich_depth + 1;
    let priority = related_harvest_priority(enrich_depth);
    let total = related.len();
    let mut resolved = 0u32;
    let mut enqueued = 0u32;
    let mut already_queued = 0u32;
    let mut exhausted = 0u32;
    let mut needs_confirmation = 0u32;
    let mut unresolved = 0u32;
    let mut lookup_failed = 0u32;

    for entry in related {
        let bwb_id = match resolve_related_bwb_id(pool, http_client, entry).await {
            RelatedResolution::Resolved(id) => id,
            RelatedResolution::NeedsConfirmation => {
                needs_confirmation += 1;
                tracing::info!(
                    parent_law_id = %parent_law_id,
                    name = %entry.name,
                    "related legislation matched multiple BWB results: needs_confirmation, skipping"
                );
                continue;
            }
            RelatedResolution::Unresolved => {
                unresolved += 1;
                continue;
            }
            RelatedResolution::LookupFailed => {
                lookup_failed += 1;
                tracing::warn!(
                    parent_law_id = %parent_law_id,
                    name = %entry.name,
                    "related legislation could not be looked up: enrichment is incomplete for this entry"
                );
                continue;
            }
        };
        resolved += 1;

        // Skip harvest for exhausted laws (mirror the follow-up harvest block).
        if let Ok(law) = law_status::get_law(pool, &bwb_id).await {
            if law.status == LawStatusValue::HarvestExhausted {
                exhausted += 1;
                tracing::info!(bwb_id = %bwb_id, "skipping related harvest: law is harvest_exhausted");
                continue;
            }
        }

        // Related harvests always want the latest consolidation (date None). The
        // dedup key uses the payload date, which is NULL here — the ON-CONFLICT
        // guard still matches existing NULL-date jobs and skips duplicates.
        let follow_up_payload = HarvestPayload {
            bwb_id: Some(bwb_id.clone()),
            cvdr_id: None,
            date: None,
            max_size_mb: None,
            depth: Some(child_depth),
        };
        let payload_json = match serde_json::to_value(&follow_up_payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(bwb_id = %bwb_id, error = %e, "failed to serialize related harvest payload");
                continue;
            }
        };
        let req = CreateJobRequest::new(JobType::Harvest, bwb_id.as_str())
            .with_priority(priority)
            .with_payload(payload_json);
        match job_queue::create_harvest_job_if_not_exists(pool, req, "").await {
            Ok(Some(_)) => enqueued += 1,
            Ok(None) => already_queued += 1, // an active harvest already exists
            Err(e) => {
                tracing::warn!(bwb_id = %bwb_id, error = %e, "failed to create related harvest job")
            }
        }
    }

    tracing::info!(
        parent_law_id = %parent_law_id,
        depth = child_depth,
        total,
        resolved,
        enqueued,
        already_queued,
        exhausted,
        needs_confirmation,
        unresolved,
        lookup_failed,
        "related-legislation harvest summary"
    );
}

/// Process the next available document-convert job: convert an uploaded
/// document to markdown (via the LLM agent) and write it back to the traject's
/// corpus as a werkdocument.
///
/// Runs in the enrich worker so it reuses the LLM CLI environment, OAuth token
/// and hourly budget. Returns `Idle` when no job is pending.
async fn process_next_document_convert_job(
    pool: &PgPool,
    enrich_config: &EnrichConfig,
    job_timeout: Duration,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::DocumentConvert)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };
    // Tells the reaper this job is still being worked on, until it is done.
    let _heartbeat = job_queue::JobHeartbeat::start(pool.clone(), job.id, job_timeout);

    // Parse the payload. A malformed payload is deterministic — fail terminally
    // (no retry) and still drop the transient upload if we can recover its id.
    let payload: DocumentConvertPayload = match job
        .payload
        .as_ref()
        .ok_or_else(|| PipelineError::Worker("document_convert job has no payload".to_string()))
        .and_then(|p| {
            serde_json::from_value(p.clone())
                .map_err(|e| PipelineError::Worker(format!("payload deserialization failed: {e}")))
        }) {
        Ok(payload) => payload,
        Err(e) => {
            let msg = format!("invalid document_convert payload: {e}");
            tracing::error!(job_id = %job.id, error = %msg);
            // Best-effort: if the raw payload still yields a usable upload id,
            // drop the orphaned bytes so a malformed job doesn't leak them.
            if let Some(id) = job
                .payload
                .as_ref()
                .and_then(|p| p.get("upload_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
            {
                let _ = document_convert::delete_upload(pool, id).await;
            }
            job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
                .await?;
            return Ok(JobOutcome::Processed);
        }
    };

    // Contract-guard, vóór de (kostbare) conversie: een document-convert-job
    // heeft een traject-doel en mag dus uitsluitend via de taak-flow
    // opleveren — het directe-push-pad naar de traject-repo is verwijderd
    // (worker/traject-contract, zie de crate-doc). Dit dekt zowel
    // `deliver != "task"` (waaronder jobs van vóór het taken-mechanisme) als
    // een ontbrekende `requested_by` (geen assignee voor de review-taak).
    // Terminaal falen zónder taak, spiegel van de `law_convert`-gate
    // hieronder; de upload-bytes worden meteen opgeruimd, er is toch geen
    // aflever-pad meer voor ze.
    if let Err(e) = payload.require_task_delivery() {
        let msg = e.to_string();
        tracing::error!(job_id = %job.id, error = %msg);
        let _ = document_convert::delete_upload(pool, payload.upload_id).await;
        job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
            .await?;
        return Ok(JobOutcome::Processed);
    }

    // Apply the payload's provider override (if any) and bound the run by the
    // worker's job timeout, mirroring the enrich path. The heartbeat keeps the
    // reaper away for as long as the run takes.
    let mut job_config = match &payload.provider {
        Some(provider) => enrich_config.with_provider_override(provider),
        None => enrich_config.clone(),
    };
    let convert_deadline = job_timeout;
    job_config.timeout = convert_deadline;

    match run_document_convert(pool, &job, &payload, &job_config, convert_deadline).await {
        Ok(()) => {
            // Taak-flow: `finish_document_convert_task_job` already inserted the
            // result-blob, completed the job, and created the review-taak,
            // atomically. Only the transient upload bytes remain to clean up.
            if let Err(e) = document_convert::delete_upload(pool, payload.upload_id).await {
                tracing::warn!(job_id = %job.id, error = %e, "failed to delete document upload after success");
            }
            Ok(JobOutcome::Processed)
        }
        Err(e) => {
            let msg = e.to_string();
            tracing::error!(job_id = %job.id, error = %msg, "document-convert job failed");
            // Document-convert jobs are single-attempt (the upload handler sets
            // max_attempts=1): a retry would re-run the expensive LLM conversion
            // and most failures are deterministic. Fail terminally and drop the
            // transient upload bytes — there is normally no retry to feed them to.
            //
            // The fail-marking and the informative `job_failed` task (so the
            // mislukking niet stil in de eeuwigheid verdwijnt — the job is also
            // filtered out of the documenten-jobs-lijst by then) are committed
            // together in one transaction: a crash or DB error between them
            // would otherwise leave a terminally-failed job with no task ever
            // to notify the uploader. This is no longer best-effort: if task
            // creation fails, the whole transaction (including the fail
            // marking) rolls back and the job stays 'processing'. With the
            // current max_attempts=1 the orphan reaper will then mark it
            // failed without a task (a rare double-fault: a DB error exactly
            // here) — but the rollback at least keeps the fail-marking and
            // the task atomic in every case where the DB is healthy.
            let fail_result: Result<()> = async {
                let mut tx = pool.begin().await?;
                job_queue::fail_job_terminal(
                    &mut *tx,
                    job.id,
                    Some(serde_json::json!({ "error": msg.clone() })),
                )
                .await?;
                if let Some(account_id) = payload.requested_by {
                    crate::tasks::create_task(
                        &mut *tx,
                        crate::tasks::NewTask {
                            task_type: crate::tasks::TaskType::JobFailed,
                            assignee_account_id: Some(account_id),
                            traject_id: Some(payload.traject_id),
                            job_id: Some(job.id),
                            title: format!("Conversie mislukt: {}", payload.target_path),
                            payload: Some(serde_json::json!({
                                "traject_ref": payload.traject_ref,
                                "target_path": payload.target_path,
                                "error": msg.clone(),
                            })),
                        },
                    )
                    .await?;
                }
                tx.commit().await?;
                Ok(())
            }
            .await;
            // Only drop the upload bytes when the fail+task transaction actually
            // committed. On rollback (a DB error above) the job stays
            // 'processing'; under the current max_attempts=1 the orphan reaper
            // will mark it failed rather than retry it, and
            // `cleanup_orphaned_uploads` (in the poll loop) sweeps the orphaned
            // row. Keeping the bytes here is forward-compatible defense: if
            // max_attempts is ever raised, the reaper's re-queue genuinely
            // re-reads these bytes. This deletion targets a separate
            // `document_uploads` row, not the job/task tables, so it stays
            // outside the transaction above.
            if fail_result.is_ok() {
                if let Err(e) = document_convert::delete_upload(pool, payload.upload_id).await {
                    tracing::warn!(job_id = %job.id, error = %e, "failed to delete document upload after terminal failure");
                }
            }
            fail_result?;

            Ok(document_convert_outcome(&e, &msg))
        }
    }
}

/// Convert the uploaded document to markdown and deliver it as a result-blob +
/// review-taak (taak-flow — het enige aflever-pad; de guard in de aanroeper
/// heeft al geborgd dat `payload.deliver_as_task()` waar is, en het oude
/// directe-push-pad naar de traject-repo bestaat niet meer).
/// `finish_document_convert_task_job` completes the job as part of its own
/// transaction, so the caller must NOT call `complete_job` again.
async fn run_document_convert(
    pool: &PgPool,
    job: &crate::models::Job,
    payload: &DocumentConvertPayload,
    config: &EnrichConfig,
    convert_deadline: Duration,
) -> Result<()> {
    // Buitenste begrenzing ónder de reaper-window (zie de aanroeper): de
    // gedropte future komt niet meer aan zijn eigen tempdir-cleanup toe, dus
    // die doen we hier; kill_on_drop ruimt het agent-subprocess op.
    let converted = match tokio::time::timeout(
        convert_deadline,
        document_convert::execute_document_convert(pool, payload, config, &LlmDocumentConverter),
    )
    .await
    {
        Err(_elapsed) => {
            let _ = tokio::fs::remove_dir_all(
                std::env::temp_dir().join(format!("docconvert-{}", payload.upload_id)),
            )
            .await;
            return Err(PipelineError::Enrich(format!(
                "conversie-time-out na {}s (begrensd onder de reaper-window)",
                convert_deadline.as_secs()
            )));
        }
        Ok(r) => r?,
    };
    finish_document_convert_task_job(pool, job, payload, &converted).await?;
    Ok(())
}

/// Verwerk één `law_convert`-job: geüpload document → gevalideerde
/// basis-wet-YAML → geketende taak-flow-enrich-job (het kettingstuk zit in
/// `law_convert::finish_law_convert_job`). Spiegel van
/// [`process_next_document_convert_job`]: single-attempt, terminale fouten
/// krijgen atomair een `job_failed`-taak en de upload-bytes worden opgeruimd.
async fn process_next_law_convert_job(
    pool: &PgPool,
    enrich_config: &EnrichConfig,
    job_timeout: Duration,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::LawConvert)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };
    // Tells the reaper this job is still being worked on, until it is done.
    let _heartbeat = job_queue::JobHeartbeat::start(pool.clone(), job.id, job_timeout);
    tracing::info!(job_id = %job.id, law_id = %job.law_id, "processing law-convert job");

    // Malformed payload is deterministisch: terminaal falen, en de upload
    // best-effort opruimen als het id nog uit de raw payload te vissen is.
    let payload: LawConvertPayload = match job
        .payload
        .as_ref()
        .ok_or_else(|| PipelineError::Worker("law_convert job has no payload".to_string()))
        .and_then(|p| {
            serde_json::from_value(p.clone())
                .map_err(|e| PipelineError::Worker(format!("payload deserialization failed: {e}")))
        }) {
        Ok(payload) => payload,
        Err(e) => {
            let msg = format!("invalid law_convert payload: {e}");
            tracing::error!(job_id = %job.id, error = %msg);
            if let Some(id) = job
                .payload
                .as_ref()
                .and_then(|p| p.get("upload_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
            {
                let _ = document_convert::delete_upload(pool, id).await;
            }
            job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
                .await?;
            return Ok(JobOutcome::Processed);
        }
    };

    // Law-convert kent alléén de taak-flow: zonder `requested_by` is er geen
    // assignee voor de uiteindelijke review-taak, en zonder `deliver: task`
    // is er geen aflever-pad (er bestaat geen direct-push-variant).
    if !payload.deliver_as_task() || payload.requested_by.is_none() {
        let msg = "law_convert vereist deliver=task met requested_by".to_string();
        tracing::error!(job_id = %job.id, error = %msg);
        let _ = document_convert::delete_upload(pool, payload.upload_id).await;
        job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
            .await?;
        return Ok(JobOutcome::Processed);
    }

    let mut job_config = match &payload.provider {
        Some(provider) => enrich_config.with_provider_override(provider),
        None => enrich_config.clone(),
    };
    // Bound the whole conversion (both agent runs together) by the job
    // timeout, so a run that is too slow ends in the visible failure path.
    // kill_on_drop on the subprocess cleans up the agent when it fires.
    let convert_deadline = job_timeout;
    job_config.timeout = convert_deadline;

    let run_result = match tokio::time::timeout(
        convert_deadline,
        law_convert::execute_law_convert(pool, &payload, &job_config, &LlmLawStructurer),
    )
    .await
    {
        Err(_elapsed) => {
            // De gedropte future komt niet meer aan zijn eigen tempdir-cleanup
            // toe; ruim de werkdirectory hier op (pad is deterministisch).
            let _ = tokio::fs::remove_dir_all(
                std::env::temp_dir().join(format!("lawconvert-{}", payload.upload_id)),
            )
            .await;
            Err(PipelineError::Enrich(format!(
                "conversie-time-out na {}s (begrensd onder de reaper-window)",
                convert_deadline.as_secs()
            )))
        }
        Ok(Ok(law)) => law_convert::finish_law_convert_job(pool, &job, &payload, &law).await,
        Ok(Err(e)) => Err(e),
    };

    match run_result {
        Ok(()) => {
            // De keten staat (enrich-job + input-blob + complete, atomair);
            // alleen de transiënte upload-bytes resten nog.
            if let Err(e) = document_convert::delete_upload(pool, payload.upload_id).await {
                tracing::warn!(job_id = %job.id, error = %e, "failed to delete law upload after success");
            }
            Ok(JobOutcome::Processed)
        }
        Err(e) => {
            let msg = e.to_string();
            tracing::error!(job_id = %job.id, error = %msg, "law-convert job failed");
            // Zelfde atomaire fail+taak-transactie als document-convert:
            // single-attempt, dus terminaal, en de aanvrager moet het zien.
            let fail_result: Result<()> = async {
                let mut tx = pool.begin().await?;
                job_queue::fail_job_terminal(
                    &mut *tx,
                    job.id,
                    Some(serde_json::json!({ "error": msg.clone() })),
                )
                .await?;
                if let Some(account_id) = payload.requested_by {
                    crate::tasks::create_task(
                        &mut *tx,
                        crate::tasks::NewTask {
                            task_type: crate::tasks::TaskType::JobFailed,
                            assignee_account_id: Some(account_id),
                            traject_id: Some(payload.traject_id),
                            job_id: Some(job.id),
                            title: format!(
                                "Conversie naar regelwerk mislukt: {}",
                                payload.filename
                            ),
                            payload: Some(serde_json::json!({
                                "traject_ref": payload.traject_ref,
                                "filename": payload.filename,
                                "error": msg.clone(),
                            })),
                        },
                    )
                    .await?;
                }
                tx.commit().await?;
                Ok(())
            }
            .await;
            // Upload-bytes alleen weg wanneer de fail+taak echt gecommit is
            // (zelfde afweging als het document-convert-pad).
            if fail_result.is_ok() {
                if let Err(e) = document_convert::delete_upload(pool, payload.upload_id).await {
                    tracing::warn!(job_id = %job.id, error = %e, "failed to delete law upload after terminal failure");
                }
            }
            fail_result?;

            Ok(outcome_for_error(&msg))
        }
    }
}

/// Verwerk één `traject_harvest`-job: BWB-download → gevalideerde
/// basis-wet-YAML → geketende taak-flow-enrich-job (het kettingstuk is
/// [`law_convert::chain_enrich_and_complete`] — zelfde keten als law-convert).
/// Draait op de harvest-worker (die heeft de BWB-machinerie); de geketende
/// enrich draait daarna op de enrich-worker.
async fn process_next_traject_harvest_job(
    pool: &PgPool,
    config: &WorkerConfig,
    http_client: &Client,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::TrajectHarvest)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };
    // Tells the reaper this job is still being worked on, until it is done.
    let _heartbeat = job_queue::JobHeartbeat::start(pool.clone(), job.id, config.job_timeout);
    tracing::info!(job_id = %job.id, law_id = %job.law_id, attempt = job.attempts, "processing traject-harvest job");

    // Malformed payload is deterministisch: terminaal falen. Zonder payload
    // is er ook geen aanvrager om een taak voor te maken.
    let payload: TrajectHarvestPayload = match job
        .payload
        .as_ref()
        .ok_or_else(|| PipelineError::Worker("traject_harvest job has no payload".to_string()))
        .and_then(|p| {
            serde_json::from_value(p.clone())
                .map_err(|e| PipelineError::Worker(format!("payload deserialization failed: {e}")))
        }) {
        Ok(payload) => payload,
        Err(e) => {
            let msg = format!("invalid traject_harvest payload: {e}");
            tracing::error!(job_id = %job.id, error = %msg);
            job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
                .await?;
            return Ok(JobOutcome::Processed);
        }
    };

    // Traject-harvest kent alléén de taak-flow: zonder `requested_by` is er
    // geen assignee voor de uiteindelijke review-taak, en zonder
    // `deliver: task` geen aflever-pad (er bestaat geen direct-push-variant).
    if !payload.deliver_as_task() || payload.requested_by.is_none() {
        let msg = "traject_harvest vereist deliver=task met requested_by".to_string();
        tracing::error!(job_id = %job.id, error = %msg);
        job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
            .await?;
        return Ok(JobOutcome::Processed);
    }

    // Zelfde buitenboord-timeout als de corpus-brede harvest: de BWB-fetches
    // hebben geen eigen deadline en dit is een sequentiële workerloop.
    let run_result = match tokio::time::timeout(
        config.job_timeout,
        traject_harvest::execute_traject_harvest(&payload, http_client),
    )
    .await
    {
        Err(_elapsed) => Err(PipelineError::Worker(format!(
            "traject-harvest timed out after {}s",
            config.job_timeout.as_secs()
        ))),
        Ok(Ok(law)) => {
            let ctx = law_convert::EnrichChainContext {
                provider: payload.provider.clone(),
                requested_by: payload.requested_by,
                traject_id: payload.traject_id,
                traject_ref: payload.traject_ref.clone(),
            };
            law_convert::chain_enrich_and_complete(pool, &job, &ctx, &law).await
        }
        Ok(Err(e)) => Err(e),
    };

    match run_result {
        Ok(()) => Ok(JobOutcome::Processed),
        Err(e) => {
            let msg = e.to_string();
            tracing::error!(job_id = %job.id, error = %msg, "traject-harvest job failed");
            // Deterministische fouten (geen geconsolideerde tekst; YAML die
            // niet valideert; een al-lopende enrich op dezelfde slug) falen
            // terminaal — een retry reproduceert exact hetzelfde. Transiënte
            // fouten (BWB-outage, netwerk, timeout) doorlopen fail_job met
            // backoff tot max_attempts; pas bij definitief falen krijgt de
            // aanvrager de job_failed-taak.
            let deterministic = matches!(
                e,
                PipelineError::Harvester(
                    regelrecht_harvester::HarvesterError::NoConsolidatedText { .. }
                )
            ) || msg.contains(traject_harvest::SCHEMA_MISMATCH_MARKER)
                || msg.contains(law_convert::ENRICH_IN_PROGRESS_MARKER);

            let fail_and_notify: Result<()> = async {
                let mut tx = pool.begin().await?;
                let terminal = if deterministic {
                    job_queue::fail_job_terminal(
                        &mut *tx,
                        job.id,
                        Some(serde_json::json!({ "error": msg.clone() })),
                    )
                    .await?;
                    true
                } else {
                    let failed = job_queue::fail_job(
                        &mut *tx,
                        job.id,
                        Some(serde_json::json!({ "error": msg.clone() })),
                    )
                    .await?;
                    failed.status == crate::models::JobStatus::Failed
                };
                if terminal {
                    if let Some(account_id) = payload.requested_by {
                        crate::tasks::create_task(
                            &mut *tx,
                            crate::tasks::NewTask {
                                task_type: crate::tasks::TaskType::JobFailed,
                                assignee_account_id: Some(account_id),
                                traject_id: Some(payload.traject_id),
                                job_id: Some(job.id),
                                title: format!("Wet ophalen mislukt: {}", payload.display_name()),
                                payload: Some(serde_json::json!({
                                    "traject_ref": payload.traject_ref,
                                    "bwb_id": payload.bwb_id,
                                    "law_name": payload.law_name,
                                    "error": msg.clone(),
                                })),
                            },
                        )
                        .await?;
                    }
                }
                tx.commit().await?;
                Ok(())
            }
            .await;
            fail_and_notify?;

            Ok(outcome_for_error(&msg))
        }
    }
}

/// Taak-flow: materialiseer de input-blob(s) van een enrich-taak-job in een
/// eigen werkdirectory, zodat `execute_enrich` zonder git-checkout kan
/// draaien. Retourneert de workdir-root (TempDir houdt hem in leven).
pub async fn materialize_task_workdir(
    pool: &PgPool,
    job_id: uuid::Uuid,
) -> Result<tempfile::TempDir> {
    let blobs = crate::tasks::load_blobs(pool, job_id, crate::tasks::BlobKind::Input).await?;
    if blobs.is_empty() {
        return Err(PipelineError::Enrich(format!(
            "taak-job {job_id} heeft geen input-blob"
        )));
    }
    let dir = tempfile::tempdir()
        .map_err(|e| PipelineError::Enrich(format!("kan werkdirectory niet aanmaken: {e}")))?;
    for blob in &blobs {
        // Blob-paden zijn door editor-api gezet (synthetisch, vast formaat);
        // normaliseer defensief tegen path-traversal vóór het join'en.
        let rel = crate::enrich::normalize_yaml_path(&blob.path)?;
        let abs = dir.path().join(&rel);
        if let Some(parent) = abs.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&abs, &blob.content).await?;
    }
    Ok(dir)
}

/// Reden waarmee een geslaagde run die niets beoordeelbaars opleverde alsnog
/// als mislukking bij de aanvrager landt. Zonder deze stap zou de review-taak
/// gewoon worden aangemaakt en zou de gebruiker een taak openen die per
/// definitie niet af te maken is.
pub(crate) const NO_REVIEWABLE_LAW: &str =
    "de verrijking leverde geen regelwerk op om te beoordelen";

/// Whether an enrich run wrote something the review UI can actually open.
///
/// Mirrors how `useTaskReview` picks the law out of the result blobs: the
/// exact `yaml_path` from the payload, else a file whose basename is neither
/// dot-prefixed nor a sidecar. The worker stages `features/*.feature` files
/// alongside the law, and those are not a proposal on their own — a run that
/// produced only those has nothing to review.
///
/// Pure so the contract can be pinned without a live database or workdir.
pub(crate) fn produced_reviewable_law(
    workdir: &Path,
    written_files: &[std::path::PathBuf],
    yaml_path: &str,
) -> bool {
    written_files.iter().any(|abs| {
        let rel = abs.strip_prefix(workdir).unwrap_or(abs);
        if rel.to_string_lossy() == yaml_path {
            return true;
        }
        rel.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.starts_with('.') && name.ends_with(".yaml"))
    })
}

/// Article numbers whose content differs between the source snapshot and the
/// proposal, in the proposal's own order.
///
/// The enrichment writes `machine_readable` per article, so a run typically
/// touches a handful of them while leaving the rest byte-identical. Splitting
/// the review per article means a reviewer decides on what actually changed
/// instead of on the whole law at once.
///
/// Compared as untyped YAML rather than through the law model on purpose: the
/// only question here is "did this article change", and an untyped compare
/// keeps answering that when the schema grows a field this crate does not know
/// about yet. An article the source does not have at all counts as changed.
///
/// A parse failure yields None, and the caller then falls back to one task for
/// the whole law — a proposal we cannot read is still reviewable by hand.
pub(crate) fn changed_articles(source_yaml: &str, proposal_yaml: &str) -> Option<Vec<String>> {
    /// `number:` als string, of de YAML hem nu quootte of niet.
    ///
    /// Een ongequote `number: 2` parset als YAML-integer. Alleen `as_str()`
    /// lezen liet zo'n artikel stil uit beide lijsten vallen: geen review-taak
    /// voor een wél gewijzigd artikel, en ook geen parse-fout die de
    /// hele-wet-fallback aanzet. De frontend normaliseert hetzelfde veld op
    /// dezelfde manier (`String(a.number)` in `proposalDivergence`), en de
    /// taak-payload draagt het nummer als string, dus dat is hier de eenheid.
    fn article_number(article: &serde_yaml_ng::Value) -> Option<String> {
        match article.get("number")? {
            serde_yaml_ng::Value::String(s) => Some(s.clone()),
            serde_yaml_ng::Value::Number(n) => Some(n.to_string()),
            // Alles anders (ontbrekend, null, een lijst) is geen artikelnummer
            // waar een reviewer iets aan heeft; die artikelen blijven buiten
            // de diff.
            _ => None,
        }
    }

    fn articles(yaml: &str) -> Option<Vec<(String, serde_yaml_ng::Value)>> {
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).ok()?;
        let list = doc.get("articles")?.as_sequence()?;
        Some(
            list.iter()
                .filter_map(|a| Some((article_number(a)?, a.clone())))
                .collect(),
        )
    }

    let source = articles(source_yaml)?;
    let proposal = articles(proposal_yaml)?;
    Some(
        proposal
            .into_iter()
            .filter(|(number, article)| {
                source
                    .iter()
                    .find(|(n, _)| n == number)
                    .is_none_or(|(_, before)| before != article)
            })
            .map(|(number, _)| number)
            .collect(),
    )
}

/// Taak-flow succes: schrijf de door de enrichment aangeraakte bestanden als
/// result-blobs, verwijder de input-blobs, complete de job en maak de
/// review-taak aan — alles in één transactie.
///
/// Is de wet na dit venster nog niet af (`law_complete == false`), dan komt er
/// in dezelfde transactie een vervolgjob bij. Zijn input is het resultaat van
/// deze job: het voorstel, zodat het volgende venster voortbouwt op wat hier
/// verrijkt is, en de `.enrichment.yaml` met de cursor, zodat het weet waar het
/// verder moet. Elk venster wordt zo een eigen verrijking met eigen
/// review-taken; de diff tegen zijn input bevat alleen wat dát venster deed.
pub async fn finish_enrich_task_job(
    pool: &PgPool,
    job: &crate::models::Job,
    workdir: &Path,
    written_files: &[std::path::PathBuf],
    law_complete: bool,
    result_json: Option<serde_json::Value>,
) -> Result<Option<crate::models::Job>> {
    let payload: EnrichPayload = serde_json::from_value(job.payload.clone().unwrap_or_default())
        .map_err(|e| PipelineError::Enrich(format!("invalid enrich payload: {e}")))?;

    // Vóór de transactie, want delete_blobs_for_job hieronder gooit de
    // input-blobs weg: dit is de bronsnapshot waartegen we de proposal diffen.
    let inputs = crate::tasks::load_blobs(pool, job.id, crate::tasks::BlobKind::Input).await?;
    // Alleen een vervolgjob krijgt de cursor-sidecar mee; de eerste job van
    // een reeks heeft enkel de wet-YAML uit de editor als input.
    let continuation_input = inputs
        .iter()
        .any(|b| Path::new(&b.path).file_name() == Some(std::ffi::OsStr::new(".enrichment.yaml")));
    let source_yaml = inputs
        .into_iter()
        .find(|b| b.path == payload.yaml_path)
        .map(|b| b.content);

    let mut tx = pool.begin().await?;
    crate::tasks::delete_blobs_for_job(&mut *tx, job.id).await?;
    let mut proposal_yaml: Option<String> = None;
    let mut result_blobs: Vec<(String, String)> = Vec::new();
    for abs in written_files {
        let rel = abs
            .strip_prefix(workdir)
            .map_err(|_| {
                PipelineError::Enrich(format!(
                    "geschreven bestand {} ligt buiten de werkdirectory",
                    abs.display()
                ))
            })?
            .to_string_lossy()
            .to_string();
        let content = tokio::fs::read_to_string(abs).await?;
        if rel == payload.yaml_path {
            proposal_yaml = Some(content.clone());
        }
        crate::tasks::insert_blob(
            &mut *tx,
            job.id,
            crate::tasks::BlobKind::Result,
            &rel,
            &content,
        )
        .await?;
        result_blobs.push((rel, content));
    }
    job_queue::complete_job(&mut *tx, job.id, result_json).await?;
    // Een nieuwe wet (geketend vanuit law_convert) krijgt een eigen titel en
    // een `kind`-discriminator, zodat de editor het voorstel als aan te maken
    // wet behandelt (create-pad) i.p.v. als wijziging van een bestaande.
    let new_law = payload.new_law == Some(true);
    let mut task_payload = serde_json::json!({
        "law_id": payload.law_id,
        "yaml_path": payload.yaml_path,
        "traject_ref": payload.traject_ref,
        "source_etag": payload.source_etag,
        "provider": payload.provider,
    });
    if new_law {
        task_payload["kind"] = serde_json::json!("law_create");
    }

    // Eén taak per gewijzigd artikel. Een verrijking raakt doorgaans een
    // handvol artikelen en laat de rest ongemoeid; per artikel beslissen laat de
    // beoordelaar zich uitspreken over wat er werkelijk veranderd is.
    //
    // Terug naar één taak voor de hele wet wanneer we niet kunnen diffen: bij
    // een nieuwe wet is er geen bron om tegen af te zetten, en bij een
    // onleesbare YAML weten we het simpelweg niet. Beide zijn met de hand nog
    // prima te beoordelen, dus dat is een terugval en geen fout.
    let per_article = if new_law {
        None
    } else {
        match (source_yaml.as_deref(), proposal_yaml.as_deref()) {
            (Some(before), Some(after)) => changed_articles(before, after),
            _ => None,
        }
    };

    // Binnen een reeks vensters van een bestaande wet is die terugval er niet,
    // ook niet in het laatste venster: het voorstel bevat dan ook de nog niet
    // beoordeelde (of afgewezen) artikelen van eerdere vensters, en één taak
    // voor het geheel zou die ongezien terugzetten. Alleen een wet die in één
    // venster past, diffte tegen de echte traject-snapshot. De transactie rolt
    // terug en de aanroeper probeert het venster opnieuw.
    if per_article.is_none() && !new_law && (!law_complete || continuation_input) {
        return Err(PipelineError::Enrich(
            "voorstel van dit venster is niet per artikel te vergelijken met de bron".into(),
        ));
    }

    // Een nieuwe wet wordt als geheel aangemaakt: de tussenvensters leveren
    // nog geen taak op, het laatste venster de hele wet als één voorstel.
    if new_law && !law_complete {
        tracing::info!(
            job_id = %job.id,
            law_id = %payload.law_id,
            "nieuwe wet nog niet af; de review-taak volgt na het laatste venster"
        );
    } else {
        match per_article {
            // Een lege lijst betekent: de proposal verschilt nergens van de
            // bron. Dan valt er niets te beoordelen en komt er geen taak; de
            // job is klaar (bij een venster: het volgende venster volgt).
            Some(articles) => {
                tracing::info!(
                    job_id = %job.id,
                    law_id = %payload.law_id,
                    articles = articles.len(),
                    "review-taken per gewijzigd artikel (0 = geen wijziging, geen taak)"
                );
                for number in articles {
                    let mut article_payload = task_payload.clone();
                    article_payload["article"] = serde_json::json!(number);
                    crate::tasks::create_task(
                        &mut *tx,
                        crate::tasks::NewTask {
                            task_type: crate::tasks::TaskType::JobReview,
                            assignee_account_id: payload.requested_by,
                            traject_id: payload.traject_id,
                            job_id: Some(job.id),
                            title: format!(
                                "Verrijking beoordelen: {} artikel {}",
                                payload.law_id, number
                            ),
                            payload: Some(article_payload),
                        },
                    )
                    .await?;
                }
            }
            None => {
                let title = if new_law {
                    format!("Nieuw regelwerk beoordelen: {}", payload.law_id)
                } else {
                    format!("Verrijking beoordelen: {}", payload.law_id)
                };
                crate::tasks::create_task(
                    &mut *tx,
                    crate::tasks::NewTask {
                        task_type: crate::tasks::TaskType::JobReview,
                        assignee_account_id: payload.requested_by,
                        traject_id: payload.traject_id,
                        job_id: Some(job.id),
                        title,
                        payload: Some(task_payload),
                    },
                )
                .await?;
            }
        }
    }

    let continuation = if law_complete {
        None
    } else {
        // Na `complete_job` in dezelfde transactie, zodat
        // `idx_unique_active_enrich_job` deze job niet meer als actief ziet
        // (zelfde patroon als `complete_enrich_success_tx`).
        let next_json = serde_json::to_value(&payload).map_err(|e| {
            PipelineError::Enrich(format!("serialize continuation enrich payload: {e}"))
        })?;
        let mut req = CreateJobRequest::new(JobType::Enrich, &job.law_id)
            .with_priority(Priority::new(job.priority))
            .with_payload(next_json)
            .with_max_attempts(job.max_attempts);
        if let Some(ref traject_ref) = payload.traject_ref {
            req = req.with_traject_ref(traject_ref.clone());
        }
        let created = job_queue::create_enrich_job_if_not_exists(&mut *tx, req).await?;
        match &created {
            Some(next) => {
                // Alleen de wet en zijn sidecars (de cursor): wat de agent
                // verder schreef, zoals `features/*.feature` met een vrij
                // gekozen naam, kan bij het materialiseren stranden.
                let carried = result_blobs.iter().filter(|(rel, _)| {
                    rel == &payload.yaml_path
                        || Path::new(rel)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with('.'))
                });
                for (rel, content) in carried {
                    crate::tasks::insert_blob(
                        &mut *tx,
                        next.id,
                        crate::tasks::BlobKind::Input,
                        rel,
                        content,
                    )
                    .await?;
                }
            }
            // In de praktijk onbereikbaar: zolang deze job `processing` was,
            // weigerde de index elke tweede actieve verrijking van deze wet in
            // dit traject, dus na `complete_job` staat er niets om mee te
            // botsen. Mocht het toch gebeuren, dan stopt de reeks hier.
            None => tracing::warn!(
                job_id = %job.id,
                law_id = %payload.law_id,
                "vervolgjob niet aangemaakt: er loopt al een verrijking voor deze wet"
            ),
        }
        created
    };

    tx.commit().await?;
    Ok(continuation)
}

/// Taak-flow succes voor document-convert: sla de gegenereerde markdown op
/// als result-blob en maak de review-taak aan - atomair met complete_job.
/// De worker pusht niets; goedkeuren gebeurt in de request-context van de
/// gebruiker (met diens token wanneer enforcement aan staat).
///
/// De genomen route (`pandoc` / `pdftotext` / `llm`) reist mee als
/// `converted_with`. Twee plekken, met opzet: in `jobs.result` als duurzaam
/// spoor van wat er gebeurd is, en in de taak-payload omdat dát is wat de
/// reviewbanner leest (`job_blobs` heeft geen metadata-kolom om het aan de
/// result-blob zelf te hangen). Dit is expliciet iets anders dan het vinkje bij
/// de upload: dat zegt wat mócht, dit zegt wat er is gebeurd.
pub async fn finish_document_convert_task_job(
    pool: &PgPool,
    job: &crate::models::Job,
    payload: &DocumentConvertPayload,
    converted: &document_convert::ConvertedDocument,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    // Hygiëne: er zijn geen input-blobs voor document-convert (de bytes
    // leven in `document_uploads`), maar dit ruimt defensief een eventuele
    // stale result-blob van een eerdere poging op.
    crate::tasks::delete_blobs_for_job(&mut *tx, job.id).await?;
    crate::tasks::insert_blob(
        &mut *tx,
        job.id,
        crate::tasks::BlobKind::Result,
        &payload.target_path,
        &converted.markdown,
    )
    .await?;
    job_queue::complete_job(
        &mut *tx,
        job.id,
        Some(serde_json::json!({ "converted_with": converted.route.as_str() })),
    )
    .await?;
    crate::tasks::create_task(
        &mut *tx,
        crate::tasks::NewTask {
            task_type: crate::tasks::TaskType::JobReview,
            assignee_account_id: payload.requested_by,
            traject_id: Some(payload.traject_id),
            job_id: Some(job.id),
            title: format!("Werkdocument beoordelen: {}", payload.target_path),
            payload: Some(serde_json::json!({
                "kind": "document",
                "traject_ref": payload.traject_ref,
                "target_path": payload.target_path,
                "converted_with": converted.route.as_str(),
            })),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Taak+blob-opruiming die hoort bij een definitief gefaalde taak-job: ruim
/// de input-blobs op en maak een informatieve `job_failed`-taak aan zodat de
/// aanvrager het ziet. Werkt op een bestaande transactie zodat de aanroeper
/// dit samen met de fail-markering atomair kan committen (of terugrollen) —
/// zie [`fail_enrich_task_job`] (altijd terminaal) en
/// [`fail_enrich_task_job_with_retry`] (alleen wanneer `fail_job` de job
/// daadwerkelijk op `Failed` zet).
async fn finalize_failed_task_job_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &crate::models::Job,
    error: &str,
) -> Result<()> {
    let payload: EnrichPayload = serde_json::from_value(job.payload.clone().unwrap_or_default())
        .map_err(|e| PipelineError::Enrich(format!("invalid enrich payload: {e}")))?;

    crate::tasks::delete_blobs_for_job(&mut **tx, job.id).await?;
    let title = if payload.new_law == Some(true) {
        // De geketende enrich van een geüploade wet: de gebruiker kent geen
        // "verrijking", alleen de wet die er niet kwam. De input-blob (de
        // basis-YAML) gaat hier mee weg; opnieuw uploaden is het herstel.
        format!("Regelwerk aanmaken mislukt: {}", payload.law_id)
    } else {
        format!("Verrijking mislukt: {}", payload.law_id)
    };
    crate::tasks::create_task(
        &mut **tx,
        crate::tasks::NewTask {
            task_type: crate::tasks::TaskType::JobFailed,
            assignee_account_id: payload.requested_by,
            traject_id: payload.traject_id,
            job_id: Some(job.id),
            title,
            payload: Some(serde_json::json!({
                "law_id": payload.law_id,
                "traject_ref": payload.traject_ref,
                "error": error,
            })),
        },
    )
    .await?;
    Ok(())
}

/// Taak-flow falen (terminaal): faal de job onvoorwaardelijk en ruim
/// taak+blobs op, atomair in één transactie — een crash of DB-fout tussen de
/// fail-markering en de taak-aanmaak zou anders een definitief gefaalde job
/// zonder `job_failed`-taak achterlaten (onzichtbaar voor de aanvrager). Voor
/// deterministische content-fouten (dezelfde input reproduceert dezelfde
/// fout, retrying verspilt alleen LLM-budget).
pub async fn fail_enrich_task_job(
    pool: &PgPool,
    job: &crate::models::Job,
    error: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    job_queue::fail_job_terminal(
        &mut *tx,
        job.id,
        Some(serde_json::json!({ "error": error })),
    )
    .await?;
    finalize_failed_task_job_tx(&mut tx, job, error).await?;
    tx.commit().await?;
    Ok(())
}

/// Taak-flow falen met retry-semantiek: `fail_job` bepaalt pending-met-backoff
/// (attempts over) versus definitief Failed. Pas bij definitief falen krijgt
/// de aanvrager de `job_failed`-taak en gaan de input-blobs weg — eerder niet,
/// want een retry her-materialiseert uit die blobs. De fail-markering en de
/// (eventuele) finalize zitten in dezelfde transactie, zodat een fout tussen
/// beide de fail-markering meeneemt in de rollback — de job blijft dan
/// 'processing' voor de reaper, in plaats van definitief gefaald zonder taak.
pub async fn fail_enrich_task_job_with_retry(
    pool: &PgPool,
    job: &crate::models::Job,
    error: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let failed_job = job_queue::fail_job(
        &mut *tx,
        job.id,
        Some(serde_json::json!({ "error": error })),
    )
    .await?;
    if failed_job.status == crate::models::JobStatus::Failed {
        finalize_failed_task_job_tx(&mut tx, job, error).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Of de taak-flow na deze run klaar is met de wet, of er een vervolgjob moet
/// komen.
///
/// Zonder vertaalstap (`ENRICH_STEPS` zonder `window`) schuift de cursor niet
/// op; een vervolgjob zou hetzelfde venster dan eindeloos herhalen.
fn task_walk_complete(law_complete: bool, steps: crate::enrich::RunSteps) -> bool {
    law_complete || !steps.window
}

/// Taak-flow-verwerking van een enrich-job: werkdirectory uit input-blobs,
/// enrichment draaien, resultaat als blobs + taak terugschrijven. Raakt
/// bewust geen law_entries, untranslatables of vervolg-harvests aan: dit is
/// een traject-persoonlijke enrichment, geen corpus-brede.
async fn process_enrich_task_job(
    pool: &PgPool,
    job: &crate::models::Job,
    payload: &EnrichPayload,
    enrich_config: &EnrichConfig,
    job_timeout: Duration,
) -> Result<JobOutcome> {
    let effective_config = match &payload.provider {
        Some(provider_name) => enrich_config.with_provider_override(provider_name),
        None => enrich_config.clone(),
    };
    tracing::info!(
        job_id = %job.id,
        law_id = %job.law_id,
        attempt = job.attempts,
        provider = %effective_config.provider.name(),
        "processing enrich job (task flow)"
    );
    let workdir = match materialize_task_workdir(pool, job.id).await {
        Ok(dir) => dir,
        Err(e) => {
            let msg = e.to_string();
            if is_missing_input_blob_error(&msg) {
                // Geen input-blobs = deterministisch kapot: een retry vindt
                // hetzelfde niets. Terminaal falen met taak.
                fail_enrich_task_job(pool, job, &msg).await?;
            } else {
                // DB/IO-fout tijdens materialiseren is transient: retryable.
                fail_enrich_task_job_with_retry(pool, job, &msg).await?;
            }
            return Ok(JobOutcome::Processed);
        }
    };

    // Ensure skill files are available in the workdir so the LLM can read
    // them. In the container the skills are baked into /opt/skills/; this
    // symlinks them into the per-job workdir, mirroring the corpus-path setup
    // below (`ensure_skills` in `process_next_enrich_job`).
    if let Err(e) = crate::enrich::ensure_skills(workdir.path()).await {
        tracing::warn!(error = %e, "failed to set up skill symlinks");
    }

    let mut bounded_config = effective_config.clone();
    bound_llm_timeout(&mut bounded_config, job_timeout);
    // De taak-flow loopt in vensters, net als het corpus-pad: een grote wet in
    // één sessie haalt de timeout per agent-call niet (de Kieswet, 593
    // artikelen). De cursor reist mee in de `.enrichment.yaml` die
    // `finish_enrich_task_job` als input-blob aan de vervolgjob doorgeeft.

    let outcome = tokio::time::timeout(
        job_timeout,
        execute_enrich(payload, workdir.path(), &bounded_config, ""),
    )
    .await;

    match outcome {
        Err(_elapsed) => {
            // Time-out: retryable. De uur-cap begrenst LLM-spend en
            // max_attempts=3 begrenst de schade van herhaalde pogingen.
            let msg = format!("verrijking time-out na {}s", job_timeout.as_secs());
            fail_enrich_task_job_with_retry(pool, job, &msg).await?;
            Ok(JobOutcome::Processed)
        }
        Ok(Err(e)) => {
            let err_str = e.to_string();
            if is_deterministic_content_failure(&err_str) {
                // Zelfde input reproduceert dezelfde fout: terminaal, spiegelt
                // het corpus-pad (`is_deterministic_content_failure` daar).
                fail_enrich_task_job(pool, job, &err_str).await?;
                Ok(JobOutcome::Processed)
            } else {
                // Resource-exhaustion en overige fouten zijn retryable. Geef
                // hetzelfde JobOutcome terug als het corpus-pad zodat de
                // fork-exhaustion-circuit-breaker van de worker meetelt.
                fail_enrich_task_job_with_retry(pool, job, &err_str).await?;
                Ok(outcome_for_error(&err_str))
            }
        }
        Ok(Ok((result, written_files))) => {
            // De run meldde succes, maar zonder wet-YAML valt er niets te
            // beoordelen. Een review-taak aanmaken zou de aanvrager een
            // onmogelijke taak geven; met retry-semantiek krijgt hij in plaats
            // daarvan een nieuwe poging, en pas na de laatste een
            // `job_failed`-taak met deze reden erin.
            if !produced_reviewable_law(workdir.path(), &written_files, &payload.yaml_path) {
                tracing::warn!(
                    job_id = %job.id,
                    law_id = %job.law_id,
                    files = written_files.len(),
                    "enrich-run zonder beoordeelbare wet-YAML"
                );
                fail_enrich_task_job_with_retry(pool, job, NO_REVIEWABLE_LAW).await?;
                return Ok(JobOutcome::Processed);
            }
            let result_json = match serde_json::to_value(&result) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!(error = %e, job_id = %job.id, "failed to serialize enrich result");
                    None
                }
            };
            let law_complete = task_walk_complete(result.law_complete, bounded_config.steps);
            match finish_enrich_task_job(
                pool,
                job,
                workdir.path(),
                &written_files,
                law_complete,
                result_json,
            )
            .await
            {
                Ok(Some(continuation)) => {
                    tracing::info!(
                        job_id = %job.id,
                        continuation_job_id = %continuation.id,
                        law_id = %job.law_id,
                        enrich_cursor = result.enrich_cursor,
                        "wet nog niet af na dit venster; vervolgjob aangemaakt"
                    );
                }
                Ok(None) => {}
                Err(e) => {
                    // Een DB-hik bij het wegschrijven, of een voorstel dat niet per
                    // artikel te vergelijken is; de fout noemt de oorzaak. Retry
                    // zolang er pogingen over zijn.
                    tracing::error!(job_id = %job.id, error = %e, "taak-resultaat afronden mislukt");
                    fail_enrich_task_job_with_retry(pool, job, &e.to_string()).await?;
                }
            }
            Ok(JobOutcome::Processed)
        }
    }
}

/// Finish a successful (non-task-flow) enrich run atomically.
///
/// One transaction: complete the job, mirror the captured untranslatables,
/// and then either
/// - `law_complete == true` → mark the law `enriched` (the pre-chunking
///   behavior); or
/// - `law_complete == false` (mid-chunk-loop) → leave the law status alone
///   (it stays `enriching`) and create the continuation enrich job — same
///   provider, priority, depth and traject scope, bare payload — in the SAME
///   transaction. Creating it after `complete_job` inside one tx means
///   `idx_unique_active_enrich_job` no longer sees the completed job as
///   active, and either both the completion and the continuation commit or
///   neither does: there is never a law left `enriching` without an
///   active/pending job.
///
/// Returns the continuation job when one was created. An `Ok(None)` conflict
/// (another active job in the same scope already exists — e.g. a manually
/// requested re-enrich) is logged and accepted: that job will carry the loop
/// forward.
///
/// `pub` so the DB-integration tests can drive the completion/continuation
/// contract against a real Postgres without spawning an LLM.
pub async fn complete_enrich_success_tx(
    pool: &PgPool,
    job: &crate::models::Job,
    payload: &EnrichPayload,
    result: &EnrichResult,
    result_json: Option<serde_json::Value>,
) -> Result<Option<crate::models::Job>> {
    let mut tx = pool.begin().await?;
    job_queue::complete_job(&mut *tx, job.id, result_json).await?;
    // Mirror the captured flags into their tables so they surface in the
    // harvester UI. Atomic with the completion: delete-and-replace per
    // (law_id, provider).
    //
    // Both channels are written on every run, and which one carries anything
    // follows from the law's schema version: a law on v0.5.x yields
    // untranslatables and no markings, one on v0.7.0 the reverse. Writing both
    // unconditionally is what makes a migration safe in either direction,
    // because each call clears its own table for this (law, provider) before
    // inserting. Skipping the empty one would leave the other channel's stale
    // rows standing after a law changed schema version.
    crate::untranslatables::replace_untranslatables(
        &mut tx,
        &result.law_id,
        &result.provider,
        job.id,
        &result.untranslatables,
    )
    .await?;
    crate::markings::replace_markings(
        &mut tx,
        &result.law_id,
        &result.provider,
        job.id,
        &result.markings,
    )
    .await?;

    let continuation = if result.law_complete {
        law_status::update_status(&mut *tx, &job.law_id, LawStatusValue::Enriched).await?;
        None
    } else {
        // Bare continuation payload: the chunk window is recomputed from the
        // cursor on the enrich branch at claim time, so it does NOT ride in
        // the queue payload (`chunk_articles`/`skip_mvt` stay transport-only).
        let continuation_payload = EnrichPayload {
            pass: Default::default(),
            law_id: payload.law_id.clone(),
            yaml_path: payload.yaml_path.clone(),
            provider: Some(result.provider.clone()),
            depth: payload.depth,
            requested_by: None,
            deliver: None,
            traject_id: None,
            traject_ref: payload.traject_ref.clone(),
            source_etag: None,
            new_law: None,
            chunk_articles: None,
            skip_mvt: None,
            // The continuation is a new window and opens its own session.
            session: None,
        };
        let continuation_json = serde_json::to_value(&continuation_payload).map_err(|e| {
            PipelineError::Enrich(format!("serialize continuation enrich payload: {e}"))
        })?;
        let mut req = CreateJobRequest::new(JobType::Enrich, &job.law_id)
            .with_priority(Priority::new(job.priority))
            .with_payload(continuation_json);
        // Inherit the traject scope from the payload (the `Job` model does not
        // carry the `traject_ref` column), so the continuation lands in the
        // same uniqueness scope of `idx_unique_active_enrich_job`.
        if let Some(ref traject_ref) = payload.traject_ref {
            req = req.with_traject_ref(traject_ref.clone());
        }
        let created = job_queue::create_enrich_job_if_not_exists(&mut *tx, req).await?;
        if created.is_none() {
            // Another active enrich job for this law/provider/scope already
            // exists (e.g. a manual re-enrich request); it will pick the
            // cursor up from the branch, so the loop still progresses.
            tracing::info!(
                law_id = %job.law_id,
                provider = %result.provider,
                "continuation enrich job skipped: active job already exists"
            );
        }
        created
    };

    tx.commit().await?;
    Ok(continuation)
}

/// Shrink the per-call LLM timeouts so a whole run fits the job budget.
///
/// A run is not one agent call. Translation, the feedback rounds of every
/// gate, the closing pass and the final gates all count, as many as
/// `ENRICH_FEEDBACK_ROUNDS` allows ([`FeedbackRounds::max_calls`]), and the
/// job timeout covers all of them at once, so each call gets a share of the
/// budget (minus a reserve for commit and cleanup). Lowering a ceiling is
/// right here: a call that would overrun the job is a call whose result is
/// thrown away.
///
/// The shares are not equal. The translation reads the window and writes
/// every `machine_readable` in it; a feedback round answers a list of
/// findings. Splitting evenly gave both about 550 s under the production
/// budget, and a 15-article window of the Kieswet did not translate in that.
/// The translation weighs [`TRANSLATE_SHARE_WEIGHT`] shares, each other call
/// one.
///
/// [`FeedbackRounds::max_calls`]: crate::enrich::FeedbackRounds::max_calls
fn bound_llm_timeout(config: &mut crate::enrich::EnrichConfig, job_timeout: Duration) {
    let reserve = Duration::from_secs(30);
    let budget = job_timeout.saturating_sub(reserve);
    let feedback_calls = config.feedback_rounds.max_calls();
    let unit = budget / TRANSLATE_SHARE_WEIGHT.saturating_add(feedback_calls);
    let translate_share = unit * TRANSLATE_SHARE_WEIGHT;
    config.timeout = config.timeout.min(translate_share);
    config.feedback_timeout = config.feedback_timeout.min(unit);
    tracing::info!(
        job_timeout = ?job_timeout,
        feedback_calls,
        translate_timeout = ?config.timeout,
        feedback_timeout = ?config.feedback_timeout,
        "LLM ceilings sized to fit a full chain in the job budget"
    );
}

/// How many per-call shares of the job budget the translation call gets; every
/// other call of a run gets one. See [`bound_llm_timeout`].
const TRANSLATE_SHARE_WEIGHT: u32 = 3;

/// Process the next available enrich job.
///
/// Returns the [`JobOutcome`]: `Processed` when a job was handled, `Idle` when
/// none was available, or `ResourceExhausted` when the job failed because the
/// container could not spawn processes/threads (fork()/EAGAIN).
///
/// Each enrichment creates a separate branch (`enrich/{provider}`)
/// so results can be reviewed before merging. A dedicated `CorpusClient` is
/// created per job pointing at the enrichment branch.
#[allow(clippy::too_many_arguments)]
async fn process_next_enrich_job(
    pool: &PgPool,
    repo_path: &Path,
    enrich_config: &EnrichConfig,
    corpus_config: Option<&CorpusConfig>,
    job_timeout: Duration,
    exhausted_threshold: i32,
    http_client: &Client,
    related_harvest_max_depth: u32,
) -> Result<JobOutcome> {
    let job = match job_queue::claim_job(pool, Some(JobType::Enrich)).await? {
        Some(job) => job,
        None => return Ok(JobOutcome::Idle),
    };
    // Tells the reaper this job is still being worked on, until it is done.
    let _heartbeat = job_queue::JobHeartbeat::start(pool.clone(), job.id, job_timeout);

    let payload: EnrichPayload = match &job.payload {
        Some(p) => match serde_json::from_value(p.clone()) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "invalid enrich payload");
                let error_json =
                    serde_json::json!({ "error": format!("invalid enrich payload: {e}") });
                if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                }
                return Ok(JobOutcome::Processed);
            }
        },
        None => {
            tracing::error!(job_id = %job.id, "enrich job has no payload");
            let error_json = serde_json::json!({ "error": "enrich job requires a payload" });
            if let Err(fail_err) = job_queue::fail_job(pool, job.id, Some(error_json)).await {
                tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
            }
            return Ok(JobOutcome::Processed);
        }
    };

    if payload.deliver_as_task() {
        // Ongeldige taak-flow-payload (geen aanvrager/traject): er is geen
        // geldige assignee, dus een taak zou permanent onzichtbaar zijn.
        // Terminaal falen zónder taak; de blob-GC ruimt de input op.
        if payload.requested_by.is_none() || payload.traject_id.is_none() {
            tracing::error!(job_id = %job.id, law_id = %job.law_id, "taak-flow-payload zonder requested_by/traject_id");
            job_queue::fail_job_terminal(
                pool,
                job.id,
                Some(serde_json::json!({ "error": "taak-flow-payload zonder requested_by/traject_id" })),
            )
            .await?;
            return Ok(JobOutcome::Processed);
        }
        return process_enrich_task_job(pool, &job, &payload, enrich_config, job_timeout).await;
    }

    // Contract-borging (worker/traject-contract, zie de crate-doc): vanaf hier
    // loopt het corpus-brede pad, dat met het centrale corpus-token naar de
    // centrale corpus-repo pusht. Een payload met een traject-doel die niet
    // via de taak-flow hierboven is afgebogen, is een enqueue-fout — terminaal
    // falen in plaats van met het server-token door te pushen.
    if let Err(e) = payload.require_corpus_wide_target() {
        let msg = e.to_string();
        tracing::error!(job_id = %job.id, law_id = %job.law_id, error = %msg);
        job_queue::fail_job_terminal(pool, job.id, Some(serde_json::json!({ "error": msg })))
            .await?;
        return Ok(JobOutcome::Processed);
    }

    // Override the provider if the payload specifies one
    let effective_config = match &payload.provider {
        Some(provider_name) => enrich_config.with_provider_override(provider_name),
        None => enrich_config.clone(),
    };

    tracing::info!(
        job_id = %job.id,
        law_id = %job.law_id,
        attempt = job.attempts,
        provider = %effective_config.provider.name(),
        "processing enrich job"
    );

    // Atomically transition to Enriching only if not already Enriched or EnrichExhausted.
    if let Err(e) = sqlx::query(
        "UPDATE law_entries SET status = 'enriching'::law_status, updated_at = now() \
         WHERE law_id = $1 AND status NOT IN ('enriched', 'enrich_exhausted')",
    )
    .bind(&job.law_id)
    .execute(pool)
    .await
    {
        tracing::warn!(error = %e, law_id = %job.law_id, "failed to set status to enriching");
    }

    // Create a branch-specific corpus client for this enrichment.
    // Pass the job ID to get a unique checkout directory per worker.
    let branch = enrich_branch_name(effective_config.provider.name());
    let enrich_corpus = if let Some(base_config) = corpus_config {
        match create_enrich_corpus(base_config, &branch, job.id, &payload.yaml_path).await {
            Ok(enrich_corpus) => {
                tracing::info!(branch = %branch, "created enrichment branch corpus");
                Some(enrich_corpus)
            }
            Err(e @ PipelineError::BaseDrift { .. }) => {
                // A previously-enriched law's base moved. Do NOT enrich on a
                // stale base and do NOT overwrite the existing enrichment —
                // fail the job loudly for human review / re-enrich.
                tracing::error!(error = %e, law_id = %job.law_id, branch = %branch, "base drift detected; failing enrich job");
                let error_json =
                    serde_json::json!({ "error": e.to_string(), "kind": "base_drift" });
                // Terminal-fail (not fail_job): base drift is deterministic
                // against the same base, so it must NOT re-enter the job-level
                // retry loop. fail_job_terminal marks the job Failed in one shot
                // instead of bouncing it back to 'pending' up to max_attempts —
                // which would deterministically re-fail against the same base
                // and flip-flop the law status Enriching -> Harvested on each
                // non-final attempt before finally landing on Failed.
                match job_queue::fail_job_terminal(pool, job.id, Some(error_json)).await {
                    Ok(_failed_job) => {
                        if let Err(se) = sqlx::query(
                            "UPDATE law_entries SET status = 'enrich_failed'::law_status, updated_at = now() \
                             WHERE law_id = $1 AND status NOT IN ('enriched', 'enrich_exhausted')",
                        )
                        .bind(&job.law_id)
                        .execute(pool)
                        .await
                        {
                            tracing::warn!(error = %se, law_id = %job.law_id, "failed to update law status to enrich_failed");
                        }
                        // Deliberately NOT calling handle_enrich_exhausted_or_retry here,
                        // and the job was terminal-failed above: unlike the other enrich
                        // failures (timeout, commit failure, enrich error), base drift is
                        // part of neither the job-level retry loop nor the law-level
                        // exhaust loop. The base is unchanged-but-stale relative to the
                        // recorded provenance, so any retry would just re-fail against the
                        // same base. Drift requires a human to review and re-enrich.
                    }
                    Err(fe) => {
                        tracing::error!(error = %fe, "failed to mark base-drift enrich job as failed")
                    }
                }
                return Ok(JobOutcome::Processed);
            }
            Err(e) => {
                tracing::warn!(error = %e, branch = %branch, "failed to create enrichment branch corpus, proceeding without");
                None
            }
        }
    } else {
        None
    };

    // Use the enrichment branch repo if available, otherwise the base repo
    let effective_repo = enrich_corpus
        .as_ref()
        .map(|c| c.client.repo_path().to_path_buf())
        .unwrap_or_else(|| repo_path.to_path_buf());

    // Ensure skill files are available in the repo checkout so the LLM can
    // read them. In the container the skills are baked into /opt/skills/;
    // this symlinks them into the per-job checkout.
    if let Err(e) = crate::enrich::ensure_skills(&effective_repo).await {
        tracing::warn!(error = %e, "failed to set up skill symlinks");
    }

    // Capture the per-job checkout path for cleanup after the job completes.
    let checkout_path = enrich_corpus
        .as_ref()
        .map(|c| c.client.repo_path().to_path_buf());

    // Compute the progress file path and spawn a background polling task.
    // The LLM writes phase info to this file; we relay it to the DB every 10s.
    let normalized_yaml_path = crate::enrich::normalize_yaml_path(&payload.yaml_path).ok();
    let progress_path = normalized_yaml_path
        .as_ref()
        .map(|p| progress_file_path(&effective_repo.join(p)));

    let cancel_token = tokio_util::sync::CancellationToken::new();
    let poll_handle = if let Some(ref ppath) = progress_path {
        let token = cancel_token.clone();
        let pool = pool.clone();
        let job_id = job.id;
        let ppath = ppath.clone();
        Some(tokio::spawn(async move {
            poll_progress_file(&pool, job_id, &ppath, token).await;
        }))
    } else {
        None
    };

    // Ensure the LLM's internal timeout fires before the outer job timeout
    // so ProcessLlmRunner can kill the child process cleanly. Without this,
    // if LLM_TIMEOUT_SECS > WORKER_JOB_TIMEOUT_SECS, the outer timeout would
    // drop the future while the OS subprocess keeps running.
    let mut bounded_config = effective_config.clone();
    bound_llm_timeout(&mut bounded_config, job_timeout);

    let source_hash = enrich_corpus
        .as_ref()
        .map(|c| c.source_hash.clone())
        .unwrap_or_default();

    let enrich_outcome = tokio::time::timeout(
        job_timeout,
        execute_enrich(&payload, &effective_repo, &bounded_config, &source_hash),
    )
    .await;

    let job_result = match enrich_outcome {
        Err(_elapsed) => {
            // Job timed out
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                timeout = ?job_timeout,
                "enrich job timed out"
            );

            let error_json = serde_json::json!({
                "error": format!("job timed out after {}s", job_timeout.as_secs())
            });
            match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                Ok(failed_job) => {
                    if failed_job.status == crate::models::JobStatus::Failed {
                        // Set EnrichFailed only if not already Enriched or EnrichExhausted.
                        if let Err(e) = law_status::mark_enrich_failed(pool, &job.law_id).await {
                            tracing::warn!(error = %e, law_id = %job.law_id, "failed to update law status to enrich_failed");
                        }

                        handle_enrich_exhausted_or_retry(
                            pool,
                            &job.law_id,
                            &payload,
                            job.priority,
                            exhausted_threshold,
                        )
                        .await;
                    } else if let Err(e) = law_status::update_status_if(
                        pool,
                        &job.law_id,
                        LawStatusValue::Enriching,
                        LawStatusValue::Harvested,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset law status to harvested");
                    }
                }
                Err(fail_err) => {
                    tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark timed-out job as failed");
                }
            }

            Ok(JobOutcome::Processed)
        }
        Ok(Ok((result, written_files))) => {
            tracing::info!(
                job_id = %job.id,
                articles_total = result.articles_total,
                articles_with_machine_readable = result.articles_with_machine_readable,
                coverage_score = result.coverage_score,
                law_complete = result.law_complete,
                enrich_cursor = result.enrich_cursor,
                provider = %result.provider,
                branch = %result.branch,
                "enrichment completed successfully"
            );

            // Push to corpus, complete the job in DB, and update law status.
            // If any of these fail, mark the job as failed so it gets retried
            // instead of orphaning it in 'processing' state for 30 minutes.
            let commit_result: std::result::Result<Option<crate::models::Job>, PipelineError> =
                async {
                    if let Some(ref corpus) = enrich_corpus {
                        let message = format!(
                            "enrich({}): {} ({})",
                            result.provider, result.law_id, result.yaml_path
                        );
                        corpus
                            .client
                            .commit_and_push(&written_files, &message)
                            .await
                            .map_err(|e| {
                                PipelineError::Enrich(format!("corpus push failed: {e}"))
                            })?;
                    }

                    let result_json = match serde_json::to_value(&result) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            tracing::warn!(error = %e, job_id = %job.id, "failed to serialize enrich result");
                            None
                        }
                    };

                    complete_enrich_success_tx(pool, &job, &payload, &result, result_json).await
                }
                .await;

            match commit_result {
                Err(e) => {
                    let outcome = outcome_for_error(&e.to_string());
                    tracing::error!(
                        job_id = %job.id,
                        error = %e,
                        "post-enrichment commit failed, marking job as failed for retry"
                    );
                    let error_json = serde_json::json!({ "error": e.to_string() });
                    match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                        Ok(failed_job) if failed_job.status == crate::models::JobStatus::Failed => {
                            // Set EnrichFailed only if not already Enriched or EnrichExhausted.
                            if let Err(e) = law_status::mark_enrich_failed(pool, &job.law_id).await
                            {
                                tracing::warn!(error = %e, law_id = %job.law_id, "failed to update law status to enrich_failed");
                            }

                            handle_enrich_exhausted_or_retry(
                                pool,
                                &job.law_id,
                                &payload,
                                job.priority,
                                exhausted_threshold,
                            )
                            .await;
                        }
                        Ok(_) => {
                            if let Err(e) = law_status::update_status_if(
                                pool,
                                &job.law_id,
                                LawStatusValue::Enriching,
                                LawStatusValue::Harvested,
                            )
                            .await
                            {
                                tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset law status to harvested");
                            }
                        }
                        Err(fail_err) => {
                            tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                        }
                    }
                    Ok(outcome)
                }
                Ok(continuation) => {
                    if let Some(ref cont) = continuation {
                        tracing::info!(
                            job_id = %job.id,
                            continuation_job_id = %cont.id,
                            law_id = %job.law_id,
                            enrich_cursor = result.enrich_cursor,
                            "law not complete after chunk; continuation enrich job created"
                        );
                    }

                    // A successful chunk resets the fail counter: only
                    // *consecutive* failed runs count toward exhaustion.
                    if let Err(e) =
                        law_status::reset_fail_count(pool, &job.law_id, JobType::Enrich).await
                    {
                        tracing::warn!(error = %e, law_id = %job.law_id, "failed to reset enrich fail count after success");
                    }

                    // Set coverage score outside the transaction (non-critical).
                    // With dual providers, whichever finishes last writes the score.
                    // The stored score is the CUMULATIVE fraction of articles with
                    // machine_readable, not the per-run delta — mid-chunk-loop the
                    // delta would misrepresent a partially processed law. The
                    // per-run delta stays visible in the job's `EnrichResult`.
                    let cumulative_coverage = if result.articles_total > 0 {
                        result.articles_with_machine_readable as f64 / result.articles_total as f64
                    } else {
                        0.0
                    };
                    if let Err(e) =
                        law_status::set_coverage_score(pool, &job.law_id, cumulative_coverage).await
                    {
                        tracing::warn!(error = %e, provider = %result.provider, "failed to set coverage score");
                    } else {
                        tracing::info!(
                            law_id = %job.law_id,
                            provider = %result.provider,
                            coverage_score = cumulative_coverage,
                            "coverage score updated"
                        );
                    }

                    // Enqueue follow-up harvests for the related legislation the
                    // enrichment agent declared (delegated regelingen, cross-law
                    // sources, legal bases the extref-only harvester misses).
                    // Always on, but depth-capped so the recursion is bounded (and
                    // the LLM-costly re-enrichment of those harvests is separately
                    // gated by ENRICH_AUTO_ENQUEUE + ENRICH_HOURLY_LIMIT).
                    let enrich_depth = payload.depth.unwrap_or(0);
                    if enrich_depth < related_harvest_max_depth {
                        harvest_related_legislation(
                            pool,
                            http_client,
                            &job.law_id,
                            &result.related_legislation,
                            enrich_depth,
                        )
                        .await;
                    } else if !result.related_legislation.is_empty() {
                        tracing::info!(
                            law_id = %job.law_id,
                            depth = enrich_depth,
                            max_depth = related_harvest_max_depth,
                            related = result.related_legislation.len(),
                            "skipping related-legislation harvest: max depth reached"
                        );
                    }

                    Ok(JobOutcome::Processed)
                }
            }
        }
        Ok(Err(e)) => {
            let err_str = e.to_string();
            let outcome = outcome_for_error(&err_str);
            tracing::error!(
                job_id = %job.id,
                law_id = %job.law_id,
                error = %e,
                "enrichment failed"
            );

            let error_json = serde_json::json!({ "error": &err_str });

            if is_deterministic_content_failure(&err_str) {
                // Deterministic content failure (LLM produced no machine_readable
                // sections, or its output failed to parse/validate). Re-running the
                // same law with the same provider reproduces it, so the normal
                // retry ladder (3 inner attempts × up to EXHAUSTED_THRESHOLD
                // recreated jobs) only wastes LLM budget and blocks the serial
                // queue behind ~30 doomed runs. Fail terminally and exhaust the law
                // in one step. The exhaust is guarded (enrich_failed → enrich_exhausted
                // only), so a provider that already enriched this law keeps 'enriched'.
                // Terminal-fail the job and transition the law atomically: if any
                // step errors, the transaction rolls back and the job is left in
                // 'processing' for the reaper to reclaim and retry (self-healing
                // once the DB recovers) — never a law stranded in 'enriching' with
                // a failed job and no follow-up. `mark_enrich_failed` is guarded
                // (NOT IN enriched/enrich_exhausted), so 0 rows means another
                // provider already reached a terminal state — keep it, skip the
                // exhaust.
                let fast_fail = async {
                    let mut tx = pool.begin().await?;
                    job_queue::fail_job_terminal(&mut *tx, job.id, Some(error_json)).await?;
                    let rows = law_status::mark_enrich_failed(&mut *tx, &job.law_id).await?;
                    if rows > 0 {
                        law_status::exhaust_law(&mut *tx, &job.law_id, JobType::Enrich).await?;
                    }
                    tx.commit().await?;
                    Ok::<u64, PipelineError>(rows)
                }
                .await;
                match fast_fail {
                    Ok(0) => tracing::warn!(
                        job_id = %job.id,
                        law_id = %job.law_id,
                        "deterministic content failure — job failed without retry; law already in a terminal state, status kept"
                    ),
                    Ok(_) => tracing::warn!(
                        job_id = %job.id,
                        law_id = %job.law_id,
                        "deterministic content failure — marked enrich_exhausted without retry"
                    ),
                    Err(e) => tracing::error!(
                        job_id = %job.id,
                        law_id = %job.law_id,
                        error = %e,
                        "fast-fail transaction rolled back; job left in 'processing' for the reaper to retry"
                    ),
                }
            } else {
                match job_queue::fail_job(pool, job.id, Some(error_json)).await {
                    Ok(failed_job) => {
                        if failed_job.status == crate::models::JobStatus::Failed {
                            // Set EnrichFailed only if not already Enriched or EnrichExhausted.
                            if let Err(status_err) =
                                law_status::mark_enrich_failed(pool, &job.law_id).await
                            {
                                tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to set status to enrich_failed");
                            }

                            handle_enrich_exhausted_or_retry(
                                pool,
                                &job.law_id,
                                &payload,
                                job.priority,
                                exhausted_threshold,
                            )
                            .await;
                        } else {
                            // Job will be retried — atomically reset to Harvested only if
                            // status is currently Enriching. Cannot regress from Enriched.
                            if let Err(status_err) = law_status::update_status_if(
                                pool,
                                &job.law_id,
                                LawStatusValue::Enriching,
                                LawStatusValue::Harvested,
                            )
                            .await
                            {
                                tracing::warn!(error = %status_err, law_id = %job.law_id, "failed to reset status to harvested for retry");
                            }
                        }
                    }
                    Err(fail_err) => {
                        tracing::error!(job_id = %job.id, error = %fail_err, "failed to mark job as failed");
                    }
                }
            }

            Ok(outcome)
        }
    };

    // Stop the progress polling task and clean up the progress file.
    cancel_token.cancel();
    if let Some(handle) = poll_handle {
        let _ = handle.await;
    }
    if let Some(ref ppath) = progress_path {
        let _ = tokio::fs::remove_file(ppath).await;
    }

    // Clean up the per-job corpus checkout directory (regardless of outcome).
    // Each enrich job creates a full git clone; without cleanup these accumulate.
    if let Some(path) = checkout_path {
        if let Err(e) = tokio::fs::remove_dir_all(&path).await {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to clean up per-job corpus checkout"
            );
        } else {
            tracing::debug!(path = %path.display(), "cleaned up per-job corpus checkout");
        }
    }

    job_result
}

/// Poll the progress file written by the LLM and relay its contents to the DB.
///
/// Runs until the cancellation token is cancelled. Reads the file every 10
/// seconds; parse errors are silently ignored (the file may be half-written).
async fn poll_progress_file(
    pool: &PgPool,
    job_id: uuid::Uuid,
    path: &Path,
    cancel: tokio_util::sync::CancellationToken,
) {
    let interval = Duration::from_secs(10);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(interval) => {}
        }

        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => continue, // file doesn't exist yet
        };

        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue, // half-written or invalid JSON
        };

        if let Err(e) = job_queue::update_progress(pool, job_id, value).await {
            tracing::warn!(job_id = %job_id, error = %e, "failed to update job progress");
        }
    }

    // Final read to capture the last phase the LLM wrote before the job finished.
    if let Ok(content) = tokio::fs::read_to_string(path).await {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            let _ = job_queue::update_progress(pool, job_id, value).await;
        }
    }
}

/// Execute the harvest and write results to the output directory.
///
/// When a corpus client is provided, the written files are committed and pushed
/// to the corpus repository.
///
/// # At-least-once semantics
///
/// The corpus push happens before the DB transaction that marks the job as
/// completed. If the process crashes after a successful push but before the
/// DB commit, the job will be retried on restart. This is safe because the
/// commit is content-gated: re-harvesting unchanged law content produces no
/// new commit (`commit_and_push_content` returns `false`), so retries are
/// idempotent even though the `status.yaml` timestamp churns every run.
///
/// Sets `result.changed` to reflect whether a commit was actually made, so the
/// caller can skip follow-up work (enrich, referenced-law harvests) when the
/// re-harvest was a no-op.
async fn execute_harvest_job(
    output_dir: &Path,
    config: &WorkerConfig,
    payload: &HarvestPayload,
    corpus: Option<&CorpusClient>,
    http_client: &Client,
) -> Result<HarvestResult> {
    let (mut result, files) = execute_harvest(
        payload,
        output_dir,
        &config.regulation_output_base,
        http_client,
    )
    .await?;

    if let Some(corpus) = corpus {
        let message = format!("harvest: {} ({})", result.law_name, result.slug);
        // Only a change to the law YAML (content) counts as a real change; the
        // status.yaml timestamp (metadata) alone must not produce a new version.
        result.changed = corpus
            .commit_and_push_content(
                std::slice::from_ref(&files.content),
                std::slice::from_ref(&files.metadata),
                &message,
            )
            .await?;
    }

    Ok(result)
}

/// Nazorg voor door de reaper terminaal gefaalde (niet-taak-flow) enrich-jobs.
///
/// `reap_orphaned_jobs` zet een job op `failed` zonder `law_entries` aan te
/// raken; alleen het synchrone failure-pad in `process_next_enrich_job` deed
/// de statusovergang plus retry/exhausted-bookkeeping. Zonder deze nazorg
/// blijft een wet na een crash-reap voorgoed in `enriching` staan zonder
/// actieve of pending job — precies de toestand die de chunk-lus (D2) uitsluit
/// voor het succespad, en die de frontend sinds de poll-cap-exemptie voor
/// `enriching` niet meer met een (vals) timeout-signaal afdekt. Spiegel daarom
/// het synchrone pad: markeer de wet `enrich_failed` en laat
/// `handle_enrich_exhausted_or_retry` óf een retry-job met backoff plannen
/// (de lus hervat bij de cursor op de branch) óf de wet `enrich_exhausted`
/// maken. Taak-flow-jobs (`deliver=task`) volgen het bestaande
/// task-notificatiepad (`tasks::notify_reaped_task_jobs`) en raken
/// `law_entries` niet.
///
/// `pub` zodat de DB-integratietests dit contract zonder reaper-task kunnen
/// aanroepen.
pub async fn handle_reaped_enrich_jobs(
    pool: &PgPool,
    reaped: &[job_queue::ReapedJob],
    exhausted_threshold: i32,
) {
    for job in reaped {
        if job.job_type != JobType::Enrich || job.status != crate::models::JobStatus::Failed {
            continue;
        }
        let Some(payload_json) = job.payload.as_ref() else {
            continue;
        };
        let payload: EnrichPayload = match serde_json::from_value(payload_json.clone()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    job_id = %job.id,
                    law_id = %job.law_id,
                    error = %e,
                    "reaped enrich job has an unparseable payload; skipping law-status aftercare"
                );
                continue;
            }
        };
        if payload.deliver_as_task() {
            continue;
        }
        tracing::warn!(
            job_id = %job.id,
            law_id = %job.law_id,
            "enrich job reaped to terminal failure; running law-status aftercare"
        );
        // Set EnrichFailed only if not already Enriched or EnrichExhausted
        // (same guard as the synchronous failure path).
        if let Err(e) = law_status::mark_enrich_failed(pool, &job.law_id).await {
            tracing::warn!(error = %e, law_id = %job.law_id, "failed to update law status to enrich_failed");
        }
        handle_enrich_exhausted_or_retry(
            pool,
            &job.law_id,
            &payload,
            job.priority,
            exhausted_threshold,
        )
        .await;
    }
}

/// Increment the enrich fail count and either mark the law as exhausted
/// or schedule a new enrich job for retry.
async fn handle_enrich_exhausted_or_retry(
    pool: &PgPool,
    law_id: &str,
    payload: &EnrichPayload,
    priority: i32,
    exhausted_threshold: i32,
) {
    match law_status::increment_fail_count(pool, law_id, JobType::Enrich).await {
        Ok(count) if count >= exhausted_threshold => {
            if let Err(e) = law_status::exhaust_law(pool, law_id, JobType::Enrich).await {
                tracing::warn!(error = %e, law_id = %law_id, "failed to mark law as enrich_exhausted");
            }
        }
        Ok(count) => {
            // Not yet exhausted — queue a new enrich job so the
            // fail_count can accumulate toward the threshold. The job starts
            // with a backoff delay that grows with the law's fail count.
            let retry_delay = job_queue::retry_backoff(count);
            tracing::info!(
                law_id = %law_id,
                fail_count = count,
                threshold = exhausted_threshold,
                delay = ?retry_delay,
                "scheduling auto-retry enrich job"
            );
            match serde_json::to_value(payload) {
                Ok(payload_json) => {
                    let req = CreateJobRequest::new(JobType::Enrich, law_id)
                        .with_priority(Priority::new(priority))
                        .with_payload(payload_json)
                        .with_initial_delay(retry_delay);
                    match job_queue::create_enrich_job_if_not_exists(pool, req).await {
                        Ok(Some(new_job)) => {
                            tracing::info!(
                                new_job_id = %new_job.id,
                                law_id = %law_id,
                                "auto-retry enrich job created"
                            );
                        }
                        Ok(None) => {
                            tracing::debug!(
                                law_id = %law_id,
                                "auto-retry enrich job skipped: active job already exists"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                law_id = %law_id,
                                "failed to create auto-retry enrich job"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        law_id = %law_id,
                        "failed to serialize retry payload, skipping auto-retry enrich job"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, law_id = %law_id, "failed to increment enrich fail count");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::api::bwb_search::BwbSearchResult;

    fn hit(bwb_id: &str) -> BwbSearchResult {
        BwbSearchResult {
            bwb_id: bwb_id.to_string(),
            title: "Een wet".to_string(),
            law_type: "wet".to_string(),
        }
    }

    #[test]
    fn a_throttled_sru_lookup_is_not_a_law_that_does_not_exist() {
        assert!(matches!(
            classify_sru_result(Err(
                "BWB search returned HTTP 429 Too Many Requests".to_string()
            )),
            RelatedResolution::LookupFailed
        ));
    }

    #[test]
    fn zero_hits_stays_unresolved() {
        assert!(matches!(
            classify_sru_result(Ok(vec![])),
            RelatedResolution::Unresolved
        ));
    }

    #[test]
    fn one_well_formed_hit_resolves() {
        assert!(matches!(
            classify_sru_result(Ok(vec![hit("BWBR0018451")])),
            RelatedResolution::Resolved(id) if id == "BWBR0018451"
        ));
    }

    #[test]
    fn one_malformed_hit_does_not_resolve() {
        assert!(matches!(
            classify_sru_result(Ok(vec![hit("CVDR123456")])),
            RelatedResolution::Unresolved
        ));
    }

    #[test]
    fn several_hits_need_a_human() {
        assert!(matches!(
            classify_sru_result(Ok(vec![hit("BWBR0018451"), hit("BWBR0000001")])),
            RelatedResolution::NeedsConfirmation
        ));
    }

    use super::*;
    use std::path::PathBuf;

    #[test]
    fn hourly_cap_day_vs_night() {
        // Day hours [8, 24) use the base limit.
        assert_eq!(hourly_cap(4, 5, 8), 4, "08:00 is the first day hour");
        assert_eq!(hourly_cap(4, 5, 12), 4);
        assert_eq!(hourly_cap(4, 5, 23), 4);
        // Night hours [0, 8) get base * multiplier.
        assert_eq!(hourly_cap(4, 5, 0), 20, "midnight is a night hour");
        assert_eq!(hourly_cap(4, 5, 3), 20);
        assert_eq!(hourly_cap(4, 5, 7), 20, "07:00 is the last night hour");
        // Default multiplier 1 means no boost.
        assert_eq!(hourly_cap(4, 1, 3), 4);
        // Saturating: a huge multiplier can't overflow.
        assert_eq!(hourly_cap(u32::MAX, 5, 3), u32::MAX);
    }

    #[test]
    fn auto_enrich_priority_by_depth() {
        // Root/direct harvests keep the default priority.
        assert_eq!(
            auto_enrich_priority(None).value(),
            Priority::default().value()
        );
        assert_eq!(
            auto_enrich_priority(Some(0)).value(),
            Priority::default().value()
        );
        // Recursive follow-up harvests (depth >= 1) drop to the low priority so
        // they yield to directly/manually requested enrich work.
        assert_eq!(
            auto_enrich_priority(Some(1)).value(),
            RECURSIVE_ENRICH_PRIORITY
        );
        assert_eq!(
            auto_enrich_priority(Some(5)).value(),
            RECURSIVE_ENRICH_PRIORITY
        );
    }

    #[test]
    fn classifies_fork_eagain_errors_as_resource_exhaustion() {
        // Real strings observed from failed harvest jobs.
        assert!(is_resource_exhaustion(
            "corpus error: git command failed: git pull failed: error: cannot fork() for \
             merge-base: Resource temporarily unavailable"
        ));
        assert!(is_resource_exhaustion(
            "corpus error: IO error: Resource temporarily unavailable (os error 11)"
        ));
        assert!(is_resource_exhaustion(
            "task join error: task 5 panicked with message \"event loop thread panicked\""
        ));
        assert!(is_resource_exhaustion(
            "Cannot allocate memory (os error 12)"
        ));
        assert!(is_resource_exhaustion(
            "fatal: unable to create thread: Resource temporarily unavailable"
        ));
    }

    #[test]
    fn does_not_classify_per_law_failures_as_resource_exhaustion() {
        // These are real per-law failures that must still burn the retry budget.
        assert!(!is_resource_exhaustion(
            "harvester error: Missing required XML element: _latestItem attribute in manifest"
        ));
        assert!(!is_resource_exhaustion(
            "harvester error: CVDR SRU search failed for CVDR756485: No records found"
        ));
        assert!(!is_resource_exhaustion(
            "enrichment error: claude exited with exit status: 1"
        ));
        assert!(!is_resource_exhaustion(
            "enrichment error: opencode timed out after 600s"
        ));
        assert!(!is_resource_exhaustion(
            "YAML error: did not find expected key at line 5 column 3"
        ));
    }

    const SOURCE: &str = r#"
$id: wet_x
articles:
  - number: "1"
    text: Eerste artikel
  - number: "2"
    text: Tweede artikel
    machine_readable:
      execution:
        output: []
"#;

    #[test]
    fn reports_only_the_articles_that_differ() {
        let proposal = r#"
$id: wet_x
articles:
  - number: "1"
    text: Eerste artikel
  - number: "2"
    text: Tweede artikel
    machine_readable:
      execution:
        output:
          - name: bedrag
"#;
        assert_eq!(
            changed_articles(SOURCE, proposal),
            Some(vec!["2".to_string()])
        );
    }

    #[test]
    fn reports_nothing_when_the_proposal_matches_the_source() {
        assert_eq!(changed_articles(SOURCE, SOURCE), Some(Vec::new()));
    }

    #[test]
    fn counts_an_article_the_source_does_not_have_as_changed() {
        let proposal = r#"
$id: wet_x
articles:
  - number: "1"
    text: Eerste artikel
  - number: "2"
    text: Tweede artikel
    machine_readable:
      execution:
        output: []
  - number: "3"
    text: Nieuw artikel
"#;
        assert_eq!(
            changed_articles(SOURCE, proposal),
            Some(vec!["3".to_string()])
        );
    }

    // Een ongequote `number: 2` is geldige YAML en komt in de praktijk voor.
    // Vroeger viel zo'n artikel stil uit de diff (as_str() op een integer), dus
    // kreeg een wél gewijzigd artikel geen review-taak - en zonder parse-fout
    // sloeg de hele-wet-fallback ook niet aan: het voorstel verdween geruisloos.
    #[test]
    fn reports_a_changed_article_with_an_unquoted_number() {
        let source = r#"
$id: wet_x
articles:
  - number: 1
    text: Eerste artikel
  - number: 2
    text: Tweede artikel
"#;
        let proposal = r#"
$id: wet_x
articles:
  - number: 1
    text: Eerste artikel
  - number: 2
    text: Tweede artikel
    machine_readable:
      execution:
        output:
          - name: bedrag
"#;
        assert_eq!(
            changed_articles(source, proposal),
            Some(vec!["2".to_string()])
        );
        assert_eq!(changed_articles(source, source), Some(Vec::new()));
    }

    #[test]
    fn yields_none_on_unreadable_yaml_so_the_caller_falls_back() {
        // Geen paniek en geen lege lijst: "ik weet het niet" is iets anders dan
        // "er is niets veranderd", en de caller maakt er één taak van.
        assert_eq!(
            changed_articles(SOURCE, "dit is: [geen: geldige yaml"),
            None
        );
        assert_eq!(changed_articles("zonder articles-sleutel: 1", SOURCE), None);
    }

    #[test]
    fn accepts_a_run_that_wrote_the_law_yaml() {
        let workdir = Path::new("/tmp/wd");
        let files = vec![
            PathBuf::from("/tmp/wd/features/zorgtoeslag.feature"),
            PathBuf::from("/tmp/wd/laws/wet_op_de_zorgtoeslag/law.yaml"),
        ];
        assert!(produced_reviewable_law(
            workdir,
            &files,
            "laws/wet_op_de_zorgtoeslag/law.yaml"
        ));
    }

    #[test]
    fn rejects_a_run_that_only_wrote_feature_files() {
        // De worker staged features naast de wet. Alleen features betekent dat
        // er niets te beoordelen is, ook al meldde de run succes.
        let workdir = Path::new("/tmp/wd");
        let files = vec![
            PathBuf::from("/tmp/wd/features/zorgtoeslag.feature"),
            PathBuf::from("/tmp/wd/features/premie.feature"),
        ];
        assert!(!produced_reviewable_law(
            workdir,
            &files,
            "laws/wet_op_de_zorgtoeslag/law.yaml"
        ));
    }

    #[test]
    fn rejects_a_run_that_only_wrote_sidecars() {
        let workdir = Path::new("/tmp/wd");
        let files = vec![PathBuf::from("/tmp/wd/laws/x/.enrichment.yaml")];
        assert!(!produced_reviewable_law(workdir, &files, "laws/x/law.yaml"));
    }

    #[test]
    fn rejects_a_run_that_wrote_nothing() {
        assert!(!produced_reviewable_law(
            Path::new("/tmp/wd"),
            &[],
            "laws/x/law.yaml"
        ));
    }

    #[test]
    fn accepts_a_law_yaml_on_a_path_other_than_the_payload_one() {
        // Fallback, gelijk aan die van useTaskReview: schrijft het model de wet
        // ergens anders neer, dan is er nog steeds iets te beoordelen. Alleen
        // een run zonder énige wet-YAML faalt.
        let workdir = Path::new("/tmp/wd");
        let files = vec![PathBuf::from("/tmp/wd/regulation/nl/wet/anders.yaml")];
        assert!(produced_reviewable_law(workdir, &files, "laws/x/law.yaml"));
    }

    #[test]
    fn classifies_content_failures_as_deterministic() {
        // Real strings observed from failed enrich jobs (opencode) — retrying
        // these reproduces the same failure, so they must fail fast.
        assert!(is_deterministic_content_failure(
            "enrichment error: LLM produced no machine_readable sections (159 articles needed enrichment)"
        ));
        assert!(is_deterministic_content_failure(
            "YAML error: articles[4].machine_readable.untranslatables[0]: missing field `construct` at line 445 column 11"
        ));
        assert!(is_deterministic_content_failure(
            "YAML error: articles[9].machine_readable: invalid type: sequence, expected struct MachineReadable at line 395 column 7"
        ));
        assert!(is_deterministic_content_failure(
            "YAML error: did not find expected key at line 177 column 3"
        ));
    }

    #[test]
    fn does_not_classify_transient_failures_as_deterministic() {
        // Transient faults must stay retryable — a later attempt can succeed.
        assert!(!is_deterministic_content_failure(
            "reaped: job stuck in processing"
        ));
        assert!(!is_deterministic_content_failure(
            "job timed out after 600s"
        ));
        assert!(!is_deterministic_content_failure(
            "enrichment error: corpus push failed: git command failed: could not read from remote"
        ));
        assert!(!is_deterministic_content_failure(
            "enrichment error: claude exited with exit status: 1"
        ));
        assert!(!is_deterministic_content_failure(
            "IO error: Resource temporarily unavailable (os error 11)"
        ));
    }

    #[test]
    fn deterministic_markers_track_error_display_format() {
        // The classifier matches on error Display strings, so its markers are
        // coupled to PipelineError's `#[error(...)]` formats. Construct the real
        // errors and run them through the classifier: if a format string drifts
        // (e.g. "YAML error:" is renamed), this fails instead of silently
        // regressing content failures back to the 30-retry ladder.
        let yaml_err: PipelineError = serde_yaml_ng::from_str::<i32>("[1, 2]")
            .expect_err("deserializing a sequence into i32 must fail")
            .into();
        assert!(
            is_deterministic_content_failure(&yaml_err.to_string()),
            "PipelineError::Yaml Display must still match a classifier marker"
        );

        let enrich_err = PipelineError::Enrich(
            "LLM produced no machine_readable sections (3 articles needed enrichment)".to_string(),
        );
        assert!(
            is_deterministic_content_failure(&enrich_err.to_string()),
            "PipelineError::Enrich Display must preserve the message the classifier keys on"
        );
    }

    #[test]
    fn chunk_no_output_is_not_deterministic() {
        // A chunk that produced no output at all fails RETRYABLE: its message
        // must never match the deterministic-content classifier, or a healthy
        // large law could be terminally exhausted by one hiccupped chunk. Pin
        // the full formatted message, not just the marker, so a rewording that
        // accidentally introduces a classifier marker fails here.
        let chunk_err = PipelineError::Enrich(format!(
            "{}: no new machine_readable additions in the window, no chunk_report \
             referencing this window, no new untranslatables in the window \
             (articles 15..30 of 324)",
            crate::enrich::CHUNK_NO_OUTPUT_MARKER
        ));
        assert!(
            !is_deterministic_content_failure(&chunk_err.to_string()),
            "chunk no-output failures must stay retryable (never terminal)"
        );
    }

    #[test]
    fn handle_resource_exhaustion_increments_until_threshold() {
        // Below the threshold it must not exit (test would abort if it did).
        let mut counter = 0u32;
        handle_resource_exhaustion(&mut counter, 3, "test");
        assert_eq!(counter, 1);
        handle_resource_exhaustion(&mut counter, 3, "test");
        assert_eq!(counter, 2);
    }

    /// De uitputtings-breaker sluit de worker af, dus mag een gebruiker hem niet
    /// kunnen laten afgaan. De weigering-zonder-toestemming noemt de geüploade
    /// bestandsnaam; heet die `cannot fork.pdf`, dan matcht de tekst-classificatie
    /// een fork-marker. Het fouttype moet daar dwars doorheen gaan.
    #[test]
    fn document_convert_refusal_never_counts_as_resource_exhaustion() {
        let err = PipelineError::LlmNotPermitted(
            "Conversie van cannot fork.pdf gestopt: de omzetting zonder AI leverde geen \
             bruikbare tekst op. Er is geen taalmodel gebruikt."
                .to_string(),
        );
        let msg = err.to_string();
        // De tekst zou de breaker wél laten aanslaan — daarom classificeren we
        // deze mislukking op type.
        assert!(is_resource_exhaustion(&msg));
        assert_eq!(outcome_for_error(&msg), JobOutcome::ResourceExhausted);
        assert_eq!(
            document_convert_outcome(&err, &msg),
            JobOutcome::Processed,
            "een geweigerde conversie is een gewone mislukking, geen resource-uitputting"
        );
    }

    /// De tegenproef: echte fork/OOM-uitputting op deze zelfde job moet de
    /// breaker nog steeds voeden.
    #[test]
    fn document_convert_real_exhaustion_still_trips_the_breaker() {
        let err =
            PipelineError::Enrich("cannot fork: Resource temporarily unavailable".to_string());
        let msg = err.to_string();
        assert_eq!(
            document_convert_outcome(&err, &msg),
            JobOutcome::ResourceExhausted
        );
    }

    /// De uitleg gaat ongewijzigd naar de gebruiker (taak-payload, `jobs.result`),
    /// dus zonder `enrichment error:`-voorvoegsel voor iets dat niet verrijkt is.
    #[test]
    fn llm_not_permitted_displays_without_prefix() {
        let err = PipelineError::LlmNotPermitted("Conversie van brief.doc gestopt.".to_string());
        assert_eq!(err.to_string(), "Conversie van brief.doc gestopt.");
    }

    #[test]
    fn is_valid_bwb_id_matches_bwbr_seven_digits() {
        assert!(is_valid_bwb_id("BWBR0018451"));
        // Wrong prefix, wrong digit count, extra chars, or wrong casing all fail.
        assert!(!is_valid_bwb_id("BWBR001845")); // 6 digits
        assert!(!is_valid_bwb_id("BWBR00184510")); // 8 digits
        assert!(!is_valid_bwb_id("CVDR0018451"));
        assert!(!is_valid_bwb_id("BWBR001845x"));
        assert!(!is_valid_bwb_id("bwbr0018451"));
        assert!(!is_valid_bwb_id(""));
    }

    #[test]
    fn slugify_normalizes_names() {
        assert_eq!(slugify("Wet op de zorgtoeslag"), "wet_op_de_zorgtoeslag");
        // Collapses runs of punctuation/whitespace and trims the edges.
        assert_eq!(
            slugify("  Regeling: standaard-premie!!  "),
            "regeling_standaard_premie"
        );
        assert_eq!(slugify("---"), "");
    }

    #[test]
    fn related_harvest_priority_drops_one_per_level_and_clamps() {
        // Base is 40; child depth = enrich_depth + 1, priority = 40 - child_depth.
        assert_eq!(related_harvest_priority(0).value(), 39);
        assert_eq!(related_harvest_priority(1).value(), 38);
        assert_eq!(related_harvest_priority(2).value(), 37);
        // Deep chains clamp at 0 rather than going negative.
        assert_eq!(related_harvest_priority(39).value(), 0);
        assert_eq!(related_harvest_priority(100).value(), 0);
    }

    #[test]
    fn harvest_payload_depth_round_trips_through_serde() {
        let payload = HarvestPayload {
            bwb_id: Some("BWBR0018451".to_string()),
            cvdr_id: None,
            date: None,
            max_size_mb: None,
            depth: Some(2),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["depth"], 2);
        let back: HarvestPayload = serde_json::from_value(json).unwrap();
        assert_eq!(back.depth, Some(2));

        // depth None is omitted from the wire form (backward compatible).
        let root = HarvestPayload::for_law("BWBR0018451", None);
        let root_json = serde_json::to_string(&root).unwrap();
        assert!(!root_json.contains("depth"));
    }

    #[test]
    fn task_walk_stops_only_when_the_law_is_done_or_nothing_walks() {
        use crate::enrich::RunSteps;
        let walking = RunSteps::all();
        let reconcile_only = RunSteps {
            window: false,
            reconcile: true,
        };
        assert!(!task_walk_complete(false, walking));
        assert!(task_walk_complete(true, walking));
        assert!(task_walk_complete(false, reconcile_only));
        assert!(task_walk_complete(true, reconcile_only));
    }

    fn config_with_timeouts(secs: u64) -> crate::enrich::EnrichConfig {
        let mut config =
            crate::enrich::EnrichConfig::for_test(crate::enrich::LlmProvider::Claude {
                path: "claude".into(),
                model: None,
            });
        config.timeout = Duration::from_secs(secs);
        config.feedback_timeout = Duration::from_secs(secs);
        config
    }

    #[test]
    fn bound_llm_timeout_gives_the_translation_a_larger_share() {
        // Production: LLM_TIMEOUT_SECS=3600, WORKER_JOB_TIMEOUT_SECS=3900.
        let mut config = config_with_timeouts(3600);
        bound_llm_timeout(&mut config, Duration::from_secs(3900));
        // (3900 - 30) / (3 + 6) = 430 per share.
        assert_eq!(config.feedback_timeout, Duration::from_secs(430));
        assert_eq!(config.timeout, Duration::from_secs(1290));
        // A run that uses every call still fits the job.
        let all_calls =
            config.timeout + config.feedback_timeout * config.feedback_rounds.max_calls();
        assert!(all_calls <= Duration::from_secs(3900 - 30));
    }

    #[test]
    fn bound_llm_timeout_counts_the_configured_feedback_rounds() {
        let mut config = config_with_timeouts(3600);
        config.feedback_rounds = crate::enrich::FeedbackRounds::uniform(2);
        bound_llm_timeout(&mut config, Duration::from_secs(3900));
        // (3900 - 30) / (3 + 12) = 258 per share.
        assert_eq!(config.feedback_timeout, Duration::from_secs(258));
        assert_eq!(config.timeout, Duration::from_secs(774));
    }

    #[test]
    fn bound_llm_timeout_leaves_ceilings_that_already_fit() {
        let mut config = config_with_timeouts(300);
        bound_llm_timeout(&mut config, Duration::from_secs(3900));
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert_eq!(config.feedback_timeout, Duration::from_secs(300));
    }

    #[test]
    fn bound_llm_timeout_lowers_each_ceiling_on_its_own() {
        // Between the two shares: only the feedback ceiling has to come down.
        let mut config = config_with_timeouts(1000);
        bound_llm_timeout(&mut config, Duration::from_secs(3900));
        assert_eq!(config.timeout, Duration::from_secs(1000));
        assert_eq!(config.feedback_timeout, Duration::from_secs(430));
    }
}

/// DB-backed borging van het worker/traject-contract op jobniveau (zie de
/// crate-doc): een document-convert-job die niet via de taak-flow oplevert,
/// wordt door [`process_next_document_convert_job`] terminaal geweigerd —
/// vóór de conversie, zonder taak, zonder blob, en (structureel: het push-pad
/// bestaat niet meer in `document_convert`) zonder ook maar een git-backend
/// aan te raken.
#[cfg(test)]
#[cfg(feature = "test-utils")]
mod contract_tests {
    use super::*;
    use crate::enrich::LlmProvider;
    use crate::test_utils::TestDb;
    use serde_json::json;

    /// Seed een upload + document-convert-job zoals de upload-handler dat doet,
    /// maar met een vrij te kiezen payload (om het contract te kunnen schenden).
    async fn seed_job(db: &TestDb, payload: serde_json::Value) -> (uuid::Uuid, uuid::Uuid) {
        let (upload_id,): (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO document_uploads (traject_ref, filename, content_type, bytes) \
             VALUES ('testtraject-abcd1234', 'bron.pdf', 'application/pdf', $1) RETURNING id",
        )
        .bind(vec![1u8, 2, 3])
        .fetch_one(&db.pool)
        .await
        .expect("insert upload");

        let mut payload = payload;
        payload["upload_id"] = json!(upload_id);
        let job = job_queue::create_job(
            &db.pool,
            CreateJobRequest::new(
                JobType::DocumentConvert,
                "doc:testtraject-abcd1234/report.md",
            )
            .with_traject_ref("testtraject-abcd1234")
            .with_payload(payload)
            .with_max_attempts(1),
        )
        .await
        .expect("create job");
        (job.id, upload_id)
    }

    async fn process_one(db: &TestDb) {
        // De guard vuurt vóór de conversie; er wordt dus nooit een LLM
        // gespawnd en de timeouts zijn niet relevant.
        let config = EnrichConfig::for_test(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        });
        process_next_document_convert_job(&db.pool, &config, Duration::from_secs(60))
            .await
            .expect("verwerking hoort netjes af te ronden (met een terminaal gefaalde job)");
    }

    /// Assert dat de job terminaal gefaald is met de contractfout, de upload is
    /// opgeruimd, en er niets is afgeleverd: geen review-taak, geen result-blob
    /// — en dus aantoonbaar geen push-materiaal.
    async fn assert_rejected_without_delivery(db: &TestDb, job_id: uuid::Uuid) {
        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status::text, result->>'error' FROM jobs WHERE id = $1")
                .bind(job_id)
                .fetch_one(&db.pool)
                .await
                .expect("job row");
        assert_eq!(status, "failed", "de job faalt terminaal");
        assert!(
            error
                .as_deref()
                .unwrap_or_default()
                .contains("deliver=task"),
            "de fout benoemt het contract, kreeg: {error:?}"
        );

        let (uploads, tasks, blobs): (i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM document_uploads), \
                    (SELECT COUNT(*) FROM tasks), \
                    (SELECT COUNT(*) FROM job_blobs)",
        )
        .fetch_one(&db.pool)
        .await
        .expect("counts");
        assert_eq!(uploads, 0, "de upload-bytes zijn opgeruimd");
        assert_eq!(tasks, 0, "geen taak: de guard faalt zonder aflever-pad");
        assert_eq!(blobs, 0, "geen result-blob: er is niets opgeleverd");
    }

    #[tokio::test]
    async fn document_convert_without_task_delivery_fails_terminally() {
        // Een pre-taken-mechanisme-payload (geen deliver-veld): het oude
        // directe-push-gedrag bestaat niet meer, dus dit moet terminaal falen.
        let db = TestDb::new().await;
        let (job_id, _upload) = seed_job(
            &db,
            json!({
                "traject_id": uuid::Uuid::new_v4(),
                "traject_ref": "testtraject-abcd1234",
                "target_path": "report.md",
            }),
        )
        .await;

        process_one(&db).await;
        assert_rejected_without_delivery(&db, job_id).await;
    }

    /// Een account + traject om een echte taak aan te kunnen hangen (de
    /// `tasks`-FK's eisen bestaande rijen). De contract-tests hierboven hebben
    /// dit niet nodig omdat daar juist géén taak ontstaat.
    async fn seed_account_and_traject(db: &TestDb) -> (uuid::Uuid, uuid::Uuid) {
        let (account_id,): (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO accounts (person_sub, email, name) \
             VALUES ('sub-uploader', 'uploader@example.test', 'Uploader') RETURNING id",
        )
        .fetch_one(&db.pool)
        .await
        .expect("insert account");
        let (traject_id,): (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO trajects (name, description, scope, created_by) \
             VALUES ('Testtraject', '', '', $1) RETURNING id",
        )
        .bind(account_id)
        .fetch_one(&db.pool)
        .await
        .expect("insert traject");
        (account_id, traject_id)
    }

    /// De belofte end-to-end door de worker heen: zonder `allow_llm` faalt een
    /// conversie die de deterministische route niet haalt terminaal, mét de
    /// bestaande "Conversie mislukt"-taak — en zonder ooit de agent te starten.
    /// De seed-upload is een `.pdf` met onzin-bytes: een formaat dát een
    /// converter heeft, waarvan de tool op deze input stukloopt. Precies de
    /// stille terugval die dit ticket dichtzet (en die niet zichtbaar zou zijn
    /// als we alleen een onbekende extensie zouden testen).
    #[tokio::test]
    async fn document_convert_without_llm_permission_fails_terminally_with_task() {
        let db = TestDb::new().await;
        let (account_id, traject_id) = seed_account_and_traject(&db).await;
        let (job_id, _upload) = seed_job(
            &db,
            json!({
                "traject_id": traject_id,
                "traject_ref": "testtraject-abcd1234",
                "target_path": "report.md",
                "deliver": "task",
                "requested_by": account_id,
                "allow_llm": false,
            }),
        )
        .await;

        process_one(&db).await;

        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status::text, result->>'error' FROM jobs WHERE id = $1")
                .bind(job_id)
                .fetch_one(&db.pool)
                .await
                .expect("job row");
        assert_eq!(status, "failed", "de job faalt terminaal, zonder retry");
        let error = error.unwrap_or_default();
        assert!(
            error.contains("Er is geen taalmodel gebruikt"),
            "de fout legt uit waarom er niets is omgezet, kreeg: {error}"
        );

        // Het bestaande faal-pad levert 'm af als taak; geen nieuw mechanisme.
        let (task_type, title): (String, String) =
            sqlx::query_as("SELECT task_type, title FROM tasks WHERE job_id = $1")
                .bind(job_id)
                .fetch_one(&db.pool)
                .await
                .expect("taak-rij");
        assert_eq!(task_type, "job_failed");
        assert_eq!(title, "Conversie mislukt: report.md");

        let (uploads, blobs): (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM document_uploads), (SELECT COUNT(*) FROM job_blobs)",
        )
        .fetch_one(&db.pool)
        .await
        .expect("counts");
        assert_eq!(uploads, 0, "de upload-bytes zijn opgeruimd");
        assert_eq!(blobs, 0, "er is niets opgeleverd");
    }

    #[tokio::test]
    async fn document_convert_task_delivery_without_requested_by_fails_terminally() {
        // deliver=task zonder aanvrager: geen assignee voor de review-taak,
        // dus ook geen aflever-pad — zelfde terminale weigering.
        let db = TestDb::new().await;
        let (job_id, _upload) = seed_job(
            &db,
            json!({
                "traject_id": uuid::Uuid::new_v4(),
                "traject_ref": "testtraject-abcd1234",
                "target_path": "report.md",
                "deliver": "task",
            }),
        )
        .await;

        process_one(&db).await;
        assert_rejected_without_delivery(&db, job_id).await;
    }

    #[tokio::test]
    async fn related_legislation_with_a_bwb_id_enqueues_a_follow_up_harvest() {
        // Path (a): an explicit, valid bwb_id resolves without the slug table
        // or the SRU search, so no network is involved. The follow-up harvest
        // must land one level deeper, at the related-harvest priority.
        let db = TestDb::new().await;
        let related = vec![crate::enrich::RelatedLegislation {
            name: "Wet op de zorgtoeslag".to_string(),
            relation: "legal_basis".to_string(),
            bwb_id: Some("BWBR0018451".to_string()),
            slug: None,
            open_term: None,
        }];

        harvest_related_legislation(&db.pool, &Client::new(), "parent_law", &related, 0).await;

        let rows: Vec<(String, i32, Option<serde_json::Value>)> =
            sqlx::query_as("SELECT law_id, priority, payload FROM jobs WHERE job_type = 'harvest'")
                .fetch_all(&db.pool)
                .await
                .expect("query jobs");
        assert_eq!(rows.len(), 1, "exactly one follow-up harvest");
        let (law_id, priority, payload) = &rows[0];
        assert_eq!(law_id, "BWBR0018451");
        assert_eq!(*priority, related_harvest_priority(0).value());
        let payload = payload.as_ref().expect("harvest payload");
        assert_eq!(payload["bwb_id"], "BWBR0018451");
        assert_eq!(payload["depth"], 1);

        // Running it again finds the pending job and does not duplicate it.
        harvest_related_legislation(&db.pool, &Client::new(), "parent_law", &related, 0).await;
        let (count,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM jobs WHERE job_type = 'harvest'")
                .fetch_one(&db.pool)
                .await
                .expect("count jobs");
        assert_eq!(count, 1);
    }
}
