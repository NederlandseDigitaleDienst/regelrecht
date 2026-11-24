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
//! | `GET /api/uiterlijk` | anyone | the accent colour, and the organisation the process belongs to (`organisatie: { naam, url }`, or `null`) |
//! | `GET /api/casus` | citizen or employee | the case, and today |
//! | `GET /api/formulier` | citizen or employee | the forms, with what the UI spec says of their fields |
//! | `GET /api/klok` | citizen or employee | today, and whether the clock can be set |
//! | `GET /api/ik` | citizen or employee | who is logged in, and what they may do ([`me`]) |
//! | `GET /api/dossier` | citizen | the records about the citizen, in every cell, with what the kern says of them (`stand`) and how far the acts after each of their grants are |
//! | `GET /api/dossier/waarom/{id}` | citizen | why a decision of their dossier came out so |
//! | `GET /api/dossier/tijdlijn/{id}` | citizen; a record of their dossier | the timeline of its application, within the dossier ([`crate::timeline`]) |
//! | `POST /api/indieningen` | citizen | a submission ([`Submit`]) for themself |
//! | `GET /api/backoffice/cellen/{cel}/kroniek` | reader; the executor only | the executor's chronicle, with what the kern says of it (`stand`) |
//! | `GET /api/backoffice/werkvoorraad` | reader | the executor's submissions that wait for a decision ([`crate::service::WorkQueue`]) |
//! | `GET /api/backoffice/waarom/{id}` | reader; a decision of the executor | why it came out so |
//! | `GET /api/backoffice/tijdlijn/{id}` | reader; a record the executor sees | the timeline of its application: the executor's records ([`crate::timeline`]) |
//! | `POST /api/backoffice/voorvertoning` | decider; a submission of the executor | what a decision ([`Decide`]) would come to; records nothing |
//! | `GET /api/backoffice/verzoeken` | reader | the requests the executor's policy made, and whether an answer came back ([`requests`]) |
//! | `POST /api/backoffice/vastleggingen` | decider; for the executor | a fact the executor records ([`Record`]) |
//! | `GET /api/backoffice/vervolg` | reader | every grant and how far the acts after it are ([`grants`]) |
//! | `POST /api/backoffice/toezendingen` | decider; a decision of the executor | a copy of a decision ([`SendCopy`]) |
//! | `POST /api/backoffice/besluiten` | decider; a submission of the executor | a decision ([`Decide`]) |
//! | `PUT /api/backoffice/klok` | admin | set the clock, test mode only: `{ "datum": "YYYY-MM-DD" }` |
//! | `POST /api/backoffice/testomgeving/leegmaken` | admin; only in test mode, absent otherwise | empty every chronicle, the kept answers and the test clock ([`reset_test_environment`]) |
//! | `GET /api/backoffice/beheer/partijen` | admin | per external party what waits for it ([`crate::service::ExternalParty`]) |
//! | `POST /api/backoffice/beheer/vastleggingen` | admin | a fact an external party records; with `verzoek`, the answer to that open request |
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
use crate::link::Link;
use crate::login::{Host, Login};
use crate::organisatie::statement::Acceptor;
use crate::service::{Decide, Record, SendCopy, Service, ServiceError, Step, Submit};
use crate::store::{Actor, IdempotencyKey, StoreError};
use crate::surface::Surface;
use crate::theme::AccentColor;
use crate::visibility;

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
    /// The accent colour of the screens; none keeps the design system's.
    accent: Option<AccentColor>,
    /// The link with the servers of other cells, when one is configured.
    link: Option<Arc<Link>>,
    /// The organisation the process belongs to, when it belongs to one.
    organisation: Option<Organisation>,
}

