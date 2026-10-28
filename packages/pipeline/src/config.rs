use std::path::PathBuf;
use std::time::Duration;

use regelrecht_corpus::CorpusConfig;

use crate::error::{PipelineError, Result};

fn resolve_database_url() -> Result<String> {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_SERVER_FULL"))
        .map_err(|_| PipelineError::Config("DATABASE_URL or DATABASE_SERVER_FULL not set".into()))
}

fn resolve_max_connections() -> u32 {
    std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}

#[derive(Clone)]
pub struct PipelineConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl std::fmt::Debug for PipelineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineConfig")
            .field("database_url", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

impl PipelineConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: resolve_database_url()?,
            max_connections: resolve_max_connections(),
        })
    }

    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_connections: 5,
        }
    }
}

#[derive(Clone)]
pub struct WorkerConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub output_dir: PathBuf,
    pub regulation_output_base: String,
    pub poll_interval: Duration,
    pub max_poll_interval: Duration,
    pub corpus_config: Option<CorpusConfig>,
    /// Maximum time a single job may run before being aborted by the worker.
    /// Default: 20 minutes. Configurable via `WORKER_JOB_TIMEOUT_SECS`.
    pub job_timeout: Duration,
    /// How long a processing job may go without a heartbeat before the reaper
    /// takes it back (reset or failed). Default: 5 minutes, at least four
    /// heartbeats.
    /// Configurable via `WORKER_ORPHAN_TIMEOUT_SECS`.
    pub orphan_timeout: Duration,
    /// Number of consecutive failures before a law is marked as exhausted.
    /// Default: 10. Configurable via `EXHAUSTED_THRESHOLD`.
    pub exhausted_threshold: i32,
    /// Number of consecutive *resource-exhaustion* failures (fork()/EAGAIN/OOM)
    /// before the worker exits so the orchestrator restarts it with a clean
    /// process table. These faults are environmental and only clear on restart,
    /// so retrying in-process just burns job retry budget in a tight loop.
    /// Default: 5. Configurable via `WORKER_MAX_CONSECUTIVE_RESOURCE_FAILURES`.
    pub max_consecutive_resource_failures: u32,
    /// Maximum number of enrichment jobs (for this worker's provider) that may
    /// run per local clock-hour (Europe/Amsterdam). Configurable via
    /// `ENRICH_HOURLY_LIMIT`.
    ///
    /// **Fail-closed**: absent (or unparseable) reads as `0`, and `0` pauses
    /// enrichment entirely — a worker must be given an explicit positive limit
    /// to run. This protects a personal Claude subscription token from being
    /// spent by accident (a forgotten env var runs nothing rather than the whole
    /// corpus).
    ///
    /// Enforced by the enrich worker against the durable `jobs` table, so the
    /// cap survives restarts. The cap keys on this worker's configured provider
    /// (`LLM_PROVIDER`), and once reached it pauses the whole worker until the
    /// next local hour — so it is meant for a provider-dedicated worker (e.g. a
    /// claude-only enrichworker).
    pub enrich_hourly_limit: u32,
    /// Multiplier applied to `enrich_hourly_limit` during the local night window
    /// (00:00–08:00 Europe/Amsterdam), so bulk enrichment runs mostly overnight.
    /// Configurable via `ENRICH_NIGHT_MULTIPLIER`. Default `1` (no boost) so a
    /// missing or typo'd value never silently amplifies spend; clamped to a
    /// minimum of `1` so an explicit `0` can't invert the intent (night pause).
    pub enrich_night_multiplier: u32,
    /// When true, a completed harvest auto-enqueues enrich jobs for that law.
    /// Off by default; enrichment is otherwise requested explicitly via the admin
    /// API. Configurable via `ENRICH_AUTO_ENQUEUE`.
    pub auto_enrich_enqueue: bool,
    /// Maximum recursion depth for related-legislation follow-up harvests.
    /// A depth-0 enrichment may enqueue harvests at depth 1, whose enrichments
    /// may enqueue at depth 2, etc., up to this cap. Default: 2. Configurable
    /// via `RELATED_HARVEST_MAX_DEPTH`.
    pub related_harvest_max_depth: u32,
}

