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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;

use axum::extract::State;
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
use serde::Deserialize;
use serde_json::{Value, json};
use tower_sessions::session::{Id, Record};
use tower_sessions::{Session, SessionStore, session_store};

use crate::access::{CITIZEN, Level};
use crate::api::{ApiError, AppState};
use crate::config::LoginChoice;
use crate::surface::Surface;

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
pub const SESSION_KEY_SURFACE: &str = "wetsproces.surface";

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
    pub fn routes(&self, surface: Surface) -> Router<AppState> {
        match self {
            Self::Oidc(_) => {
                regelrecht_auth::auth_routes::<AppState>().layer(axum::middleware::from_fn(
                    move |request, next| login_failed_to_screen(surface, request, next),
                ))
            }
            Self::TestLogin => Router::new()
                .route("/auth/testlogin", post(test_login))
                .route("/auth/testpersonen", get(test_personas))
                .route(
                    "/auth/status",
                    get(regelrecht_auth::handlers::status::<AppState>),
                )
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
    Digid(SimulatedDigid),
    Account(TestLogin),
}

/// A login of the simulated DigiD: a citizen's number and name, nothing
/// else. The server makes the account itself, with the citizen's role only,
/// so a request cannot add a role to it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SimulatedDigid {
    bsn: String,
    naam: String,
}

/// The longest name the simulated DigiD takes, in characters.
const MAX_NAME_CHARS: usize = 100;

/// The fictional names of the citizens of the test login, in the order of
/// [`TEST_CITIZENS`].
const TEST_CITIZEN_NAMES: [&str; 2] = ["Anna Voorbeeld", "Bram Proef"];
// A citizen without a name would drop out of the list in silence.
const _: () = assert!(TEST_CITIZEN_NAMES.len() == TEST_CITIZENS.len());

/// A citizen service number of the test range: nine digits that pass the
/// eleven test and start with [`regelrecht_fsc4b::wire::TEST_BSN_PREFIX`].
/// The simulated DigiD takes no other, so a number typed into it is never a
/// real one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bsn(String);

impl Bsn {
    /// The number in `text`, with spaces around it left out.
    ///
    /// # Errors
    ///
    /// A message for the citizen when it is not nine digits, fails the eleven
    /// test, or is outside the test range.
    fn parse(text: &str) -> Result<Self, &'static str> {
        let text = text.trim();
        if text.len() != 9 || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Een BSN bestaat uit negen cijfers.");
        }
        if !passes_eleven_test(text) {
            return Err("Dit BSN haalt de elfproef niet. Kijk de cijfers na.");
        }
        if !regelrecht_fsc4b::wire::is_test_bsn(text) {
            return Err(
                "Deze simulatie neemt alleen een BSN uit de testreeks aan: negen cijfers die beginnen met 99999.",
            );
        }
        Ok(Self(text.to_owned()))
    }

    /// The number as text.
    fn as_str(&self) -> &str {
        &self.0
    }
}

/// The eleven test of a citizen service number of nine ASCII digits: the
/// first eight times 9 down to 2, minus the last, is a multiple of 11.
fn passes_eleven_test(digits: &str) -> bool {
    let sum: i64 = digits
        .bytes()
        .zip([9, 8, 7, 6, 5, 4, 3, 2, -1])
        .map(|(b, weight)| i64::from(b - b'0') * weight)
        .sum();
    sum % 11 == 0
}

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
async fn test_login(
    State(state): State<AppState>,
    session: Session,
    Json(request): Json<TestLoginRequest>,
) -> Result<Json<Value>, ApiError> {
    let login = match request {
        TestLoginRequest::Account(login) => login,
        TestLoginRequest::Digid(digid) => simulated_citizen(&state, &digid)?,
    };
    if login.sub.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "a test login needs a `sub`",
        ));
    }
    write_session(&session, &login).await?;
    tracing::info!(sub = %login.sub, roles = ?login.rollen, "test login");
    Ok(Json(json!({ "sub": login.sub, "rollen": login.rollen })))
}

/// The account of a citizen of the simulated DigiD: the citizen's role and
/// nothing else, their number under the subject claim, and their name.
fn simulated_citizen(state: &AppState, digid: &SimulatedDigid) -> Result<TestLogin, ApiError> {
    if !state.surface().serves_citizens() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "this instance serves no citizens",
        ));
    }
    let bad = |message: &str| ApiError::new(StatusCode::BAD_REQUEST, message);
    let bsn = Bsn::parse(&digid.bsn).map_err(bad)?;
    let name = digid.naam.trim();
    if name.is_empty() {
        return Err(bad("Vul een naam in."));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(bad(&format!(
            "Een naam is hoogstens {MAX_NAME_CHARS} tekens lang."
        )));
    }
    // A name goes into the session and onto every screen that shows it.
    if name.chars().any(char::is_control) {
        return Err(bad("Een naam bestaat uit letters, spaties en leestekens."));
    }
    Ok(TestLogin {
        sub: format!("burger-{}", bsn.as_str()),
        rollen: BTreeSet::from([CITIZEN.to_owned()]),
        claims: BTreeMap::from([(state.subject_claim().to_owned(), Value::from(bsn.as_str()))]),
        naam: Some(name.to_owned()),
    })
}