/// The organisation a process belongs to: what its screens show of it, and
/// the check of its statements.
#[derive(Debug, Clone)]
pub struct Organisation {
    /// Its name.
    pub name: String,
    /// Its front on this instance's side, for the link back.
    pub url: Option<String>,
    pub acceptor: Arc<Acceptor>,
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
            accent: None,
            link: None,
            organisation: None,
        }
    }

    /// The state of a process that belongs to `organisation`.
    pub fn with_organisation(mut self, organisation: Organisation) -> Self {
        self.organisation = Some(organisation);
        self
    }

    /// The state with the screens in the accent colour `accent`.
    pub fn with_accent(mut self, accent: Option<AccentColor>) -> Self {
        self.accent = accent;
        self
    }

    /// The state with the FSC voor Burgers channel `fsc4b` open.
    pub fn with_fsc4b(mut self, fsc4b: Arc<Fsc4b>) -> Self {
        self.fsc4b = Some(fsc4b);
        self
    }

    /// The state with the link `link` to the servers of other cells.
    pub fn with_link(mut self, link: Arc<Link>) -> Self {
        self.link = Some(link);
        self
    }

    /// The link with the servers of other cells, when one is configured.
    pub fn link(&self) -> Option<&Arc<Link>> {
        self.link.as_ref()
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

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    /// The process this server runs.
    pub fn service(&self) -> &Service {
        &self.service
    }
}

impl Host for AppState {
    fn surface(&self) -> Surface {
        self.surface
    }
    fn subject_claim(&self) -> &str {
        &self.subject_claim
    }
    fn login(&self) -> &Login {
        &self.login
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
    fn acceptor(&self) -> Option<&Acceptor> {
        self.organisation.as_ref().map(|o| o.acceptor.as_ref())
    }
    async fn test_personas(&self) -> Vec<Value> {
        crate::login::process_personas(self)
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
            ServiceError::Store(StoreError::Conflict { .. } | StoreError::Reset { .. }) => {
                StatusCode::CONFLICT
            }
            ServiceError::Store(
                StoreError::Backend(_)
                | StoreError::UnknownFormat { .. }
                | StoreError::DuplicateGram { .. },
            )
            | ServiceError::Internal(_)
            | ServiceError::LoginField(_)
            | ServiceError::Prefill(_)
            | ServiceError::LoginClaim(_) => {
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
        .route("/api/dossier/tijdlijn/{id}", get(dossier_timeline))
        .route("/api/indieningen", post(submit))
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_citizen));
    let mut backoffice = Router::new()
        .route("/api/backoffice/cellen/{cel}/kroniek", get(chronicle))
        .route("/api/backoffice/werkvoorraad", get(work_queue))
        .route("/api/backoffice/besluiten", post(decide))
        .route("/api/backoffice/voorvertoning", post(preview))
        .route("/api/backoffice/waarom/{id}", get(backoffice_explain))
        .route("/api/backoffice/tijdlijn/{id}", get(backoffice_timeline))
        .route("/api/backoffice/verzoeken", get(requests))
        .route("/api/backoffice/vastleggingen", post(record))
        .route("/api/backoffice/vervolg", get(grants))
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
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_entering));
    // The server of an external party: its staff enter what the party does,
    // and nothing of the executor's back office exists. The clock and the
    // reset stay, for an admin in test mode.
    let mut party_tools = Router::new().route("/api/backoffice/klok", put(set_clock));
    if state.clock.is_some() {
        party_tools = party_tools.route(
            "/api/backoffice/testomgeving/leegmaken",
            post(reset_test_environment),
        );
    }
    let party_tools =
        party_tools.route_layer(middleware::from_fn_with_state(state.clone(), gate_employee));
    let fsc4b = state
        .fsc4b
        .as_ref()
        .map_or_else(Router::new, |f| crate::fsc4b::routes(f));
    let mut routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        // Before any login: the login screen wears the accent too.
        .route("/api/uiterlijk", get(appearance))
        .merge(state.login.routes(state.surface))
        .merge(shared);
    if state.organisation.is_some() {
        routes = routes.route("/auth/organisatie", get(crate::login::organisation_handoff));
    }
    if state.surface.serves_citizens() {
        routes = routes.merge(citizen).merge(fsc4b);
    }
    if state.surface.serves_employees() {
        routes = if state.service.party().is_some() {
            routes.merge(party_tools).merge(admin_panel)
        } else {
            routes.merge(backoffice).merge(admin_panel)
        };
    }
    if state.surface.serves_employees() {
        routes = routes.route(crate::link::PARTIES_PATH, get(crate::link::parties));
    }
    if state.link.is_some() {
        routes = routes.merge(Link::routes());
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

/// Who enters what an external party does: an admin, in the admin panel of
/// the executor's server; on the server of the party itself its own staff,
/// from the employee who decides upward.
async fn gate_entering(
    State(state): State<AppState>,
    principal: Principal,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let allowed: fn(&Principal) -> bool = if state.service.party().is_some() {
        |p| p.has(Level::Decider)
    } else {
        |p| p.has(Level::Admin)
    };
    gate(principal, allowed, request, next).await
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
        // Every submission whose form has fields the system fills, such as
        // the case's application and an application for a duplicate.
        let articles: Vec<String> = forms["indieningen"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, f)| f["systeem"].as_object().is_some_and(|s| !s.is_empty()))
            .map(|(article, _)| article.clone())
            .collect();
        for article in articles {
            let preview = state
                .service
                .system_preview(article.clone(), submitter(state, principal, &citizen))
                .await;
            with_system_values(&mut forms, &article, preview);
        }
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
    let (dossier, stand) = state.service.dossier_with_stand(citizen.subject()).await?;
    // How far the acts after a grant are, for the grants in this dossier only.
    // It plays the aggregate over every grant and keeps this citizen's; when
    // that fails, the dossier still loads, without it.
    let standen: Vec<_> = match state.service.standen().await {
        Ok(standen) => standen
            .into_iter()
            // Both records a row names must be the citizen's: the grant and
            // the decision whose aggregate it is.
            .filter(|s| dossier.holds(&s.toekenning) && dossier.holds(&s.besluit))
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "how far the acts after a grant are does not play");
            Vec::new()
        }
    };
    // Which of its records the system made by an article of the policy.
    let door_het_systeem = door_het_systeem(state, |id| dossier.holds(id)).await?;
    let verzoeken = requests_in(state, &dossier).await;
    Ok(json!({
        "grammen": dossier.grams,
        "stand": stand,
        "vervolg": standen,
        "door_het_systeem": door_het_systeem,
        "verzoeken": verzoeken,
    }))
}

