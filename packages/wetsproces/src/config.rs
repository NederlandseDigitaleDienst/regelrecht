//! The configuration of the server, from the environment.
//!
//! - `WETSPROCES_MODE`: `test` or `stable`, required. There is no default:
//!   the mode decides whether the clock is real and who may use the server,
//!   so a deployment says which it wants.
//! - `WETSPROCES_SURFACE`: which side the instance serves
//!   ([`crate::surface::Surface`]): `burger`, `backoffice` or `all`. By
//!   default `all` in test mode, so one command runs the whole process on a
//!   machine; stable mode has no default and refuses `all`, so each
//!   deployment says which side it is.
//! - `WETSPROCES_STORE`: where the chronicles live, `memory` or `postgres`.
//!   By default `memory` in test mode and `postgres` in stable mode, which
//!   takes no other.
//! - `DATABASE_URL`, or else `DATABASE_SERVER_FULL` as ZAD sets it: the
//!   database of the `postgres` store, required with it.
//! - `WETSPROCES_DB_MAX_CONNECTIONS`: how many connections the instance
//!   holds to that database at most, in its one pool; by default 5, at
//!   least 2 ([`PoolSize`]). Read only for the `postgres` store.
//! - `WETSPROCES_MANIFEST`: the source manifest to run, by default
//!   `packages/wetsproces/bron.yaml` of this checkout.
//! - `WETSPROCES_PORT`: the port, by default 8000.
//! - `WETSPROCES_FRONTEND`: the directory of the built screens, one
//!   build per surface in a directory of its name (`burger`, `backoffice`,
//!   `all`), by default `frontend-wetsproces/dist` of this checkout. The
//!   server serves the build of its own surface beside the API, and the API
//!   alone when that build has no `index.html`.
//! - `WETSPROCES_ACCENT_COLOR`: the accent colour of the screens, a theme
//!   of the NLDD design system such as `paars` ([`crate::theme`]). Without
//!   it the screens keep the design system's own accent; a name that is no
//!   theme does not start.
//!
//! The login ([`LoginChoice`]):
//!
//! - `WETSPROCES_LOGIN`: `testlogin` or `oidc`. Without it the server logs
//!   in through Keycloak when the OIDC variables below are set, and else,
//!   in test mode, offers the test login. `testlogin` forces the test login
//!   in test mode and ignores the OIDC variables, for a test environment
//!   whose platform sets them while the realm is not ready; stable mode
//!   refuses it. `oidc` requires the OIDC variables.
//! - `OIDC_CLIENT_ID`, `OIDC_CLIENT_SECRET` and `OIDC_DISCOVERY_URL` (or
//!   `KEYCLOAK_BASE_URL` with `KEYCLOAK_REALM`), as `regelrecht_auth` reads
//!   them: with these the server logs in through Keycloak. Stable mode
//!   requires them; test mode without them offers the test login instead.
//! - `OIDC_REQUIRED_ROLE`: the realm role every account needs to log in at
//!   all, required with OIDC and one of this server's roles
//!   ([`crate::access::ROLE_PREFIX`]): the realm is shared with the editor,
//!   so its default would let every account of the realm in.
//! - `BASE_URL`: where the server is reached, for the redirect after a
//!   login; required in stable mode.
//! - `WETSPROCES_SUBJECT_CLAIM`: the claim of a citizen's account that
//!   identifies them, by default `bsn`. The login keeps it in the session.
//!
//! The FSC voor Burgers channel ([`Fsc4bChoice`], [`crate::fsc4b`]), off
//! unless configured:
//!
//! - `WETSPROCES_FSC4B_OIN`: the OIN of this organization, the `aud` an app
//!   signs its nonce for;
//! - `WETSPROCES_FSC4B_SCOPE`: the scope a credential must carry;
//! - `WETSPROCES_FSC4B_KEY_FILE`, or `WETSPROCES_FSC4B_KEY` with the key
//!   itself: the organization's private ES256 key, PKCS#8 PEM, which signs
//!   the challenges. A secret: from the environment or a mounted file, never
//!   from the repository;
//! - `WETSPROCES_FSC4B_STELSELDOCUMENT_FILE`: the Stelseldocument, a JWS,
//!   whose `app_managers` are the Activatiediensten the server trusts;
//! - `WETSPROCES_FSC4B_STELSELBEHEERDER_KEY`: the public key of the
//!   Stelselbeheerder that signed it, a JWK;
//! - `WETSPROCES_FSC4B_PUBLIC_URL`: where apps reach the server, for the
//!   discovery configuration; `BASE_URL` when not set.
//!
//! These come all together or not at all. `WETSPROCES_FSC4B_DEMO=true`
//! instead opens the channel on a demo Stelsel the server makes at start,
//! with a test Activatiedienst; test mode only, and with none of the above.
//! The channel is for citizens: the back office refuses either.

use std::path::PathBuf;

use regelrecht_auth::OidcConfig;

use crate::access::{CITIZEN, Level, ROLE_PREFIX};
use crate::store::postgres::PoolSize;
use crate::surface::Surface;
use crate::theme::AccentColor;

/// The variables of the OIDC configuration besides `OIDC_CLIENT_ID`: one of
/// them set without it is a configuration half done, not a choice for the
/// test login.
const OIDC_VARIABLES: [&str; 5] = [
    "OIDC_CLIENT_SECRET",
    "OIDC_DISCOVERY_URL",
    "KEYCLOAK_BASE_URL",
    "KEYCLOAK_REALM",
    "OIDC_REQUIRED_ROLE",
];

