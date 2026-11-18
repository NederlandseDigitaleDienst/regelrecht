use std::sync::atomic::{AtomicI64, AtomicU64};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use regelrecht_pipeline::models::{JobStatus, LawStatusValue};
use sqlx::PgPool;
use strum::IntoEnumIterator;
use tokio::sync::Mutex;

use crate::state::AppState;

/// Cache TTL — avoids hitting the database on every Prometheus scrape.
const CACHE_TTL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct StatusLabel {
    pub status: String,
}

/// Raw metrics data fetched from the database.
#[derive(Clone, Debug)]
pub struct MetricsSnapshot {
    pub jobs_by_status: Vec<(String, i64)>,
    pub laws_by_status: Vec<(String, i64)>,
    pub avg_job_duration_secs: Option<f64>,
    /// Jobs that permanently failed (exhausted retries) in the last hour.
    pub recently_failed_jobs: i64,
    /// Jobs that permanently failed (exhausted retries) in the last 24 hours.
    pub recently_failed_jobs_24h: i64,
    /// Failed jobs broken down by job type (harvest/enrich) and time window.
    pub failed_by_type: FailedByType,
    /// Number of laws with harvest_exhausted status.
    pub exhausted_harvest: i64,
    /// Number of laws with enrich_exhausted status.
    pub exhausted_enrich: i64,
}

/// Failed job counts split by job type.
#[derive(Clone, Debug, Default)]
pub struct FailedByType {
    /// All-time total of permanently failed harvest jobs.
    pub harvest_total: i64,
    /// All-time total of permanently failed enrich jobs.
    pub enrich_total: i64,
    /// Permanently failed harvest jobs in the last 24 hours.
    pub harvest_24h: i64,
    /// Permanently failed enrich jobs in the last 24 hours.
    pub enrich_24h: i64,
}

/// Cached response: the encoded Prometheus text and when it was generated.
pub struct CachedMetrics {
    pub body: String,
    pub generated_at: Instant,
}

/// Global metrics cache, initialised once in [`AppState`].
pub type MetricsCache = Mutex<Option<CachedMetrics>>;

/// Create a new (empty) metrics cache.
pub fn new_cache() -> MetricsCache {
    Mutex::new(None)
}

/// Fetch all metrics from the database in as few queries as possible.
pub async fn fetch_metrics(pool: &PgPool) -> Result<MetricsSnapshot, sqlx::Error> {
    // Two queries instead of five: jobs grouped by status + laws grouped by status + avg duration.
    // We combine the avg-duration query with a CTE so it's a single round-trip for jobs.
    let jobs_by_status = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, COUNT(*) FROM jobs GROUP BY status",
    )
    .fetch_all(pool)
    .await?;

    let laws_by_status = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::text, COUNT(*) FROM law_entries GROUP BY status",
    )
    .fetch_all(pool)
    .await?;

    let avg_duration: (Option<f64>,) = sqlx::query_as(
        "SELECT AVG(EXTRACT(EPOCH FROM (completed_at - started_at)))::float8 \
         FROM jobs WHERE status = 'completed' \
         AND completed_at > NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await?;

    // job_type literals must match pipeline JobType enum variants.
    let recently_failed: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
             COUNT(*) FILTER (WHERE completed_at > NOW() - INTERVAL '1 hour'), \
             COUNT(*) FILTER (WHERE completed_at > NOW() - INTERVAL '24 hours'), \
             COUNT(*) FILTER (WHERE job_type = 'harvest'), \
             COUNT(*) FILTER (WHERE job_type = 'enrich'), \
             COUNT(*) FILTER (WHERE job_type = 'harvest' \
                 AND completed_at > NOW() - INTERVAL '24 hours'), \
             COUNT(*) FILTER (WHERE job_type = 'enrich' \
                 AND completed_at > NOW() - INTERVAL '24 hours') \
         FROM jobs WHERE status = 'failed'",
    )
    .fetch_one(pool)
    .await?;

    let exhausted: (i64, i64) = sqlx::query_as(
        "SELECT \
             COUNT(*) FILTER (WHERE status = 'harvest_exhausted'), \
             COUNT(*) FILTER (WHERE status = 'enrich_exhausted') \
         FROM law_entries",
    )
    .fetch_one(pool)
    .await?;

    Ok(MetricsSnapshot {
        jobs_by_status,
        laws_by_status,
        avg_job_duration_secs: avg_duration.0,
        recently_failed_jobs: recently_failed.0,
        recently_failed_jobs_24h: recently_failed.1,
        failed_by_type: FailedByType {
            harvest_total: recently_failed.2,
            enrich_total: recently_failed.3,
            harvest_24h: recently_failed.4,
            enrich_24h: recently_failed.5,
        },
        exhausted_harvest: exhausted.0,
        exhausted_enrich: exhausted.1,
    })
}

