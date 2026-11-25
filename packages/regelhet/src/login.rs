//! How people log in, and the sessions that remember it.
//!
//! Two logins, one session format. With OIDC the routes of
//! `regelrecht_auth` log in through Keycloak and write the account's `sub`,
//! realm roles and the claims of [`crate::config::Config::subject_claim`]
//! into the session. In test mode without OIDC configuration the test login
//! ([`test_login`]) writes the same keys with whatever roles and claims it is
//! sent, or, for the simulated DigiD of the citizen's side, a citizen with
//! the number and name it is sent ([`SimulatedDigid`]). The rest of the
//! server reads only the session
//! ([`crate::access::Principal`]), so a request after the test login goes
//! through the same checks as one after Keycloak. The test login is a route
//! only in that case: stable mode never registers it.
//!
//! Sessions live where the chronicles live: in memory with the store in
//! memory, and in the table `wetsproces.sessions` with the store in
//! Postgres, under a cookie per surface
//! ([`crate::surface::Surface::session_cookie`]). The instance for citizens
//! and the one for the back office share that table, so each session
//! carries the surface it was made on, and an instance reads a session of
//! another surface as no session at all ([`SurfaceSessions`]).
//!
//! Each instance logs in with its own OIDC client, since each is a
//! deployment of its own with its own `BASE_URL`: the redirect after a
//! login goes back to the instance that sent the browser out.
//!
//! A process that belongs to an organisation has a third way in:
//! `GET /auth/organisatie?token=<statement>&naar=<path>` ([`organisation_handoff`]).
//! The front of the organisation logged the account in and sends the
//! browser here with a statement it signed for this process
//! ([`crate::organisatie::statement`]); the route checks it and writes a
//! session as a login does, marked as handed over ([`OrganisationMark`]), so
//! what the account records is kept as through the organisation.
//!
//! Both the process ([`AppState`]) and the front of an organisation
//! ([`crate::organisatie::FrontState`]) log in this way; [`Host`] is what
//! each gives the login and the principal of a request.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use regelrecht_auth::{
    ConfiguredClient, OidcConfig, SESSION_KEY_AUTHENTICATED, SESSION_KEY_CLAIMS, SESSION_KEY_NAME,
    SESSION_KEY_ROLES, SESSION_KEY_SUB,
};
// The made-up citizen numbers of the BSN test range, shared with the app.
use regelrecht_fsc4b::wire::TEST_CITIZENS;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_sessions::session::{Id, Record};
use tower_sessions::{Session, SessionStore, session_store};

use crate::access::{CITIZEN, Level, Principal};
use crate::api::{ApiError, AppState};
use crate::config::LoginChoice;
use crate::identity::{Identifier, Kind};
use crate::organisatie::statement::Acceptor;
use crate::service::LoginOutcome;
use crate::store::{Channel, IdempotencyKey};
use crate::surface::Surface;

/// What a server's state gives the login and the principal of a request:
/// the state of a process ([`AppState`]) or of an organisation's front.
pub trait Host: Clone + Send + Sync + 'static {
    /// The side the instance serves.
    fn surface(&self) -> Surface;
    /// The claim of a citizen's account that identifies them.
    fn subject_claim(&self) -> &str;
    /// The login the server runs.
    fn login(&self) -> &Login;
    fn http_client(&self) -> &reqwest::Client;
    /// The check of the organisation's statements, on a process that belongs
    /// to one.
    fn acceptor(&self) -> Option<&Acceptor> {
        None
    }
    /// The accounts the test login offers, as `GET /auth/testpersonen`
    /// gives them.
    fn test_personas(&self) -> impl Future<Output = Vec<Value>> + Send;
    /// The identifier of an applicant: the subject claim, and how the
    /// simulated login checks it.
    fn identifier(&self) -> Identifier {
        Identifier::for_claim(self.subject_claim())
    }
    /// Whether the case logs in through a login step of its own
    /// (`inloggen.inlogstap`): then only that step logs an applicant in.
    fn has_login_step(&self) -> bool {
        false
    }
    /// What the case's login step offers (`inlogstap::aanbod`), or `None`
    /// without one.
    fn login_offer(&self) -> impl Future<Output = Result<Option<Value>, ApiError>> + Send {
        async { Ok(None) }
    }
    /// Log in through the case's login step as `user` with `password`: the
    /// number under the case's login key and the name of whoever logged in.
    fn step_log_in(
        &self,
        user: &str,
        password: &str,
    ) -> impl Future<Output = Result<(String, String), ApiError>> + Send {
        let _ = (user, password);
        async {
            Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Deze casus heeft geen inlogstap.",
            ))
        }
    }
    /// The OIDC login, when the server logs in through Keycloak.
    fn oidc(&self) -> Option<&OidcLogin> {
        match self.login() {
            Login::Oidc(o) => Some(o),
            Login::TestLogin => None,
        }
    }
    /// The channel of a browser session: the login this server runs.
    fn session_channel(&self) -> Channel {
        match self.login() {
            Login::Oidc(_) => Channel::Keycloak,
            Login::TestLogin => Channel::TestLogin,
        }
    }
}