/// The requests of the executor's policy in `dossier`, each by the ids of
/// its records that are in it: the request (`verzoek`), its receipt by the
/// other party (`ontvangen`), the answer (`antwoord`) and the executor's
/// receipt of that (`antwoord_ontvangen`). Only ids, and only of records in
/// the dossier, so a screen can hang each under its request and learns
/// nothing else. When the requests do not read, the dossier loads without.
async fn requests_in(state: &AppState, dossier: &crate::service::Dossier) -> Vec<Value> {
    let requests = match state.service.requests().await {
        Ok(requests) => requests,
        Err(e) => {
            tracing::warn!(error = %e, "the requests of the policy do not read");
            return Vec::new();
        }
    };
    let held = |g: Option<&regelrecht_cel::Gram>| {
        g.map(|g| g.id.clone())
            .filter(|id| dossier.holds(id))
            .map_or(Value::Null, Value::String)
    };
    requests
        .iter()
        .filter(|r| dossier.holds(&r.verzoek.id))
        .map(|r| {
            json!({
                "verzoek": r.verzoek.id,
                "ontvangen": held(r.ontvangen.as_ref()),
                "antwoord": held(r.antwoord.as_ref()),
                "antwoord_ontvangen": held(r.antwoord_ontvangen.as_ref()),
            })
        })
        .collect()
}

/// Per record for which `include` holds that the system made by an article
/// of the executor's policy, without a person, that article
/// (`Service::by_policy`).
async fn door_het_systeem(
    state: &AppState,
    include: impl Fn(&str) -> bool,
) -> Result<Map<String, Value>, ApiError> {
    Ok(state
        .service
        .by_policy()
        .await?
        .into_iter()
        .filter(|(id, _)| include(id))
        .map(|(id, artikel)| (id, Value::String(artikel)))
        .collect())
}

