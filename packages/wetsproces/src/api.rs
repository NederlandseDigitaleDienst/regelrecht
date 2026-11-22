//! The HTTP API over the [`Service`], in JSON. The keys of a request and an
//! answer are those of the kern (`velden`, `bevindingen`, `op`), as the
//! chronolexografie poc speaks them.
//!
//! Every route but `/health` and the login (`/auth/...`, [`crate::login`])
//! needs a session; without one it answers 401. The routes come in four
//! groups, each behind its own gate ([`crate::access`] holds the roles).
//!
//! The back office works for one cell, the executor
//! ([`Service::executor`]): its routes reach the executor's records only.
//! The other cells of the case are external parties; the admin panel
//! (`/api/backoffice/beheer/...`) lets an admin enter what such a party did,
//! and reaches only that party's records and only what the input needs
//! ([`crate::external`]). What it records is kept as entered through the
//! panel ([`Channel::AdminPanel`]), by the admin's account.
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
//! | `GET /api/backoffice/cellen/{cel}/kroniek` | reader; the executor only | the executor's chronicle |
//! | `GET /api/backoffice/waarom/{id}` | reader; a decision of the executor | why it came out so |
//! | `POST /api/backoffice/voorvertoning` | decider; a submission of the executor | what a decision ([`Decide`]) would come to; records nothing |
//! | `POST /api/backoffice/vastleggingen` | handler; for the executor | a fact the executor records ([`Record`]) |
//! | `POST /api/backoffice/toezendingen` | handler; a decision of the executor | a copy of a decision ([`SendCopy`]) |
//! | `POST /api/backoffice/besluiten` | decider; a submission of the executor | a decision ([`Decide`]) |
//! | `PUT /api/backoffice/klok` | admin | set the clock, test mode only: `{ "datum": "YYYY-MM-DD" }` |
//! | `POST /api/backoffice/testomgeving/leegmaken` | admin; only in test mode, absent otherwise | empty every chronicle, the kept answers and the test clock ([`reset_test_environment`]) |
//! | `GET /api/backoffice/beheer/partijen` | admin | per external party what waits for it ([`crate::service::ExternalParty`]) |
//! | `POST /api/backoffice/beheer/vastleggingen` | admin | a fact an external party records |
//! | `POST /api/backoffice/beheer/voorvertoning` | admin; a submission of an external party | what its decision would come to; records nothing |
//! | `POST /api/backoffice/beheer/besluiten` | admin; a submission of an external party | its decision |
//! | `POST /api/backoffice/beheer/toezendingen` | admin; a decision of an external party | a copy of it to the cell it concerns |
//!
//! A citizen acts only for themself. The login field of their application
//! comes from the claim of their account, never from the request, and a
//! submission that refers to a record must refer to one in their own
//! dossier. A record an employee names by id must be in the executor's
//! chronicle, and one the admin panel names in an external party's; one
//! that is not answers 404, the same as one that does not exist, so an id
//! does not tell whether it exists elsewhere.
//!
//! The citizen's routes are offered a second time, under `/fsc4b`, to apps
//! of the FSC voor Burgers channel ([`crate::fsc4b`]), when that channel is
//! configured; the handlers below serve both.
//!
//! Which groups exist follows from the instance's surface
//! ([`crate::surface::Surface`]): an instance for citizens registers the
//! shared routes, the citizen's and those of FSC voor Burgers; one for the
//! back office the shared routes and those of the back office. A route of
//! the other side is not there, so it answers 404 like any unknown path.
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
use regelrecht_poc_chronolexografie::kern::Indiener;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::access::{self, Citizen, Level, Principal};
use crate::clock::ControllableClock;
use crate::config::Mode;
use crate::fsc4b::Fsc4b;
use crate::login::{Login, OidcLogin};
use crate::service::{Decide, Record, SendCopy, Service, ServiceError, Step, Submit};
use crate::store::{Actor, Channel, IdempotencyKey, StoreError};
use crate::surface::Surface;

/// The header that carries a request's [`IdempotencyKey`].
pub const IDEMPOTENCY_KEY: &str = "idempotency-key";