/// `regelrecht_auth::OidcAppState` for a [`Host`]: the trait is foreign,
/// so each state implements it, all in the same way.
macro_rules! oidc_app_state {
    ($state:ty) => {
        impl regelrecht_auth::OidcAppState for $state {
            fn oidc_client(&self) -> Option<&std::sync::Arc<regelrecht_auth::ConfiguredClient>> {
                $crate::login::Host::oidc(self).map(|o| &o.client)
            }
            fn end_session_url(&self) -> Option<&str> {
                $crate::login::Host::oidc(self).and_then(|o| o.end_session_url.as_deref())
            }
            fn oidc_config(&self) -> Option<&regelrecht_auth::OidcConfig> {
                $crate::login::Host::oidc(self).map(|o| &o.config)
            }
            fn is_auth_enabled(&self) -> bool {
                $crate::login::Host::oidc(self).is_some()
            }
            fn base_url(&self) -> Option<&str> {
                $crate::login::Host::oidc(self).and_then(|o| o.base_url.as_deref())
            }
            fn http_client(&self) -> &reqwest::Client {
                <$state as $crate::login::Host>::http_client(self)
            }
        }
    };
}
pub(crate) use oidc_app_state;

/// The key in a session's data that marks it as handed over by the
/// organisation ([`OrganisationMark`]).
pub const SESSION_KEY_ORGANISATION: &str = "regelhet.organisatie";

/// The mark of a session the organisation handed over: for which account,
/// and how the front logged it in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganisationMark {
    /// The account of the session, `<organisation id>:<sub>`; the mark
    /// counts only while the session holds that account.
    pub sub: String,
    /// `testlogin` or `keycloak`.
    pub inlog: String,
}

impl OrganisationMark {
    /// How the front logged the account in.
    pub fn login(&self) -> Channel {
        match self.inlog.as_str() {
            "keycloak" => Channel::Keycloak,
            _ => Channel::TestLogin,
        }
    }
}

/// The screen of `surface`'s build that logs in, where a failed login is
/// told: the citizen's build has its own (`/toegang`), every other build
/// `/inloggen`.
fn login_screen(surface: Surface) -> &'static str {
    match surface {
        Surface::Citizen => "/toegang",
        Surface::Backoffice | Surface::All => "/inloggen",
    }
}

/// The path of the shared OIDC routes that Keycloak sends the browser back
/// to (`regelrecht_auth::auth_routes`).
const CALLBACK: &str = "/auth/callback";

/// A failed `/auth/callback` answered with a redirect to the login screen of
/// `surface` instead of a bare status: `?fout=rol` when Keycloak logged the
/// account in but it lacks the role this server requires (a 403 without an
/// `error` from Keycloak), `?fout=inloggen` for anything else (Keycloak sent
/// an error, the state did not match, the code did not redeem). The app
/// shows a message there and does not send the browser to Keycloak again by
/// itself, so a login that keeps failing makes no loop. Every other route of
/// the login passes as it is.
async fn login_failed_to_screen(
    surface: Surface,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let callback = request.uri().path() == CALLBACK;
    let from_keycloak = request
        .uri()
        .query()
        .is_some_and(|q| q.split('&').any(|p| p.starts_with("error=")));
    let response = next.run(request).await;
    let status = response.status();
    if !callback || !(status.is_client_error() || status.is_server_error()) {
        return response;
    }
    let fout = if status == StatusCode::FORBIDDEN && !from_keycloak {
        "rol"
    } else {
        "inloggen"
    };
    tracing::warn!(%status, fout, "the login through Keycloak failed");
    Redirect::to(&format!("{}?fout={fout}", login_screen(surface))).into_response()
}

/// The key in a session's data under which it carries its surface.
pub const SESSION_KEY_SURFACE: &str = "regelhet.surface";

/// The table the sessions live in, in the schema of the store.
pub const SESSION_TABLE: &str = "sessions";

/// The login, ready to serve.
#[derive(Clone)]
pub enum Login {
    /// Through Keycloak.
    Oidc(Arc<OidcLogin>),
    /// The test login of test mode without OIDC.
    TestLogin,
}

impl std::fmt::Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oidc(o) => f.debug_tuple("Oidc").field(&o.config).finish(),
            Self::TestLogin => f.write_str("TestLogin"),
        }
    }
}

/// A discovered OIDC client and what the shared auth routes need beside it.
pub struct OidcLogin {
    pub config: OidcConfig,
    pub base_url: Option<String>,
    pub client: Arc<ConfiguredClient>,
    pub end_session_url: Option<String>,
}

impl Login {
    /// The login `choice` names; for OIDC, the client discovered from the
    /// issuer.
    ///
    /// # Errors
    ///
    /// When discovery fails.
    pub async fn discover(choice: &LoginChoice) -> Result<Self, String> {
        match choice {
            LoginChoice::TestLogin => Ok(Self::TestLogin),
            LoginChoice::Oidc { oidc, base_url } => {
                let found = regelrecht_auth::discover_client(oidc).await?;
                Ok(Self::Oidc(Arc::new(OidcLogin {
                    config: oidc.clone(),
                    base_url: base_url.clone(),
                    client: Arc::new(found.client),
                    end_session_url: found.end_session_url,
                })))
            }
        }
    }

    /// The routes under `/auth` for this login, for a server of `surface`.
    /// With OIDC a failed return from Keycloak sends the browser to the
    /// screen that logs in, with `?fout=rol` or `?fout=inloggen`, so the app
    /// says what went wrong instead of the browser showing a bare status
    /// ([`login_failed_to_screen`]).
    pub fn routes<S: Host + regelrecht_auth::OidcAppState>(&self, surface: Surface) -> Router<S> {
        match self {
            Self::Oidc(_) => regelrecht_auth::auth_routes::<S>().layer(axum::middleware::from_fn(
                move |request, next| login_failed_to_screen(surface, request, next),
            )),
            Self::TestLogin => Router::new()
                .route("/auth/testlogin", post(test_login::<S>))
                .route("/auth/testpersonen", get(test_personas::<S>))
                .route("/auth/simulatie", get(simulation::<S>))
                .route("/auth/status", get(regelrecht_auth::handlers::status::<S>))
                .route("/auth/logout", get(test_logout)),
        }
    }