/// How the server keeps its chronicles and tells the date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Chronicles in memory, empty at every start, and a clock the API can
    /// set: for trying the process out.
    Test,
    /// Chronicles that survive a restart and the real date in the
    /// Netherlands.
    Stable,
}

impl std::str::FromStr for Mode {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "test" => Ok(Self::Test),
            "stable" => Ok(Self::Stable),
            other => Err(ConfigError::UnknownMode(other.to_string())),
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Test => "test",
            Self::Stable => "stable",
        })
    }
}

/// Where the chronicles live, apart from the mode: test mode runs on either,
/// so a process with a clock the API sets can still keep its chronicles.
#[derive(Clone, PartialEq, Eq)]
pub enum StoreChoice {
    /// In memory, empty at every start.
    Memory,
    /// In PostgreSQL at `url`, in the schema `wetsproces`, with at most
    /// `max_connections` connections.
    Postgres {
        url: String,
        max_connections: PoolSize,
    },
}

impl StoreChoice {
    /// The postgres store at `url` with the default pool.
    pub fn postgres(url: impl Into<String>) -> Self {
        Self::Postgres {
            url: url.into(),
            max_connections: PoolSize::default(),
        }
    }

    /// How many connections the store holds at most; none in memory.
    pub fn max_connections(&self) -> Option<u32> {
        match self {
            Self::Memory => None,
            Self::Postgres {
                max_connections, ..
            } => Some(max_connections.get()),
        }
    }

    /// Its name, as `WETSPROCES_STORE` takes it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Postgres { .. } => "postgres",
        }
    }
}

impl std::fmt::Debug for StoreChoice {
    // The URL holds the database password.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory => f.write_str("Memory"),
            Self::Postgres {
                max_connections, ..
            } => write!(
                f,
                "Postgres {{ url: <redacted>, max_connections: {} }}",
                max_connections.get()
            ),
        }
    }
}

/// How people log in.
#[derive(Debug, Clone)]
pub enum LoginChoice {
    /// Through Keycloak, with OIDC.
    Oidc {
        oidc: OidcConfig,
        base_url: Option<String>,
    },
    /// Test mode without OIDC: `POST /auth/testlogin` gives a session with
    /// the roles and claims it is sent ([`crate::login`]).
    TestLogin,
}

impl LoginChoice {
    /// Its name, as `WETSPROCES_LOGIN` takes it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Oidc { .. } => "oidc",
            Self::TestLogin => "testlogin",
        }
    }
}

/// Whether and how the FSC voor Burgers channel is open.
#[derive(Clone, Default)]
pub enum Fsc4bChoice {
    /// Not configured: no route of the channel exists.
    #[default]
    Off,
    /// On the Stelseldocument and keys of the deployment.
    On(Fsc4bSettings),
    /// On a demo Stelsel made at start, with a test Activatiedienst; test
    /// mode only.
    Demo {
        /// Where apps reach the server, for the discovery configuration.
        public_url: String,
    },
}

impl std::fmt::Debug for Fsc4bChoice {
    // The settings hold the organization's private key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => f.write_str("Off"),
            Self::On(s) => f
                .debug_struct("On")
                .field("oin", &s.oin)
                .field("scope", &s.scope)
                .field("public_url", &s.public_url)
                .finish_non_exhaustive(),
            Self::Demo { public_url } => f
                .debug_struct("Demo")
                .field("public_url", public_url)
                .finish(),
        }
    }
}

/// Where the private key of the organization comes from.
#[derive(Clone)]
pub enum KeySource {
    /// The PEM itself, from the environment.
    Pem(String),
    /// A file holding the PEM, such as a mounted secret.
    File(PathBuf),
}

/// The configuration of the FSC voor Burgers channel on a real Stelsel.
#[derive(Clone)]
pub struct Fsc4bSettings {
    pub oin: String,
    pub scope: String,
    pub key: KeySource,
    pub stelseldocument: PathBuf,
    /// The Stelselbeheerder's public key, a JWK as JSON text.
    pub stelselbeheerder_key: String,
    pub public_url: String,
}

/// The variables of the FSC voor Burgers channel on a real Stelsel; the key
/// comes from one of `WETSPROCES_FSC4B_KEY` and `WETSPROCES_FSC4B_KEY_FILE`.
const FSC4B_VARIABLES: [&str; 6] = [
    "WETSPROCES_FSC4B_OIN",
    "WETSPROCES_FSC4B_SCOPE",
    "WETSPROCES_FSC4B_KEY",
    "WETSPROCES_FSC4B_KEY_FILE",
    "WETSPROCES_FSC4B_STELSELDOCUMENT_FILE",
    "WETSPROCES_FSC4B_STELSELBEHEERDER_KEY",
];

/// The claim that identifies a citizen when `WETSPROCES_SUBJECT_CLAIM` is
/// not set.
pub const DEFAULT_SUBJECT_CLAIM: &str = "bsn";

/// The port the server listens on when `WETSPROCES_PORT` is not set.
pub const DEFAULT_PORT: u16 = 8000;

/// The manifest when `WETSPROCES_MANIFEST` is not set: the one in this
/// crate, for a run from the checkout.
pub const DEFAULT_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/bron.yaml");

/// The built frontend when `WETSPROCES_FRONTEND` is not set: the one of this
/// checkout, after `npm run build -w frontend-wetsproces`.
pub const DEFAULT_FRONTEND: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../frontend-wetsproces/dist"
);

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    /// The side of the process this instance serves.
    pub surface: Surface,
    pub store: StoreChoice,
    pub manifest: PathBuf,
    /// The directory of the built screens, one build per surface in a
    /// directory of its name; the server serves its own surface's.
    pub frontend: PathBuf,
    pub port: u16,
    pub login: LoginChoice,
    /// The claim of a citizen's account that identifies them.
    pub subject_claim: String,
    /// The FSC voor Burgers channel.
    pub fsc4b: Fsc4bChoice,
    /// The accent colour of the screens; none keeps the design system's.
    pub accent: Option<AccentColor>,
}

