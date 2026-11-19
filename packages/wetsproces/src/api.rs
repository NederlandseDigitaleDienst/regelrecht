//! The HTTP API over the [`Service`], in JSON. The keys of a request and an
//! answer are those of the kern (`velden`, `bevindingen`, `op`), as the
//! chronolexografie poc speaks them.
//!
//! | Method and path | What |
//! |---|---|
//! | `GET /health` | `ok` once the server runs |
//! | `GET /api/casus` | the case, and today |
//! | `GET /api/formulier` | the forms, with what the UI spec says of their fields |
//! | `GET /api/cellen/{cel}/kroniek` | the chronicle of one cell |
//! | `POST /api/indieningen` | a submission ([`Submit`]) |
//! | `POST /api/besluiten` | a decision ([`Decide`]) |
//! | `POST /api/vastleggingen` | a fact a cell records ([`Record`]) |
//! | `POST /api/toezendingen` | a copy of a decision ([`SendCopy`]) |
//! | `GET /api/klok` | today, and whether the clock can be set |
//! | `PUT /api/klok` | set the clock, test mode only: `{ "datum": "YYYY-MM-DD" }` |
//!
//! A `POST` takes an `Idempotency-Key` header: sent again with the same
//! request it answers as the first time and records nothing; with another
//! request it is refused. Without the header every request records anew. An
//! error from the service answers `{ "fout": "<why>" }`; a body that does not
//! parse gets axum's own plain-text rejection.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::clock::ControllableClock;
use crate::config::Mode;
use crate::service::{Decide, Record, SendCopy, Service, ServiceError, Step, Submit};
use crate::store::{IdempotencyKey, StoreError};

/// The header that carries a request's [`IdempotencyKey`].
pub const IDEMPOTENCY_KEY: &str = "idempotency-key";

/// What every handler shares.
#[derive(Debug, Clone)]
pub struct AppState {
    service: Arc<Service>,
    mode: Mode,
    /// The clock the API sets, in test mode; none when the clock is real.
    clock: Option<Arc<ControllableClock>>,
}

impl AppState {
    pub fn new(service: Arc<Service>, mode: Mode, clock: Option<Arc<ControllableClock>>) -> Self {
        Self {
            service,
            mode,
            clock,
        }
    }
}

/// An error as the API answers it.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<ServiceError> for ApiError {
    fn from(e: ServiceError) -> Self {
        let status = match &e {
            ServiceError::Refused(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ServiceError::UnknownCell(_) => StatusCode::NOT_FOUND,
            ServiceError::KeyReused(_) | ServiceError::Busy => StatusCode::CONFLICT,
            ServiceError::Store(StoreError::Conflict { .. }) => StatusCode::CONFLICT,
            ServiceError::Store(
                StoreError::Backend(_)
                | StoreError::UnknownFormat { .. }
                | StoreError::DuplicateGram { .. },
            )
            | ServiceError::Internal(_) => {
                // The full error, with what the database said, goes to the
                // log only: the API has no login, so anyone may read the
                // answer.
                tracing::error!(error = %e, "request failed");
                return Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the server failed on this request; the cause is in its log",
                );
            }
        };
        Self::new(status, e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "fout": self.message }))).into_response()
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

/// The routes of the API on `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/casus", get(case))
        .route("/api/formulier", get(forms))
        .route("/api/cellen/{cel}/kroniek", get(chronicle))
        .route("/api/indieningen", post(submit))
        .route("/api/besluiten", post(decide))
        .route("/api/vastleggingen", post(record))
        .route("/api/toezendingen", post(send_copy))
        .route("/api/klok", get(clock).put(set_clock))
        .with_state(state)
}

async fn case(State(state): State<AppState>) -> Json<Value> {
    Json(state.service.case())
}

async fn forms(State(state): State<AppState>) -> ApiResult {
    Ok(Json(state.service.forms().await?))
}

async fn chronicle(State(state): State<AppState>, Path(cel): Path<String>) -> ApiResult {
    Ok(Json(state.service.chronicle(&cel).await?))
}

/// The request's key, or none without the header.
fn key(headers: &HeaderMap) -> Result<Option<IdempotencyKey>, ApiError> {
    let Some(value) = headers.get(IDEMPOTENCY_KEY) else {
        return Ok(None);
    };
    value
        .to_str()
        .map_err(|_| "an idempotency key is visible ASCII, without spaces".to_string())
        .and_then(str::parse)
        .map(Some)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e))
}

async fn perform(state: &AppState, headers: &HeaderMap, step: Step) -> ApiResult {
    let key = key(headers)?;
    Ok(Json(state.service.perform(key, step).await?))
}

async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(s): Json<Submit>,
) -> ApiResult {
    perform(&state, &headers, Step::Submit(s)).await
}

async fn decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(d): Json<Decide>,
) -> ApiResult {
    perform(&state, &headers, Step::Decide(d)).await
}

async fn record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<Record>,
) -> ApiResult {
    perform(&state, &headers, Step::Record(r)).await
}

async fn send_copy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(s): Json<SendCopy>,
) -> ApiResult {
    perform(&state, &headers, Step::SendCopy(s)).await
}

fn clock_state(state: &AppState) -> Value {
    json!({
        "datum": state.service.today(),
        "modus": state.mode.to_string(),
        "bestuurbaar": state.clock.is_some(),
    })
}

async fn clock(State(state): State<AppState>) -> Json<Value> {
    Json(clock_state(&state))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetClock {
    datum: NaiveDate,
}

async fn set_clock(State(state): State<AppState>, Json(body): Json<SetClock>) -> ApiResult {
    let Some(clock) = &state.clock else {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            format!(
                "the clock follows the real date in {} mode and cannot be set",
                state.mode
            ),
        ));
    };
    clock.set(body.datum);
    Ok(Json(clock_state(&state)))
}
