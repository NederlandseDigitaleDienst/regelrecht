use axum::Router;
use axum::routing::{get, post};
use tower_http::trace::TraceLayer;

use regelrecht_pipeline::ApiState;
use regelrecht_pipeline::api::{bwb_search, harvest, status};

#[tokio::main]
async fn main() {
    regelrecht_shared::telemetry::init_subscriber("info");

    let database_url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_SERVER_FULL"))
        .unwrap_or_else(|_| {
            tracing::error!("DATABASE_URL or DATABASE_SERVER_FULL must be set");
            std::process::exit(1);
        });

    let pool = match sqlx::PgPool::connect(&database_url).await {
        Ok(pool) => {
            tracing::info!("connected to pipeline database");
            pool
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to connect to pipeline database");
            std::process::exit(1);
        }
    };

    let http_client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::error!(error = %e, "failed to build HTTP client");
            std::process::exit(1);
        }
    };

    if let Err(e) = regelrecht_pipeline::ensure_schema(&pool).await {
        tracing::error!(error = %e, "failed to run database migrations");
        std::process::exit(1);
    }

    let state = ApiState { pool, http_client };

    let app = Router::new()
        .route("/harvest/search", get(bwb_search::search_bwb))
        .route("/harvest", post(harvest::request_harvest))
        .route("/harvest/batch", post(harvest::request_harvest_batch))
        .route("/harvest/status", get(status::harvest_status))
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{port}");

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => {
            tracing::info!(addr = %addr, "pipeline-api listening");
            l
        }
        Err(e) => {
            tracing::error!(error = %e, addr = %addr, "failed to bind");
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "server exited with error");
        std::process::exit(1);
    }
}

async fn health() -> &'static str {
    "OK"
}