/// Why the environment gives no configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("WETSPROCES_MODE is not set: set it to `test` or `stable`")]
    NoMode,
    #[error("WETSPROCES_MODE is `{0}`: it is `test` or `stable`")]
    UnknownMode(String),
    #[error("WETSPROCES_SURFACE is `{0}`: it is `burger`, `backoffice` or `all`")]
    UnknownSurface(String),
    #[error(
        "stable mode serves one side per deployment: set WETSPROCES_SURFACE to `burger` or \
         `backoffice`"
    )]
    StableNeedsSurface,
    #[error(
        "WETSPROCES_SURFACE is `all`, which serves citizens and employees on one address and is \
         for test mode only; stable mode serves `burger` or `backoffice`"
    )]
    StableSurfaceAll,
    #[error(
        "FSC voor Burgers is configured on the back office; the channel serves citizens, so it \
         belongs on WETSPROCES_SURFACE=burger"
    )]
    Fsc4bOnBackoffice,
    #[error("WETSPROCES_LOGIN is `{0}`: it is `testlogin` or `oidc`")]
    UnknownLogin(String),
    #[error(
        "WETSPROCES_LOGIN is `testlogin`, which lets anyone log in as any account and is for \
         test mode only; stable mode logs in through Keycloak"
    )]
    StableTestLogin,
    #[error(
        "WETSPROCES_LOGIN is `oidc`, but the OIDC configuration is not set: set OIDC_CLIENT_ID, \
         OIDC_CLIENT_SECRET and OIDC_DISCOVERY_URL (or KEYCLOAK_BASE_URL and KEYCLOAK_REALM)"
    )]
    OidcLoginWithoutOidc,
    #[error("WETSPROCES_ACCENT_COLOR: {0}")]
    UnknownAccentColor(#[from] crate::theme::UnknownAccentColor),
    #[error("WETSPROCES_PORT is `{0}`, which is no port")]
    BadPort(String),
    #[error("WETSPROCES_STORE is `{0}`: it is `memory` or `postgres`")]
    UnknownStore(String),
    #[error(
        "stable mode keeps its chronicles in Postgres, and WETSPROCES_STORE is `memory`; \
         set it to `postgres` or leave it out"
    )]
    StableNeedsPostgres,
    #[error("the postgres store needs DATABASE_URL or DATABASE_SERVER_FULL, and neither is set")]
    NoDatabase,
    #[error("WETSPROCES_DB_MAX_CONNECTIONS: {0}")]
    BadPoolSize(String),
    #[error("the OIDC configuration: {0}")]
    Oidc(String),
    #[error(
        "stable mode needs the login through Keycloak; set OIDC_CLIENT_ID, OIDC_CLIENT_SECRET \
         and OIDC_DISCOVERY_URL (or KEYCLOAK_BASE_URL and KEYCLOAK_REALM)"
    )]
    StableNeedsOidc,
    #[error(
        "OIDC_REQUIRED_ROLE is not set: set it to the realm role every account of this server \
         holds, such as wetsproces-toegang; the realm is shared, so there is no default"
    )]
    NoAccessRole,
    #[error(
        "OIDC_REQUIRED_ROLE is `{0}`; it is a role of this server (wetsproces-...) that every \
         account holds, such as wetsproces-toegang, and no citizen or level role"
    )]
    ForeignAccessRole(String),
    #[error(
        "{0} is set but OIDC_CLIENT_ID is not: set the OIDC configuration completely or not at \
         all; without it test mode opens the test login to anyone"
    )]
    PartialOidc(&'static str),
    #[error("BASE_URL: {0}")]
    BadBaseUrl(String),
    #[error("stable mode needs BASE_URL, the address the login redirects back to")]
    StableNeedsBaseUrl,
    #[error(
        "{0} is set, so FSC voor Burgers is being configured, but {1} is not: set \
         WETSPROCES_FSC4B_OIN, _SCOPE, _KEY or _KEY_FILE, _STELSELDOCUMENT_FILE and \
         _STELSELBEHEERDER_KEY together, or none of them"
    )]
    PartialFsc4b(&'static str, &'static str),
    #[error("set WETSPROCES_FSC4B_KEY or WETSPROCES_FSC4B_KEY_FILE, not both")]
    Fsc4bTwoKeys,
    #[error(
        "WETSPROCES_FSC4B_DEMO opens FSC voor Burgers on a demo Stelsel with a test \
         Activatiedienst, which only test mode allows"
    )]
    StableFsc4bDemo,
    #[error("WETSPROCES_FSC4B_DEMO is `{0}`: it is `true` or `false`")]
    BadFsc4bDemo(String),
    #[error("WETSPROCES_FSC4B_DEMO and {0} are both set: the demo makes its own Stelsel")]
    Fsc4bDemoAndSettings(&'static str),
    #[error(
        "FSC voor Burgers identifies a citizen by the `bsn` of a credential, and \
         WETSPROCES_SUBJECT_CLAIM is `{0}`"
    )]
    Fsc4bNeedsBsn(String),
    #[error("FSC voor Burgers needs WETSPROCES_FSC4B_PUBLIC_URL or BASE_URL")]
    Fsc4bNeedsPublicUrl,
    #[error("WETSPROCES_FSC4B_PUBLIC_URL: {0}")]
    BadFsc4bPublicUrl(String),
    #[error(
        "the address of FSC voor Burgers is `{0}`: stable mode publishes it in the discovery \
         configuration, which §4.2.2.1 asks to be HTTPS"
    )]
    StableFsc4bNeedsHttps(String),
}

