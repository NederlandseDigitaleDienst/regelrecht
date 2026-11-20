//! The wetsproces server. See the crate documentation in `lib.rs`.

use std::process::ExitCode;

use regelrecht_wetsproces::app;
use regelrecht_wetsproces::config::Config;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> ExitCode {
    regelrecht_shared::telemetry::init_subscriber("info");
    regelrecht_auth::install_crypto_provider();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "wetsproces stopped");
            eprintln!("wetsproces: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env(|k| std::env::var(k).ok())?;
    let router = app::build(&config).await?.layer(TraceLayer::new_for_http());
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
    tracing::info!(
        mode = %config.mode,
        surface = %config.surface,
        login = config.login.name(),
        store = config.store.name(),
        "listening on 0.0.0.0:{}",
        config.port
    );
    axum::serve(listener, router).await?;
    Ok(())
}