    /// Whether the session cookie is marked `Secure`: always, except where
    /// the server is reached over plain HTTP, the test login on a developer's
    /// machine or an `http://` `BASE_URL`.
    pub fn secure_cookie(&self) -> bool {
        match self {
            Self::Oidc(o) => !o
                .base_url
                .as_deref()
                .is_some_and(|u| u.starts_with("http://")),
            Self::TestLogin => false,
        }
    }
}

/// What `POST /auth/testlogin` takes: a citizen by number and name, or an
/// account with whatever roles and claims. A body with both shapes' fields
/// fits neither and is refused.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TestLoginRequest {
    Simulated(SimulatedLogin),
    Step(StepLogin),
    Account(TestLogin),
}

/// A login of the simulated DigiD or eHerkenning: the applicant's number
/// (`nummer`, or `bsn` as before) and name, nothing else. Which number it is
/// follows from the case ([`Identifier`]). The server makes the account
/// itself, with the citizen's role only, so a request cannot add a role to
/// it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulatedLogin {
    #[serde(alias = "bsn")]
    nummer: String,
    naam: String,
}

/// A login through the login step of the case (`inloggen.inlogstap`), as
/// with eHerkenning: a user name the step's cell issued, and the password
/// of the simulation. The cell answers whether that user may act for the
/// service, and records the authentication when it may
/// ([`crate::service::Service::log_in`]).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepLogin {
    gebruikersnaam: String,
    wachtwoord: String,
}

/// The longest name the simulated DigiD takes, in characters.
const MAX_NAME_CHARS: usize = 100;

/// The fictional names of the citizens of the test login, in the order of
/// [`TEST_CITIZENS`].
const TEST_CITIZEN_NAMES: [&str; 2] = ["Anna Voorbeeld", "Bram Proef"];
// A citizen without a name would drop out of the list in silence.
const _: () = assert!(TEST_CITIZEN_NAMES.len() == TEST_CITIZENS.len());
/// The made-up names of the employees of the test login: the decider and the admin.
const TEST_DECIDER_NAME: &str = "Carla Fictief";
const TEST_ADMIN_NAME: &str = "Daan Verzonnen";

/// A test login: the account, its realm roles and its claims.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestLogin {
    sub: String,
    #[serde(default)]
    rollen: BTreeSet<String>,
    #[serde(default)]
    claims: BTreeMap<String, Value>,
    #[serde(default)]
    naam: Option<String>,
}

/// `POST /auth/testlogin`: a session for the account in the body, as the
/// OIDC callback would have made it, or for a citizen of the simulated
/// DigiD. There only in test mode without OIDC configuration
/// ([`Login::routes`]).
async fn test_login<S: Host>(
    State(state): State<S>,
    session: Session,
    Json(request): Json<TestLoginRequest>,
) -> Result<Json<Value>, ApiError> {
    let login = match request {
        TestLoginRequest::Account(login) => login,
        TestLoginRequest::Simulated(simulated) => simulated_citizen(&state, &simulated)?,
        TestLoginRequest::Step(step) => {
            serves_citizens(&state)?;
            let (number, name) = state
                .step_log_in(step.gebruikersnaam.trim(), &step.wachtwoord)
                .await?;
            tracing::info!(gebruikersnaam = %step.gebruikersnaam.trim(), "login through the login step");
            applicant(&state.identifier(), &number, &name)
        }
    };
    if login.sub.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "a test login needs a `sub`",
        ));
    }
    write_session(
        &session,
        &SessionAccount {
            sub: &login.sub,
            roles: &login.rollen,
            claims: &login.claims,
            name: login.naam.as_deref().unwrap_or(&login.sub),
        },
        None,
    )
    .await?;
    tracing::info!(sub = %login.sub, roles = ?login.rollen, "test login");
    Ok(Json(json!({ "sub": login.sub, "rollen": login.rollen })))
}

/// The account of an applicant of the simulated login: the citizen's role
/// and nothing else, their number under the subject claim, and their name.
/// Refused for a case with a login step: there only the step logs in, as
/// it records the authentication the case requires.
fn simulated_citizen(state: &impl Host, simulated: &SimulatedLogin) -> Result<TestLogin, ApiError> {
    serves_citizens(state)?;
    let bad = |message: &str| ApiError::new(StatusCode::BAD_REQUEST, message);
    if state.has_login_step() {
        return Err(bad(
            "In deze casus logt u in met een gebruikersnaam van de inlogstap, niet met een nummer.",
        ));
    }
    let identifier = state.identifier();
    let number = identifier.parse(&simulated.nummer).map_err(bad)?;
    let name = check_name(&simulated.naam).map_err(|m| bad(&m))?;
    Ok(applicant(&identifier, &number, name))
}

/// `Ok` on an instance that serves citizens, `404` elsewhere.
fn serves_citizens(state: &impl Host) -> Result<(), ApiError> {
    if state.surface().serves_citizens() {
        return Ok(());
    }
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "this instance serves no citizens",
    ))
}

