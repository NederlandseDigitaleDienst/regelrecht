//! The HTTP API over the [`Service`], in JSON. The keys of a request and an
//! answer are those of the kern (`velden`, `bevindingen`, `op`), as the
//! chronolexografie poc speaks them.
//!
//! Every route but `/health` and the login (`/auth/...`, [`crate::login`])
//! needs a session; without one it answers 401. The routes come in three
//! groups, each behind its own gate ([`crate::access`] holds the roles):
//!
//! | Method and path | Who | What |
//! |---|---|---|
//! | `GET /health` | anyone | `ok` once the server runs |
//! | `GET /api/casus` | citizen or employee | the case, and today |
//! | `GET /api/formulier` | citizen or employee | the forms, with what the UI spec says of their fields |
//! | `GET /api/klok` | citizen or employee | today, and whether the clock can be set |
//! | `GET /api/dossier` | citizen | the records about the citizen, in every cell |
//! | `POST /api/indieningen` | citizen | a submission ([`Submit`]) for themself |
//! | `GET /api/backoffice/cellen/{cel}/kroniek` | reader of the cell | the chronicle of one cell |
//! | `POST /api/backoffice/vastleggingen` | handler of the cell | a fact a cell records ([`Record`]) |
//! | `POST /api/backoffice/toezendingen` | handler of the cell holding the decision | a copy of a decision ([`SendCopy`]) |
//! | `POST /api/backoffice/besluiten` | decider of the cell holding the submission | a decision ([`Decide`]) |
//! | `PUT /api/backoffice/klok` | admin | set the clock, test mode only: `{ "datum": "YYYY-MM-DD" }` |
//!
//! A citizen acts only for themself. The login field of their application
//! comes from the claim of their account, never from the request, and a
//! submission that refers to a record must refer to one in their own
//! dossier. A record an employee names by id must be in a cell whose
//! chronicle they may read; one that is not answers 404, the same as one
//! that does not exist, so an id does not tell whether it exists elsewhere.
//!
//! A `POST` takes an `Idempotency-Key` header: sent again by the same
//! account with the same request it answers as the first time and records
//! nothing; otherwise it is refused. Without the header every request
//! records anew. An error answers `{ "fout": "<why>" }`; a body that does
//! not parse gets axum's own plain-text rejection.

use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::access::{self, Level, Principal};
use crate::clock::ControllableClock;
use crate::config::Mode;
use crate::login::{Login, OidcLogin};
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
    login: Login,
    /// The claim of a citizen's account that identifies them.
    subject_claim: Arc<str>,
    http_client: reqwest::Client,
}

impl AppState {
    pub fn new(
        service: Arc<Service>,
        mode: Mode,
        clock: Option<Arc<ControllableClock>>,
        login: Login,
        subject_claim: &str,
    ) -> Self {
        Self {
            service,
            mode,
            clock,
            login,
            subject_claim: subject_claim.into(),
            http_client: regelrecht_auth::http_client(),
        }
    }

