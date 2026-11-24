//! An organisation in front of several processes: one login, one page with
//! a tile per process, and a hand-off to each process without a second
//! login.
//!
//! The front is this server in a second role (`WETSPROCES_ROL=organisatie`,
//! [`crate::organisatie::settings`]), from the same image: the login, the
//! sessions, the surfaces, the accent colour and the serving of the screens
//! are the process's own. It runs no process and keeps nothing; it knows the
//! processes from a file ([`processes`]) and speaks for whoever is logged in
//! with a signed statement ([`statement`]):
//!
//! - it calls a process's API inside the cluster, with the statement in the
//!   header, for a short list of `GET` routes ([`relay`]);
//! - it sends the browser to a process's `/auth/organisatie` with a
//!   statement, where the process makes a session of its own and shows its
//!   own screens.
//!
//! One role ladder holds for the whole organisation: a decider at the front
//! is a decider in every process of it.
//!
//! | Method and path | Who | What |
//! |---|---|---|
//! | `GET /health`, `/auth/...` | anyone | the login; the test login offers the accounts of every process |
//! | `GET /api/uiterlijk` | anyone | the accent colour, and `organisatie: { naam, logo, voorkant: true }` |
//! | `GET /api/ik` | citizen or employee | who is logged in, without cells |
//! | `GET /api/organisatie` | citizen or employee | the name, the logo, and per process its `id`, `naam`, `titel` and whether it answered (`bereikbaar`) |
//! | `GET /api/processen/{id}/api/{*pad}` | citizen or employee | the process's own answer, for the routes of [`relay::ALLOWED`] |
//! | `GET /api/processen/{id}/openen?naar=<path>` | citizen or employee | `303` to the process's `/auth/organisatie` with a statement |
//! | `GET /organisatie/v1/sleutel` | anyone | the public key of the statements, a JWK |

pub mod processes;
pub mod relay;
pub mod settings;
pub mod statement;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Json, Router};
use regelrecht_fsc4b::jose::{JoseError, SigningKey};
use serde_json::{Value, json};

use crate::access::{self, Level, Principal, ROLE_PREFIX};
use crate::api::ApiError;
use crate::config::{Config, FrontSettings, Mode};
use crate::login::{Host, Login};
use crate::organisatie::processes::{Process, ProcessesError, Side};
use crate::organisatie::statement::{Signer, Statement};
use crate::surface::Surface;
use crate::theme::AccentColor;

/// The path of the front's public key.
pub const KEY_PATH: &str = "/organisatie/v1/sleutel";

/// How long the front waits for a process.
pub(crate) const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);

/// Why a front does not start.
#[derive(Debug, thiserror::Error)]
pub enum FrontError {
    #[error("the processes: {0}")]
    Processes(#[from] ProcessesError),
    #[error("reading the key {path}: {source}")]
    ReadKey {
        path: String,
        source: std::io::Error,
    },
    #[error("the key: {0}")]
    Key(JoseError),
}

/// The organisation behind a front.
#[derive(Debug)]
struct Front {
    name: String,
    logo: Option<String>,
    processes: Vec<Process>,
    signer: Signer,
}

/// What every handler of the front shares.
#[derive(Debug, Clone)]
pub struct FrontState {
    front: Arc<Front>,
    mode: Mode,
    surface: Surface,
    login: Login,
    subject_claim: Arc<str>,
    http_client: reqwest::Client,
    accent: Option<AccentColor>,
}

impl FrontState {
    /// The front `settings` describe, for `config`, logging in with `login`.
    ///
    /// # Errors
    ///
    /// When the processes or the key cannot be read.
    pub fn build(
        config: &Config,
        settings: &FrontSettings,
        login: Login,
    ) -> Result<Self, FrontError> {
        let processes = processes::load(&settings.processes)?;
        let key = match &settings.key {
            Some(path) => {
                let pem = std::fs::read_to_string(path).map_err(|source| FrontError::ReadKey {
                    path: path.display().to_string(),
                    source,
                })?;
                SigningKey::from_pkcs8_pem(&pem).map_err(FrontError::Key)?
            }
            None => {
                tracing::warn!("no key for the statements; one made for this run");
                SigningKey::generate().map_err(FrontError::Key)?
            }
        };
        tracing::info!(
            organisation = %settings.id,
            processes = processes.len(),
            "the front of an organisation"
        );
        Ok(Self {
            front: Arc::new(Front {
                name: settings.name.clone(),
                logo: settings.logo.clone(),
                processes,
                signer: Signer::new(key, &settings.id),
            }),
            mode: config.mode,
            surface: config.surface,
            login,
            subject_claim: config.subject_claim.as_str().into(),
            http_client: regelrecht_auth::http_client(),
            accent: config.accent,
        })
    }