/// The account of an applicant with `number` under the claim of
/// `identifier`, and `name`: the citizen's role and nothing else.
fn applicant(identifier: &Identifier, number: &str, name: &str) -> TestLogin {
    TestLogin {
        sub: identifier.subject(number),
        rollen: BTreeSet::from([CITIZEN.to_owned()]),
        claims: BTreeMap::from([(identifier.claim().to_owned(), Value::from(number))]),
        naam: Some(name.to_owned()),
    }
}

/// The name of the simulated login, trimmed, or a message for the
/// applicant.
fn check_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Vul een naam in.".to_owned());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "Een naam is hoogstens {MAX_NAME_CHARS} tekens lang."
        ));
    }
    // A name goes into the session and onto every screen that shows it.
    if name.chars().any(char::is_control) {
        return Err("Een naam bestaat uit letters, spaties en leestekens.".to_owned());
    }
    Ok(name)
}

/// A login through the login step of the process's case: the step's cell
/// checks the user name and the password and records the authentication
/// in a commit; its claims give the number under the case's login key
/// (`inloggen.sleutel`) and, when it asserts one, the name. The number and
/// the name of whoever logged in.
///
/// # Errors
///
/// `400` with a message for the applicant when the step refuses: a wrong
/// password, or a user the register does not authorize for the service;
/// and what the service refuses.
pub async fn process_step_log_in(
    state: &AppState,
    user: &str,
    password: &str,
    key: Option<IdempotencyKey>,
) -> Result<(String, String), ApiError> {
    let bad = |message: &str| ApiError::new(StatusCode::BAD_REQUEST, message);
    let service = state.service();
    let Some(login) = service.login().filter(|_| service.login_step().is_some()) else {
        return Err(bad("Deze casus heeft geen inlogstap."));
    };
    let claims = match service
        .log_in(user, password, key)
        .await
        .map_err(ApiError::from)?
    {
        LoginOutcome::LoggedIn(claims) => claims,
        LoginOutcome::WrongPassword => {
            return Err(bad(&format!(
                "Het wachtwoord klopt niet. In deze simulatie is het {}.",
                regelrecht_poc_chronolexografie::inlogstap::WACHTWOORD
            )));
        }
        LoginOutcome::NotAuthorized => {
            return Err(bad(
                "De erkende dienst geeft deze gebruiker geen toegang tot deze dienst.",
            ));
        }
    };
    // The number the step asserts is checked as a typed one would be: a
    // case whose register holds no valid number is a mistake in the case.
    let asserted = match claims.get(login.sleutel.as_str()) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    let Some(number) = asserted.and_then(|n| state.identifier().parse(&n).ok()) else {
        return Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the login step of the case asserts no valid number under its login key; the cause is in the case",
        ));
    };
    let name = login
        .naam
        .as_deref()
        .and_then(|n| claims.get(n).and_then(Value::as_str))
        .map(str::to_owned)
        .or_else(|| {
            service
                .begin_identities(&login.sleutel)
                .into_iter()
                .find(|(v, _)| *v == number)
                .and_then(|(_, n)| n)
        })
        .unwrap_or_else(|| user.to_owned());
    Ok((number, name))
}

/// The key in a session that says the case's login step logged the
/// citizen of a login through Keycloak in, with the number it asserted.
const SESSION_KEY_STEP_LOGIN: &str = "regelhet.inlogstap";

/// After a login through Keycloak, on a case with a login step of its own
/// (`inloggen.inlogstap`): log the account in at that step once per
/// session, as the test login does with a user name and the password of
/// the simulation, so the step's cell records the authentication the case
/// requires. The Keycloak user name (`preferred_username`) is the user name
/// at the step; the step must assert the number the token carries.
///
/// A refusal, or a number other than the token's, records nothing and is
/// noted in the session too, so it is not tried on every request: the
/// application then lacks the authentication, as it would after a real
/// refusal.
///
/// # Errors
///
/// When the session does not read or write, or the service fails.
pub async fn record_step_login(
    state: &AppState,
    session: &Session,
    principal: &Principal,
) -> Result<(), ApiError> {
    if principal.channel() != Channel::Keycloak || !state.has_login_step() {
        return Ok(());
    }
    let failed = |e: tower_sessions::session::Error| {
        tracing::error!(error = %e, "the session of a login step");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the server failed on this request; the cause is in its log",
        )
    };
    // The marker names the account: a session another account logs in on
    // keeps its data (the OIDC callback only cycles its id).
    let done: Option<Value> = session.get(SESSION_KEY_STEP_LOGIN).await.map_err(failed)?;
    if done.is_some_and(|d| d["sub"] == principal.sub()) {
        return Ok(());
    }
    // Requests that arrive at once each see no marker: the key makes the
    // step commit once for this session and account, and the others replay
    // its answer.
    // A session not stored yet has no id, and no key: it is one request.
    let key = session.id().and_then(|id| {
        format!("inlogstap:{id}:{}", principal.sub())
            .parse::<IdempotencyKey>()
            .ok()
    });
    let claim = state.subject_claim();
    let user = principal
        .claims()
        .get(crate::config::USERNAME_CLAIM)
        .and_then(Value::as_str);
    let number = principal.claims().get(claim).and_then(Value::as_str);
    let outcome = match (user, number) {
        (Some(user), Some(number)) => {
            match process_step_log_in(
                state,
                user,
                regelrecht_poc_chronolexografie::inlogstap::WACHTWOORD,
                key,
            )
            .await
            {
                Ok((asserted, _)) if asserted == number => {
                    tracing::info!(
                        user,
                        number,
                        "the login step recorded the login through Keycloak"
                    );
                    json!({ "vastgelegd": number })
                }
                Ok((asserted, _)) => {
                    // The step recorded its own user's login; the realm gave
                    // that user another number.
                    tracing::error!(
                        user,
                        number,
                        asserted,
                        "the login step asserts another number than the token: a mistake in the realm"
                    );
                    json!({ "vastgelegd_voor": asserted, "token": number })
                }
                Err(e) if e.status().is_client_error() => {
                    let reason = e.body()["fout"].clone();
                    tracing::warn!(user, %reason, "the login step refused the account of Keycloak");
                    json!({ "geweigerd": reason })
                }
                Err(e) => return Err(e),
            }
        }
        _ => {
            tracing::warn!(sub = %principal.sub(), claim, "a login through Keycloak without a user name or number for the login step");
            json!({ "geweigerd": "geen gebruikersnaam of nummer" })
        }
    };
    let mut outcome = outcome;
    outcome["sub"] = Value::from(principal.sub());
    session
        .insert(SESSION_KEY_STEP_LOGIN, outcome)
        .await
        .map_err(failed)
}