    /// The OIDC login, when the server logs in through Keycloak.
    pub fn oidc(&self) -> Option<&OidcLogin> {
        match &self.login {
            Login::Oidc(o) => Some(o),
            Login::TestLogin => None,
        }
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

/// An error as the API answers it.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// The answer for a record the caller may not see, whether or not it
    /// exists.
    fn not_yours(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            format!("there is no {what} with this id that you may act on"),
        )
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
            | ServiceError::Internal(_)
            | ServiceError::LoginField(_) => {
                // The full error, with what the database said, goes to the
                // log only.
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

/// The routes of the API on `state`, with the login's routes; the session
/// layer goes around them ([`crate::app`]).
pub fn router(state: AppState) -> Router {
    let shared = Router::new()
        .route("/api/casus", get(case))
        .route("/api/formulier", get(forms))
        .route("/api/klok", get(clock))
        .route_layer(middleware::from_fn(gate_user));
    let citizen = Router::new()
        .route("/api/dossier", get(dossier))
        .route("/api/indieningen", post(submit))
        .route_layer(middleware::from_fn(gate_citizen));
    let backoffice = Router::new()
        .route("/api/backoffice/cellen/{cel}/kroniek", get(chronicle))
        .route("/api/backoffice/besluiten", post(decide))
        .route("/api/backoffice/vastleggingen", post(record))
        .route("/api/backoffice/toezendingen", post(send_copy))
        .route("/api/backoffice/klok", put(set_clock))
        .route_layer(middleware::from_fn(gate_employee));
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(state.login.routes())
        .merge(shared)
        .merge(citizen)
        .merge(backoffice)
        .with_state(state)
}

/// Let the request through when `allowed` holds for its principal, with
/// the principal in its extensions for the handler.
async fn gate(
    principal: Principal,
    allowed: fn(&Principal) -> bool,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !allowed(&principal) {
        tracing::warn!(sub = %principal.sub(), path = %request.uri().path(), "refused at the gate");
        return Err(access::forbidden());
    }
    request.extensions_mut().insert(principal);
    Ok(next.run(request).await)
}

/// A citizen or an employee.
async fn gate_user(
    principal: Principal,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    gate(principal, Principal::is_user, request, next).await
}

/// A citizen.
async fn gate_citizen(
    principal: Principal,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    gate(principal, |p| p.is_citizen(), request, next).await
}

/// An employee: a reader or more, of any cell; the handler checks the cell.
async fn gate_employee(
    principal: Principal,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    gate(principal, |p| p.has(Level::Reader), request, next).await
}

async fn case(State(state): State<AppState>) -> Json<Value> {
    Json(state.service.case())
}

async fn forms(State(state): State<AppState>) -> ApiResult {
    let mut forms = state.service.forms().await?;
    without_login_offer(&mut forms);
    Ok(Json(forms))
}

/// Drop from the forms what the kern offers to log in with (`inloggen`).
/// Without a login step of its own, a case offers there the identifiers
/// recorded in its chronicles, of every applicant; this server logs in
/// through its own login and must not show them to anyone.
fn without_login_offer(forms: &mut Value) {
    if let Some(submissions) = forms.get_mut("indieningen").and_then(Value::as_object_mut) {
        for submission in submissions.values_mut() {
            if let Some(submission) = submission.as_object_mut() {
                submission.remove("inloggen");
            }
        }
    }
}

async fn dossier(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> ApiResult {
    let citizen = principal.citizen(&state.subject_claim)?;
    let dossier = state.service.dossier(citizen.subject()).await?;
    Ok(Json(json!({ "grammen": dossier.grams })))
}

async fn chronicle(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(cel): Path<String>,
) -> ApiResult {
    principal.require(Level::Reader, &cel)?;
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

async fn perform(state: &AppState, headers: &HeaderMap, step: Step, actor: &str) -> ApiResult {
    let key = key(headers)?;
    Ok(Json(state.service.perform(key, step, actor).await?))
}

/// The cell holding the record `id`, when `principal` may read its
/// chronicle; `what` names the record in the 404 otherwise.
async fn readable_holder(
    state: &AppState,
    principal: &Principal,
    id: &str,
    what: &str,
) -> Result<String, ApiError> {
    match state.service.holder(id).await? {
        Some(cell) if principal.may(Level::Reader, &cell) => Ok(cell),
        _ => Err(ApiError::not_yours(what)),
    }
}

/// A citizen submits for themself: the login field is theirs, and a
/// reference goes to a record of their own dossier.
async fn submit(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(mut s): Json<Submit>,
) -> ApiResult {
    let citizen = principal.citizen(&state.subject_claim)?;
    let field = state.service.login_field();
    if s.fields.contains_key(field) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("'{field}' is filled from your login, not from the request"),
        ));
    }
    if let Some(id) = &s.refers_to
        && !state.service.dossier(citizen.subject()).await?.holds(id)
    {
        return Err(ApiError::not_yours("record in your dossier"));
    }
    let application = s
        .article
        .as_deref()
        .is_none_or(|a| a == state.service.application_article());
    if application {
        s.fields
            .insert(field.to_string(), Value::from(citizen.subject()));
    } else if s.refers_to.is_none() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "a submission other than the application refers to a record of your dossier (`op`)",
        ));
    }
    perform(&state, &headers, Step::Submit(s), citizen.sub()).await
}

/// A decision, by a decider of the cell that holds the submission.
async fn decide(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(d): Json<Decide>,
) -> ApiResult {
    let cell = readable_holder(&state, &principal, &d.submission, "submission").await?;
    if let Some(after) = &d.after {
        readable_holder(&state, &principal, after, "record").await?;
    }
    principal.require(Level::Decider, &cell)?;
    perform(&state, &headers, Step::Decide(d), principal.sub()).await
}

/// A fact, by a handler of the cell that records it.
async fn record(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(r): Json<Record>,
) -> ApiResult {
    principal.require(Level::Handler, &r.cell)?;
    if let Some(id) = &r.refers_to {
        readable_holder(&state, &principal, id, "record").await?;
    }
    perform(&state, &headers, Step::Record(r), principal.sub()).await
}

/// A copy of a decision, by a handler of the cell that holds it.
async fn send_copy(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(s): Json<SendCopy>,
) -> ApiResult {
    let cell = readable_holder(&state, &principal, &s.decision, "decision").await?;
    principal.require(Level::Handler, &cell)?;
    perform(&state, &headers, Step::SendCopy(s), principal.sub()).await
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

async fn set_clock(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<SetClock>,
) -> ApiResult {
    if !principal.has(Level::Admin) {
        return Err(access::forbidden());
    }
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
    tracing::info!(sub = %principal.sub(), day = %body.datum, "clock set");
    Ok(Json(clock_state(&state)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forms_offer_no_identifiers_to_log_in_with() {
        let mut forms = json!({
            "indieningen": {
                "a#1": { "soort": "AANVRAAG", "inloggen": { "keuzes": ["999990019"] } },
                "a#2": { "soort": "BEZWAAR" },
            },
            "besluiten": {},
        });
        without_login_offer(&mut forms);
        assert_eq!(
            forms["indieningen"],
            json!({ "a#1": { "soort": "AANVRAAG" }, "a#2": { "soort": "BEZWAAR" } })
        );
    }
}