/// Encode a [`MetricsSnapshot`] into Prometheus/OpenMetrics text format.
pub fn encode_metrics(snapshot: &MetricsSnapshot) -> Result<String, std::fmt::Error> {
    let mut registry = Registry::default();

    let jobs_total = Family::<StatusLabel, Gauge<i64, AtomicI64>>::default();
    registry.register(
        "regelrecht_jobs",
        "Number of jobs per status",
        jobs_total.clone(),
    );

    let laws_total = Family::<StatusLabel, Gauge<i64, AtomicI64>>::default();
    registry.register(
        "regelrecht_laws",
        "Number of laws per status",
        laws_total.clone(),
    );

    let job_duration_avg = Gauge::<f64, AtomicU64>::default();
    registry.register(
        "regelrecht_job_duration_avg_seconds",
        "Average job duration in seconds (last 24h)",
        job_duration_avg.clone(),
    );

    // Seed all known statuses with zero so Prometheus always discovers them.
    for status in JobStatus::iter() {
        jobs_total
            .get_or_create(&StatusLabel {
                status: status.to_string(),
            })
            .set(0);
    }
    for status in LawStatusValue::iter() {
        laws_total
            .get_or_create(&StatusLabel {
                status: status.to_string(),
            })
            .set(0);
    }

    // Overwrite with actual values from the database.
    for (status, count) in &snapshot.jobs_by_status {
        jobs_total
            .get_or_create(&StatusLabel {
                status: status.clone(),
            })
            .set(*count);
    }

    for (status, count) in &snapshot.laws_by_status {
        laws_total
            .get_or_create(&StatusLabel {
                status: status.clone(),
            })
            .set(*count);
    }

    let recently_failed = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_recently_failed",
        "Number of jobs that permanently failed in the last hour",
        recently_failed.clone(),
    );
    recently_failed.set(snapshot.recently_failed_jobs);

    let recently_failed_24h = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_recently_failed_24h",
        "Number of jobs that permanently failed in the last 24 hours",
        recently_failed_24h.clone(),
    );
    recently_failed_24h.set(snapshot.recently_failed_jobs_24h);

    // Per-type failed metrics (harvest vs enrich).
    let harvest_failed_total = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_failed_harvest",
        "Total number of permanently failed harvest jobs",
        harvest_failed_total.clone(),
    );
    harvest_failed_total.set(snapshot.failed_by_type.harvest_total);

    let enrich_failed_total = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_failed_enrich",
        "Total number of permanently failed enrich jobs",
        enrich_failed_total.clone(),
    );
    enrich_failed_total.set(snapshot.failed_by_type.enrich_total);

    let harvest_failed_24h = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_failed_harvest_24h",
        "Number of permanently failed harvest jobs in the last 24 hours",
        harvest_failed_24h.clone(),
    );
    harvest_failed_24h.set(snapshot.failed_by_type.harvest_24h);

    let enrich_failed_24h = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_jobs_failed_enrich_24h",
        "Number of permanently failed enrich jobs in the last 24 hours",
        enrich_failed_24h.clone(),
    );
    enrich_failed_24h.set(snapshot.failed_by_type.enrich_24h);

    let exhausted_harvest = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_exhausted_harvest",
        "Number of laws with harvest_exhausted status",
        exhausted_harvest.clone(),
    );
    exhausted_harvest.set(snapshot.exhausted_harvest);

    let exhausted_enrich = Gauge::<i64, AtomicI64>::default();
    registry.register(
        "regelrecht_exhausted_enrich",
        "Number of laws with enrich_exhausted status",
        exhausted_enrich.clone(),
    );
    exhausted_enrich.set(snapshot.exhausted_enrich);

    if let Some(avg) = snapshot.avg_job_duration_secs {
        job_duration_avg.set(avg);
    }

    let mut buffer = String::new();
    encode(&mut buffer, &registry)?;
    Ok(buffer)
}

