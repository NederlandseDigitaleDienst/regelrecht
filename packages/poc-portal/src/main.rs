//! The PoC portal: one hostname, several proof-of-concepts, each behind its own
//! password.

use regelrecht_poc_portal::REGISTRY_YAML;
use regelrecht_poc_portal::app::{AppState, router};
use regelrecht_poc_portal::config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    regelrecht_shared::telemetry::init_subscriber("info");

    // Fail loud and early. Every way this can fail ends with a PoC that is
    // either unreachable or, worse, reachable without its password — so none
    // of them may be a warning that scrolls past.
    let config = Config::from_env(REGISTRY_YAML)?;

    tracing::info!(
        pocs = config.registry.pocs.len(),
        static_root = %config.static_root,
        deployment = ?config.deployment,
        "poc-portal starting",
    );

    let port = config.port;
    let app = router(AppState::new(config));

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("listening on 0.0.0.0:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
