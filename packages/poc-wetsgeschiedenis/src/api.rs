//! The HTTP API, the same contract as the page expects:
//!
//! - `GET  api/status`             can fetch, the running task, the queue
//! - `GET  api/taak?id=`           one task, also while it waits or after it finished
//! - `GET  api/log`                the log book
//! - `GET  api/diagnose?bwb=`      what the service reaches, and why fetching fails
//! - `GET  api/regeling/{bwb}`     the versions of a regulation and their dossiers
//! - `POST api/haal`               `{bwb, keten: true}` or `{bwb, dossier}`: into the queue
//! - `GET  gegevens/dossiers.json` the dropdown; `gegevens/{id}.json` one timeline
//!
//! Every error is `{"fout": "..."}` with a status, so the page can say it.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::rij::{Plek, Wat};
use crate::{diagnose, id, werker, AppState};

pub struct Fout(StatusCode, String);

impl IntoResponse for Fout {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "fout": self.1 }))).into_response()
    }
}

const BWB_UITLEG: &str = "een BWB-id ziet eruit als BWBR0007376";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/taak", get(taak))
        .route(
            "/api/log",
            get(|State(s): State<AppState>| async move { Json(s.logboek.alles()) }),
        )
        .route("/api/diagnose", get(diagnose))
        .route("/api/regeling/{bwb}", get(regeling))
        .route("/api/haal", post(haal))
        .route("/gegevens/{bestand}", get(gegevens))
}

/// JSON that must not be cached: it changes while tasks run.
fn vers(v: Value) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(v)).into_response()
}

async fn status(State(s): State<AppState>) -> Response {
    vers(json!({ "ophalen": true, "taak": s.rij.huidig(), "rij": s.rij.overzicht() }))
}

#[derive(Deserialize)]
struct TaakVraag {
    id: Option<String>,
}

async fn taak(State(s): State<AppState>, Query(q): Query<TaakVraag>) -> Result<Response, Fout> {
    match q.id {
        Some(id) => s
            .rij
            .taak(&id)
            .map(vers)
            .ok_or(Fout(StatusCode::NOT_FOUND, format!("geen opdracht {id}"))),
        None => Ok(vers(s.rij.huidig().unwrap_or(Value::Null))),
    }
}

#[derive(Deserialize)]
struct DiagnoseVraag {
    bwb: Option<String>,
}

async fn diagnose(
    State(s): State<AppState>,
    Query(q): Query<DiagnoseVraag>,
) -> Result<Response, Fout> {
    let bwb = q
        .bwb
        .map(|b| b.trim().to_uppercase())
        .filter(|b| !b.is_empty());
    if bwb.as_deref().is_some_and(|b| !id::is_bwb(b)) {
        return Err(Fout(StatusCode::BAD_REQUEST, BWB_UITLEG.into()));
    }
    Ok(vers(diagnose::diagnose(&s, bwb.as_deref()).await))
}

/// What wetten.overheid.nl knows about this regulation, with per version its
/// dossiers: there already, in the queue, or failed before.
async fn regeling(State(s): State<AppState>, Path(bwb): Path<String>) -> Result<Response, Fout> {
    if !id::is_bwb(&bwb) {
        return Err(Fout(StatusCode::BAD_REQUEST, BWB_UITLEG.into()));
    }
    let t0 = std::time::Instant::now();
    let reg = werker::regeling(&s, &bwb).await.map_err(|e| {
        s.logboek.schrijf(
            "fout",
            json!({ "bij": format!("api/regeling/{bwb}"), "fout": e }),
        );
        let status = if e.contains("kent") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::BAD_GATEWAY
        };
        Fout(
            status,
            format!("De opzoeking van {bwb} op wetten.overheid.nl mislukte. {e}"),
        )
    })?;
    let mislukt = s.opslag.mislukt();
    let versies: Vec<Value> = reg["versies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["dossiers"].as_array().is_some_and(|d| !d.is_empty()))
        .map(|v| {
            let dossiers: Vec<Value> = v["dossiers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|nr| {
                    let er = s.opslag.heeft(nr);
                    let mut d = json!({ "nr": nr, "beschikbaar": er, "rij": s.rij.in_de_rij(nr) });
                    if let (false, Some(m)) = (er, mislukt.get(nr)) {
                        d["mislukt"] = m["reden"].clone();
                    }
                    d
                })
                .collect();
            json!({ "inwerking": v["inwerking"], "betreft": v["betreft"], "staatsblad": v["staatsblad"],
                    "dossiers": dossiers, "artikelen": v["artikelen"].as_array().map_or(0, Vec::len) })
        })
        .collect();
    s.logboek.schrijf(
        "regeling",
        json!({ "bwb": bwb, "versies": reg["versies"].as_array().map_or(0, Vec::len), "met_dossier": versies.len(),
                "s": (t0.elapsed().as_secs_f64() * 10.0).round() / 10.0 }),
    );
    let titel = reg["titel"]
        .as_str()
        .unwrap_or_default()
        .split('|')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(vers(
        json!({ "bwb": bwb, "titel": titel, "afkorting": reg["afkorting"], "keten": s.opslag.heeft(&bwb), "versies": versies }),
    ))
}

#[derive(Deserialize)]
struct HaalVraag {
    #[serde(default)]
    bwb: String,
    dossier: Option<Value>,
    #[serde(default)]
    keten: bool,
}

/// Into the queue. The body is read as bytes and parsed here, so that a body
/// without a content type, or sent in chunks by the portal, works the same.
async fn haal(State(s): State<AppState>, body: Bytes) -> Result<Response, Fout> {
    if body.is_empty() {
        s.logboek
            .schrijf("fout", json!({ "bij": "api/haal", "fout": "lege body" }));
    }
    let v: HaalVraag = serde_json::from_slice(if body.is_empty() { b"{}" } else { &body })
        .map_err(|e| Fout(StatusCode::BAD_REQUEST, format!("geen JSON ({e})")))?;
    if !id::is_bwb(&v.bwb) {
        return Err(Fout(StatusCode::BAD_REQUEST, BWB_UITLEG.into()));
    }
    let nr = v.dossier.as_ref().map(|d| {
        d.as_str()
            .map(String::from)
            .unwrap_or_else(|| d.to_string())
    });
    let taak = if v.keten {
        s.rij.voeg(Wat::Keten, &v.bwb, None, None, Plek::Bezoeker)
    } else if let Some(nr) = nr.filter(|n| id::is_dossier(n)) {
        s.rij
            .voeg(Wat::Dossier, &v.bwb, Some(&nr), None, Plek::Bezoeker)
    } else {
        return Err(Fout(
            StatusCode::BAD_REQUEST,
            "kies een dossier (5 cijfers) of de hele wet".into(),
        ));
    };
    Ok((StatusCode::ACCEPTED, Json(taak)).into_response())
}

async fn gegevens(
    State(s): State<AppState>,
    Path(bestand): Path<String>,
) -> Result<Response, Fout> {
    let niet = || Fout(StatusCode::NOT_FOUND, "onbekend".into());
    let id = bestand.strip_suffix(".json").ok_or_else(niet)?;
    if id == "dossiers" {
        return Ok(vers(Value::Array(s.opslag.index())));
    }
    let t = s
        .opslag
        .tijdlijn(id)
        .ok_or_else(|| Fout(StatusCode::NOT_FOUND, format!("{id} is er niet")))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        t,
    )
        .into_response())
}
