/// The commands of the core that the webview may call; the capability in
/// `capabilities/default.json` allows exactly these for the main window.
const COMMANDS: &[&str] = &[
    "status",
    "activeren",
    "afmelden",
    "casus",
    "formulier",
    "dossier",
    "waarom",
    "indienen",
];

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )?;
    Ok(())
}
