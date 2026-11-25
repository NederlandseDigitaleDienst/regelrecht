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
//! - [`handout`]: what an app needs to compute on the citizen's device: the
//!   signed source data about the citizen and the case as a bundle;
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
//!
//! A browser app on another address reaches the routes only when its origin
//! is configured ([`crate::config::Fsc4bOrigins`]): those get CORS headers,
//! a preflight included.

pub mod citizen;
/// The reference client, from the crate the app runs on too.
#[cfg(feature = "test-utils")]
pub use regelrecht_fsc4b::client;
pub mod demo;
pub mod discovery;
pub mod handout;
/// The JOSE formats, from the crate the app shares with this server.
pub use regelrecht_fsc4b::jose;
pub mod presentation;
pub mod session;
pub mod store;
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;
pub mod trust;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{get, post};
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::api::AppState;
use crate::config::{Fsc4bChoice, Fsc4bOrigins, Fsc4bSettings, KeySource, Mode};
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
/// `POST`: why a decision of the citizen's own dossier came out so.
pub const EXPLAIN: &str = "/fsc4b/burger/v1/waarom";
/// `POST`: the source data the executor holds about the citizen, signed.
pub const GEGEVENS: &str = "/fsc4b/burger/v1/gegevens";
/// `POST`: the timeline of the application a record of the citizen's own
/// dossier belongs to.
pub const TIMELINE: &str = "/fsc4b/burger/v1/tijdlijn";
/// `GET`: the case as a bundle of files, to compute on in the app; without
/// a session.
pub const REGELWERKEN: &str = "/fsc4b/regelwerken";
/// `GET`: how the screens of the process look (accent colour, title,
/// organisation), as the web front shows anyone; without a session.
pub use regelrecht_fsc4b::wire::APPEARANCE;
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
pub const ROUTES: [(&str, &str); 13] = [
    ("post", CHALLENGE),
    ("post", COMPLETE),
    ("post", CASE),
    ("post", FORMS),
    ("post", DOSSIER),
    ("post", SUBMIT),
    ("post", EXPLAIN),
    ("post", GEGEVENS),
    ("post", TIMELINE),
    ("get", REGELWERKEN),
    ("get", APPEARANCE),
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
/// sessions, where apps reach it, the demo Stelsel when it runs on one, the
/// case it hands apps, and the browser apps it answers.
#[derive(Debug)]
pub struct Fsc4b {
    trust: Trust,
    store: Arc<dyn Fsc4bStore>,
    public_url: String,
    demo: Option<Demo>,
    bundle: BTreeMap<String, String>,
    bundle_hash: String,
    origins: Fsc4bOrigins,
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
            bundle_hash: bundle_hash(&BTreeMap::new()),
            bundle: BTreeMap::new(),
            origins: Fsc4bOrigins::default(),
        }
    }

    /// The channel handing apps the case in `bundle`: the files of the
    /// source the server runs, by their path in it
    /// ([`crate::case::files`]).
    #[must_use]
    pub fn with_bundle(mut self, bundle: BTreeMap<String, String>) -> Self {
        self.bundle_hash = bundle_hash(&bundle);
        self.bundle = bundle;
        self
    }

    /// The channel answering the browser apps of `origins` with CORS
    /// headers.
    #[must_use]
    pub fn with_origins(mut self, origins: Fsc4bOrigins) -> Self {
        self.origins = origins;
        self
    }

    /// The case as the channel hands it to apps ([`REGELWERKEN`]).
    pub fn bundle(&self) -> &BTreeMap<String, String> {
        &self.bundle
    }

    /// The hash of [`Fsc4b::bundle`] ([`bundle_hash`]), which the signed
    /// source data carry so that an app can check the unsigned bundle.
    pub fn bundle_hash(&self) -> &str {
        &self.bundle_hash
    }

    /// The origins of the browser apps the channel answers.
    pub fn origins(&self) -> &Fsc4bOrigins {
        &self.origins
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

/// The hash of a bundle of files by path, as the source data carry it
/// (`bundel_hash`): `sha256:` and the lowercase hex SHA-256 of, per path in
/// ascending byte order, the decimal byte length of the path, `:`, the path,
/// the decimal byte length of its text, `:` and the text, all UTF-8 and
/// without separators. The lengths make the concatenation unambiguous
/// whatever a path or text holds. `{"a.yaml": "x"}` hashes `6:a.yaml1:x`.
pub fn bundle_hash(bundle: &BTreeMap<String, String>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    // A BTreeMap of Strings iterates in byte order of the keys.
    for (path, text) in bundle {
        hasher.update(format!("{}:", path.len()));
        hasher.update(path);
        hasher.update(format!("{}:", text.len()));
        hasher.update(text);
    }
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("sha256:{hex}")
}

/// The CORS headers for the browser apps of `origins`, on every path under
/// `/fsc4b/` and no other, `None` when there are no origins. It goes
/// around the whole router, so a path that is no route (a 404) and any
/// error answer carry the headers too, and the app can read why instead of
/// seeing a network error. An origin that is no header value was refused by
/// the configuration already; it is left out here rather than failing.
pub fn cors(origins: &Fsc4bOrigins) -> Option<CorsLayer> {
    let allowed: Vec<HeaderValue> = origins
        .as_slice()
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    if allowed.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::predicate(move |origin, request| {
                request.uri.path().starts_with(CHANNEL_PREFIX) && allowed.contains(origin)
            }))
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
            .max_age(std::time::Duration::from_secs(CORS_MAX_AGE_SECONDS)),
    )
}

