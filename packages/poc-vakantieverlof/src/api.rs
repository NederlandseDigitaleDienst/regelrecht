//! The HTTP API under `/api`: one handler per step of [`crate::kern`].
//!
//! The state is one cell in memory, shared by every visitor; `POST
//! /api/reset` empties its chronicle. That is enough for a PoC that plays
//! the process through and loses nothing worth keeping on a restart.

use std::sync::{Arc, Mutex, MutexGuard};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use regelrecht_cel::{Cell, Error};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::kern::{self, Corpus};

#[derive(Clone)]
pub struct AppState {
    corpus: Arc<Corpus>,
    cell: Arc<Mutex<Cell>>,
}

impl AppState {
    pub fn new(corpus: Corpus) -> kern::Result<Self> {
        let cell = corpus.cell()?;
        Ok(Self {
            corpus: Arc::new(corpus),
            cell: Arc::new(Mutex::new(cell)),
        })
    }

    /// The cell. A step that panicked halfway leaves the chronicle as the
    /// cell left it: it appends whole or not at all.
    fn cell(&self) -> MutexGuard<'_, Cell> {
        self.cell.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The routes under `/api`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/vormen", get(vormen))
        .route("/api/kroniek", get(kroniek))
        .route("/api/aanvragen", post(aanvragen))
        .route("/api/besluiten", post(besluiten))
        .route("/api/besluiten/{id}/waarom", get(waarom))
        .route("/api/besluiten/{id}/antwoord/{begrip}", get(hoe_beantwoord))
        .route("/api/bezwaren", post(bezwaren))
        .route("/api/bezwaren/{id}/heroverweging", get(heroverweging))
        .route("/api/beslissingen-op-bezwaar", post(beslissen_op_bezwaar))
        .route("/api/reducties", get(reducties))
        .route("/api/reset", post(reset))
}

/// An error as the frontend reads it: `{ "fout": ... }`. What the cell or
/// the engine refuses is the visitor's to correct (422); a chronicle that
/// cannot be written, or an answer that cannot be serialized, is ours (500).
enum Fout {
    Cel(Error),
    Intern(String),
}

impl From<Error> for Fout {
    fn from(e: Error) -> Self {
        Self::Cel(e)
    }
}

impl IntoResponse for Fout {
    fn into_response(self) -> Response {
        let (status, fout) = match self {
            Self::Cel(e @ Error::Io(_)) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            Self::Cel(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
            Self::Intern(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        (status, Json(json!({ "fout": fout }))).into_response()
    }
}

type Antwoord = std::result::Result<Json<Value>, Fout>;

fn ok(value: impl serde::Serialize) -> Antwoord {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|e| Fout::Intern(e.to_string()))
}

#[derive(Deserialize)]
struct Dag {
    datum: NaiveDate,
}

async fn vormen(State(s): State<AppState>, Query(q): Query<Dag>) -> Antwoord {
    ok(kern::vormen(&s.corpus, &s.cell(), q.datum)?)
}

async fn kroniek(State(s): State<AppState>) -> Antwoord {
    let cell = s.cell();
    ok(json!({ "cel": cell.recording_actor(), "grammen": cell.grams() }))
}

#[derive(Deserialize)]
struct Indiening {
    #[serde(default)]
    velden: Map<String, Value>,
    datum: NaiveDate,
}

async fn aanvragen(State(s): State<AppState>, Json(b): Json<Indiening>) -> Antwoord {
    ok(kern::aanvragen(
        &s.corpus,
        &mut s.cell(),
        &b.velden,
        b.datum,
    )?)
}

#[derive(Deserialize)]
struct Beslissing {
    /// The application, or the objection, decided on.
    op: String,
    /// The head's findings for this case (RFC-050).
    #[serde(default)]
    bevindingen: Map<String, Value>,
    datum: NaiveDate,
}

async fn besluiten(State(s): State<AppState>, Json(b): Json<Beslissing>) -> Antwoord {
    ok(kern::besluiten(
        &s.corpus,
        &mut s.cell(),
        &b.op,
        &b.bevindingen,
        b.datum,
    )?)
}

async fn waarom(State(s): State<AppState>, Path(id): Path<String>) -> Antwoord {
    ok(kern::waarom(&s.corpus, &s.cell(), &id)?)
}

async fn hoe_beantwoord(
    State(s): State<AppState>,
    Path((id, begrip)): Path<(String, String)>,
) -> Antwoord {
    ok(kern::hoe_beantwoord(&s.corpus, &s.cell(), &id, &begrip)?)
}

#[derive(Deserialize)]
struct Bezwaar {
    besluit: String,
    #[serde(default)]
    velden: Map<String, Value>,
    datum: NaiveDate,
}

async fn bezwaren(State(s): State<AppState>, Json(b): Json<Bezwaar>) -> Antwoord {
    ok(kern::bezwaren(
        &s.corpus,
        &mut s.cell(),
        &b.besluit,
        &b.velden,
        b.datum,
    )?)
}

async fn heroverweging(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<Dag>,
) -> Antwoord {
    ok(kern::heroverweging(&s.corpus, &s.cell(), &id, q.datum)?)
}

async fn beslissen_op_bezwaar(State(s): State<AppState>, Json(b): Json<Beslissing>) -> Antwoord {
    ok(kern::beslissen_op_bezwaar(
        &s.corpus,
        &mut s.cell(),
        &b.op,
        &b.bevindingen,
        b.datum,
    )?)
}

async fn reducties(State(s): State<AppState>, Query(q): Query<Dag>) -> Antwoord {
    ok(kern::reducties(&s.corpus, &s.cell(), q.datum)?)
}

async fn reset(State(s): State<AppState>) -> Antwoord {
    *s.cell() = s.corpus.cell()?;
    ok(json!({ "grammen": [] }))
}
