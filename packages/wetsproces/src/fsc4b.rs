//! The FSC voor Burgers channel: the citizen's routes for an app that proves
//! who it is with a Verifiable Credential instead of a browser with a
//! cookie, after the werkversie of the standard "FSC voor Burgers"
//! (<https://docs.fsc4b.nl/specs/protocol/>). Section numbers in this module
//! are those of that specification.
//!
//! The parts:
//!
//! - [`jose`]: JWS, JWE and the keys, narrowed to what the channel accepts;
//! - [`trust`]: the Stelseldocument, its signature, and the
//!   Activatiediensten it names;
//! - [`presentation`]: the checks of a Verifiable Presentation and its
//!   credential;
//! - [`store`]: challenges and sessions, in memory or in Postgres;
//! - [`session`]: `CreateSession` (§4.3.4), the challenge and its answer;
//! - [`citizen`]: the citizen's routes, encrypted under the session key;
//! - [`discovery`]: the discovery configuration (§4.2) and the OpenAPI
//!   specification (§2.2.1);
//! - [`demo`]: a demo Stelsel with a test Activatiedienst, for test mode;
//! - `client` (feature `test-utils`): a reference client of the whole
//!   protocol.
//!
//! The channel exists only when it is configured
//! ([`crate::config::Fsc4bChoice`]); without it [`routes`] is never called
//! and none of its paths answers. A session of the channel is a citizen and
//! nothing else ([`crate::access::Principal::fsc4b_citizen`]), and reaches
//! only the routes below.

pub mod citizen;
#[cfg(feature = "test-utils")]
pub mod client;
pub mod demo;
pub mod discovery;
pub mod jose;
pub mod presentation;
pub mod session;
pub mod store;
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;
pub mod trust;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::api::AppState;
use crate::config::{Fsc4bChoice, Fsc4bSettings, KeySource, Mode};
use crate::fsc4b::demo::Demo;
use crate::fsc4b::jose::{JoseError, SigningKey, VerifyingKey};
use crate::fsc4b::store::{Fsc4bStore, InMemoryFsc4bStore, PostgresFsc4bStore};
use crate::fsc4b::trust::{Trust, TrustError, TrustInput};

/// `POST`: a new challenge (§4.3.4.2.2.2).
pub const CHALLENGE: &str = "/fsc4b/session/v1/challenge";
/// `POST`: the answer to a challenge, for a session (§4.3.4.2.2.8).
pub const COMPLETE: &str = "/fsc4b/session/v1/complete";
/// `POST`: the case, and today.
pub const CASE: &str = "/fsc4b/burger/v1/casus";
/// `POST`: the forms.
pub const FORMS: &str = "/fsc4b/burger/v1/formulier";
/// `POST`: the citizen's own dossier.
pub const DOSSIER: &str = "/fsc4b/burger/v1/dossier";
/// `POST`: a submission for themself.
pub const SUBMIT: &str = "/fsc4b/burger/v1/indieningen";
/// `GET`: the discovery configuration (§4.2).
pub const DISCOVERY: &str = "/fsc4b/discovery";
/// `GET`: the OpenAPI specification of the routes above.
pub const OPENAPI: &str = "/fsc4b/openapi.json";

/// The base of the session service, as the discovery configuration names
/// it; its routes are `/v1/...` under it.
pub const SESSION_BASE: &str = "/fsc4b/session";
/// The base of the citizen's service.
pub const CITIZEN_BASE: &str = "/fsc4b/burger";

/// Every route of the channel outside the demo, with its method: what the
/// router registers and what the OpenAPI specification must describe.
pub const ROUTES: [(&str, &str); 8] = [
    ("post", CHALLENGE),
    ("post", COMPLETE),
    ("post", CASE),
    ("post", FORMS),
    ("post", DOSSIER),
    ("post", SUBMIT),
    ("get", DISCOVERY),
    ("get", OPENAPI),
];