impl Config {
    /// The configuration in the environment `env`, a lookup by name such as
    /// `|k| std::env::var(k).ok()`. An empty variable counts as not set.
    ///
    /// # Errors
    ///
    /// When the mode is missing or unknown, the port is no number, the
    /// store is unknown or does not fit the mode, the postgres store has no
    /// database, or the accent colour is no theme of the design system.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let set = |k: &str| env(k).filter(|v| !v.trim().is_empty());
        let mode = set("WETSPROCES_MODE").ok_or(ConfigError::NoMode)?.parse()?;
        let surface = surface_choice(mode, set("WETSPROCES_SURFACE").as_deref())?;
        let store = store_choice(
            mode,
            set("WETSPROCES_STORE").as_deref(),
            || regelrecht_shared::postgres::database_url(&env),
            set("WETSPROCES_DB_MAX_CONNECTIONS").as_deref(),
        )?;
        let manifest = set("WETSPROCES_MANIFEST")
            .map_or_else(|| PathBuf::from(DEFAULT_MANIFEST), PathBuf::from);
        let frontend = set("WETSPROCES_FRONTEND")
            .map_or_else(|| PathBuf::from(DEFAULT_FRONTEND), PathBuf::from);
        let port = match set("WETSPROCES_PORT") {
            Some(p) => p.trim().parse().map_err(|_| ConfigError::BadPort(p))?,
            None => DEFAULT_PORT,
        };
        let subject_claim = set("WETSPROCES_SUBJECT_CLAIM").map_or_else(
            || DEFAULT_SUBJECT_CLAIM.to_string(),
            |c| c.trim().to_string(),
        );
        let accent = set("WETSPROCES_ACCENT_COLOR")
            .map(|c| c.parse())
            .transpose()?;
        let login = login_choice(mode, &subject_claim, &set)?;
        let fsc4b = fsc4b_choice(mode, &subject_claim, port, &set)?;
        if !surface.serves_citizens() && !matches!(fsc4b, Fsc4bChoice::Off) {
            return Err(ConfigError::Fsc4bOnBackoffice);
        }
        Ok(Self {
            mode,
            surface,
            store,
            manifest,
            frontend,
            port,
            login,
            subject_claim,
            fsc4b,
            accent,
        })
    }
}

/// The surface `named` names for `mode`, or test mode's own when it names
/// none: [`Surface::All`], so one command runs the whole process locally.
/// Stable mode has no default and refuses `all`.
fn surface_choice(mode: Mode, named: Option<&str>) -> Result<Surface, ConfigError> {
    let surface = match named {
        Some(name) => name.parse().map_err(ConfigError::UnknownSurface)?,
        None if mode == Mode::Stable => return Err(ConfigError::StableNeedsSurface),
        None => Surface::All,
    };
    if mode == Mode::Stable && surface == Surface::All {
        return Err(ConfigError::StableSurfaceAll);
    }
    Ok(surface)
}