    /// The process with the id `id`.
    fn process(&self, id: &str) -> Result<&Process, ApiError> {
        self.front
            .processes
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "there is no such process"))
    }

    /// The side of the processes `principal` reaches from this front: a
    /// citizen the citizen's, an employee the back office, each where the
    /// front serves it.
    fn side_of(&self, principal: &Principal) -> Result<Side, ApiError> {
        if principal.is_citizen() && Side::Citizen.served_on(self.surface) {
            Ok(Side::Citizen)
        } else if principal.has(Level::Reader) && Side::Backoffice.served_on(self.surface) {
            Ok(Side::Backoffice)
        } else {
            Err(access::forbidden())
        }
    }

    /// What a statement for `side` says of `principal`: only the roles of
    /// this server (a Keycloak session also holds the realm's own, such as
    /// `offline_access`, which a process refuses), and of those only the
    /// ones that side admits.
    fn statement(&self, principal: &Principal, side: Side) -> Statement {
        let ours = principal
            .roles()
            .iter()
            .filter(|r| r.starts_with(ROLE_PREFIX))
            .cloned()
            .collect();
        Statement {
            sub: principal.sub().to_string(),
            roles: side.surface().admitted_roles(ours),
            claims: principal.claims().clone(),
            name: principal.name().map(str::to_string),
            login: self.session_channel(),
            mode: self.mode,
        }
    }
}

impl Host for FrontState {
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
    async fn test_personas(&self) -> Vec<Value> {
        relay::test_personas(self).await
    }
}

crate::login::oidc_app_state!(FrontState);

/// The routes of the front on `state`, with the login's; the session layer
/// goes around them ([`crate::app`]).
pub fn router(state: FrontState) -> Router {
    let user = Router::new()
        .route("/api/ik", get(me))
        .route("/api/organisatie", get(overview))
        .route("/api/processen/{id}/openen", get(relay::open))
        .route("/api/processen/{id}/api/{*pad}", get(relay::relay))
        .route_layer(middleware::from_fn_with_state(state.clone(), gate_user));
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/uiterlijk", get(appearance))
        .route(KEY_PATH, get(key))
        .merge(state.login.routes(state.surface))
        .merge(user)
        .with_state(state)
}

/// A citizen or an employee, with the principal in the request for the
/// handler.
async fn gate_user(
    principal: Principal,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !principal.is_user() {
        tracing::warn!(sub = %principal.sub(), "refused at the gate of the front");
        return Err(access::forbidden());
    }
    request.extensions_mut().insert(principal);
    Ok(next.run(request).await)
}

async fn appearance(State(state): State<FrontState>) -> Json<Value> {
    Json(json!({
        "accentkleur": state.accent.map(AccentColor::name),
        "organisatie": {
            "naam": state.front.name,
            "logo": state.front.logo,
            "voorkant": true,
        },
    }))
}

async fn key(State(state): State<FrontState>) -> Json<Value> {
    Json(json!(state.front.signer.public_jwk()))
}

/// Who is logged in, in the shape of a process's `GET /api/ik`, without
/// cells: the front works for no cell.
async fn me(
    State(state): State<FrontState>,
    Extension(principal): Extension<Principal>,
) -> Json<Value> {
    let citizen = principal.citizen(&state.subject_claim).ok();
    Json(json!({
        "sub": principal.sub(),
        "naam": principal.name(),
        "burger": principal.is_citizen(),
        "identiteit": citizen.as_ref().map(access::Citizen::subject),
        "niveau": principal.level().map(Level::name),
        "cellen": [],
        "partijen_invoeren": false,
        "klok_zetten": false,
    }))
}

/// The organisation and its processes, with the title of each process's
/// case and whether it answered.
async fn overview(
    State(state): State<FrontState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Value>, ApiError> {
    let side = state.side_of(&principal)?;
    let processes = relay::cases(&state, &principal, side).await;
    Ok(Json(json!({
        "naam": state.front.name,
        "logo": state.front.logo,
        "kant": side.name(),
        "processen": processes,
    })))
}
