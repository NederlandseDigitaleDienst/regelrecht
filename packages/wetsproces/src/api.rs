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
//! | `GET /api/ik` | citizen or employee | who is logged in, and what they may do ([`me`]) |
//! | `GET /api/dossier` | citizen | the records about the citizen, in every cell |
//! | `GET /api/dossier/waarom/{id}` | citizen | why a decision of their dossier came out so |
//! | `POST /api/indieningen` | citizen | a submission ([`Submit`]) for themself |
//! | `GET /api/backoffice/cellen/{cel}/kroniek` | reader of the cell | the chronicle of one cell |
//! | `GET /api/backoffice/waarom/{id}` | reader of the cell holding the decision | why it came out so |
//! | `POST /api/backoffice/voorvertoning` | decider of the cell holding the submission | what a decision ([`Decide`]) would come to; records nothing |
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
//! The citizen's routes are offered a second time, under `/fsc4b`, to apps
//! of the FSC voor Burgers channel ([`crate::fsc4b`]), when that channel is
//! configured; the handlers below serve both.
//!
//! A `POST` takes an `Idempotency-Key` header: sent again by the same
//! account with the same request it answers as the first time and records
//! nothing; otherwise it is refused. Without the header every request
//! records anew. An error answers `{ "fout": "<why>" }`; a body that does
//! not parse gets axum's own plain-text rejection.

use std::collections::BTreeSet;
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
use crate::fsc4b::Fsc4b;
use crate::login::{Login, OidcLogin};
use crate::service::{Decide, Record, SendCopy, Service, ServiceError, Step, Submit};
use crate::store::{Channel, IdempotencyKey, StoreError};

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
    /// The FSC voor Burgers channel, when it is configured.
    fsc4b: Option<Arc<Fsc4b>>,
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
            fsc4b: None,
        }
    }

    /// The state with the FSC voor Burgers channel `fsc4b` open.
    pub fn with_fsc4b(mut self, fsc4b: Arc<Fsc4b>) -> Self {
        self.fsc4b = Some(fsc4b);
        self
    }

    /// The FSC voor Burgers channel, when it is configured.
    pub fn fsc4b(&self) -> Option<&Arc<Fsc4b>> {
        self.fsc4b.as_ref()
    }

    /// The claim of a citizen's account that identifies them.
    pub fn subject_claim(&self) -> &str {
        &self.subject_claim
    }

    /// The channel of a browser session: the login this server runs.
    pub fn session_channel(&self) -> Channel {
        match self.login {
            Login::Oidc(_) => Channel::Keycloak,
            Login::TestLogin => Channel::TestLogin,
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

    /// The process this server runs.
    pub fn service(&self) -> &Service {
        &self.service
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

    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The answer's body, as [`IntoResponse`] writes it.
    pub fn body(&self) -> Value {
        json!({ "fout": self.message })
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
        (self.status, Json(self.body())).into_response()
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

/// What a handler of the citizen's routes answers, before a channel wraps it.
pub(crate) type CitizenResult = Result<Value, ApiError>;

/// The routes of the API on `state`, with the login's routes; the session
/// layer goes around them ([`crate::app`]).
pub fn router(state: AppState) -> Router {
    let shared = Router::new()
        .route("/api/casus", get(case))
        .route("/api/formulier", get(forms))
        .route("/api/klok", get(clock))
        .route("/api/ik", get(me))
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_user));
    let citizen = Router::new()
        .route("/api/dossier", get(dossier))
        .route("/api/dossier/waarom/{id}", get(dossier_explain))
        .route("/api/indieningen", post(submit))
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_citizen));
    let backoffice = Router::new()
        .route("/api/backoffice/cellen/{cel}/kroniek", get(chronicle))
        .route("/api/backoffice/besluiten", post(decide))
        .route("/api/backoffice/voorvertoning", post(preview))
        .route("/api/backoffice/waarom/{id}", get(backoffice_explain))
        .route("/api/backoffice/vastleggingen", post(record))
        .route("/api/backoffice/toezendingen", post(send_copy))
        .route("/api/backoffice/klok", put(set_clock))
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_employee));
    let fsc4b = state
        .fsc4b
        .as_ref()
        .map_or_else(Router::new, |f| crate::fsc4b::routes(f));
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(state.login.routes())
        .merge(shared)
        .merge(citizen)
        .merge(backoffice)
        .merge(fsc4b)
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
    Json(case_of(&state))
}

/// The case, and today.
pub(crate) fn case_of(state: &AppState) -> Value {
    state.service.case()
}