/// The login the environment `set` configures for `mode`, with the claim
/// `subject_claim` among the claims it keeps. `WETSPROCES_LOGIN` names it,
/// or else the OIDC variables decide.
fn login_choice(
    mode: Mode,
    subject_claim: &str,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<LoginChoice, ConfigError> {
    let required = match set("WETSPROCES_LOGIN").as_deref().map(str::trim) {
        // The OIDC variables are not read at all: a platform may set them
        // for a realm that cannot log anyone in to this server yet.
        Some("testlogin") => {
            return match mode {
                Mode::Stable => Err(ConfigError::StableTestLogin),
                Mode::Test => Ok(LoginChoice::TestLogin),
            };
        }
        Some("oidc") => true,
        None => false,
        Some(other) => return Err(ConfigError::UnknownLogin(other.to_string())),
    };
    let Some(mut oidc) = regelrecht_auth::parse_oidc_from(set).map_err(ConfigError::Oidc)? else {
        if let Some(name) = OIDC_VARIABLES.into_iter().find(|n| set(n).is_some()) {
            return Err(ConfigError::PartialOidc(name));
        }
        return match (mode, required) {
            (_, true) => Err(ConfigError::OidcLoginWithoutOidc),
            (Mode::Stable, false) => Err(ConfigError::StableNeedsOidc),
            (Mode::Test, false) => Ok(LoginChoice::TestLogin),
        };
    };
    // `parse_oidc_from` falls back to a role every account of the realm may
    // hold; this server takes only an explicit role of its own.
    let role = set("OIDC_REQUIRED_ROLE").ok_or(ConfigError::NoAccessRole)?;
    let role = role.trim();
    if !is_access_role(role) {
        return Err(ConfigError::ForeignAccessRole(role.to_string()));
    }
    oidc.required_role = role.to_string();
    if !oidc.extra_claims.iter().any(|c| c == subject_claim) {
        oidc.extra_claims.push(subject_claim.to_string());
    }
    let base_url = regelrecht_auth::parse_base_url_from(set).map_err(ConfigError::BadBaseUrl)?;
    if mode == Mode::Stable && base_url.is_none() {
        return Err(ConfigError::StableNeedsBaseUrl);
    }
    Ok(LoginChoice::Oidc { oidc, base_url })
}

/// The FSC voor Burgers channel the environment `set` configures for
/// `mode`.
fn fsc4b_choice(
    mode: Mode,
    subject_claim: &str,
    port: u16,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<Fsc4bChoice, ConfigError> {
    let demo = match set("WETSPROCES_FSC4B_DEMO").as_deref().map(str::trim) {
        None | Some("false") => false,
        Some("true") => true,
        Some(other) => return Err(ConfigError::BadFsc4bDemo(other.to_string())),
    };
    let given = FSC4B_VARIABLES.into_iter().find(|n| set(n).is_some());
    if demo {
        if mode == Mode::Stable {
            return Err(ConfigError::StableFsc4bDemo);
        }
        if let Some(name) = given {
            return Err(ConfigError::Fsc4bDemoAndSettings(name));
        }
    }
    let Some(first) = given.filter(|_| !demo) else {
        if !demo {
            return Ok(Fsc4bChoice::Off);
        }
        check_bsn_claim(subject_claim)?;
        let public_url = public_url(set)?.unwrap_or_else(|| format!("http://localhost:{port}"));
        return Ok(Fsc4bChoice::Demo { public_url });
    };
    check_bsn_claim(subject_claim)?;
    let need = |name: &'static str| set(name).ok_or(ConfigError::PartialFsc4b(first, name));
    let key = match (
        set("WETSPROCES_FSC4B_KEY"),
        set("WETSPROCES_FSC4B_KEY_FILE"),
    ) {
        (Some(pem), None) => KeySource::Pem(pem),
        (None, Some(path)) => KeySource::File(PathBuf::from(path.trim())),
        (Some(_), Some(_)) => return Err(ConfigError::Fsc4bTwoKeys),
        (None, None) => return Err(ConfigError::PartialFsc4b(first, "WETSPROCES_FSC4B_KEY")),
    };
    let settings = Fsc4bSettings {
        oin: need("WETSPROCES_FSC4B_OIN")?.trim().to_string(),
        scope: need("WETSPROCES_FSC4B_SCOPE")?.trim().to_string(),
        key,
        stelseldocument: PathBuf::from(need("WETSPROCES_FSC4B_STELSELDOCUMENT_FILE")?.trim()),
        stelselbeheerder_key: need("WETSPROCES_FSC4B_STELSELBEHEERDER_KEY")?,
        public_url: public_url(set)?.ok_or(ConfigError::Fsc4bNeedsPublicUrl)?,
    };
    if mode == Mode::Stable && !settings.public_url.starts_with("https://") {
        return Err(ConfigError::StableFsc4bNeedsHttps(settings.public_url));
    }
    Ok(Fsc4bChoice::On(settings))
}

/// The channel reads the citizen's identity from the `bsn` of a credential,
/// which fills the same claim as the login does.
fn check_bsn_claim(subject_claim: &str) -> Result<(), ConfigError> {
    if subject_claim == DEFAULT_SUBJECT_CLAIM {
        Ok(())
    } else {
        Err(ConfigError::Fsc4bNeedsBsn(subject_claim.to_string()))
    }
}

/// `WETSPROCES_FSC4B_PUBLIC_URL`, or else `BASE_URL`, without a trailing
/// slash; it must be an `http(s)` URL.
fn public_url(set: &impl Fn(&str) -> Option<String>) -> Result<Option<String>, ConfigError> {
    let Some(url) = set("WETSPROCES_FSC4B_PUBLIC_URL").or_else(|| set("BASE_URL")) else {
        return Ok(None);
    };
    let url = url.trim().trim_end_matches('/').to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ConfigError::BadFsc4bPublicUrl(url));
    }
    Ok(Some(url))
}

/// Whether `role` can be the role every account needs to log in: a role
/// of this server, and none that only some accounts hold.
fn is_access_role(role: &str) -> bool {
    role.strip_prefix(ROLE_PREFIX)
        .is_some_and(|rest| !rest.is_empty())
        && role != CITIZEN
        && !Level::ALL.iter().any(|l| l.role() == role)
}