/// Axum handler for `GET /metrics`.
pub async fn metrics_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let content_type = [(
        header::CONTENT_TYPE,
        "application/openmetrics-text; version=1.0.0; charset=utf-8",
    )];

    // Single-flight: hold the mutex for the entire check-fetch-update cycle
    // so concurrent scrapers don't redundantly hit the database.
    let mut cache = state.metrics_cache.lock().await;

    if let Some(ref cached) = *cache
        && cached.generated_at.elapsed() < CACHE_TTL
    {
        return Ok((content_type, cached.body.clone()));
    }

    let snapshot = fetch_metrics(&state.pool).await.map_err(|e| {
        tracing::error!(error = %e, "failed to fetch metrics");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let body = encode_metrics(&snapshot).map_err(|e| {
        tracing::error!(error = %e, "failed to encode metrics");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    *cache = Some(CachedMetrics {
        body: body.clone(),
        generated_at: Instant::now(),
    });

    Ok((content_type, body))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty_snapshot() {
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![],
            laws_by_status: vec![],
            avg_job_duration_secs: None,
            recently_failed_jobs: 0,
            recently_failed_jobs_24h: 0,
            failed_by_type: FailedByType::default(),
            exhausted_harvest: 0,
            exhausted_enrich: 0,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");
        assert!(
            body.contains("regelrecht_jobs"),
            "should contain jobs metric family"
        );
        assert!(
            body.contains("regelrecht_laws"),
            "should contain laws metric family"
        );
        assert!(
            body.contains("regelrecht_job_duration_avg_seconds"),
            "should contain duration metric"
        );
        assert!(
            body.contains("regelrecht_jobs_recently_failed 0"),
            "should contain recently failed metric"
        );
        assert!(
            body.contains("regelrecht_jobs_recently_failed_24h 0"),
            "should contain recently failed 24h metric"
        );
        assert!(
            body.contains("regelrecht_exhausted_harvest 0"),
            "should contain exhausted harvest metric"
        );
        assert!(
            body.contains("regelrecht_exhausted_enrich 0"),
            "should contain exhausted enrich metric"
        );

        // Default zero-value gauges should be present for all known statuses.
        for status in JobStatus::iter() {
            let s = status.to_string();
            assert!(
                body.contains(&format!("regelrecht_jobs{{status=\"{s}\"}} 0")),
                "jobs should have default zero for {s}"
            );
        }
        for status in LawStatusValue::iter() {
            let s = status.to_string();
            assert!(
                body.contains(&format!("regelrecht_laws{{status=\"{s}\"}} 0")),
                "laws should have default zero for {s}"
            );
        }
    }

    #[test]
    fn encode_snapshot_with_data() {
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![
                ("completed".to_string(), 42),
                ("failed".to_string(), 3),
                ("pending".to_string(), 7),
            ],
            laws_by_status: vec![("harvested".to_string(), 100), ("enriched".to_string(), 5)],
            avg_job_duration_secs: Some(12.5),
            recently_failed_jobs: 2,
            recently_failed_jobs_24h: 5,
            failed_by_type: FailedByType {
                harvest_total: 10,
                enrich_total: 20,
                harvest_24h: 1,
                enrich_24h: 4,
            },
            exhausted_harvest: 3,
            exhausted_enrich: 1,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");

        assert!(
            body.contains("regelrecht_jobs_recently_failed 2"),
            "recently failed metric"
        );
        assert!(
            body.contains("regelrecht_jobs_recently_failed_24h 5"),
            "recently failed 24h metric"
        );

        // Job counts by status should appear as labeled gauges.
        assert!(
            body.contains("regelrecht_jobs{status=\"completed\"} 42"),
            "completed jobs"
        );
        assert!(
            body.contains("regelrecht_jobs{status=\"failed\"} 3"),
            "failed jobs"
        );
        assert!(
            body.contains("regelrecht_jobs{status=\"pending\"} 7"),
            "pending jobs"
        );

        // Law counts.
        assert!(
            body.contains("regelrecht_laws{status=\"harvested\"} 100"),
            "harvested laws"
        );
        assert!(
            body.contains("regelrecht_laws{status=\"enriched\"} 5"),
            "enriched laws"
        );

        // Average duration.
        assert!(
            body.contains("regelrecht_job_duration_avg_seconds 12.5"),
            "avg duration"
        );

        // Per-type failed metrics.
        assert!(
            body.contains("regelrecht_jobs_failed_harvest 10"),
            "harvest failed total"
        );
        assert!(
            body.contains("regelrecht_jobs_failed_enrich 20"),
            "enrich failed total"
        );
        assert!(
            body.contains("regelrecht_jobs_failed_harvest_24h 1"),
            "harvest failed 24h"
        );
        assert!(
            body.contains("regelrecht_jobs_failed_enrich_24h 4"),
            "enrich failed 24h"
        );
    }

    #[test]
    fn encode_snapshot_without_avg_duration() {
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![("running".to_string(), 1)],
            laws_by_status: vec![],
            avg_job_duration_secs: None,
            recently_failed_jobs: 0,
            recently_failed_jobs_24h: 0,
            failed_by_type: FailedByType::default(),
            exhausted_harvest: 0,
            exhausted_enrich: 0,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");
        // When no avg duration, the gauge should remain at default (0).
        assert!(body.contains("regelrecht_job_duration_avg_seconds"));
    }

    #[test]
    fn cache_is_initially_empty() {
        let cache = new_cache();
        // Mutex::try_lock is sync-safe in tests.
        let guard = cache.try_lock().expect("should be lockable");
        assert!(guard.is_none(), "cache should start empty");
    }

    #[test]
    fn redundant_completed_failed_metrics_removed() {
        // The old implementation had separate `regelrecht_jobs_completed` and
        // `regelrecht_jobs_failed` metrics. Verify they no longer appear — users
        // should use `regelrecht_jobs{status="completed"}` instead.
        let snapshot = MetricsSnapshot {
            jobs_by_status: vec![("completed".to_string(), 10), ("failed".to_string(), 2)],
            laws_by_status: vec![],
            avg_job_duration_secs: None,
            recently_failed_jobs: 0,
            recently_failed_jobs_24h: 0,
            failed_by_type: FailedByType::default(),
            exhausted_harvest: 0,
            exhausted_enrich: 0,
        };
        let body = encode_metrics(&snapshot).expect("encode should succeed");
        assert!(
            !body.contains("regelrecht_jobs_completed"),
            "redundant completed metric should not exist"
        );
        // Check there's no standalone `regelrecht_jobs_failed` metric (the old
        // name). The new per-type metrics (`regelrecht_jobs_failed_harvest_*`)
        // are fine — we only reject the bare name followed by a space.
        assert!(
            !body.contains("regelrecht_jobs_failed "),
            "redundant failed metric should not exist"
        );
    }
}