/// Every grant of the executor, with how far the acts that follow it are,
/// for any employee: what a reader may see of the executor's chronicle.
/// Each carries the day of the record that granted (`toegekend_op`) and
/// whom it concerns (`aanvrager`, the login field of the decision, and
/// `naam`, the applicant's name from the application when the UI spec has a
/// field for it), so a screen can name it without reading the chronicle.
/// Employees only: the route is the back office's.
async fn grants(State(state): State<AppState>) -> ApiResult {
    let service = &state.service;
    let standen = service.standen().await?;
    let kroniek = service.chronicle(service.executor()).await?;
    let grams = kroniek["grammen"].as_array().cloned().unwrap_or_default();
    let record = |id: &str| grams.iter().find(|g| g["id"] == id);
    let mut vervolg = Vec::with_capacity(standen.len());
    for stand in standen {
        let toegekend_op = record(&stand.toekenning).map(|g| g["recorded_on"].clone());
        let besluit = record(&stand.besluit);
        let aanvrager = besluit
            .and_then(|g| g["fields"].get(service.login_field()))
            .cloned();
        // The name as the application recorded it, from the login; on the
        // decision too when it took the application's fields along.
        let aanvraag = besluit
            .and_then(|g| g["refers_to"].as_str())
            .and_then(&record);
        let naam = service.name_field().and_then(|veld| {
            [besluit, aanvraag]
                .into_iter()
                .flatten()
                .find_map(|g| g["fields"].get(veld).filter(|v| v.is_string()))
                .cloned()
        });
        let mut rij = serde_json::to_value(&stand)
            .map_err(|e| ApiError::from(ServiceError::Internal(e.to_string())))?;
        rij["toegekend_op"] = toegekend_op.unwrap_or(Value::Null);
        rij["aanvrager"] = aanvrager.unwrap_or(Value::Null);
        rij["naam"] = naam.unwrap_or(Value::Null);
        vervolg.push(rij);
    }
    Ok(Json(json!({ "vervolg": vervolg })))
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
) -> ApiResult {
    // The name is only shown; a login or a statement that does not give it
    // leaves it out.
    let name = principal.name();
    let citizen = principal.citizen(&state.subject_claim).ok();
    let service = &state.service;
    // On the server of an external party nobody reads or decides for the
    // executor: its staff enter what their own party does.
    let cells: Vec<Value> = (principal.has(Level::Reader) && service.party().is_none())
        .then(|| {
            json!({
                "cel": service.executor(),
                "cel_naam": service.executor_name(),
                "lezen": true,
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
        "partijen_invoeren": match service.party() {
            Some(_) => principal.has(Level::Decider),
            None => principal.has(Level::Admin),
        },
        "partij": service.party().map(|cel| json!({
            "cel": cel,
            "cel_naam": service.cell_name(cel),
        })),
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
    let dossier = state.service.dossier(citizen.subject()).await?;
    if !dossier.holds(id) {
        return Err(ApiError::not_yours("decision in your dossier"));
    }
    // A citizen works for no cell: no cell's reading of its chronicle, and
    // no record outside their dossier.
    let visible = dossier.grams.into_iter().map(|g| g.id).collect();
    Ok(visibility::for_reader(
        state.service.explain(id).await?,
        None,
        &visible,
    ))
}

/// The timeline of the application a record of the citizen's own dossier
/// belongs to ([`crate::timeline`]): only records of that dossier, and no
/// next step that names another record.
async fn dossier_timeline(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
) -> ApiResult {
    timeline_for(&state, &principal, &id).await.map(Json)
}

/// [`dossier_timeline`] for the citizen `principal`.
pub(crate) async fn timeline_for(
    state: &AppState,
    principal: &Principal,
    id: &str,
) -> CitizenResult {
    let citizen = principal.citizen(&state.subject_claim)?;
    let dossier = state.service.dossier(citizen.subject()).await?;
    if !dossier.holds(id) {
        return Err(ApiError::not_yours("record in your dossier"));
    }
    let timeline = state
        .service
        .timeline(id)
        .await?
        .ok_or_else(|| ApiError::not_yours("record in your dossier"))?
        .within(|x| dossier.holds(x), |x| dossier.holds(x));
    to_value(&timeline)
}

/// The timeline of the application a record the executor's employees may
/// see belongs to ([`crate::timeline`]): only records of the executor, and
/// of another cell no more than the id of what reached the executor, as for
/// why a decision came out so.
async fn backoffice_timeline(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let executor = state.service.executor();
    let own = state.service.record_ids(|cell| cell == executor).await?;
    let visible = state.service.seen_by_executor().await?;
    if !visible.contains(&id) {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "no record of the executor with this id",
        ));
    }
    let timeline = state
        .service
        .timeline(&id)
        .await?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "no record with this id"))?
        .within(|x| own.contains(x), |x| visible.contains(x));
    Ok(Json(to_value(&timeline)?))
}

