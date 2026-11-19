//! How people log in, and the sessions that remember it.
//!
//! Two logins, one session format. With OIDC the routes of
//! `regelrecht_auth` log in through Keycloak and write the account's `sub`,
//! realm roles and the claims of [`crate::config::Config::subject_claim`]
//! into the session. In test mode without OIDC configuration the test login
//! ([`test_login`]) writes the same keys with whatever roles and claims it is
//! sent. The rest of the server reads only the session
//! ([`crate::access::Principal`]), so a request after the test login goes
//! through the same checks as one after Keycloak. The test login is a route
//! only in that case: stable mode never registers it.
//!
//! Sessions live where the chronicles live: in memory with the store in
//! memory, and in the table `wetsproces.sessions` with the store in
//! Postgres, under a cookie of their own ([`SESSION_COOKIE`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
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
use tower_sessions::Session;

use crate::access::{self, CITIZEN, Level};
use crate::api::{ApiError, AppState};
use crate::config::LoginChoice;

/// The name of the session cookie: its own, so it never mixes with the
/// editor's or the admin's on a shared domain.
pub const SESSION_COOKIE: &str = "wetsproces_session";

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

    /// The routes under `/auth` for this login.
    pub fn routes(&self) -> Router<AppState> {
        match self {
            Self::Oidc(_) => regelrecht_auth::auth_routes::<AppState>(),
            Self::TestLogin => Router::new()
                .route("/auth/testlogin", post(test_login))
                .route("/auth/testpersonen", get(test_personas))
                .route(
                    "/auth/status",
                    get(regelrecht_auth::handlers::status::<AppState>),
                )
                .route(
                    "/auth/logout",
                    get(regelrecht_auth::handlers::logout::<AppState>),
                ),
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
/// OIDC callback would have made it. There only in test mode without OIDC
/// configuration ([`Login::routes`]).
async fn test_login(
    session: Session,
    Json(login): Json<TestLogin>,
) -> Result<Json<Value>, ApiError> {
    if login.sub.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "a test login needs a `sub`",
        ));
    }
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
        .map_err(failed)?;
    tracing::info!(sub = %login.sub, roles = ?login.rollen, "test login");
    Ok(Json(json!({ "sub": login.sub, "rollen": login.rollen })))
}

/// `GET /auth/testpersonen`: the accounts a test login can take, each as
/// `POST /auth/testlogin` takes it, with what it is (`soort`: `burger`,
/// `medewerker` or `beheerder`) so a screen can name it. Two citizens with
/// a made-up number of the test range; per cell of the case a handler and
/// a decider; an admin who may decide for every cell. Derived from the
/// case, so another case gets its own. There only beside the test login.
async fn test_personas(State(state): State<AppState>) -> Json<Value> {
    let claim = state.subject_claim();
    let mut personas: Vec<Value> = TEST_CITIZENS
        .iter()
        .map(|bsn| {
            json!({
                "soort": "burger",
                "sub": format!("burger-{bsn}"),
                "rollen": [CITIZEN],
                "claims": { claim: bsn },
                "identiteit": bsn,
            })
        })
        .collect();
    for (cell, cell_name) in state.service().cells() {
        for level in [Level::Handler, Level::Decider] {
            personas.push(json!({
                "soort": "medewerker",
                "sub": format!("{}-{cell}", level.name()),
                "rollen": [level.role(), access::cell_role(cell)],
                "niveau": level.name(),
                "cel": cell,
                "cel_naam": cell_name,
            }));
        }
    }
    let mut admin_roles = vec![Level::Admin.role().to_string()];
    admin_roles.extend(state.service().cells().map(|(c, _)| access::cell_role(c)));
    personas.push(json!({
        "soort": "beheerder",
        "sub": "beheerder",
        "rollen": admin_roles,
        "niveau": Level::Admin.name(),
    }));
    Json(Value::Array(personas))
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
    fn the_session_cookie_is_secure_except_over_plain_http() {
        assert!(offline_login().secure_cookie());
        assert!(!Login::TestLogin.secure_cookie());
    }
}
