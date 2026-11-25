//! The RegelHet server. See the crate documentation in `lib.rs`.
//!
//! `regelrecht-regelhet check <manifest> [case]` starts no server: it says
//! whether the case of that manifest starts, and what it is about
//! (`regelrecht_regelhet::check`).

use std::path::Path;
use std::process::ExitCode;

use regelrecht_regelhet::app;
use regelrecht_regelhet::check;
use regelrecht_regelhet::config::Config;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> ExitCode {
    regelrecht_auth::install_crypto_provider();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {}
        [command, manifest] if command == "check" => {
            return run_check(Path::new(manifest), None).await;
        }
        [command, manifest, case] if command == "check" => {
            return run_check(Path::new(manifest), Some(case)).await;
        }
        _ => {
            eprintln!("usage: regelrecht-regelhet [check <manifest> [case]]");
            return ExitCode::from(2);
        }
    }
    regelrecht_shared::telemetry::init_subscriber("info");
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "regelhet stopped");
            eprintln!("regelhet: {e}");
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
        db_max_connections = config
            .store
            .max_connections()
            .map_or_else(|| "none".to_string(), |n| n.to_string()),
        "listening on 0.0.0.0:{}",
        config.port
    );
    axum::serve(listener, router).await?;
    Ok(())
}

/// The check, without the log of the start it runs: its verdict goes to
/// stdout, or to stderr with exit code 1.
async fn run_check(manifest: &Path, case: Option<&str>) -> ExitCode {
    match check::check(manifest, case).await {
        Ok(summary) => {
            print!("{summary}");
            println!("ok: {} starts", manifest.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}: {e}", manifest.display());
            ExitCode::FAILURE
        }
    }
}