/// `GET /auth/simulatie`: what the simulated login of the citizens' side
/// asks. The number that identifies an applicant (`identiteit`: its claim,
/// label, kind and digits) and, for a case with a login step, what that
/// step offers (`inlogstap`: the cell, the service, the password of the
/// simulation and the user names), else `null`. There only beside the
/// test login, on an instance that serves citizens.
async fn simulation<S: Host>(State(state): State<S>) -> Result<Json<Value>, ApiError> {
    serves_citizens(&state)?;
    let offer = state.login_offer().await?;
    Ok(Json(json!({
        "identiteit": state.identifier().describe(),
        "inlogstap": offer,
    })))
}

/// An account as a session holds it.
struct SessionAccount<'a> {
    sub: &'a str,
    roles: &'a BTreeSet<String>,
    claims: &'a BTreeMap<String, Value>,
    name: &'a str,
}

/// Write `account` into `session` under the keys the OIDC callback uses,
/// with the organisation's `mark` when the organisation handed it over, and
/// without one otherwise. What the session held before goes first: a
/// refresh token of an earlier login through Keycloak would otherwise renew
/// that account's roles into this one.
async fn write_session(
    session: &Session,
    account: &SessionAccount<'_>,
    mark: Option<&OrganisationMark>,
) -> Result<(), ApiError> {
    let failed = |e: tower_sessions::session::Error| {
        tracing::error!(error = %e, "writing the session of a login");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the server failed on this request; the cause is in its log",
        )
    };
    session.clear().await;
    session.cycle_id().await.map_err(failed)?;
    session
        .insert(SESSION_KEY_AUTHENTICATED, true)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_SUB, account.sub)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_ROLES, account.roles)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_CLAIMS, account.claims)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_NAME, account.name)
        .await
        .map_err(failed)?;
    match mark {
        Some(mark) => session
            .insert(SESSION_KEY_ORGANISATION, mark)
            .await
            .map_err(failed),
        None => Ok(()),
    }
}

/// What `GET /auth/organisatie` takes: the statement, and where to go.
#[derive(Debug, Deserialize)]
pub struct Handoff {
    token: String,
    naar: Option<String>,
}

/// `GET /auth/organisatie?token=<statement>&naar=<path>`: a session for the
/// account the organisation's statement names, then on to `naar` on this
/// server (`/` without it). There only on a process that belongs to an
/// organisation ([`crate::api::router`]).
///
/// # Errors
///
/// 400 when `naar` is no path of this server, 401 when the statement is
/// refused ([`Acceptor::accept`]), 403 when it gives no role here.
pub async fn organisation_handoff(
    State(state): State<AppState>,
    session: Session,
    Query(handoff): Query<Handoff>,
) -> Result<Redirect, ApiError> {
    let Some(acceptor) = Host::acceptor(&state) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "there is no such route",
        ));
    };
    let target = match handoff.naar.as_deref() {
        None | Some("") => "/",
        Some(path) => local_path(path).ok_or_else(|| {
            ApiError::new(StatusCode::BAD_REQUEST, "`naar` is a path on this server")
        })?,
    };
    let accepted = acceptor
        .accept(&handoff.token, crate::fsc4b::now_unix())
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "a hand-off of the organisation refused");
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "the organisation's statement is not valid; log in again at the organisation",
            )
        })?;
    let inlog = accepted.statement.login.as_str().to_string();
    let principal = Principal::of_organisation(accepted, Host::surface(&state));
    if !principal.is_user() {
        return Err(crate::access::forbidden());
    }
    let mark = OrganisationMark {
        sub: principal.sub().to_string(),
        inlog,
    };
    write_session(
        &session,
        &SessionAccount {
            sub: principal.sub(),
            roles: principal.roles(),
            claims: principal.claims(),
            name: principal.name().unwrap_or(principal.sub()),
        },
        Some(&mark),
    )
    .await?;
    tracing::info!(sub = %principal.sub(), naar = target, "handed over by the organisation");
    Ok(Redirect::to(target))
}

/// `path` when it is a path on this server: it starts with one `/`, and no
/// browser reads it as another host (`//host`, `/\host`).
pub(crate) fn local_path(path: &str) -> Option<&str> {
    let rest = path.strip_prefix('/')?;
    (!rest.starts_with('/') && !rest.starts_with('\\') && !path.chars().any(char::is_control))
        .then_some(path)
}