/// What every path of the channel starts with.
const CHANNEL_PREFIX: &str = "/fsc4b/";

/// How long a browser may keep the answer to a preflight.
const CORS_MAX_AGE_SECONDS: u64 = 600;

/// The routes of the open channel `fsc4b`: the session service, the
/// citizen's service, the case for apps, the discovery configuration and
/// the OpenAPI specification, and the demo Stelsel's routes when it runs on
/// one. The CORS headers come from [`cors`], around the whole router.
pub fn routes(fsc4b: &Fsc4b) -> Router<AppState> {
    let router = Router::new()
        .route(CHALLENGE, post(session::challenge_route))
        .route(COMPLETE, post(session::complete_route))
        .route(CASE, post(citizen::case_route))
        .route(FORMS, post(citizen::forms_route))
        .route(DOSSIER, post(citizen::dossier_route))
        .route(SUBMIT, post(citizen::submit_route))
        .route(EXPLAIN, post(citizen::explain_route))
        .route(GEGEVENS, post(citizen::source_data_route))
        .route(TIMELINE, post(citizen::timeline_route))
        .route(REGELWERKEN, get(handout::regelwerken_route))
        .route(APPEARANCE, get(handout::appearance_route))
        .route(DISCOVERY, get(discovery::discovery_route))
        .route(OPENAPI, get(discovery::openapi_route));
    match fsc4b.demo() {
        Some(_) => router.merge(demo::routes()),
        None => router,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_hash_is_over_length_prefixed_paths_and_texts_in_order() {
        use sha2::{Digest, Sha256};
        let hex = |bytes: &[u8]| -> String {
            Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        };
        let one = BTreeMap::from([("a.yaml".to_string(), "x".to_string())]);
        assert_eq!(bundle_hash(&one), format!("sha256:{}", hex(b"6:a.yaml1:x")));
        let two = BTreeMap::from([
            ("b".to_string(), "ë".to_string()),
            ("a".to_string(), String::new()),
        ]);
        // Sorted by path; lengths in bytes, so "ë" counts 2.
        assert_eq!(
            bundle_hash(&two),
            format!("sha256:{}", hex("1:a0:1:b2:ë".as_bytes()))
        );
    }
}