async fn forms(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> ApiResult {
    forms_for(&state, &principal).await.map(Json)
}

/// The forms as `principal` may see them.
pub(crate) async fn forms_for(state: &AppState, principal: &Principal) -> CitizenResult {
    let mut forms = state.service.forms().await?;
    without_login_offer(&mut forms);
    let visible = visible_records(state, principal).await?;
    only_visible_records(&mut forms, &visible);
    Ok(forms)
}

/// The ids of the records `principal` may see: those of their dossier as a
/// citizen, and those in the cells they may read as an employee.
async fn visible_records(
    state: &AppState,
    principal: &Principal,
) -> Result<BTreeSet<String>, ApiError> {
    let mut ids = state
        .service
        .record_ids(|cell| principal.may(Level::Reader, cell))
        .await?;
    if principal.is_citizen()
        && let Ok(citizen) = principal.citizen(&state.subject_claim)
    {
        let dossier = state.service.dossier(citizen.subject()).await?;
        ids.extend(dossier.grams.into_iter().map(|g| g.id));
    }
    Ok(ids)
}

/// Keep in the forms only what they say of the records in `visible`: the
/// kern lists, per record of every cell, the fields it holds itself
/// (`eigen_velden`), and its ids would tell anyone which records exist.
fn only_visible_records(forms: &mut Value, visible: &BTreeSet<String>) {
    if let Some(own) = forms.get_mut("eigen_velden").and_then(Value::as_object_mut) {
        own.retain(|id, _| visible.contains(id));
    }
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
    dossier_of(&state, &principal).await.map(Json)
}

/// The dossier of the citizen `principal`.
pub(crate) async fn dossier_of(state: &AppState, principal: &Principal) -> CitizenResult {
    let citizen = principal.citizen(&state.subject_claim)?;
    let dossier = state.service.dossier(citizen.subject()).await?;
    Ok(json!({ "grammen": dossier.grams }))
}

/// Who is logged in, for a screen to show what they may do: their name,
/// whether they are a citizen and with which identifier, their level on the
/// ladder, and per cell of the case they work for what that level lets them
/// do there. The screen hides what the account may not do; the routes still
/// check every request themselves.
async fn me(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    session: tower_sessions::Session,
) -> ApiResult {
    // The name is only shown; a session that does not give it leaves it out.
    let name: Option<String> = session
        .get(regelrecht_auth::SESSION_KEY_NAME)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "reading the name from the session");
            None
        });
    let citizen = principal.citizen(&state.subject_claim).ok();
    let cells: Vec<Value> = state
        .service
        .cells()
        .filter(|(cell, _)| principal.may(Level::Reader, cell))
        .map(|(cell, cell_name)| {
            json!({
                "cel": cell,
                "cel_naam": cell_name,
                "lezen": true,
                "behandelen": principal.may(Level::Handler, cell),
                "beslissen": principal.may(Level::Decider, cell),
            })
        })
        .collect();
    Ok(Json(json!({
        "sub": principal.sub(),
        "naam": name,
        "burger": principal.is_citizen(),
        "identiteit": citizen.as_ref().map(access::Citizen::subject),
        "niveau": principal.level().map(Level::name),
        "cellen": cells,
        "klok_zetten": principal.has(Level::Admin) && state.clock.is_some(),
    })))
}

/// Why a decision of the citizen's own dossier came out so.
async fn dossier_explain(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult {
    explain_for(&state, &principal, &id).await.map(Json)
}

/// Why the decision `id` of the dossier of the citizen `principal` came out
/// so.
pub(crate) async fn explain_for(
    state: &AppState,
    principal: &Principal,
    id: &str,
) -> CitizenResult {
    let citizen = principal.citizen(&state.subject_claim)?;
    if !state.service.dossier(citizen.subject()).await?.holds(id) {
        return Err(ApiError::not_yours("decision in your dossier"));
    }
    Ok(state.service.explain(id).await?)
}

/// Why a decision of a cell the employee may read came out so.
async fn backoffice_explain(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult {
    readable_holder(&state, &principal, &id, "decision").await?;
    Ok(Json(state.service.explain(&id).await?))
}

/// What a decision would come to, for a decider of the cell that holds the
/// submission, as [`decide`] checks it; nothing is recorded.
async fn preview(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(d): Json<Decide>,
) -> ApiResult {
    let cell = readable_holder(&state, &principal, &d.submission, "submission").await?;
    if let Some(after) = &d.after {
        readable_holder(&state, &principal, after, "record").await?;
    }
    principal.require(Level::Decider, &cell)?;
    Ok(Json(state.service.preview(d).await?))
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

async fn perform(
    state: &AppState,
    headers: &HeaderMap,
    step: Step,
    principal: &Principal,
) -> CitizenResult {
    let key = key(headers)?;
    Ok(state.service.perform(key, step, &principal.actor()).await?)
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
    Json(s): Json<Submit>,
) -> ApiResult {
    submit_for(&state, &principal, &headers, s).await.map(Json)
}

/// The submission `s` of the citizen `principal`.
pub(crate) async fn submit_for(
    state: &AppState,
    principal: &Principal,
    headers: &HeaderMap,
    mut s: Submit,
) -> CitizenResult {
    // The checks read the chronicles apart from the step. That is safe
    // because a chronicle only grows and a record never changes cell: what
    // was in the dossier or a readable cell when checked still is when the
    // step runs.
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
    perform(state, headers, Step::Submit(s), principal).await
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
    perform(&state, &headers, Step::Decide(d), &principal)
        .await
        .map(Json)
}

/// A fact, by a handler of the cell that records it.
async fn record(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(r): Json<Record>,
) -> ApiResult {
    // The kern resolves `verwijst_naar` only within the chronicle of
    // `r.cell` (a record id or a kenmerk of its beginning), which the
    // handler of that cell may read, so it needs no check of its own here.
    principal.require(Level::Handler, &r.cell)?;
    perform(&state, &headers, Step::Record(r), &principal)
        .await
        .map(Json)
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
    perform(&state, &headers, Step::SendCopy(s), &principal)
        .await
        .map(Json)
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

    #[test]
    fn the_forms_name_only_visible_records() {
        let mut forms = json!({ "eigen_velden": { "mine": ["a"], "theirs": ["b"] } });
        only_visible_records(&mut forms, &BTreeSet::from(["mine".to_string()]));
        assert_eq!(forms["eigen_velden"], json!({ "mine": ["a"] }));
    }
}