/// `GET /auth/logout` beside the test login: end the session and go back to
/// the start, where a screen to get in waits. The shared handler of
/// `regelrecht_auth` sends a server without `BASE_URL` to `//`, which a
/// browser reads as a host and cannot open; the test login has no `BASE_URL`
/// and no Keycloak to send anyone to.
async fn test_logout(session: Session) -> Result<Redirect, ApiError> {
    session.flush().await.map_err(|e| {
        tracing::error!(error = %e, "ending the session of a test login");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the server failed on this request; the cause is in its log",
        )
    })?;
    Ok(Redirect::to("/"))
}

/// `GET /auth/testpersonen`: the accounts a test login can take, each as
/// `POST /auth/testlogin` takes it, with what it is (`soort`: `burger`,
/// `medewerker` or `beheerder`) so a screen can name it. Two citizens with
/// a made-up name and a made-up number of the test range; a decider of the
/// executor and an admin, who also enters what the external parties do,
/// each with a made-up name too.
/// Derived from the case, so another case gets its own, and only those of
/// the sides the
/// surface serves: the citizens on the instance for citizens, the employees
/// and the admin on the back office. There only beside the test login.
async fn test_personas<S: Host>(State(state): State<S>) -> Json<Value> {
    Json(Value::Array(state.test_personas().await))
}

/// The accounts of the test login of a process: see [`test_personas`].
pub fn process_personas(state: &AppState) -> Vec<Value> {
    let surface = state.surface();
    let mut personas = Vec::new();
    if surface.serves_citizens() {
        personas.extend(applicant_personas(state));
    }
    if surface.serves_employees() {
        personas.extend(employee_personas(state));
    }
    personas
}

/// The citizens of the test login, with a made-up name and a made-up number
/// of the test range under the claim `claim`.
pub fn citizen_personas(claim: &str) -> impl Iterator<Item = Value> + '_ {
    TEST_CITIZENS
        .iter()
        .zip(TEST_CITIZEN_NAMES)
        .map(move |(bsn, name)| {
            json!({
                "soort": "burger",
                "sub": format!("burger-{bsn}"),
                "rollen": [CITIZEN],
                "claims": { claim: bsn },
                "identiteit": bsn,
                "naam": name,
            })
        })
}

/// The applicants of the test login of a process. A case that identifies
/// them by BSN gets the test citizens ([`citizen_personas`]). A case that
/// logs in on another key, such as a KvK number, gets the numbers of that
/// key in what its cells record before the first step, with the name a
/// record beside them gives: the businesses of the Chamber of Commerce
/// cell, say. A case with a login step gets none: there one logs in with a
/// user name of the step (`GET /auth/simulatie`).
fn applicant_personas(state: &AppState) -> Vec<Value> {
    let identifier = state.identifier();
    let service = state.service();
    if service.login_step().is_some() {
        return Vec::new();
    }
    match (identifier.kind(), service.login()) {
        (Kind::Bsn, _) => citizen_personas(identifier.claim()).collect(),
        // Another number than a BSN, without a login in the case to say
        // where such numbers are: the screen asks for one.
        (_, None) => Vec::new(),
        (_, Some(login)) => service
            .begin_identities(&login.sleutel)
            .iter()
            .filter(|(number, _)| identifier.parse(number).is_ok())
            .map(|(number, name)| {
                json!({
                    "soort": "burger",
                    "sub": identifier.subject(number),
                    "rollen": [CITIZEN],
                    "claims": { identifier.claim(): number },
                    "identiteit": number,
                    "label": identifier.label(),
                    "naam": name,
                })
            })
            .collect(),
    }
}

/// The employees of the test login, each with a made-up name: a decider of
/// the executor, with that step of the ladder and nothing else, and an admin.
///
/// On the server of an external party they are that party's own: its staff
/// log in there, not the executor's, and `partij` says so, so the login
/// screen words them as the party's.
fn employee_personas(state: &AppState) -> Vec<Value> {
    let service = state.service();
    let party = service.party();
    let cell = service.own_cell();
    let cell_name = service.cell_name(cell);
    let decider = Level::Decider;
    vec![
        json!({
            "soort": "medewerker",
            "sub": format!("{}-{cell}", decider.name()),
            "rollen": [decider.role()],
            "niveau": decider.name(),
            "naam": TEST_DECIDER_NAME,
            "cel": cell,
            "cel_naam": cell_name,
            "partij": party.is_some(),
        }),
        json!({
            "soort": "beheerder",
            "sub": "beheerder",
            "rollen": [Level::Admin.role()],
            "niveau": Level::Admin.name(),
            "naam": TEST_ADMIN_NAME,
            "cel": party,
            "cel_naam": party.map(|p| service.cell_name(p)),
            "partij": party.is_some(),
        }),
    ]
}

/// A session store that marks every session with the surface it was made
/// on, and gives no session back that carries another mark or none.
/// The instance for citizens and the one for the back office share the
/// table `wetsproces.sessions`; with this a cookie taken from one side is
/// no session on the other, whatever roles it holds.
///
/// It also stores in a session only the roles its surface admits
/// ([`Surface::admitted_roles`]), whatever the login or a refresh of the
/// token wrote: a stored session on the instance for citizens never holds
/// an employee's role. A refresh writes the token's roles into the live
/// session first, so the response of that one request may still show them
/// (`/auth/status`); what a request may do is decided by the principal,
/// which filters once more when it reads the session.
#[derive(Debug, Clone)]
pub struct SurfaceSessions<S> {
    inner: S,
    surface: Surface,
}

impl<S> SurfaceSessions<S> {
    /// The sessions of `inner` that belong to `surface`.
    pub fn new(inner: S, surface: Surface) -> Self {
        Self { inner, surface }
    }

