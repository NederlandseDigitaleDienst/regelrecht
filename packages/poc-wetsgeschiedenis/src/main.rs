//! De server van de poc. Zie de crate-documentatie in `lib.rs`.

use std::path::PathBuf;

use regelrecht_poc_wetsgeschiedenis::{opslag, router, AppState, Instellingen};
use tower_http::trace::TraceLayer;

fn env(naam: &str) -> Option<String> {
    std::env::var(naam).ok().filter(|v| !v.is_empty())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "regelrecht_poc_wetsgeschiedenis=info,tower_http=warn".into()),
        )
        .init();

    let hier = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Een volume dat als root is aangekoppeld, is niet van de gebruiker `app`
    // (gemeten in de preview, 20-10-2026). Dan wijkt de dienst uit naar een
    // tijdelijke map: ophalen werkt, maar wat is opgehaald, is na een herstart
    // weg. De diagnose zegt het.
    let gevraagd = env("WG_DATA").map_or_else(|| hier.join("data"), PathBuf::from);
    let (data, uitgeweken) = if opslag::schrijfbaar(&gevraagd) {
        (gevraagd, None)
    } else {
        let tijdelijk = std::env::temp_dir().join("wetsgeschiedenis");
        let reden = format!("{} is niet schrijfbaar; de dienst schrijft in {}, en wat is opgehaald, is na een herstart weg",
                            gevraagd.display(), tijdelijk.display());
        tracing::warn!("{reden}");
        (tijdelijk, Some(reden))
    };
    let inst = Instellingen {
        vast: env("WG_VAST_DIR").map_or_else(
            || hier.join("../../corpus-poc/wetsgeschiedenis/gegevens"),
            PathBuf::from,
        ),
        data,
        // Een wet met haar Kamerdossiers telt al snel twintig (de Archiefwet 1995: 24).
        max: env("WG_MAX").and_then(|m| m.parse().ok()).unwrap_or(150),
        python: env("WG_PYTHON").unwrap_or_else(|| "python3".into()),
        werker: env("WG_WERKER").map_or_else(|| hier.join("werker.py"), PathBuf::from),
        verbinden: env("WG_CONNECT_TIMEOUT")
            .and_then(|t| t.parse().ok())
            .unwrap_or(15),
        uitgeweken,
    };
    let state = AppState::new(inst);
    state.start();

    let base = env("WG_BASE_PATH").unwrap_or_else(|| "/wetsgeschiedenis/".into());
    let static_dir = env("WG_STATIC_DIR").unwrap_or_else(|| {
        hier.join("../../frontend-poc-wetsgeschiedenis/dist")
            .to_string_lossy()
            .into()
    });
    let app = router(state, &base, &static_dir).layer(TraceLayer::new_for_http());

    let port: u16 = env("WG_PORT").and_then(|p| p.parse().ok()).unwrap_or(8420);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(%base, "wetsgeschiedenis luistert op http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