/// `value` as JSON.
fn to_value(value: &impl serde::Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|e| ServiceError::Internal(e.to_string()).into())
}

/// `value`, which explains a decision, as an employee of the executor may
/// see it ([`visibility::for_reader`]): nothing of another cell's own chronicle
/// but what reached the executor.
async fn for_executor(state: &AppState, value: Value) -> Result<Value, ApiError> {
    let visible = state.service.seen_by_executor().await?;
    Ok(visibility::for_reader(
        value,
        Some(state.service.executor()),
        &visible,
    ))
}

/// Why a decision of the executor came out so.
async fn backoffice_explain(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult {
    executor_record(&state, &id, "decision").await?;
    let waarom = state.service.explain(&id).await?;
    Ok(Json(for_executor(&state, waarom).await?))
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
    let preview = state.service.preview(d).await?;
    Ok(Json(for_executor(&state, preview).await?))
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
    let mut kroniek = state.service.chronicle(&cel).await?;
    let ids: BTreeSet<String> = kroniek["grammen"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| g["id"].as_str().map(str::to_string))
        .collect();
    kroniek["door_het_systeem"] =
        Value::Object(door_het_systeem(&state, |id| ids.contains(id)).await?);
    Ok(Json(kroniek))
}

/// What waits for a decision of the executor: per submission the record the
/// decision is to be taken after, when an earlier one was revoked (`na`).
async fn work_queue(State(state): State<AppState>) -> ApiResult {
    let queue = state.service.work_queue().await?;
    Ok(Json(json!({
        "cel": state.service.executor(),
        "cel_naam": state.service.executor_name(),
        "open": queue.open,
    })))
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
    let answer = state.service.perform(key, step, actor).await?;
    // The step may have made a record a linked cell is to receive.
    if let Some(link) = &state.link {
        link.wake();
    }
    Ok(answer)
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
        Some(cell) if state.service.enters_for(&cell) => Ok(cell),
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
    // The checks read the chronicles apart from the step. In stable mode
    // that is safe because a chronicle only grows and a record never changes
    // cell: what was in the dossier or a readable cell when checked still is
    // when the step runs. In test mode a reset may empty the chronicles in
    // between; the checks then held on records that are gone, and the step
    // runs on what the reset left (`Service::perform` runs it again when the
    // store was reset since its read).
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
    // A field the system fills is the system's in every kind of submission.
    if let Some(system) = state
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
    } else if s.refers_to.is_none() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "a submission other than the application refers to a record of your dossier (`op`)",
        ));
    }
    // What the system fills in the submission, the login field among them
    // where its form asks it, comes from who submits.
    s.submitter = Some(submitter(state, principal, &citizen));
    perform(state, headers, Step::Submit(s), &principal.actor()).await
}

/// What the server knows of the citizen `principal` who submits an
/// application ([`login_claims`]), and the means of login, which follows from
/// the channel ([`access::login_method`]).
fn submitter(state: &AppState, principal: &Principal, citizen: &Citizen) -> Indiener {
    Indiener {
        inlog: login_claims(principal, state.service.login_field(), citizen.subject()),
        inlogmiddel: access::login_method(principal.login_channel()).map(str::to_string),
    }
}

/// The claims of `principal`'s login by name, for a field the UI spec fills
/// with `login: <claim>`: the login field `login_field` with the citizen's
/// `subject`, and `naam` for the name the login gave. No other claim: a UI
/// spec that asks one does not start the server ([`crate::service::KEPT_CLAIMS`]), and the
/// submission, with what it holds of the login, is kept with its
/// idempotency key.
fn login_claims(principal: &Principal, login_field: &str, subject: &str) -> Map<String, Value> {
    let mut inlog = Map::new();
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
    let taken = perform(&state, &headers, Step::Decide(d), &principal.actor()).await?;
    Ok(Json(for_executor(&state, taken).await?))
}

/// A fact the executor records, by a decider.
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
    principal.require(Level::Decider)?;
    if r.request.is_some() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "the executor answers no request; an external party does, through the admin panel",
        ));
    }
    perform(&state, &headers, Step::Record(r), &principal.actor())
        .await
        .map(Json)
}

