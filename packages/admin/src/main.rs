use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware as axum_middleware;
use axum::routing::{delete, get, post};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tower_sessions::cookie::SameSite;
use tower_sessions::{ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

mod config;
mod corpus_handlers;
mod error;
mod handlers;
mod metrics;
mod middleware;
mod models;
mod state;

use config::AppConfig;
use state::AppState;

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 10;
const RETRY_INTERVAL: Duration = Duration::from_secs(3);

async fn health(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok("OK")
}

#[tokio::main]
async fn main() {
    regelrecht_shared::telemetry::init_subscriber("info");
    regelrecht_auth::install_crypto_provider();

    let app_config = AppConfig::from_env();

    let database_url = match env::var("DATABASE_URL").or_else(|_| env::var("DATABASE_SERVER_FULL"))
    {
        Ok(url) => url,
        Err(_) => {
            tracing::error!("DATABASE_URL or DATABASE_SERVER_FULL environment variable is not set");
            std::process::exit(1);
        }
    };

    tracing::info!("connecting to database...");

    let mut pool = None;
    for attempt in 1..=MAX_RETRIES {
        match PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .connect(&database_url)
            .await
        {
            Ok(p) => {
                pool = Some(p);
                break;
            }
            Err(e) => {
                tracing::warn!(attempt, error = %e, "failed to connect, retrying...");
                if attempt == MAX_RETRIES {
                    tracing::error!("exhausted all {MAX_RETRIES} connection attempts");
                    std::process::exit(1);
                }
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
        }
    }
    let pool: PgPool = pool.unwrap_or_else(|| {
        tracing::error!("unreachable: no pool after retry loop");
        std::process::exit(1);
    });

    tracing::info!("connected to database");

    if let Err(e) = regelrecht_pipeline::ensure_schema(&pool).await {
        tracing::error!(error = %e, "database migration failed");
        std::process::exit(1);
    }

    let (oidc_client, end_session_url) = if let Some(ref oidc_config) = app_config.oidc {
        match regelrecht_auth::discover_client(oidc_config).await {
            Ok(result) => (Some(Arc::new(result.client)), result.end_session_url),
            Err(e) => {
                tracing::error!(error = %e, "OIDC discovery failed");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    let session_store = PostgresStore::new(pool.clone());
    if let Err(e) = session_store.migrate().await {
        tracing::error!(error = %e, "failed to create session table");
        std::process::exit(1);
    }
    tracing::info!("session store ready (PostgreSQL-backed)");

    let deletion_handle = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
    );

    // Initialize corpus registry
    let corpus_state = init_corpus();

    let http_client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "failed to build HTTP client");
            std::process::exit(1);
        }
    };

    let app_state = AppState {
        pool,
        oidc_client,
        end_session_url,
        config: Arc::new(app_config),
        metrics_cache: Arc::new(metrics::new_cache()),
        http_client,
        corpus: Arc::new(tokio::sync::RwLock::new(corpus_state)),
    };

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(8)))
        .with_same_site(SameSite::Lax)
        .with_http_only(true)
        .with_secure(true);

    // Reader routes — anyone with `harvester-reader` (or higher) can list
    // jobs, sources, law entries, and platform info.
    // Note: `/api/jobs` is split by HTTP method across this router and
    // `admin_routes` (GET here, DELETE there). This works because each
    // router's `route_layer` is baked into its `MethodRouter` before merge,
    // so the per-method middleware stays attached when the routers combine.
    let reader_routes = Router::new()
        .route("/api/law_entries", get(handlers::list_law_entries))
        .route("/api/jobs", get(handlers::list_jobs))
        .route("/api/jobs/summary", get(handlers::list_jobs_summary))
        .route("/api/dashboard-stats", get(handlers::dashboard_stats))
        .route("/api/jobs/{job_id}", get(handlers::get_job))
        .route("/api/untranslatables", get(handlers::list_untranslatables))
        .route("/api/markings", get(handlers::list_markings))
        .route(
            "/api/markings/clusters",
            get(handlers::list_marking_clusters),
        )
        .route("/api/sources", get(corpus_handlers::list_sources))
        .route("/api/corpus/laws", get(corpus_handlers::list_corpus_laws))
        .route("/api/info", get(handlers::platform_info))
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::require_auth("harvester-reader"),
        ));

    // Writer routes — `harvester-writer` can enqueue work.
    let writer_routes = Router::new()
        .route("/api/harvest-jobs", post(handlers::create_harvest_job))
        .route("/api/enrich-jobs", post(handlers::create_enrich_jobs))
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::require_auth("harvester-writer"),
        ));

    // Admin routes — destructive/state-mutating ops require `harvester-admin`.
    // Reset-exhausted and source sync change shared state across the queue.
    // Job deletion is destructive even though it's a DELETE method.
    // Note: `DELETE /api/jobs` shares its path with `GET /api/jobs` in
    // `reader_routes`; see the comment there for why this is safe.
    let admin_routes = Router::new()
        .route(
            "/api/jobs",
            delete(handlers::delete_jobs).layer(axum::extract::DefaultBodyLimit::max(64 * 1024)),
        )
        .route(
            "/api/law_entries/{law_id}/reset-exhausted",
            post(handlers::reset_exhausted),
        )
        .route(
            "/api/sources/{source_id}/sync",
            post(corpus_handlers::sync_source),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::require_auth("harvester-admin"),
        ));

    let auth_routes = regelrecht_auth::auth_routes::<AppState>();

    let metrics_route = Router::new()
        .route("/metrics", get(metrics::metrics_handler))
        .route_layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::require_metrics_auth,
        ));

    // Clone for the refresh layer: with_state below consumes app_state.
    let refresh_state = app_state.clone();
    let app = Router::new()
        .route("/health", get(health))
        .merge(metrics_route)
        .merge(auth_routes)
        .merge(reader_routes)
        .merge(writer_routes)
        .merge(admin_routes)
        .with_state(app_state)
        // Inside the session layer, outside the route role gates. No-op for
        // API-key-authenticated requests (no session auth marker).
        .layer(axum_middleware::from_fn_with_state(
            refresh_state,
            middleware::refresh_session_token::<AppState>,
        ))
        .layer(session_layer)
        .layer(axum_middleware::from_fn(middleware::security_headers(
            middleware::API_CSP,
        )))
        .layer(TraceLayer::new_for_http());
    // API-only service: the harvester-admin dashboard UI now lives in the
    // editor (frontend/src/harvester), which reaches this API through the
    // editor-api /api/harvest-admin/* proxy. This binary no longer serves a
    // SPA, so there is no static fallback — unmatched paths 404. The API
    // surface (/health, /metrics, /auth/*, /api/*) stays a standalone,
    // publicly-addressable harvest API (OIDC + ADMIN_API_KEY on GET/POST/DELETE).

    let port: u16 = env::var("ADMIN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to bind on {addr}");
            std::process::exit(1);
        });

    let shutdown = async {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate()).unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to install SIGTERM handler");
            std::process::exit(1);
        });
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        tracing::info!("shutdown signal received, draining connections");
    };

    tokio::select! {
        result = axum::serve(listener, app).with_graceful_shutdown(shutdown) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "server error");
                std::process::exit(1);
            }
        }
        result = deletion_handle => {
            match result {
                Ok(_) => tracing::error!("session deletion task exited unexpectedly"),
                Err(e) => tracing::error!(error = %e, "session deletion task panicked"),
            }
            std::process::exit(1);
        }
    }
}