/// What every handler shares.
#[derive(Debug, Clone)]
pub struct AppState {
    service: Arc<Service>,
    mode: Mode,
    /// The side of the process this instance serves.
    surface: Surface,
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
        surface: Surface,
        clock: Option<Arc<ControllableClock>>,
        login: Login,
        subject_claim: &str,
    ) -> Self {
        Self {
            service,
            mode,
            surface,
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

    /// The side of the process this instance serves.
    pub fn surface(&self) -> Surface {
        self.surface
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
            | ServiceError::LoginField(_)
            | ServiceError::Prefill(_) => {
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

/// The routes of the API on `state`, with the login's routes, for the
/// surface of `state`; the session layer goes around them
/// ([`crate::app`]).
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
    let mut backoffice = Router::new()
        .route("/api/backoffice/cellen/{cel}/kroniek", get(chronicle))
        .route("/api/backoffice/besluiten", post(decide))
        .route("/api/backoffice/voorvertoning", post(preview))
        .route("/api/backoffice/waarom/{id}", get(backoffice_explain))
        .route("/api/backoffice/vastleggingen", post(record))
        .route("/api/backoffice/toezendingen", post(send_copy))
        .route("/api/backoffice/klok", put(set_clock));
    // Emptying the setup exists only where the clock can be set: in test
    // mode. In stable mode the route is not there at all.
    if state.clock.is_some() {
        backoffice = backoffice.route(
            "/api/backoffice/testomgeving/leegmaken",
            post(reset_test_environment),
        );
    }
    let backoffice =
        backoffice.route_layer(middleware::from_fn_with_state(state.clone(), gate_employee));
    let admin_panel = Router::new()
        .route("/api/backoffice/beheer/partijen", get(external_parties))
        .route(
            "/api/backoffice/beheer/vastleggingen",
            post(record_external),
        )
        .route(
            "/api/backoffice/beheer/voorvertoning",
            post(preview_external),
        )
        .route("/api/backoffice/beheer/besluiten", post(decide_external))
        .route(
            "/api/backoffice/beheer/toezendingen",
            post(send_copy_external),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_admin));
    let fsc4b = state
        .fsc4b
        .as_ref()
        .map_or_else(Router::new, |f| crate::fsc4b::routes(f));
    let mut routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(state.login.routes())
        .merge(shared);
    if state.surface.serves_citizens() {
        routes = routes.merge(citizen).merge(fsc4b);
    }
    if state.surface.serves_employees() {
        routes = routes.merge(backoffice).merge(admin_panel);
    }
    routes.with_state(state)
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

/// An employee: a reader or more; the handler checks the level it needs
/// and that the records are the executor's.
async fn gate_employee(
    principal: Principal,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    gate(principal, |p| p.has(Level::Reader), request, next).await
}

/// An admin, for the admin panel.
async fn gate_admin(
    principal: Principal,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    gate(principal, |p| p.has(Level::Admin), request, next).await
}

async fn case(State(state): State<AppState>) -> ApiResult {
    case_of(&state).await.map(Json)
}

/// The case, and today.
pub(crate) async fn case_of(state: &AppState) -> CitizenResult {
    Ok(state.service.case().await?)
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
    if let Ok(citizen) = principal.citizen(&state.subject_claim) {
        let preview = state
            .service
            .system_preview(submitter(state, principal, &citizen))
            .await;
        with_system_values(&mut forms, state.service.application_article(), preview);
    }
    Ok(forms)
}

/// Put in the application form of `forms` (under `article`) what the system
/// fills for this citizen: per field it fills, its value (`waarde`). What
/// the form shows is then what the submission records, as long as the day
/// does not change in between. A preview that fails leaves the values out
/// and says why (`fout`); the form still loads, and the submission fills
/// the fields anew or refuses with the reason.
fn with_system_values(
    forms: &mut Value,
    article: &str,
    preview: Result<Map<String, Value>, ServiceError>,
) {
    let Some(application) = forms
        .get_mut("indieningen")
        .and_then(|i| i.get_mut(article))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let values = match preview {
        Ok(values) => values,
        Err(e) => {
            tracing::warn!(error = %e, "the preview of what the system fills failed");
            application.insert("systeem_fout".into(), Value::from(e.to_string()));
            return;
        }
    };
    let Some(system) = application
        .get_mut("systeem")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for (field, value) in values {
        if let Some(fill) = system.get_mut(&field).and_then(Value::as_object_mut) {
            fill.insert("waarde".into(), value);
        }
    }
}

/// The ids of the records `principal` may see: those of their dossier as a
/// citizen, and those in the executor's chronicle as an employee. Not those
/// of an external party: the executor sees what such a party sends it, which
/// lands in its own chronicle.
async fn visible_records(
    state: &AppState,
    principal: &Principal,
) -> Result<BTreeSet<String>, ApiError> {
    let reader = principal.has(Level::Reader);
    let executor = state.service.executor();
    let mut ids = state
        .service
        .record_ids(|cell| reader && cell == executor)
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
/// ladder, and for an employee what that level lets them do for the
/// executor (`cellen`, which holds the executor only), and whether they may
/// enter the acts of external parties in the admin panel. The screen hides
/// what the account may not do; the routes still check every request
/// themselves.
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
    let service = &state.service;
    let cells: Vec<Value> = principal
        .has(Level::Reader)
        .then(|| {
            json!({
                "cel": service.executor(),
                "cel_naam": service.executor_name(),
                "lezen": true,
                "behandelen": principal.has(Level::Handler),
                "beslissen": principal.has(Level::Decider),
            })
        })
        .into_iter()
        .collect();
    Ok(Json(json!({
        "sub": principal.sub(),
        "naam": name,
        "burger": principal.is_citizen(),
        "identiteit": citizen.as_ref().map(access::Citizen::subject),
        "niveau": principal.level().map(Level::name),
        "cellen": cells,
        "partijen_invoeren": principal.has(Level::Admin),
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

/// Why a decision of the executor came out so.
async fn backoffice_explain(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult {
    executor_record(&state, &id, "decision").await?;
    Ok(Json(state.service.explain(&id).await?))
}

/// What a decision would come to, for a decider of the executor, as
/// [`decide`] checks it; nothing is recorded.
async fn preview(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(d): Json<Decide>,
) -> ApiResult {
    executor_decision(&state, &d).await?;
    principal.require(Level::Decider)?;
    Ok(Json(state.service.preview(d).await?))
}

/// The executor's chronicle. Another cell of the case is refused, as an
/// external party whose chronicle no employee reads; a cell the case does
/// not have answers 404.
async fn chronicle(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(cel): Path<String>,
) -> ApiResult {
    if state.service.is_external(&cel) {
        tracing::warn!(sub = %principal.sub(), cell = %cel, "refused an external party's chronicle");
        return Err(access::forbidden());
    }
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
    actor: &Actor,
) -> CitizenResult {
    let key = key(headers)?;
    Ok(state.service.perform(key, step, actor).await?)
}

/// Allow when the executor's chronicle holds the record `id`; `what` names
/// the record in the 404 otherwise. Every route that reaches it lets a
/// reader or more through, so a holder that is the executor is readable.
async fn executor_record(state: &AppState, id: &str, what: &str) -> Result<(), ApiError> {
    match state.service.holder(id).await? {
        Some(cell) if cell == state.service.executor() => Ok(()),
        _ => Err(ApiError::not_yours(what)),
    }
}

/// Allow a decision `d` for the executor: on a submission in its
/// chronicle, after a record in it if one is named.
async fn executor_decision(state: &AppState, d: &Decide) -> Result<(), ApiError> {
    executor_record(state, &d.submission, "submission").await?;
    if let Some(after) = &d.after {
        executor_record(state, after, "record").await?;
    }
    Ok(())
}

/// The external party whose chronicle holds the record `id`; `what` names
/// the record in the 404 otherwise, also when the executor holds it.
async fn external_holder(state: &AppState, id: &str, what: &str) -> Result<String, ApiError> {
    match state.service.holder(id).await? {
        Some(cell) if state.service.is_external(&cell) => Ok(cell),
        _ => Err(ApiError::not_yours(what)),
    }
}

/// Allow a decision `d` for an external party: on a submission in its
/// chronicle, after a record in that same chronicle if one is named.
async fn external_decision(state: &AppState, d: &Decide) -> Result<(), ApiError> {
    let cell = external_holder(state, &d.submission, "submission of an external party").await?;
    if let Some(after) = &d.after
        && external_holder(state, after, "record").await? != cell
    {
        return Err(ApiError::not_yours("record"));
    }
    Ok(())
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
    let application = s
        .article
        .as_deref()
        .is_none_or(|a| a == state.service.application_article());
    if application
        && let Some(system) = state
            .service
            .system_fields()
            .find(|f| s.fields.contains_key(*f))
    {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("'{system}' is filled by the system, not from the request"),
        ));
    }
    if let Some(id) = &s.refers_to
        && !state.service.dossier(citizen.subject()).await?.holds(id)
    {
        return Err(ApiError::not_yours("record in your dossier"));
    }
    if application {
        // The login field holds the citizen whatever the UI spec says of it:
        // it is what picks out the dossier.
        s.fields
            .insert(field.to_string(), Value::from(citizen.subject()));
        s.submitter = Some(submitter(state, principal, &citizen));
    } else if s.refers_to.is_none() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "a submission other than the application refers to a record of your dossier (`op`)",
        ));
    }
    perform(state, headers, Step::Submit(s), &principal.actor()).await
}

/// What the server knows of the citizen `principal` who submits an
/// application ([`login_claims`]), and the means of login, which follows from
/// the channel ([`access::login_method`]).
fn submitter(state: &AppState, principal: &Principal, citizen: &Citizen) -> Indiener {
    Indiener {
        inlog: login_claims(principal, state.service.login_field(), citizen.subject()),
        inlogmiddel: access::login_method(principal.channel()).map(str::to_string),
    }
}

/// The claims of `principal`'s login by name, for a field the UI spec fills
/// with `login: <claim>`: every claim the login kept, `naam` for the name it
/// gave, and the login field `login_field` with the citizen's `subject`.
fn login_claims(principal: &Principal, login_field: &str, subject: &str) -> Map<String, Value> {
    let mut inlog: Map<String, Value> = principal
        .claims()
        .iter()
        .map(|(claim, value)| (claim.clone(), value.clone()))
        .collect();
    if let Some(name) = principal.name() {
        inlog.insert("naam".into(), Value::from(name));
    }
    inlog.insert(login_field.to_string(), Value::from(subject));
    inlog
}

/// A decision, by a decider of the executor.
async fn decide(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(d): Json<Decide>,
) -> ApiResult {
    executor_decision(&state, &d).await?;
    principal.require(Level::Decider)?;
    perform(&state, &headers, Step::Decide(d), &principal.actor())
        .await
        .map(Json)
}

/// A fact the executor records, by a handler.
async fn record(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(r): Json<Record>,
) -> ApiResult {
    // The kern resolves `verwijst_naar` only within the chronicle of
    // `r.cell` (a record id or a kenmerk of its beginning), which is the
    // executor's here, so it needs no check of its own.
    if r.cell != state.service.executor() {
        tracing::warn!(sub = %principal.sub(), cell = %r.cell, "refused a fact for another cell");
        return Err(access::forbidden());
    }
    principal.require(Level::Handler)?;
    perform(&state, &headers, Step::Record(r), &principal.actor())
        .await
        .map(Json)
}

/// A copy of a decision of the executor, by a handler.
async fn send_copy(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(s): Json<SendCopy>,
) -> ApiResult {
    executor_record(&state, &s.decision, "decision").await?;
    principal.require(Level::Handler)?;
    perform(&state, &headers, Step::SendCopy(s), &principal.actor())
        .await
        .map(Json)
}

/// Per external party what the admin panel needs: the submissions that
/// wait for its decision (`open`) and its decisions of which no copy was
/// received yet (`toe_te_zenden`). The facts it records and the decision
/// forms come with the forms (`GET /api/formulier`).
async fn external_parties(State(state): State<AppState>) -> ApiResult {
    let parties: Vec<Value> = state
        .service
        .external_parties()
        .await?
        .into_iter()
        .map(|p| {
            json!({
                "cel": p.cell,
                "cel_naam": p.name,
                "open": p.open,
                "toe_te_zenden": p.unsent,
            })
        })
        .collect();
    Ok(Json(json!({
        "uitvoerder": {
            "cel": state.service.executor(),
            "cel_naam": state.service.executor_name(),
        },
        "partijen": parties,
    })))
}

/// A fact an external party records, entered by an admin.
async fn record_external(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(r): Json<Record>,
) -> ApiResult {
    // As for the executor, the kern resolves `verwijst_naar` within the
    // party's own chronicle only.
    if !state.service.is_external(&r.cell) {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("'{}' is no external party of this process", r.cell),
        ));
    }
    perform(
        &state,
        &headers,
        Step::Record(r),
        &principal.admin_panel_actor(),
    )
    .await
    .map(Json)
}

/// What an external party's decision would come to; nothing is recorded.
async fn preview_external(State(state): State<AppState>, Json(d): Json<Decide>) -> ApiResult {
    external_decision(&state, &d).await?;
    Ok(Json(state.service.preview(d).await?))
}

/// An external party's decision on a submission in its chronicle, entered by
/// an admin.
async fn decide_external(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(d): Json<Decide>,
) -> ApiResult {
    external_decision(&state, &d).await?;
    perform(
        &state,
        &headers,
        Step::Decide(d),
        &principal.admin_panel_actor(),
    )
    .await
    .map(Json)
}

/// A copy of an external party's decision to the cell it concerns, entered
/// by an admin.
async fn send_copy_external(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(s): Json<SendCopy>,
) -> ApiResult {
    external_holder(&state, &s.decision, "decision of an external party").await?;
    perform(
        &state,
        &headers,
        Step::SendCopy(s),
        &principal.admin_panel_actor(),
    )
    .await
    .map(Json)
}

async fn clock_state(state: &AppState) -> ApiResult {
    Ok(Json(json!({
        "datum": state.service.today().await?,
        "modus": state.mode.to_string(),
        "bestuurbaar": state.clock.is_some(),
    })))
}

async fn clock(State(state): State<AppState>) -> ApiResult {
    clock_state(&state).await
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
    clock
        .set(body.datum)
        .await
        .map_err(|e| ApiError::from(ServiceError::from(e)))?;
    tracing::info!(sub = %principal.sub(), day = %body.datum, "clock set");
    clock_state(&state).await
}

/// `POST /api/backoffice/testomgeving/leegmaken`: empty the test setup, for
/// the admin, only in test mode. Every chronicle goes, with what the admin
/// panel entered for external parties, the answers kept under idempotency
/// keys and the test clock, which stands on the case's start again. The
/// sessions stay, so the admin is not logged out and a citizen still in
/// sees an empty dossier. The log keeps who did it.
async fn reset_test_environment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> ApiResult {
    if !principal.has(Level::Admin) {
        return Err(access::forbidden());
    }
    // Registered only with a settable clock; checked again, so a stable
    // instance can never empty a chronicle.
    if state.clock.is_none() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "there is no such route",
        ));
    }
    state.service.reset().await?;
    tracing::warn!(sub = %principal.sub(), "the test environment was emptied");
    clock_state(&state).await
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

    #[test]
    fn a_field_filled_from_the_login_can_take_any_claim_the_login_kept() {
        use std::collections::{BTreeMap, BTreeSet};
        let principal = Principal::new(
            "burger-1".into(),
            BTreeSet::from(["wetsproces-burger".to_string()]),
            BTreeMap::from([
                ("bsn".to_string(), json!("999990032")),
                ("telefoon".to_string(), json!("0600000000")),
            ]),
            Channel::TestLogin,
        )
        .with_name(Some("Anna Voorbeeld".into()));
        let inlog = login_claims(&principal, "bsn_aanvrager", "999990032");
        assert_eq!(inlog["telefoon"], json!("0600000000"));
        assert_eq!(inlog["naam"], json!("Anna Voorbeeld"));
        assert_eq!(inlog["bsn_aanvrager"], json!("999990032"));
    }
}