/// A copy of a decision of the executor, by a decider.
async fn send_copy(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(s): Json<SendCopy>,
) -> ApiResult {
    executor_record(&state, &s.decision, "decision").await?;
    principal.require(Level::Decider)?;
    perform(&state, &headers, Step::SendCopy(s), &principal.actor())
        .await
        .map(Json)
}

/// The requests the executor's policy made when an application came in,
/// such as the request to the GGD for an examination: per request its
/// record in the executor's chronicle, the party it went to, whether it is
/// still open, and the executor's record of the answer it received. Not the
/// party's own records: the executor reads what came back to it, not the
/// party's chronicle.
async fn requests(State(state): State<AppState>) -> ApiResult {
    let requests: Vec<Value> = state
        .service
        .requests()
        .await?
        .into_iter()
        .map(|r| {
            json!({
                "verzoek": r.verzoek,
                "aan": r.aan,
                "aan_naam": state.service.cell_name(&r.aan),
                "open": r.open(),
                "antwoord_ontvangen": r.antwoord_ontvangen,
            })
        })
        .collect();
    Ok(Json(json!({ "verzoeken": requests })))
}

/// Per external party what the admin panel needs: the submissions that
/// wait for its decision (`open`), its decisions of which no copy was
/// received yet (`toe_te_zenden`), and the requests of the executor it has
/// not answered (`verzoeken`), each with the fact that answers it. The facts
/// it records and the decision forms come with the forms
/// (`GET /api/formulier`); `antwoord_op_verzoek` names those that answer a
/// request, which a party with an open request records from it.
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
                "verzoeken": p.requests.iter().map(|r| json!({
                    "verzoek": r.received,
                    "antwoordfeit": r.answer,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(Json(json!({
        "uitvoerder": {
            "cel": state.service.executor(),
            "cel_naam": state.service.executor_name(),
        },
        "partijen": parties,
        "gekoppeld": state.service.linked_cells().map(|cel| json!({
            "cel": cel,
            "cel_naam": state.service.cell_name(cel),
        })).collect::<Vec<_>>(),
        "antwoord_op_verzoek": state.service.answer_facts(),
    })))
}

/// Who enters an external party's act, as the store keeps it: an admin
/// through the admin panel, or on the party's own server the employee
/// themself, through the login.
fn entering_actor(state: &AppState, principal: &Principal) -> Actor {
    if state.service.party().is_some() {
        principal.actor()
    } else {
        principal.admin_panel_actor()
    }
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
    if !state.service.enters_for(&r.cell) {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!(
                "'{}' is no external party of this process whose acts are entered here",
                r.cell
            ),
        ));
    }
    perform(
        &state,
        &headers,
        Step::Record(r),
        &entering_actor(&state, &principal),
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
        &entering_actor(&state, &principal),
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
        &entering_actor(&state, &principal),
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

/// How the screens look: the accent colour, a theme of the NLDD design
/// system, or `null` for the design system's own.
async fn appearance(State(state): State<AppState>) -> Json<Value> {
    let organisation = state
        .organisation
        .as_ref()
        .map(|o| json!({ "naam": o.name, "url": o.url }));
    Json(json!({
        "accentkleur": state.accent.map(AccentColor::name),
        "organisatie": organisation,
    }))
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
    fn a_field_filled_from_the_login_takes_the_identifier_or_the_name_and_nothing_else() {
        use std::collections::{BTreeMap, BTreeSet};
        let principal = Principal::new(
            "burger-1".into(),
            BTreeSet::from(["wetsproces-burger".to_string()]),
            BTreeMap::from([
                ("bsn".to_string(), json!("999990032")),
                ("telefoon".to_string(), json!("0600000000")),
            ]),
            crate::store::Channel::TestLogin,
        )
        .with_name(Some("Anna Voorbeeld".into()));
        let inlog = login_claims(&principal, "bsn_aanvrager", "999990032");
        // Only what every UI spec that starts may ask: no other claim.
        assert_eq!(
            inlog,
            serde_json::from_value::<Map<String, Value>>(json!({
                "naam": "Anna Voorbeeld",
                "bsn_aanvrager": "999990032",
            }))
            .unwrap()
        );
    }
}