impl std::fmt::Debug for WorkerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerConfig")
            .field("database_url", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .field("output_dir", &self.output_dir)
            .field("regulation_output_base", &self.regulation_output_base)
            .field("poll_interval", &self.poll_interval)
            .field("max_poll_interval", &self.max_poll_interval)
            .field("corpus_config", &self.corpus_config)
            .field("job_timeout", &self.job_timeout)
            .field("orphan_timeout", &self.orphan_timeout)
            .field("exhausted_threshold", &self.exhausted_threshold)
            .field(
                "max_consecutive_resource_failures",
                &self.max_consecutive_resource_failures,
            )
            .field("enrich_hourly_limit", &self.enrich_hourly_limit)
            .field("enrich_night_multiplier", &self.enrich_night_multiplier)
            .field("auto_enrich_enqueue", &self.auto_enrich_enqueue)
            .field("related_harvest_max_depth", &self.related_harvest_max_depth)
            .finish()
    }
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self> {
        let database_url = resolve_database_url()?;
        let max_connections = resolve_max_connections();

        let output_dir = std::env::var("REGULATION_REPO_PATH")
            .unwrap_or_else(|_| "./regulation-repo".into())
            .into();

        let regulation_output_base =
            std::env::var("REGULATION_OUTPUT_BASE").unwrap_or_else(|_| "regulation/nl".into());

        let poll_interval_secs: u64 = std::env::var("WORKER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let max_poll_interval_secs: u64 = std::env::var("WORKER_MAX_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let corpus_config = CorpusConfig::from_env_optional();

        let job_timeout_secs: u64 = std::env::var("WORKER_JOB_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20 * 60); // 20 minutes

        let orphan_timeout_secs: u64 = std::env::var("WORKER_ORPHAN_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5 * 60);
        // The reaper takes a job back once its heartbeat is this old, so it
        // has to span a few beats: one slow round trip must not cost a job.
        // The default of 5 minutes is ten beats; the floor is four.
        let min_orphan_secs = crate::job_queue::HEARTBEAT_INTERVAL.as_secs() * 4;
        let orphan_timeout_secs = if orphan_timeout_secs < min_orphan_secs {
            tracing::warn!(
                configured = orphan_timeout_secs,
                using = min_orphan_secs,
                "WORKER_ORPHAN_TIMEOUT_SECS is shorter than four heartbeats; raised"
            );
            min_orphan_secs
        } else {
            orphan_timeout_secs
        };

        let exhausted_threshold: i32 = std::env::var("EXHAUSTED_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10)
            .max(1);

        let max_consecutive_resource_failures: u32 =
            std::env::var("WORKER_MAX_CONSECUTIVE_RESOURCE_FAILURES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5)
                .max(1);

        // Per-provider hourly run cap. Fail-closed: absent reads as 0 (paused),
        // and a present-but-unparseable value warns and also reads as 0 rather
        // than silently disabling the cap (this guards spend on a personal token).
        let enrich_hourly_limit: u32 = match std::env::var("ENRICH_HOURLY_LIMIT") {
            Ok(raw) => raw.parse::<u32>().unwrap_or_else(|_| {
                tracing::warn!(
                    value = %raw,
                    "ENRICH_HOURLY_LIMIT is not a valid non-negative integer; treating as 0 (enrichment paused)"
                );
                0
            }),
            Err(_) => 0,
        };

        // Night-window multiplier. Default 1 (no boost). Present-but-unparseable
        // warns and reads as 1 rather than amplifying spend on a typo. Clamped to
        // a minimum of 1 (`.max(1)`) so an explicit `0` can't invert the intent —
        // a 0 would make the night cap `base * 0 = 0`, pausing the worker at night
        // and running it only by day. Never amplifies spend.
        let enrich_night_multiplier: u32 = match std::env::var("ENRICH_NIGHT_MULTIPLIER") {
            Ok(raw) => raw.parse::<u32>().unwrap_or_else(|_| {
                tracing::warn!(
                    value = %raw,
                    "ENRICH_NIGHT_MULTIPLIER is not a valid non-negative integer; treating as 1 (no night boost)"
                );
                1
            }),
            Err(_) => 1,
        }
        .max(1);

        // Auto-enrich after harvest is opt-in; unset/unrecognized reads as false.
        let auto_enrich_enqueue = std::env::var("ENRICH_AUTO_ENQUEUE")
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);

        let related_harvest_max_depth: u32 = std::env::var("RELATED_HARVEST_MAX_DEPTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        Ok(Self {
            database_url,
            max_connections,
            output_dir,
            regulation_output_base,
            poll_interval: Duration::from_secs(poll_interval_secs),
            max_poll_interval: Duration::from_secs(max_poll_interval_secs),
            corpus_config,
            job_timeout: Duration::from_secs(job_timeout_secs),
            orphan_timeout: Duration::from_secs(orphan_timeout_secs),
            exhausted_threshold,
            max_consecutive_resource_failures,
            enrich_hourly_limit,
            enrich_night_multiplier,
            auto_enrich_enqueue,
            related_harvest_max_depth,
        })
    }

    pub fn pipeline_config(&self) -> PipelineConfig {
        PipelineConfig {
            database_url: self.database_url.clone(),
            max_connections: self.max_connections,
        }
    }
}
