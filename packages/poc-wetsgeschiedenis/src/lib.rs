//! PoC: een wet door de tijd. Elke versie van een regeling op
//! wetten.overheid.nl, elke overgang afgespeeld uit haar bekendmaking, met de
//! nota van toelichting daaruit; en op de achtergrond de Kamerdossiers die de
//! wet maakten en wijzigden, elk met zijn toelichting aan zijn overgang. Een
//! bezoeker haalt zelf een wet op vanaf haar BWB-id.
//!
//! Deze crate is de dienst: de API ([`api`]), de rij van opdrachten
//! ([`rij`]), wat er is opgehaald ([`opslag`]), het logboek ([`logboek`]) en
//! de diagnose ([`diagnose`]). Het ophalen en afspelen zelf doet de
//! afspeelketen in Python, als een eigen proces per opdracht ([`werker`]):
//! `werker.py` naast deze crate, met `afspeler/` eronder.

pub mod api;
pub mod diagnose;
pub mod id;
pub mod logboek;
pub mod opslag;
pub mod rij;
pub mod werker;

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{OriginalUri, Request, State};
use axum::middleware::{self, Next};
use axum::response::{Redirect, Response};
use axum::routing::get;
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

/// Wat de dienst van zijn omgeving krijgt.
#[derive(Clone, Debug)]
pub struct Instellingen {
    /// De gegevens die in het image zitten (corpus-poc/wetsgeschiedenis/gegevens).
    pub vast: PathBuf,
    /// Waar opgehaalde stukken en tijdlijnen landen (WG_DATA).
    pub data: PathBuf,
    /// Hoeveel opgehaalde dossiers en wetten er ten hoogste blijven staan.
    pub max: usize,
    /// De werker: `python3` en het pad naar `werker.py`.
    pub python: String,
    pub werker: PathBuf,
    /// Seconden die curl krijgt om een verbinding te maken.
    pub verbinden: u64,
    /// Waarom de dienst niet in WG_DATA schrijft maar in een tijdelijke map,
    /// als dat zo is (een volume dat als root is aangekoppeld).
    pub uitgeweken: Option<String>,
}

/// Wat elke handler deelt.
#[derive(Clone)]
pub struct AppState {
    pub inst: Arc<Instellingen>,
    pub opslag: Arc<opslag::Opslag>,
    pub rij: Arc<rij::Rij>,
    pub logboek: Arc<logboek::Logboek>,
}

impl AppState {
    pub fn new(inst: Instellingen) -> Self {
        let opslag = opslag::Opslag::new(&inst);
        Self {
            inst: Arc::new(inst),
            opslag: Arc::new(opslag),
            rij: Arc::new(rij::Rij::default()),
            logboek: Arc::new(logboek::Logboek::new(400)),
        }
    }

    /// Start de werkdraad van de rij, en zet weer in de rij wat de opgehaalde
    /// wetten na een herstart nog missen.
    pub fn start(&self) {
        tokio::spawn(rij::werk(self.clone()));
        let n = rij::herstel(self);
        if n > 0 {
            self.logboek
                .schrijf("rij", serde_json::json!({ "herstel": n }));
        }
    }
}

/// The app under `base` (`/` or `/<prefix>/`): the API, the timelines under
/// `gegevens/`, and the built frontend in `static_dir` with its single-page
/// fallback. `/healthz` answers outside the prefix, for the platform.
///
/// The prefix goes in the router, not in a layer that rewrites the URI: axum
/// matches the path before a layer runs (see the `poc-add` skill). `nest`
/// does not route the bare `/<prefix>/` to the nested root, so that one
/// redirects to `/<prefix>`.
pub fn router(state: AppState, base: &str, static_dir: &str) -> Router {
    let index = format!("{static_dir}/index.html");
    let inner = Router::new()
        .route_service("/", ServeFile::new(&index))
        .merge(api::routes())
        // `fallback`, not `not_found_service`: an unknown path gets the page
        // with 200, so the page can route it itself.
        .fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(&index)))
        .layer(middleware::from_fn_with_state(state.clone(), verzoek))
        .with_state(state);
    let health = Router::new().route("/healthz", get(|| async { "ok" }));
    let prefix = base.trim_matches('/');
    if prefix.is_empty() {
        return health.merge(inner);
    }
    let prefix = format!("/{prefix}");
    let naar = prefix.clone();
    health
        .route(
            &format!("{prefix}/"),
            get(move || async move { Redirect::permanent(&naar) }),
        )
        .nest(&prefix, inner)
}

/// Elk verzoek in het logboek, met zijn status en duur. Het pollen van de
/// voortgang en van het logboek zelf zou het logboek vullen; dat blijft eruit
/// zolang het slaagt.
async fn verzoek(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let methode = req.method().to_string();
    // Inside the prefix the router sees the path without it; the log shows
    // the path the visitor asked for.
    let pad = req
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| req.uri().path(), |o| o.0.path())
        .to_string();
    let t0 = std::time::Instant::now();
    let res = next.run(req).await;
    let status = res.status().as_u16();
    if status >= 400 || !(pad.ends_with("api/taak") || pad.ends_with("api/log")) {
        state.logboek.schrijf(
            "verzoek",
            serde_json::json!({ "methode": methode, "pad": pad, "status": status, "ms": t0.elapsed().as_millis() as u64 }),
        );
    }
    res
}