/// Write `login` into `session` under the keys the OIDC callback uses.
async fn write_session(session: &Session, login: &TestLogin) -> Result<(), ApiError> {
    let failed = |e: tower_sessions::session::Error| {
        tracing::error!(error = %e, "writing the session of a test login");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the server failed on this request; the cause is in its log",
        )
    };
    session.cycle_id().await.map_err(failed)?;
    session
        .insert(SESSION_KEY_AUTHENTICATED, true)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_SUB, &login.sub)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_ROLES, &login.rollen)
        .await
        .map_err(failed)?;
    session
        .insert(SESSION_KEY_CLAIMS, &login.claims)
        .await
        .map_err(failed)?;
    session
        .insert(
            SESSION_KEY_NAME,
            login.naam.as_deref().unwrap_or(&login.sub),
        )
        .await
        .map_err(failed)
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
/// a made-up name and a made-up number of the test range; a handler and a decider of the
/// executor; an admin, who also enters what the external parties do.
/// Derived from the case, so another case gets its own, and only those of
/// the sides the
/// surface serves: the citizens on the instance for citizens, the employees
/// and the admin on the back office. There only beside the test login.
async fn test_personas(State(state): State<AppState>) -> Json<Value> {
    let surface = state.surface();
    let mut personas = Vec::new();
    if surface.serves_citizens() {
        let claim = state.subject_claim();
        personas.extend(
            TEST_CITIZENS
                .iter()
                .zip(TEST_CITIZEN_NAMES)
                .map(|(bsn, name)| {
                    json!({
                        "soort": "burger",
                        "sub": format!("burger-{bsn}"),
                        "rollen": [CITIZEN],
                        "claims": { claim: bsn },
                        "identiteit": bsn,
                        "naam": name,
                    })
                }),
        );
    }
    if surface.serves_employees() {
        personas.extend(employee_personas(&state));
    }
    Json(Value::Array(personas))
}

/// The employees of the test login: a handler and a decider of the
/// executor, each with their step of the ladder and nothing else, and an
/// admin.
fn employee_personas(state: &AppState) -> Vec<Value> {
    let service = state.service();
    let (cell, cell_name) = (service.executor(), service.executor_name());
    let mut personas: Vec<Value> = [Level::Handler, Level::Decider]
        .into_iter()
        .map(|level| {
            json!({
                "soort": "medewerker",
                "sub": format!("{}-{cell}", level.name()),
                "rollen": [level.role()],
                "niveau": level.name(),
                "cel": cell,
                "cel_naam": cell_name,
            })
        })
        .collect();
    personas.push(json!({
        "soort": "beheerder",
        "sub": "beheerder",
        "rollen": [Level::Admin.role()],
        "niveau": Level::Admin.name(),
    }));
    personas
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

impl regelrecht_auth::OidcAppState for AppState {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
        self.oidc().map(|o| &o.client)
    }
    fn end_session_url(&self) -> Option<&str> {
        self.oidc().and_then(|o| o.end_session_url.as_deref())
    }
    fn oidc_config(&self) -> Option<&OidcConfig> {
        self.oidc().map(|o| &o.config)
    }
    fn is_auth_enabled(&self) -> bool {
        self.oidc().is_some()
    }
    fn base_url(&self) -> Option<&str> {
        self.oidc().and_then(|o| o.base_url.as_deref())
    }
    fn http_client(&self) -> &reqwest::Client {
        self.http_client()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A login through Keycloak whose client was made without discovery,
    /// for a test that never logs in through it; the host is made up.
    pub(crate) fn offline_login() -> Login {
        let issuer = "https://login.example.org/realms/voorbeeld";
        Login::Oidc(Arc::new(OidcLogin {
            config: OidcConfig {
                client_id: "wetsproces".into(),
                client_secret: "secret".into(),
                issuer_url: issuer.into(),
                required_role: "wetsproces-toegang".into(),
                extra_claims: vec!["bsn".into()],
            },
            base_url: Some("https://wetsproces.example.org".into()),
            client: Arc::new(regelrecht_auth::test_utils::offline_client(issuer)),
            end_session_url: None,
        }))
    }

    #[test]
    fn a_bsn_is_nine_digits_that_pass_the_eleven_test_in_the_test_range() {
        for bsn in TEST_CITIZENS {
            assert_eq!(
                Bsn::parse(bsn).map(|b| b.as_str().to_owned()),
                Ok(bsn.to_owned())
            );
        }
        assert_eq!(
            Bsn::parse(" 999990032 ").map(|b| b.as_str().to_owned()),
            Ok("999990032".into())
        );
        for wrong in ["", "99999003", "9999900321", "99999003a", "999 99003"] {
            assert_eq!(
                Bsn::parse(wrong),
                Err("Een BSN bestaat uit negen cijfers."),
                "{wrong}"
            );
        }
        // One digit off fails the eleven test.
        assert_eq!(
            Bsn::parse("999990033"),
            Err("Dit BSN haalt de elfproef niet. Kijk de cijfers na.")
        );
        // Passes the eleven test, but is outside the test range.
        assert!(passes_eleven_test("123456782"));
        assert!(Bsn::parse("123456782").unwrap_err().contains("testreeks"));
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
            json!([CITIZEN, "wetsproces-admin", "wetsproces-beslisser"]),
        );
        citizens.create(&mut r).await.unwrap();
        let stored = shared.load(&r.id).await.unwrap().unwrap();
        assert_eq!(stored.data[SESSION_KEY_ROLES], json!([CITIZEN]));
        // A refresh that writes the token's roles again is filtered as well.
        r.data.insert(
            SESSION_KEY_ROLES.into(),
            json!([CITIZEN, "wetsproces-admin"]),
        );
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
            .insert(SESSION_KEY_ROLES.into(), json!(["wetsproces-admin"]));
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
}