    /// Mark `record` with the surface, and keep only the roles it admits.
    fn mark(&self, record: &mut Record) {
        record
            .data
            .insert(SESSION_KEY_SURFACE.into(), Value::from(self.surface.name()));
        if let Some(roles) = record.data.get_mut(SESSION_KEY_ROLES) {
            let held: BTreeSet<String> = roles
                .as_array()
                .map(|r| {
                    r.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            *roles = json!(self.surface.admitted_roles(held));
        }
    }

    fn is_ours(&self, record: &Record) -> bool {
        record.data.get(SESSION_KEY_SURFACE).and_then(Value::as_str) == Some(self.surface.name())
    }
}

#[async_trait]
impl<S: SessionStore + Clone> SessionStore for SurfaceSessions<S> {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        self.mark(record);
        self.inner.create(record).await
    }

    /// The session layer saves only a session it created or loaded, and
    /// [`SurfaceSessions::load`] gives none of another surface, so a save
    /// never reaches a session of another surface.
    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let mut record = record.clone();
        self.mark(&mut record);
        self.inner.save(&record).await
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let record = self.inner.load(id).await?;
        Ok(record.filter(|r| {
            let ours = self.is_ours(r);
            if !ours {
                tracing::warn!(surface = %self.surface, "a session of another surface was offered");
            }
            ours
        }))
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        // Only a session of this surface may be ended from here.
        match self.inner.load(id).await? {
            Some(r) if self.is_ours(&r) => self.inner.delete(id).await,
            _ => Ok(()),
        }
    }
}