/// The store `WETSPROCES_STORE` names for `mode`, or the mode's own when it
/// names none; `database_url` and the pool size `max_connections` count only
/// for the postgres store.
fn store_choice(
    mode: Mode,
    named: Option<&str>,
    database_url: impl FnOnce() -> Option<String>,
    max_connections: Option<&str>,
) -> Result<StoreChoice, ConfigError> {
    let postgres = match named.map(str::trim) {
        None => mode == Mode::Stable,
        Some("postgres") => true,
        Some("memory") if mode == Mode::Stable => return Err(ConfigError::StableNeedsPostgres),
        Some("memory") => false,
        Some(other) => return Err(ConfigError::UnknownStore(other.to_string())),
    };
    if !postgres {
        return Ok(StoreChoice::Memory);
    }
    let url = database_url().ok_or(ConfigError::NoDatabase)?;
    let max_connections = max_connections
        .map(str::parse)
        .transpose()
        .map_err(ConfigError::BadPoolSize)?
        .unwrap_or_default();
    Ok(StoreChoice::Postgres {
        url,
        max_connections,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    type Pairs = &'static [(&'static str, &'static str)];

    /// The environment of `pairs`, the later ones over the earlier.
    fn env(pairs: &[Pairs]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(&str, &str)> = pairs.iter().flat_map(|p| p.iter().copied()).collect();
        move |k| {
            pairs
                .iter()
                .rev()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    fn error(pairs: &[Pairs]) -> ConfigError {
        Config::from_env(env(pairs)).unwrap_err()
    }

    /// A login through Keycloak, as stable mode requires; the hosts are
    /// made up.
    const OIDC: Pairs = &[
        ("OIDC_CLIENT_ID", "wetsproces"),
        ("OIDC_CLIENT_SECRET", "secret"),
        (
            "OIDC_DISCOVERY_URL",
            "https://login.example.org/realms/voorbeeld",
        ),
        ("OIDC_REQUIRED_ROLE", "wetsproces-toegang"),
        ("BASE_URL", "https://wetsproces.example.org"),
    ];

    const STABLE: Pairs = &[
        ("WETSPROCES_MODE", "stable"),
        ("WETSPROCES_SURFACE", "burger"),
        ("DATABASE_URL", "postgres://db/regelrecht"),
    ];

    #[test]
    fn the_mode_is_required_and_has_no_default() {
        assert_eq!(error(&[]), ConfigError::NoMode);
        assert_eq!(error(&[&[("WETSPROCES_MODE", " ")]]), ConfigError::NoMode);
        assert_eq!(
            error(&[&[("WETSPROCES_MODE", "prod")]]),
            ConfigError::UnknownMode("prod".into())
        );
    }

    #[test]
    fn test_mode_with_the_defaults() {
        let c = Config::from_env(env(&[&[("WETSPROCES_MODE", "test")]])).unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(c.store, StoreChoice::Memory);
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.manifest, PathBuf::from(DEFAULT_MANIFEST));
        assert!(matches!(c.login, LoginChoice::TestLogin));
        assert_eq!(c.subject_claim, DEFAULT_SUBJECT_CLAIM);
        assert_eq!(c.surface, Surface::All);
    }

    #[test]
    fn stable_mode_names_its_surface_and_it_is_not_all() {
        let c = Config::from_env(env(&[STABLE, OIDC])).unwrap();
        assert_eq!(c.surface, Surface::Citizen);
        let c = Config::from_env(env(&[
            STABLE,
            OIDC,
            &[("WETSPROCES_SURFACE", "backoffice")],
        ]))
        .unwrap();
        assert_eq!(c.surface, Surface::Backoffice);
        assert_eq!(
            error(&[STABLE, OIDC, &[("WETSPROCES_SURFACE", "")]]),
            ConfigError::StableNeedsSurface
        );
        assert_eq!(
            error(&[STABLE, OIDC, &[("WETSPROCES_SURFACE", "all")]]),
            ConfigError::StableSurfaceAll
        );
        assert_eq!(
            error(&[&[
                ("WETSPROCES_MODE", "test"),
                ("WETSPROCES_SURFACE", "medewerker")
            ]]),
            ConfigError::UnknownSurface("medewerker".into())
        );
        let c = Config::from_env(env(&[&[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_SURFACE", "backoffice"),
        ]]))
        .unwrap();
        assert_eq!(c.surface, Surface::Backoffice);
    }

    #[test]
    fn fsc_voor_burgers_is_refused_on_the_back_office() {
        let backoffice: Pairs = &[("WETSPROCES_SURFACE", "backoffice")];
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, backoffice]),
            ConfigError::Fsc4bOnBackoffice
        );
        assert_eq!(
            error(&[
                &[
                    ("WETSPROCES_MODE", "test"),
                    ("WETSPROCES_FSC4B_DEMO", "true")
                ],
                backoffice
            ]),
            ConfigError::Fsc4bOnBackoffice
        );
    }

    #[test]
    fn the_manifest_and_port_come_from_the_environment() {
        let c = Config::from_env(env(&[
            STABLE,
            OIDC,
            &[
                ("WETSPROCES_MANIFEST", "/app/bron.yaml"),
                ("WETSPROCES_PORT", "7180"),
            ],
        ]))
        .unwrap();
        assert_eq!(c.mode, Mode::Stable);
        assert_eq!(c.manifest, PathBuf::from("/app/bron.yaml"));
        assert_eq!(c.port, 7180);
        assert_eq!(
            error(&[&[("WETSPROCES_MODE", "test"), ("WETSPROCES_PORT", "x")]]),
            ConfigError::BadPort("x".into())
        );
    }

    #[test]
    fn the_accent_colour_is_a_theme_of_the_design_system_or_nothing() {
        let test = &[("WETSPROCES_MODE", "test")];
        assert_eq!(Config::from_env(env(&[test])).unwrap().accent, None);
        let c = Config::from_env(env(&[test, &[("WETSPROCES_ACCENT_COLOR", "paars")]])).unwrap();
        assert_eq!(c.accent.map(AccentColor::name), Some("paars"));
        assert!(matches!(
            error(&[test, &[("WETSPROCES_ACCENT_COLOR", "#7d3f98")]]),
            ConfigError::UnknownAccentColor(_)
        ));
    }

    #[test]
    fn the_pool_size_comes_from_the_environment_with_a_low_default() {
        const PG: Pairs = &[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_STORE", "postgres"),
            ("DATABASE_URL", "postgres://db/regelrecht"),
        ];
        let c = Config::from_env(env(&[PG])).unwrap();
        assert_eq!(c.store.max_connections(), Some(5));
        let c = Config::from_env(env(&[PG, &[("WETSPROCES_DB_MAX_CONNECTIONS", "3")]])).unwrap();
        assert_eq!(c.store.max_connections(), Some(3));
        // Too few for the migration, or no number: refused at start.
        const TOO_FEW: Pairs = &[("WETSPROCES_DB_MAX_CONNECTIONS", "1")];
        const NO_NUMBER: Pairs = &[("WETSPROCES_DB_MAX_CONNECTIONS", "veel")];
        for wrong in [TOO_FEW, NO_NUMBER] {
            assert!(matches!(error(&[PG, wrong]), ConfigError::BadPoolSize(_)));
        }
        // The store in memory has no pool and reads none.
        let c = Config::from_env(env(&[&[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_DB_MAX_CONNECTIONS", "veel"),
        ]]))
        .unwrap();
        assert_eq!(c.store.max_connections(), None);
    }

    #[test]
    fn the_store_is_chosen_apart_from_the_mode() {
        let c = Config::from_env(env(&[&[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_STORE", "postgres"),
            ("DATABASE_SERVER_FULL", "postgres://zad/regelrecht"),
        ]]))
        .unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(c.store, StoreChoice::postgres("postgres://zad/regelrecht"));
        assert_eq!(
            error(&[&[("WETSPROCES_MODE", "test"), ("WETSPROCES_STORE", "file")]]),
            ConfigError::UnknownStore("file".into())
        );
    }

    #[test]
    fn stable_mode_keeps_its_chronicles_in_postgres() {
        let c = Config::from_env(env(&[STABLE, OIDC])).unwrap();
        assert_eq!(c.store.name(), "postgres");
        assert_eq!(
            error(&[STABLE, OIDC, &[("WETSPROCES_STORE", "memory")]]),
            ConfigError::StableNeedsPostgres
        );
    }

    #[test]
    fn the_postgres_store_needs_a_database() {
        for pairs in [
            &[
                ("WETSPROCES_MODE", "stable"),
                ("WETSPROCES_SURFACE", "burger"),
            ][..],
            &[
                ("WETSPROCES_MODE", "test"),
                ("WETSPROCES_STORE", "postgres"),
            ][..],
        ] {
            let pairs: Pairs = pairs;
            assert_eq!(error(&[pairs, OIDC]), ConfigError::NoDatabase);
        }
    }

    #[test]
    fn stable_mode_logs_in_through_keycloak_with_the_subject_claim() {
        let c = Config::from_env(env(&[STABLE, OIDC])).unwrap();
        let LoginChoice::Oidc { oidc, base_url } = c.login else {
            panic!("stable mode logs in through OIDC");
        };
        assert_eq!(oidc.required_role, "wetsproces-toegang");
        assert_eq!(oidc.extra_claims, [DEFAULT_SUBJECT_CLAIM]);
        assert_eq!(base_url.as_deref(), Some("https://wetsproces.example.org"));
        let c = Config::from_env(env(&[
            STABLE,
            OIDC,
            &[
                ("WETSPROCES_SUBJECT_CLAIM", "kvk_nummer"),
                ("OIDC_EXTRA_CLAIMS", "kvk_nummer,naam"),
            ],
        ]))
        .unwrap();
        let LoginChoice::Oidc { oidc, .. } = c.login else {
            panic!("stable mode logs in through OIDC");
        };
        assert_eq!(oidc.extra_claims, ["kvk_nummer", "naam"]);
        assert_eq!(c.subject_claim, "kvk_nummer");
    }

    #[test]
    fn stable_mode_refuses_without_oidc_an_access_role_or_a_base_url() {
        assert_eq!(error(&[STABLE]), ConfigError::StableNeedsOidc);
        assert_eq!(
            error(&[STABLE, OIDC, &[("OIDC_REQUIRED_ROLE", "")]]),
            ConfigError::NoAccessRole
        );
        assert_eq!(
            error(&[STABLE, OIDC, &[("OIDC_REQUIRED_ROLE", "editor-reader")]]),
            ConfigError::ForeignAccessRole("editor-reader".into())
        );
        for role in [
            "wetsproces-",
            "wetsproces-burger",
            "wetsproces-lezer",
            "wetsproces-admin",
        ] {
            let base = env(&[STABLE, OIDC]);
            let with_role = |k: &str| match k {
                "OIDC_REQUIRED_ROLE" => Some(role.to_string()),
                other => base(other),
            };
            assert_eq!(
                Config::from_env(with_role).unwrap_err(),
                ConfigError::ForeignAccessRole(role.into())
            );
        }
        assert_eq!(
            error(&[STABLE, OIDC, &[("BASE_URL", "")]]),
            ConfigError::StableNeedsBaseUrl
        );
        assert!(matches!(
            error(&[STABLE, OIDC, &[("OIDC_CLIENT_SECRET", "")]]),
            ConfigError::Oidc(_)
        ));
    }

    #[test]
    fn test_mode_with_oidc_logs_in_through_keycloak_and_needs_the_access_role_too() {
        let test: Pairs = &[("WETSPROCES_MODE", "test")];
        let c = Config::from_env(env(&[test, OIDC, &[("BASE_URL", "")]])).unwrap();
        assert!(matches!(c.login, LoginChoice::Oidc { base_url: None, .. }));
        assert_eq!(
            error(&[test, OIDC, &[("OIDC_REQUIRED_ROLE", "")]]),
            ConfigError::NoAccessRole
        );
    }

    #[test]
    fn a_half_done_oidc_configuration_does_not_open_the_test_login() {
        let test: Pairs = &[("WETSPROCES_MODE", "test")];
        assert_eq!(
            error(&[test, OIDC, &[("OIDC_CLIENT_ID", "")]]),
            ConfigError::PartialOidc("OIDC_CLIENT_SECRET")
        );
        assert_eq!(
            error(&[test, &[("KEYCLOAK_REALM", "voorbeeld")]]),
            ConfigError::PartialOidc("KEYCLOAK_REALM")
        );
    }

    #[test]
    fn wetsproces_login_testlogin_forces_the_test_login_in_test_mode() {
        let test: Pairs = &[("WETSPROCES_MODE", "test")];
        let testlogin: Pairs = &[("WETSPROCES_LOGIN", "testlogin")];
        let c = Config::from_env(env(&[test, OIDC, testlogin])).unwrap();
        assert!(matches!(c.login, LoginChoice::TestLogin));
        // The OIDC variables are ignored, even when they would not do.
        let c = Config::from_env(env(&[
            test,
            OIDC,
            &[("OIDC_REQUIRED_ROLE", ""), ("OIDC_CLIENT_SECRET", "")],
            testlogin,
        ]))
        .unwrap();
        assert!(matches!(c.login, LoginChoice::TestLogin));
        assert_eq!(c.login.name(), "testlogin");
        assert_eq!(
            error(&[STABLE, OIDC, testlogin]),
            ConfigError::StableTestLogin
        );
    }

    #[test]
    fn wetsproces_login_oidc_requires_the_oidc_variables() {
        let test: Pairs = &[("WETSPROCES_MODE", "test")];
        let oidc: Pairs = &[("WETSPROCES_LOGIN", "oidc")];
        assert_eq!(error(&[test, oidc]), ConfigError::OidcLoginWithoutOidc);
        assert_eq!(error(&[STABLE, oidc]), ConfigError::OidcLoginWithoutOidc);
        let c = Config::from_env(env(&[test, OIDC, oidc])).unwrap();
        assert_eq!(c.login.name(), "oidc");
        // Named or not, OIDC keeps its own checks.
        assert_eq!(
            error(&[test, OIDC, oidc, &[("OIDC_REQUIRED_ROLE", "")]]),
            ConfigError::NoAccessRole
        );
    }

    #[test]
    fn an_unknown_wetsproces_login_does_not_start() {
        assert_eq!(
            error(&[&[
                ("WETSPROCES_MODE", "test"),
                ("WETSPROCES_LOGIN", "keycloak")
            ]]),
            ConfigError::UnknownLogin("keycloak".into())
        );
    }

    const FSC4B: Pairs = &[
        ("WETSPROCES_FSC4B_OIN", "00000099000000000001"),
        ("WETSPROCES_FSC4B_SCOPE", "nl.voorbeeld.wetsproces"),
        ("WETSPROCES_FSC4B_KEY_FILE", "/run/secrets/fsc4b.pem"),
        ("WETSPROCES_FSC4B_STELSELDOCUMENT_FILE", "/app/stelsel.jws"),
        ("WETSPROCES_FSC4B_STELSELBEHEERDER_KEY", "{}"),
        (
            "WETSPROCES_FSC4B_PUBLIC_URL",
            "https://wetsproces.example.org/",
        ),
    ];

    #[test]
    fn fsc_voor_burgers_is_off_unless_configured() {
        let c = Config::from_env(env(&[STABLE, OIDC])).unwrap();
        assert!(matches!(c.fsc4b, Fsc4bChoice::Off));
        let c = Config::from_env(env(&[STABLE, OIDC, FSC4B])).unwrap();
        let Fsc4bChoice::On(s) = c.fsc4b else {
            panic!("configured, so on");
        };
        assert_eq!(s.oin, "00000099000000000001");
        assert_eq!(s.public_url, "https://wetsproces.example.org");
        assert!(matches!(s.key, KeySource::File(_)));
    }

    #[test]
    fn a_half_done_fsc_voor_burgers_configuration_does_not_start() {
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, &[("WETSPROCES_FSC4B_SCOPE", "")]]),
            ConfigError::PartialFsc4b("WETSPROCES_FSC4B_OIN", "WETSPROCES_FSC4B_SCOPE")
        );
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, &[("WETSPROCES_FSC4B_KEY", "pem")]]),
            ConfigError::Fsc4bTwoKeys
        );
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, &[("WETSPROCES_SUBJECT_CLAIM", "kvk")]]),
            ConfigError::Fsc4bNeedsBsn("kvk".into())
        );
        assert_eq!(
            error(&[
                STABLE,
                OIDC,
                FSC4B,
                &[(
                    "WETSPROCES_FSC4B_PUBLIC_URL",
                    "http://wetsproces.example.org"
                )]
            ]),
            ConfigError::StableFsc4bNeedsHttps("http://wetsproces.example.org".into())
        );
    }

    #[test]
    fn the_demo_stelsel_is_for_test_mode_only() {
        assert_eq!(
            error(&[STABLE, OIDC, &[("WETSPROCES_FSC4B_DEMO", "true")]]),
            ConfigError::StableFsc4bDemo
        );
        let test: Pairs = &[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_FSC4B_DEMO", "true"),
        ];
        let c = Config::from_env(env(&[test])).unwrap();
        assert!(matches!(c.fsc4b, Fsc4bChoice::Demo { .. }));
        assert_eq!(
            error(&[test, &[("WETSPROCES_FSC4B_OIN", "1")]]),
            ConfigError::Fsc4bDemoAndSettings("WETSPROCES_FSC4B_OIN")
        );
    }

    #[test]
    fn the_private_key_stays_out_of_debug_output() {
        let mut env_pairs = FSC4B.to_vec();
        env_pairs.retain(|(k, _)| *k != "WETSPROCES_FSC4B_KEY_FILE");
        env_pairs.push(("WETSPROCES_FSC4B_KEY", "-----BEGIN PRIVATE KEY-----geheim"));
        let leaked: &'static [(&'static str, &'static str)] =
            Box::leak(env_pairs.into_boxed_slice());
        let c = Config::from_env(env(&[STABLE, OIDC, leaked])).unwrap();
        assert!(!format!("{c:?}").contains("geheim"));
    }

    #[test]
    fn the_database_url_stays_out_of_debug_output() {
        let store = StoreChoice::postgres("postgres://user:secret@db/regelrecht");
        assert!(!format!("{store:?}").contains("secret"));
    }
}