/// The Unix time now, in seconds, from the system clock: the channel's
/// lifetimes are real time, also in test mode, whose case clock only says
/// what day the process is on.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Why the channel does not open.
#[derive(Debug, thiserror::Error)]
pub enum Fsc4bError {
    #[error("reading {what} from {path}: {source}")]
    Read {
        what: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[error("the organization's private key: {0}")]
    Key(JoseError),
    #[error("the Stelselbeheerder's key: {0}")]
    StelselbeheerderKey(JoseError),
    #[error("{0}")]
    Trust(#[from] TrustError),
    #[error("the demo Stelsel: {0}")]
    Demo(JoseError),
    #[error("stable mode never accepts a demo Stelsel")]
    StableDemo,
}

/// The open channel: what it trusts, where it keeps its challenges and
/// sessions, where apps reach it, and the demo Stelsel when it runs on one.
#[derive(Debug)]
pub struct Fsc4b {
    trust: Trust,
    store: Arc<dyn Fsc4bStore>,
    public_url: String,
    demo: Option<Demo>,
}

impl Fsc4b {
    /// The channel on `trust`, keeping its state in `store`.
    pub fn new(
        trust: Trust,
        store: Arc<dyn Fsc4bStore>,
        public_url: String,
        demo: Option<Demo>,
    ) -> Self {
        Self {
            trust,
            store,
            public_url,
            demo,
        }
    }

    /// The channel `choice` configures in `mode`, keeping its state in
    /// Postgres when `pool` is given and in memory otherwise; `None` when
    /// it is off.
    ///
    /// # Errors
    ///
    /// When a file does not read, a key does not parse, the Stelseldocument
    /// does not verify, or stable mode is given a demo Stelsel.
    pub fn open(
        choice: &Fsc4bChoice,
        mode: Mode,
        pool: Option<PgPool>,
    ) -> Result<Option<Self>, Fsc4bError> {
        let (trust, public_url, demo) = match choice {
            Fsc4bChoice::Off => return Ok(None),
            Fsc4bChoice::On(settings) => (trust_of(settings)?, settings.public_url.clone(), None),
            Fsc4bChoice::Demo { public_url } => {
                let (trust, demo) = Demo::generate(public_url).map_err(Fsc4bError::Demo)?;
                (trust, public_url.clone(), Some(demo))
            }
        };
        // The configuration refuses the demo in stable mode already; this
        // holds whatever built the choice.
        if mode == Mode::Stable && (trust.is_demo() || demo.is_some()) {
            return Err(Fsc4bError::StableDemo);
        }
        let store: Arc<dyn Fsc4bStore> = match pool {
            Some(pool) => Arc::new(PostgresFsc4bStore::new(pool)),
            None => Arc::new(InMemoryFsc4bStore::default()),
        };
        Ok(Some(Self::new(trust, store, public_url, demo)))
    }

    pub fn trust(&self) -> &Trust {
        &self.trust
    }

    pub fn store(&self) -> &dyn Fsc4bStore {
        self.store.as_ref()
    }

    /// Where apps reach the server, without a trailing slash.
    pub fn public_url(&self) -> &str {
        &self.public_url
    }

    /// The demo Stelsel, when the channel runs on one.
    pub fn demo(&self) -> Option<&Demo> {
        self.demo.as_ref()
    }
}

/// The trust of a real Stelsel, from the files and keys `settings` name.
fn trust_of(settings: &Fsc4bSettings) -> Result<Trust, Fsc4bError> {
    let read = |what: &'static str, path: &std::path::Path| {
        std::fs::read_to_string(path).map_err(|source| Fsc4bError::Read {
            what,
            path: path.display().to_string(),
            source,
        })
    };
    let pem = match &settings.key {
        KeySource::Pem(pem) => pem.clone(),
        KeySource::File(path) => read("the organization's private key", path)?,
    };
    let key = SigningKey::from_pkcs8_pem(&pem).map_err(Fsc4bError::Key)?;
    let document = read("the Stelseldocument", &settings.stelseldocument)?;
    let beheerder: serde_json::Value = serde_json::from_str(&settings.stelselbeheerder_key)
        .map_err(|_| Fsc4bError::StelselbeheerderKey(JoseError::Key))?;
    let beheerder =
        VerifyingKey::from_value(&beheerder).map_err(Fsc4bError::StelselbeheerderKey)?;
    Ok(Trust::build(TrustInput {
        own_oin: &settings.oin,
        key,
        scope: &settings.scope,
        stelseldocument: document.trim(),
        stelselbeheerder: &beheerder,
    })?)
}

/// The routes of the open channel `fsc4b`: the session service, the
/// citizen's service, the discovery configuration and the OpenAPI
/// specification, and the demo Stelsel's routes when it runs on one.
pub fn routes(fsc4b: &Fsc4b) -> Router<AppState> {
    let router = Router::new()
        .route(CHALLENGE, post(session::challenge_route))
        .route(COMPLETE, post(session::complete_route))
        .route(CASE, post(citizen::case_route))
        .route(FORMS, post(citizen::forms_route))
        .route(DOSSIER, post(citizen::dossier_route))
        .route(SUBMIT, post(citizen::submit_route))
        .route(DISCOVERY, get(discovery::discovery_route))
        .route(OPENAPI, get(discovery::openapi_route));
    match fsc4b.demo() {
        Some(_) => router.merge(demo::routes()),
        None => router,
    }
}