/// Initialize the corpus registry and load local sources.
///
/// Registry file paths can be configured via environment variables:
/// - `CORPUS_REGISTRY_PATH` (default: `corpus-registry.yaml`)
/// - `CORPUS_REGISTRY_LOCAL_PATH` (default: `corpus-registry.local.yaml`)
fn init_corpus() -> state::CorpusState {
    let manifest_str =
        env::var("CORPUS_REGISTRY_PATH").unwrap_or_else(|_| "corpus-registry.yaml".to_string());
    let local_str = env::var("CORPUS_REGISTRY_LOCAL_PATH")
        .unwrap_or_else(|_| "corpus-registry.local.yaml".to_string());
    let manifest_path = std::path::PathBuf::from(&manifest_str);
    let local_path = std::path::PathBuf::from(&local_str);

    let registry = if manifest_path.exists() {
        match regelrecht_corpus::CorpusRegistry::load(&manifest_path, Some(&local_path)) {
            Ok(r) => {
                tracing::info!(sources = r.sources().len(), "Loaded corpus registry");
                r
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load corpus registry, using empty");
                regelrecht_corpus::CorpusRegistry::empty()
            }
        }
    } else {
        tracing::info!("No corpus-registry.yaml found, corpus endpoints will return empty results");
        regelrecht_corpus::CorpusRegistry::empty()
    };

    let source_map = match registry.load_local_sources(&regelrecht_shared::dates::today_str()) {
        Ok(map) => {
            tracing::info!(laws = map.len(), "Loaded corpus laws");
            map
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load corpus sources");
            regelrecht_corpus::SourceMap::new(regelrecht_shared::dates::today_str())
        }
    };

    state::CorpusState {
        registry,
        source_map,
    }
}