oidc_app_state!(AppState);

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A login through Keycloak whose client was made without discovery,
    /// for a test that never logs in through it; the host is made up.
    pub(crate) fn offline_login() -> Login {
        let issuer = "https://login.example.org/realms/voorbeeld";
        Login::Oidc(Arc::new(OidcLogin {
            config: OidcConfig {
                client_id: "regelhet".into(),
                client_secret: "secret".into(),
                issuer_url: issuer.into(),
                required_role: "regelhet-toegang".into(),
                extra_claims: vec!["bsn".into()],
            },
            base_url: Some("https://regelhet.example.org".into()),
            client: Arc::new(regelrecht_auth::test_utils::offline_client(issuer)),
            end_session_url: None,
        }))
    }

    #[tokio::test]
    async fn a_login_leaves_nothing_of_the_session_before_it() {
        let store = Arc::new(tower_sessions_memory_store::MemoryStore::default());
        let session = Session::new(None, store, None);
        // An earlier login through Keycloak, and an earlier hand-off.
        session
            .insert(
                regelrecht_auth::handlers::SESSION_KEY_REFRESH_TOKEN,
                "old-refresh",
            )
            .await
            .unwrap();
        session
            .insert(
                SESSION_KEY_ORGANISATION,
                OrganisationMark {
                    sub: "tnt-o/wilgendaal:x".into(),
                    inlog: "keycloak".into(),
                },
            )
            .await
            .unwrap();
        let roles = BTreeSet::from(["regelhet-lezer".to_string()]);
        let claims = BTreeMap::new();
        let account = SessionAccount {
            sub: "lezer-1",
            roles: &roles,
            claims: &claims,
            name: "Lezer",
        };
        write_session(&session, &account, None).await.unwrap();
        let refresh: Option<String> = session
            .get(regelrecht_auth::handlers::SESSION_KEY_REFRESH_TOKEN)
            .await
            .unwrap();
        assert_eq!(refresh, None);
        let mark: Option<OrganisationMark> = session.get(SESSION_KEY_ORGANISATION).await.unwrap();
        assert_eq!(mark, None);
        let sub: Option<String> = session.get(SESSION_KEY_SUB).await.unwrap();
        assert_eq!(sub.as_deref(), Some("lezer-1"));
    }

    #[test]
    fn a_local_path_stays_on_this_server() {
        assert_eq!(local_path("/backoffice"), Some("/backoffice"));
        assert_eq!(
            local_path("/burger#nieuwe-aanvraag"),
            Some("/burger#nieuwe-aanvraag")
        );
        for elsewhere in [
            "//evil.example.org",
            "/\\evil",
            "https://evil",
            "backoffice",
            "/a\nb",
        ] {
            assert_eq!(local_path(elsewhere), None, "{elsewhere}");
        }
    }

    #[test]
    fn the_session_cookie_is_secure_except_over_plain_http() {
        assert!(offline_login().secure_cookie());
        assert!(!Login::TestLogin.secure_cookie());
    }

    fn record(id: i128) -> Record {
        Record {
            id: Id(id),
            data: std::collections::HashMap::new(),
            expiry_date: tower_sessions::cookie::time::OffsetDateTime::now_utc()
                + tower_sessions::cookie::time::Duration::hours(1),
        }
    }

    #[tokio::test]
    async fn a_session_keeps_only_the_roles_its_surface_admits() {
        let shared = tower_sessions_memory_store::MemoryStore::default();
        let citizens = SurfaceSessions::new(shared.clone(), Surface::Citizen);
        let mut r = record(3);
        r.data.insert(
            SESSION_KEY_ROLES.into(),
            json!([CITIZEN, "regelhet-admin", "regelhet-beslisser"]),
        );
        citizens.create(&mut r).await.unwrap();
        let stored = shared.load(&r.id).await.unwrap().unwrap();
        assert_eq!(stored.data[SESSION_KEY_ROLES], json!([CITIZEN]));
        // A refresh that writes the token's roles again is filtered as well.
        r.data
            .insert(SESSION_KEY_ROLES.into(), json!([CITIZEN, "regelhet-admin"]));
        citizens.save(&r).await.unwrap();
        let stored = shared.load(&r.id).await.unwrap().unwrap();
        assert_eq!(stored.data[SESSION_KEY_ROLES], json!([CITIZEN]));
    }

    #[tokio::test]
    async fn a_session_of_one_surface_is_no_session_on_another() {
        let shared = tower_sessions_memory_store::MemoryStore::default();
        let citizens = SurfaceSessions::new(shared.clone(), Surface::Citizen);
        let backoffice = SurfaceSessions::new(shared.clone(), Surface::Backoffice);
        let mut r = record(1);
        r.data
            .insert(SESSION_KEY_ROLES.into(), json!(["regelhet-admin"]));
        backoffice.create(&mut r).await.unwrap();
        let id = r.id;
        assert!(backoffice.load(&id).await.unwrap().is_some());
        assert_eq!(citizens.load(&id).await.unwrap(), None);
        // The other side cannot end it.
        citizens.delete(&id).await.unwrap();
        assert!(backoffice.load(&id).await.unwrap().is_some());
        // A session without a mark, as made before the split, is none either.
        let unmarked = record(2);
        shared.save(&unmarked).await.unwrap();
        assert_eq!(backoffice.load(&unmarked.id).await.unwrap(), None);
    }

    /// A login through Keycloak on a case with a login step: the step
    /// logs the account in once per session, by its Keycloak user name,
    /// and records the authentication; a second request records nothing.
    #[tokio::test]
    async fn a_login_through_keycloak_is_logged_in_at_the_case_step_once() {
        let service = Arc::new(crate::service::tests::login_step_service().await);
        let state = AppState::new(
            Arc::clone(&service),
            crate::config::Mode::Stable,
            Surface::Citizen,
            None,
            offline_login(),
            "kvk_nummer",
        );
        let cell = "voorbeeld_erkende_dienst";
        let head = || async {
            service.chronicle(cell).await.unwrap()["kop"]
                .as_u64()
                .unwrap()
        };
        let before = head().await;
        let account = |user: &str, number: &str| {
            Principal::new(
                format!("kc-{user}"),
                BTreeSet::from([CITIZEN.to_string()]),
                BTreeMap::from([
                    ("kvk_nummer".to_string(), Value::from(number)),
                    (crate::config::USERNAME_CLAIM.to_string(), Value::from(user)),
                ]),
                Channel::Keycloak,
            )
        };
        let session = || {
            Session::new(
                None,
                Arc::new(tower_sessions_memory_store::MemoryStore::default()),
                None,
            )
        };

        let bakker = session();
        record_step_login(&state, &bakker, &account("bakker", "99970011"))
            .await
            .unwrap();
        assert_eq!(head().await, before + 1);
        record_step_login(&state, &bakker, &account("bakker", "99970011"))
            .await
            .unwrap();
        assert_eq!(head().await, before + 1, "once per session");

        // Refused at the step, or a number other than the token's: nothing
        // is recorded, and it is not tried again in that session.
        let kapper = session();
        record_step_login(&state, &kapper, &account("kapper", "99970022"))
            .await
            .unwrap();
        let other = session();
        record_step_login(&state, &other, &account("bakker", "99970033"))
            .await
            .unwrap();
        assert_eq!(
            head().await,
            before + 2,
            "the step records before the number is compared"
        );
        let noted: Option<Value> = other.get(SESSION_KEY_STEP_LOGIN).await.unwrap();
        assert_eq!(
            noted,
            Some(json!({ "vastgelegd_voor": "99970011", "token": "99970033", "sub": "kc-bakker" }))
        );

        // Another account on a session that holds a marker is logged in at
        // the step again.
        record_step_login(&state, &bakker, &account("bloemist", "99970033"))
            .await
            .unwrap();
        assert_eq!(head().await, before + 3);
    }

    /// Requests of one stored session that arrive before the marker is
    /// written replay the first one's step and record nothing more.
    #[tokio::test]
    async fn a_second_request_of_the_session_replays_the_step() {
        let service = Arc::new(crate::service::tests::login_step_service().await);
        let state = AppState::new(
            Arc::clone(&service),
            crate::config::Mode::Stable,
            Surface::Citizen,
            None,
            offline_login(),
            "kvk_nummer",
        );
        let head = || async {
            service.chronicle("voorbeeld_erkende_dienst").await.unwrap()["kop"]
                .as_u64()
                .unwrap()
        };
        let before = head().await;
        let bakker = Principal::new(
            "kc-bakker".into(),
            BTreeSet::from([CITIZEN.to_string()]),
            BTreeMap::from([
                ("kvk_nummer".to_string(), Value::from("99970011")),
                (
                    crate::config::USERNAME_CLAIM.to_string(),
                    Value::from("bakker"),
                ),
            ]),
            Channel::Keycloak,
        );
        let store = Arc::new(tower_sessions_memory_store::MemoryStore::default());
        let session = Session::new(None, store, None);
        session.insert("x", 1).await.unwrap();
        session.save().await.unwrap();
        record_step_login(&state, &session, &bakker).await.unwrap();
        // As a request that read the session before the marker was written.
        session.remove_value(SESSION_KEY_STEP_LOGIN).await.unwrap();
        record_step_login(&state, &session, &bakker).await.unwrap();
        assert_eq!(head().await, before + 1);
    }
}
