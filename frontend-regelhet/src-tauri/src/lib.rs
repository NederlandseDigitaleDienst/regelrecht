//! The Tauri shell of the RegelHet app: it registers the commands of the
//! core ([`AppCore`]) for the webview and does nothing else. The key, the
//! credential and the session stay in the core; a command gives the webview
//! JSON, and an error as [`CommandError`], which carries neither.
//!
//! The server's address comes from `REGELHET_APP_SERVER` (and the OIN from
//! `REGELHET_APP_OIN`, the demo's when not set), at run time or else as set
//! when the shell was built. The key lives in software,
//! in the app's data directory; see `KeyStore` in the core for where a
//! hardware-bound store goes.

use std::sync::Arc;

use regelrecht_regelhet_app::{AppConfig, AppCore, CommandError, SoftwareKeyStore, Status};
use serde_json::Value;
use tauri::{Manager, State};

type Answer = Result<Value, CommandError>;

#[tauri::command]
async fn status(core: State<'_, AppCore>) -> Result<Status, CommandError> {
    Ok(core.status().await)
}

#[tauri::command]
async fn activeren(core: State<'_, AppCore>, bsn: String) -> Result<Status, CommandError> {
    core.activate(&bsn).await
}

#[tauri::command]
async fn afmelden(core: State<'_, AppCore>) -> Result<Status, CommandError> {
    core.deactivate().await
}

#[tauri::command]
async fn casus(core: State<'_, AppCore>) -> Answer {
    core.case().await
}

#[tauri::command]
async fn formulier(core: State<'_, AppCore>) -> Answer {
    core.forms().await
}

#[tauri::command]
async fn dossier(core: State<'_, AppCore>) -> Answer {
    core.dossier().await
}

#[tauri::command]
async fn waarom(core: State<'_, AppCore>, besluit: String) -> Answer {
    core.explain(&besluit).await
}

#[tauri::command]
async fn indienen(core: State<'_, AppCore>, indiening: Value, sleutel: String) -> Answer {
    core.submit(&indiening, &sleutel).await
}

/// The variable `name` from the environment of the process, or else as it
/// was when the shell was built: a phone app has no environment to start
/// with, so its build bakes the server's address in.
fn setting(name: &str) -> Option<String> {
    let built = match name {
        regelrecht_regelhet_app::config::SERVER_VAR => option_env!("REGELHET_APP_SERVER"),
        regelrecht_regelhet_app::config::OIN_VAR => option_env!("REGELHET_APP_OIN"),
        _ => None,
    };
    std::env::var(name).ok().or(built.map(str::to_string))
}

/// Build the core and run the window until it closes.
///
/// # Errors
///
/// When the data directory, the key store or the window cannot be made.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    regelrecht_regelhet_app::install_crypto_provider();
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?.join("fsc4b");
            let store = Arc::new(SoftwareKeyStore::in_dir(&dir)?);
            let config = AppConfig::from_lookup(setting);
            app.manage(AppCore::build(config, store)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status, activeren, afmelden, casus, formulier, dossier, waarom, indienen
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn mobile_main() {
    if let Err(e) = run() {
        eprintln!("regelhet-app: {e}");
    }
}
