//! The configuration of the server, from the environment.
//!
//! - `REGELHET_MODE`: `test` or `stable`, required. There is no default:
//!   the mode decides whether the clock is real and who may use the server,
//!   so a deployment says which it wants.
//! - `REGELHET_SURFACE`: which side the instance serves
//!   ([`crate::surface::Surface`]): `burger`, `backoffice` or `all`. By
//!   default `all` in test mode, so one command runs the whole process on a
//!   machine; stable mode has no default and refuses `all`, so each
//!   deployment says which side it is.
//! - `REGELHET_STORE`: where the chronicles live, `memory` or `postgres`.
//!   By default `memory` in test mode and `postgres` in stable mode, which
//!   takes no other.
//! - `DATABASE_URL`, or else `DATABASE_SERVER_FULL` as ZAD sets it: the
//!   database of the `postgres` store, required with it.
//! - `REGELHET_DB_MAX_CONNECTIONS`: how many connections the instance
//!   holds to that database at most, in its one pool; by default 5, at
//!   least 2 ([`PoolSize`]). Read only for the `postgres` store.
//! - `REGELHET_MANIFEST`: the source manifest to run, by default
//!   `packages/regelhet/bron.yaml` of this checkout.
//! - `REGELHET_PORT`: the port, by default 8000.
//! - `REGELHET_FRONTEND`: the directory of the built screens, one
//!   build per surface in a directory of its name (`burger`, `backoffice`,
//!   `all`), by default `frontend-regelhet/dist` of this checkout. The
//!   server serves the build of its own surface beside the API, and the API
//!   alone when that build has no `index.html`.
//! - `REGELHET_ACCENT_COLOR`: the accent colour of the screens, a theme
//!   of the NLDD design system such as `paars` ([`crate::theme`]). Without
//!   it the screens wear `donkerblauw`; a name that is no
//!   theme does not start.
//! - `REGELHET_CASUS`: the case to run, by the name of its case file
//!   (`bedrijfsparkeervergunning` for `casussen/bedrijfsparkeervergunning.yaml`),
//!   for a manifest that makes several ([`crate::case`]). Without it the
//!   manifest must make exactly one; a name it does not make does not start.
//!   The front of an organisation runs no case and refuses it.
//! - `REGELHET_TITLE`: the title in the bar of the screens
//!   ([`crate::theme::Title`]), at most 80 characters on one line. Without
//!   it the screens carry the case's own `titel`. The front of an
//!   organisation carries the organisation's name and refuses it.
//!
//! The login ([`LoginChoice`]):
//!
//! - `REGELHET_LOGIN`: `testlogin` or `oidc`. Without it the server logs
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
//! - `REGELHET_SUBJECT_CLAIM`: the claim of an applicant's account that
//!   identifies them. Without it the case decides: the key of its login
//!   (`inloggen.sleutel`, such as `kvk_nummer`), or else `bsn`. The login
//!   keeps it in the session. With Keycloak it must be set when the case's
//!   key is not `bsn`, so the client asks for that claim.
//!
//! The FSC voor Burgers channel ([`Fsc4bChoice`], [`crate::fsc4b`]), off
//! unless configured:
//!
//! - `REGELHET_FSC4B_OIN`: the OIN of this organization, the `aud` an app
//!   signs its nonce for;
//! - `REGELHET_FSC4B_SCOPE`: the scope a credential must carry;
//! - `REGELHET_FSC4B_KEY_FILE`, or `REGELHET_FSC4B_KEY` with the key
//!   itself: the organization's private ES256 key, PKCS#8 PEM, which signs
//!   the challenges. A secret: from the environment or a mounted file, never
//!   from the repository;
//! - `REGELHET_FSC4B_STELSELDOCUMENT_FILE`: the Stelseldocument, a JWS,
//!   whose `app_managers` are the Activatiediensten the server trusts;
//! - `REGELHET_FSC4B_STELSELBEHEERDER_KEY`: the public key of the
//!   Stelselbeheerder that signed it, a JWK;
//! - `REGELHET_FSC4B_PUBLIC_URL`: where apps reach the server, for the
//!   discovery configuration; `BASE_URL` when not set.
//!
//! - `REGELHET_FSC4B_ORIGINS`: the origins of the web apps that call the
//!   channel from the browser, comma-separated, each `scheme://host[:port]`
//!   ([`Fsc4bOrigins`]). The routes under `/fsc4b/` answer those origins
//!   with CORS headers; without it they send none, and a browser app on
//!   another address cannot read them. Optional, with the channel open
//!   only.
//!
//! These come all together or not at all, the origins excepted.
//! `REGELHET_FSC4B_DEMO=true`
//! instead opens the channel on a demo Stelsel the server makes at start,
//! with a test Activatiedienst; test mode only, and with none of the above.
//! The channel is for citizens: the back office refuses either.
//!
//! The role ([`Role`], [`crate::organisatie::settings`]): `REGELHET_ROL`
//! is `proces` (the default), a server that runs one legal process, or
//! `organisatie`, the front of an organisation before its processes. The
//! variables of each role, and of a process that belongs to an organisation,
//! are listed there.

use std::path::PathBuf;

use regelrecht_auth::OidcConfig;

use crate::access::{CITIZEN, Level, ROLE_PREFIX};
pub use crate::organisatie::settings::{FrontSettings, Membership, OrganisationKey, Role};
use crate::store::postgres::PoolSize;
use crate::surface::Surface;
use crate::theme::{AccentColor, Title};

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
    /// In PostgreSQL at `url`, in the schema `regelhet`, with at most
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

    /// Its name, as `REGELHET_STORE` takes it.
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
    /// Its name, as `REGELHET_LOGIN` takes it.
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

/// The origins of the web apps that may call the FSC voor Burgers channel
/// from a browser (`REGELHET_FSC4B_ORIGINS`): each a scheme, a host and
/// perhaps a port, with no path. Empty sends no CORS headers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fsc4bOrigins(Vec<String>);

impl Fsc4bOrigins {
    /// The origins in `list`, comma-separated.
    ///
    /// # Errors
    ///
    /// [`ConfigError::BadFsc4bOrigin`] for an entry that is no
    /// `http(s)://host[:port]`, such as one with a path or a trailing slash,
    /// or `*`: a browser compares the origin exactly, and the channel names
    /// the apps it answers.
    pub fn parse(list: &str) -> Result<Self, ConfigError> {
        let mut origins = Vec::new();
        for origin in list.split(',').map(str::trim).filter(|o| !o.is_empty()) {
            let authority = origin
                .strip_prefix("https://")
                .or_else(|| origin.strip_prefix("http://"));
            if !authority.is_some_and(valid_authority) {
                return Err(ConfigError::BadFsc4bOrigin(origin.to_string()));
            }
            origins.push(origin.to_string());
        }
        Ok(Self(origins))
    }

    /// Whether every origin is `https`, or one on this machine
    /// (`localhost`, `127.0.0.1`), as stable mode asks.
    fn all_https(&self) -> bool {
        self.0.iter().all(|o| {
            o.starts_with("https://")
                || ["http://localhost", "http://127.0.0.1"]
                    .iter()
                    .any(|local| o == local || o.starts_with(&format!("{local}:")))
        })
    }

    /// The origins, as a browser sends them in `Origin`.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Whether `authority` is a host with perhaps a port, as a browser writes
/// it in `Origin`: a lowercase name or IPv4 address, or an IPv6 address in
/// brackets, then `:` and the digits of the port if any.
fn valid_authority(authority: &str) -> bool {
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port))
            if !host.ends_with(':') && (!host.contains(':') || host.ends_with(']')) =>
        {
            (host, Some(port))
        }
        _ => (authority, None),
    };
    let host_ok = if let Some(ip6) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !ip6.is_empty() && ip6.bytes().all(|b| b.is_ascii_hexdigit() || b == b':')
    } else {
        !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    };
    let port_ok =
        port.is_none_or(|p| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    host_ok && port_ok
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
/// comes from one of `REGELHET_FSC4B_KEY` and `REGELHET_FSC4B_KEY_FILE`.
const FSC4B_VARIABLES: [&str; 6] = [
    "REGELHET_FSC4B_OIN",
    "REGELHET_FSC4B_SCOPE",
    "REGELHET_FSC4B_KEY",
    "REGELHET_FSC4B_KEY_FILE",
    "REGELHET_FSC4B_STELSELDOCUMENT_FILE",
    "REGELHET_FSC4B_STELSELBEHEERDER_KEY",
];

/// The claim that identifies a citizen when `REGELHET_SUBJECT_CLAIM` is
/// not set.
pub const DEFAULT_SUBJECT_CLAIM: &str = "bsn";

/// The claim of the user name an OIDC login keeps beside the subject
/// claim: Keycloak's `preferred_username`.
pub const USERNAME_CLAIM: &str = "preferred_username";

/// The port the server listens on when `REGELHET_PORT` is not set.
pub const DEFAULT_PORT: u16 = 8000;

/// The manifest when `REGELHET_MANIFEST` is not set: the one in this
/// crate, for a run from the checkout.
pub const DEFAULT_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/bron.yaml");

/// The built frontend when `REGELHET_FRONTEND` is not set: the one of this
/// checkout, after `npm run build -w frontend-regelhet`.
pub const DEFAULT_FRONTEND: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../frontend-regelhet/dist");

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    /// The side of the process this instance serves.
    pub surface: Surface,
    pub store: StoreChoice,
    pub manifest: PathBuf,
    /// The case of the manifest to run, by its id; none runs its only case.
    pub case: Option<String>,
    /// The directory of the built screens, one build per surface in a
    /// directory of its name; the server serves its own surface's.
    pub frontend: PathBuf,
    pub port: u16,
    pub login: LoginChoice,
    /// The claim of an applicant's account that identifies them, when
    /// `REGELHET_SUBJECT_CLAIM` names one; without, the case decides
    /// ([`Config::subject_claim_for`]).
    pub subject_claim: Option<String>,
    /// The FSC voor Burgers channel.
    pub fsc4b: Fsc4bChoice,
    /// The origins of the browser apps that may call the channel.
    pub fsc4b_origins: Fsc4bOrigins,
    /// The accent colour of the screens; none leaves them `donkerblauw`.
    pub accent: Option<AccentColor>,
    /// The title in the bar of the screens; none carries the case's own.
    pub title: Option<Title>,
    /// A process, on its own or in an organisation, or an organisation's
    /// front.
    pub role: Role,
    /// The link with the servers of other cells of the case.
    pub link: LinkSettings,
}

/// The server of another cell of the case, as `REGELHET_LINK_PEERS` names
/// it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerSetting {
    /// The cell that server works for.
    #[serde(rename = "cel")]
    pub cell: String,
    /// Where it is reached, without a trailing slash: its own address, or
    /// with `grant_hash` the address of this server's FSC outway.
    pub url: String,
    /// The hash of the FSC grant that lets this server call the peer's
    /// service. With it, every request to the peer goes to the outway at
    /// `url` under the header `Fsc-Grant-Hash`, and the outway reaches the
    /// peer's inway over the mutual TLS of the FSC group; none calls the
    /// peer directly.
    #[serde(default, rename = "grantHash")]
    pub grant_hash: Option<String>,
    /// Where a person opens the peer's own screens, for a link in the admin
    /// panel. None takes `url` when that is the peer's own address, and
    /// knows none behind an outway.
    #[serde(default, rename = "voorkant")]
    pub front: Option<String>,
    /// Its public key, a JWK as JSON text; none reads it from the server
    /// itself, in test mode only.
    #[serde(default, rename = "sleutel", deserialize_with = "jwk_text")]
    pub key: Option<String>,
}

impl PeerSetting {
    /// Where a person opens the peer's own screens: what the configuration
    /// names, or else the peer's own address. An outway is no such address.
    pub fn front(&self) -> Option<String> {
        match (&self.front, &self.grant_hash) {
            (Some(front), _) => Some(front.clone()),
            (None, None) => Some(self.url.clone()),
            (None, Some(_)) => None,
        }
    }
}

/// A JWK in the configuration, kept as text until the link opens.
fn jwk_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let value: Option<serde_json::Value> = serde::Deserialize::deserialize(d)?;
    Ok(value.map(|v| v.to_string()))
}

/// The link with the servers of other cells of the case
/// ([`crate::link`]).
#[derive(Clone, Default)]
pub struct LinkSettings {
    /// The external party this server works for, when it is not the
    /// executor's own server.
    pub party: Option<String>,
    pub peers: Vec<PeerSetting>,
    /// This server's private ES256 key; none makes one at every start, in
    /// test mode only.
    pub key: Option<KeySource>,
}

impl std::fmt::Debug for LinkSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the key.
        f.debug_struct("LinkSettings")
            .field("party", &self.party)
            .field("peers", &self.peers)
            .field("key", &self.key.is_some())
            .finish()
    }
}

/// Why the environment gives no configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("REGELHET_MODE is not set: set it to `test` or `stable`")]
    NoMode,
    #[error("REGELHET_MODE is `{0}`: it is `test` or `stable`")]
    UnknownMode(String),
    #[error("REGELHET_SURFACE is `{0}`: it is `burger`, `backoffice` or `all`")]
    UnknownSurface(String),
    #[error(
        "stable mode serves one side per deployment: set REGELHET_SURFACE to `burger` or \
         `backoffice`"
    )]
    StableNeedsSurface,
    #[error(
        "REGELHET_SURFACE is `all`, which serves citizens and employees on one address and is \
         for test mode only; stable mode serves `burger` or `backoffice`"
    )]
    StableSurfaceAll,
    #[error(
        "FSC voor Burgers is configured on the back office; the channel serves citizens, so it \
         belongs on REGELHET_SURFACE=burger"
    )]
    Fsc4bOnBackoffice,
    #[error(
        "REGELHET_LINK_PEERS is not a JSON list of `{{\"cel\", \"url\", \"sleutel\"}}`, each cell \
         once and each url an http(s) address: {0}"
    )]
    BadLinkPeers(String),
    #[error("set REGELHET_LINK_KEY or REGELHET_LINK_KEY_FILE, not both")]
    LinkTwoKeys,
    #[error(
        "REGELHET_LINK_PARTY makes this the server of an external party, whose staff work in \
         a back office: set REGELHET_SURFACE to `backoffice`"
    )]
    LinkPartyNeedsBackoffice,
    #[error(
        "a link in stable mode needs a key of its own (REGELHET_LINK_KEY or _KEY_FILE), and \
         for every peer its public key (`sleutel`) and an https address or an FSC grant (`grantHash`): {0}"
    )]
    StableLinkNeedsKeys(String),
    #[error("REGELHET_LOGIN is `{0}`: it is `testlogin` or `oidc`")]
    UnknownLogin(String),
    #[error(
        "REGELHET_LOGIN is `testlogin`, which lets anyone log in as any account and is for \
         test mode only; stable mode logs in through Keycloak"
    )]
    StableTestLogin,
    #[error(
        "REGELHET_LOGIN is `oidc`, but the OIDC configuration is not set: set OIDC_CLIENT_ID, \
         OIDC_CLIENT_SECRET and OIDC_DISCOVERY_URL (or KEYCLOAK_BASE_URL and KEYCLOAK_REALM)"
    )]
    OidcLoginWithoutOidc,
    #[error("REGELHET_ACCENT_COLOR: {0}")]
    UnknownAccentColor(#[from] crate::theme::UnknownAccentColor),
    #[error("REGELHET_TITLE is no title: {0}")]
    BadTitle(#[from] crate::theme::BadTitle),
    #[error(
        "the front of an organisation carries the organisation's name; REGELHET_TITLE is for a process"
    )]
    TitleOnFront,
    #[error("the front of an organisation runs no case; REGELHET_CASUS is for a process")]
    CaseOnFront,
    #[error("REGELHET_PORT is `{0}`, which is no port")]
    BadPort(String),
    #[error("REGELHET_STORE is `{0}`: it is `memory` or `postgres`")]
    UnknownStore(String),
    #[error(
        "stable mode keeps its chronicles in Postgres, and REGELHET_STORE is `memory`; \
         set it to `postgres` or leave it out"
    )]
    StableNeedsPostgres,
    #[error("the postgres store needs DATABASE_URL or DATABASE_SERVER_FULL, and neither is set")]
    NoDatabase,
    #[error("REGELHET_DB_MAX_CONNECTIONS: {0}")]
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
         holds, such as regelhet-toegang; the realm is shared, so there is no default"
    )]
    NoAccessRole,
    #[error(
        "OIDC_REQUIRED_ROLE is `{0}`; it is a role of this server (regelhet-...) that every \
         account holds, such as regelhet-toegang, and no citizen or level role"
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
         REGELHET_FSC4B_OIN, _SCOPE, _KEY or _KEY_FILE, _STELSELDOCUMENT_FILE and \
         _STELSELBEHEERDER_KEY together, or none of them"
    )]
    PartialFsc4b(&'static str, &'static str),
    #[error("set REGELHET_FSC4B_KEY or REGELHET_FSC4B_KEY_FILE, not both")]
    Fsc4bTwoKeys,
    #[error(
        "REGELHET_FSC4B_DEMO opens FSC voor Burgers on a demo Stelsel with a test \
         Activatiedienst, which only test mode allows"
    )]
    StableFsc4bDemo,
    #[error("REGELHET_FSC4B_DEMO is `{0}`: it is `true` or `false`")]
    BadFsc4bDemo(String),
    #[error("REGELHET_FSC4B_DEMO and {0} are both set: the demo makes its own Stelsel")]
    Fsc4bDemoAndSettings(&'static str),
    #[error(
        "FSC voor Burgers identifies a citizen by the `bsn` of a credential, and \
         REGELHET_SUBJECT_CLAIM is `{0}`"
    )]
    Fsc4bNeedsBsn(String),
    #[error("FSC voor Burgers needs REGELHET_FSC4B_PUBLIC_URL or BASE_URL")]
    Fsc4bNeedsPublicUrl,
    #[error("REGELHET_ROL is `{0}`: it is `proces` or `organisatie`")]
    UnknownRole(String),
    #[error(
        "the front of an organisation runs in test mode for now; stable mode follows with its \
         own Keycloak"
    )]
    StableFront,
    #[error("the front of an organisation needs {0}")]
    FrontNeeds(&'static str),
    #[error("the front of an organisation keeps nothing; REGELHET_STORE is `postgres`")]
    FrontKeepsNoStore,
    #[error("the front of an organisation offers no FSC voor Burgers; its processes do")]
    Fsc4bOnFront,
    #[error("the front of an organisation links with no cell (REGELHET_LINK_*); its processes do")]
    LinkOnFront,
    #[error("REGELHET_PROCES_ID is set on the front of an organisation, which runs no process")]
    ProcessIdOnFront,
    #[error("{0} is for the front of an organisation, and REGELHET_ROL is not `organisatie`")]
    FrontVariableOnProcess(&'static str),
    #[error("REGELHET_ORGANISATIE_LOGO is `{0}`: it is an https URL")]
    BadLogo(String),
    #[error(
        "{0} is set but REGELHET_PROCES_ID is not: a process belongs to an organisation with \
         all of its variables or none"
    )]
    PartialMembership(&'static str),
    #[error("a process that belongs to an organisation needs {0}")]
    MembershipNeeds(&'static str),
    #[error(
        "stable mode fetches no key: pin the organisation's key with \
         REGELHET_ORGANISATIE_SLEUTEL_JWK instead of REGELHET_ORGANISATIE_SLEUTEL_URL"
    )]
    StableFetchesNoKey,
    #[error("an address of the organisation is `{0}`: it is an http(s) URL")]
    BadOrganisationUrl(String),
    #[error(
        "REGELHET_FSC4B_ORIGINS names `{0}`: each origin is http(s)://host[:port], without a \
         path or a trailing slash"
    )]
    BadFsc4bOrigin(String),
    #[error("REGELHET_FSC4B_ORIGINS is set, but FSC voor Burgers is not configured")]
    Fsc4bOriginsWithoutChannel,
    #[error(
        "REGELHET_FSC4B_ORIGINS names an http origin; stable mode answers only https apps, and \
         http on localhost or 127.0.0.1"
    )]
    StableFsc4bOriginNeedsHttps,
    #[error("REGELHET_FSC4B_PUBLIC_URL: {0}")]
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
        let mode = set("REGELHET_MODE").ok_or(ConfigError::NoMode)?.parse()?;
        let surface = surface_choice(mode, set("REGELHET_SURFACE").as_deref())?;
        let store = store_choice(
            mode,
            set("REGELHET_STORE").as_deref(),
            || regelrecht_shared::postgres::database_url(&env),
            set("REGELHET_DB_MAX_CONNECTIONS").as_deref(),
        )?;
        let manifest =
            set("REGELHET_MANIFEST").map_or_else(|| PathBuf::from(DEFAULT_MANIFEST), PathBuf::from);
        let case = set("REGELHET_CASUS").map(|c| c.trim().to_string());
        let frontend =
            set("REGELHET_FRONTEND").map_or_else(|| PathBuf::from(DEFAULT_FRONTEND), PathBuf::from);
        let port = match set("REGELHET_PORT") {
            Some(p) => p.trim().parse().map_err(|_| ConfigError::BadPort(p))?,
            None => DEFAULT_PORT,
        };
        let subject_claim = set("REGELHET_SUBJECT_CLAIM").map(|c| c.trim().to_string());
        let claim = subject_claim.as_deref().unwrap_or(DEFAULT_SUBJECT_CLAIM);
        let accent = set("REGELHET_ACCENT_COLOR")
            .map(|c| c.parse())
            .transpose()?;
        let title = set("REGELHET_TITLE").map(|t| t.parse()).transpose()?;
        let login = login_choice(mode, claim, &set)?;
        let fsc4b = fsc4b_choice(mode, claim, port, &set)?;
        if !surface.serves_citizens() && !matches!(fsc4b, Fsc4bChoice::Off) {
            return Err(ConfigError::Fsc4bOnBackoffice);
        }
        let fsc4b_origins = set("REGELHET_FSC4B_ORIGINS")
            .map(|list| Fsc4bOrigins::parse(&list))
            .transpose()?
            .unwrap_or_default();
        if !fsc4b_origins.is_empty() && matches!(fsc4b, Fsc4bChoice::Off) {
            return Err(ConfigError::Fsc4bOriginsWithoutChannel);
        }
        if mode == Mode::Stable && !fsc4b_origins.all_https() {
            return Err(ConfigError::StableFsc4bOriginNeedsHttps);
        }
        let link = link_settings(mode, surface, &set)?;
        let role = crate::organisatie::settings::role(mode, surface, &set)?;
        if let Role::Organisation(_) = role {
            if store != StoreChoice::Memory {
                return Err(ConfigError::FrontKeepsNoStore);
            }
            if !matches!(fsc4b, Fsc4bChoice::Off) {
                return Err(ConfigError::Fsc4bOnFront);
            }
            if link.party.is_some() || !link.peers.is_empty() || link.key.is_some() {
                return Err(ConfigError::LinkOnFront);
            }
            if title.is_some() {
                return Err(ConfigError::TitleOnFront);
            }
            if case.is_some() {
                return Err(ConfigError::CaseOnFront);
            }
        }
        Ok(Self {
            mode,
            surface,
            store,
            manifest,
            case,
            frontend,
            port,
            login,
            subject_claim,
            fsc4b,
            fsc4b_origins,
            accent,
            title,
            link,
            role,
        })
    }
}

impl Config {
    /// The claim that identifies an applicant of the case `login_key`
    /// names (`inloggen.sleutel`): `REGELHET_SUBJECT_CLAIM` when set, else
    /// that key, else [`DEFAULT_SUBJECT_CLAIM`].
    pub fn subject_claim_for(&self, login_key: Option<&str>) -> String {
        self.subject_claim
            .as_deref()
            .or(login_key)
            .unwrap_or(DEFAULT_SUBJECT_CLAIM)
            .to_owned()
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
/// `subject_claim` among the claims it keeps. `REGELHET_LOGIN` names it,
/// or else the OIDC variables decide.
fn login_choice(
    mode: Mode,
    subject_claim: &str,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<LoginChoice, ConfigError> {
    let required = match set("REGELHET_LOGIN").as_deref().map(str::trim) {
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
    // The user name too: for a case with a login step of its own the
    // server logs that user in at the step after the login through
    // Keycloak (`crate::login::record_step_login`).
    for claim in [subject_claim, USERNAME_CLAIM] {
        if !oidc.extra_claims.iter().any(|c| c == claim) {
            oidc.extra_claims.push(claim.to_string());
        }
    }
    let base_url = regelrecht_auth::parse_base_url_from(set).map_err(ConfigError::BadBaseUrl)?;
    if mode == Mode::Stable && base_url.is_none() {
        return Err(ConfigError::StableNeedsBaseUrl);
    }
    Ok(LoginChoice::Oidc { oidc, base_url })
}

/// The link the environment `set` configures for `mode` on `surface`.
fn link_settings(
    mode: Mode,
    surface: Surface,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<LinkSettings, ConfigError> {
    let party = set("REGELHET_LINK_PARTY").map(|p| p.trim().to_string());
    if party.is_some() && surface != Surface::Backoffice {
        return Err(ConfigError::LinkPartyNeedsBackoffice);
    }
    let mut peers: Vec<PeerSetting> = match set("REGELHET_LINK_PEERS") {
        Some(text) => {
            serde_json::from_str(&text).map_err(|e| ConfigError::BadLinkPeers(e.to_string()))?
        }
        None => Vec::new(),
    };
    for peer in &mut peers {
        peer.url = peer.url.trim().trim_end_matches('/').to_string();
        peer.front = peer.front.take().map(|f| f.trim().to_string());
        for address in std::iter::once(&peer.url).chain(&peer.front) {
            if !(address.starts_with("https://") || address.starts_with("http://")) {
                return Err(ConfigError::BadLinkPeers(format!(
                    "`{address}` is no http(s) address"
                )));
            }
        }
    }
    for peer in &mut peers {
        peer.grant_hash = peer.grant_hash.take().map(|h| h.trim().to_string());
        // It travels as a header value.
        if let Some(hash) = &peer.grant_hash
            && (hash.is_empty() || !hash.bytes().all(|b| b.is_ascii_graphic()))
        {
            return Err(ConfigError::BadLinkPeers(format!(
                "the `grantHash` of '{}' is empty or holds a space or a character outside ASCII",
                peer.cell
            )));
        }
    }
    let mut cells: Vec<&str> = peers.iter().map(|p| p.cell.as_str()).collect();
    cells.sort_unstable();
    if let Some(twice) = cells.windows(2).find(|w| w[0] == w[1]) {
        return Err(ConfigError::BadLinkPeers(format!(
            "'{}' is named twice",
            twice[0]
        )));
    }
    let key = match (set("REGELHET_LINK_KEY"), set("REGELHET_LINK_KEY_FILE")) {
        (Some(pem), None) => Some(KeySource::Pem(pem)),
        (None, Some(path)) => Some(KeySource::File(PathBuf::from(path.trim()))),
        (Some(_), Some(_)) => return Err(ConfigError::LinkTwoKeys),
        (None, None) => None,
    };
    if mode == Mode::Stable && !peers.is_empty() {
        if key.is_none() {
            return Err(ConfigError::StableLinkNeedsKeys(
                "this server has no key".into(),
            ));
        }
        if let Some(peer) = peers
            .iter()
            // Through an outway the address is one inside the cluster, and
            // the TLS is between the outway and the peer's inway.
            .find(|p| p.key.is_none() || !(p.url.starts_with("https://") || p.grant_hash.is_some()))
        {
            return Err(ConfigError::StableLinkNeedsKeys(format!(
                "the peer '{}'",
                peer.cell
            )));
        }
    }
    Ok(LinkSettings { party, peers, key })
}

/// The FSC voor Burgers channel the environment `set` configures for
/// `mode`.
fn fsc4b_choice(
    mode: Mode,
    subject_claim: &str,
    port: u16,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<Fsc4bChoice, ConfigError> {
    let demo = match set("REGELHET_FSC4B_DEMO").as_deref().map(str::trim) {
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
    let key = match (set("REGELHET_FSC4B_KEY"), set("REGELHET_FSC4B_KEY_FILE")) {
        (Some(pem), None) => KeySource::Pem(pem),
        (None, Some(path)) => KeySource::File(PathBuf::from(path.trim())),
        (Some(_), Some(_)) => return Err(ConfigError::Fsc4bTwoKeys),
        (None, None) => return Err(ConfigError::PartialFsc4b(first, "REGELHET_FSC4B_KEY")),
    };
    let settings = Fsc4bSettings {
        oin: need("REGELHET_FSC4B_OIN")?.trim().to_string(),
        scope: need("REGELHET_FSC4B_SCOPE")?.trim().to_string(),
        key,
        stelseldocument: PathBuf::from(need("REGELHET_FSC4B_STELSELDOCUMENT_FILE")?.trim()),
        stelselbeheerder_key: need("REGELHET_FSC4B_STELSELBEHEERDER_KEY")?,
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

/// `REGELHET_FSC4B_PUBLIC_URL`, or else `BASE_URL`, without a trailing
/// slash; it must be an `http(s)` URL.
fn public_url(set: &impl Fn(&str) -> Option<String>) -> Result<Option<String>, ConfigError> {
    let Some(url) = set("REGELHET_FSC4B_PUBLIC_URL").or_else(|| set("BASE_URL")) else {
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

/// The store `REGELHET_STORE` names for `mode`, or the mode's own when it
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
        ("OIDC_CLIENT_ID", "regelhet"),
        ("OIDC_CLIENT_SECRET", "secret"),
        (
            "OIDC_DISCOVERY_URL",
            "https://login.example.org/realms/voorbeeld",
        ),
        ("OIDC_REQUIRED_ROLE", "regelhet-toegang"),
        ("BASE_URL", "https://regelhet.example.org"),
    ];

    const STABLE: Pairs = &[
        ("REGELHET_MODE", "stable"),
        ("REGELHET_SURFACE", "burger"),
        ("DATABASE_URL", "postgres://db/regelrecht"),
    ];

    #[test]
    fn a_peer_is_reached_directly_or_through_an_outway_under_a_grant() {
        let test: Pairs = &[("REGELHET_MODE", "test")];
        let peers = |json: &'static str| -> &'static [(&'static str, &'static str)] {
            Box::leak(Box::new([("REGELHET_LINK_PEERS", json)]))
        };
        let config = Config::from_env(env(&[
            test,
            peers(r#"[{"cel":"ggd","url":"http://fsc-outway.ns.svc/","grantHash":" $1$4$abc "}]"#),
        ]))
        .unwrap();
        assert_eq!(config.link.peers[0].url, "http://fsc-outway.ns.svc");
        assert_eq!(config.link.peers[0].grant_hash.as_deref(), Some("$1$4$abc"));
        // A grant is a header value: no spaces, not empty.
        for bad in [
            r#"[{"cel":"ggd","url":"http://outway","grantHash":""}]"#,
            r#"[{"cel":"ggd","url":"http://outway","grantHash":"a b"}]"#,
        ] {
            assert!(matches!(
                error(&[test, peers(bad)]),
                ConfigError::BadLinkPeers(_)
            ));
        }
        // Stable mode: a pinned key always, and https or a grant.
        let stable: Pairs = &[
            ("REGELHET_SURFACE", "backoffice"),
            ("REGELHET_LINK_KEY", "pem"),
        ];
        let jwk = r#"{"kty":"EC","crv":"P-256","x":"x","y":"y"}"#;
        let plain: &'static str = Box::leak(
            format!(r#"[{{"cel":"ggd","url":"http://outway","sleutel":{jwk}}}]"#).into_boxed_str(),
        );
        let granted: &'static str = Box::leak(
            format!(r#"[{{"cel":"ggd","url":"http://outway","grantHash":"h","sleutel":{jwk}}}]"#)
                .into_boxed_str(),
        );
        assert!(matches!(
            error(&[STABLE, OIDC, stable, peers(plain)]),
            ConfigError::StableLinkNeedsKeys(_)
        ));
        let config = Config::from_env(env(&[STABLE, OIDC, stable, peers(granted)])).unwrap();
        assert_eq!(config.link.peers[0].grant_hash.as_deref(), Some("h"));
    }

    #[test]
    fn the_mode_is_required_and_has_no_default() {
        assert_eq!(error(&[]), ConfigError::NoMode);
        assert_eq!(error(&[&[("REGELHET_MODE", " ")]]), ConfigError::NoMode);
        assert_eq!(
            error(&[&[("REGELHET_MODE", "prod")]]),
            ConfigError::UnknownMode("prod".into())
        );
    }

    #[test]
    fn test_mode_with_the_defaults() {
        let c = Config::from_env(env(&[&[("REGELHET_MODE", "test")]])).unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(c.store, StoreChoice::Memory);
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.manifest, PathBuf::from(DEFAULT_MANIFEST));
        assert!(matches!(c.login, LoginChoice::TestLogin));
        assert_eq!(c.subject_claim, None);
        assert_eq!(c.subject_claim_for(None), DEFAULT_SUBJECT_CLAIM);
        assert_eq!(c.subject_claim_for(Some("kvk_nummer")), "kvk_nummer");
        assert_eq!(c.surface, Surface::All);
    }

    #[test]
    fn stable_mode_names_its_surface_and_it_is_not_all() {
        let c = Config::from_env(env(&[STABLE, OIDC])).unwrap();
        assert_eq!(c.surface, Surface::Citizen);
        let c =
            Config::from_env(env(&[STABLE, OIDC, &[("REGELHET_SURFACE", "backoffice")]])).unwrap();
        assert_eq!(c.surface, Surface::Backoffice);
        assert_eq!(
            error(&[STABLE, OIDC, &[("REGELHET_SURFACE", "")]]),
            ConfigError::StableNeedsSurface
        );
        assert_eq!(
            error(&[STABLE, OIDC, &[("REGELHET_SURFACE", "all")]]),
            ConfigError::StableSurfaceAll
        );
        assert_eq!(
            error(&[&[
                ("REGELHET_MODE", "test"),
                ("REGELHET_SURFACE", "medewerker")
            ]]),
            ConfigError::UnknownSurface("medewerker".into())
        );
        let c = Config::from_env(env(&[&[
            ("REGELHET_MODE", "test"),
            ("REGELHET_SURFACE", "backoffice"),
        ]]))
        .unwrap();
        assert_eq!(c.surface, Surface::Backoffice);
    }

    #[test]
    fn fsc_voor_burgers_is_refused_on_the_back_office() {
        let backoffice: Pairs = &[("REGELHET_SURFACE", "backoffice")];
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, backoffice]),
            ConfigError::Fsc4bOnBackoffice
        );
        assert_eq!(
            error(&[
                &[("REGELHET_MODE", "test"), ("REGELHET_FSC4B_DEMO", "true")],
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
                ("REGELHET_MANIFEST", "/app/bron.yaml"),
                ("REGELHET_PORT", "7180"),
            ],
        ]))
        .unwrap();
        assert_eq!(c.mode, Mode::Stable);
        assert_eq!(c.manifest, PathBuf::from("/app/bron.yaml"));
        assert_eq!(c.port, 7180);
        assert_eq!(
            error(&[&[("REGELHET_MODE", "test"), ("REGELHET_PORT", "x")]]),
            ConfigError::BadPort("x".into())
        );
    }

    #[test]
    fn the_accent_colour_is_a_theme_of_the_design_system_or_nothing() {
        let test = &[("REGELHET_MODE", "test")];
        assert_eq!(Config::from_env(env(&[test])).unwrap().accent, None);
        let c = Config::from_env(env(&[test, &[("REGELHET_ACCENT_COLOR", "paars")]])).unwrap();
        assert_eq!(c.accent.map(AccentColor::name), Some("paars"));
        assert!(matches!(
            error(&[test, &[("REGELHET_ACCENT_COLOR", "#7d3f98")]]),
            ConfigError::UnknownAccentColor(_)
        ));
    }

    #[test]
    fn the_case_is_chosen_by_its_id_or_not_at_all() {
        let test = &[("REGELHET_MODE", "test")];
        assert_eq!(Config::from_env(env(&[test])).unwrap().case, None);
        let c = Config::from_env(env(&[
            test,
            &[("REGELHET_CASUS", " bedrijfsparkeervergunning ")],
        ]))
        .unwrap();
        assert_eq!(c.case.as_deref(), Some("bedrijfsparkeervergunning"));
    }

    #[test]
    fn the_title_is_one_short_line_or_the_case_s_own() {
        let test = &[("REGELHET_MODE", "test")];
        assert_eq!(Config::from_env(env(&[test])).unwrap().title, None);
        let c = Config::from_env(env(&[test, &[("REGELHET_TITLE", " IIT Groningen ")]])).unwrap();
        assert_eq!(c.title.as_ref().map(Title::as_str), Some("IIT Groningen"));
        assert!(matches!(
            error(&[test, &[("REGELHET_TITLE", "een\ntwee")]]),
            ConfigError::BadTitle(crate::theme::BadTitle::Control)
        ));
    }

    #[test]
    fn the_pool_size_comes_from_the_environment_with_a_low_default() {
        const PG: Pairs = &[
            ("REGELHET_MODE", "test"),
            ("REGELHET_STORE", "postgres"),
            ("DATABASE_URL", "postgres://db/regelrecht"),
        ];
        let c = Config::from_env(env(&[PG])).unwrap();
        assert_eq!(c.store.max_connections(), Some(5));
        let c = Config::from_env(env(&[PG, &[("REGELHET_DB_MAX_CONNECTIONS", "3")]])).unwrap();
        assert_eq!(c.store.max_connections(), Some(3));
        // Too few for the migration, or no number: refused at start.
        const TOO_FEW: Pairs = &[("REGELHET_DB_MAX_CONNECTIONS", "1")];
        const NO_NUMBER: Pairs = &[("REGELHET_DB_MAX_CONNECTIONS", "veel")];
        for wrong in [TOO_FEW, NO_NUMBER] {
            assert!(matches!(error(&[PG, wrong]), ConfigError::BadPoolSize(_)));
        }
        // The store in memory has no pool and reads none.
        let c = Config::from_env(env(&[&[
            ("REGELHET_MODE", "test"),
            ("REGELHET_DB_MAX_CONNECTIONS", "veel"),
        ]]))
        .unwrap();
        assert_eq!(c.store.max_connections(), None);
    }

    #[test]
    fn the_store_is_chosen_apart_from_the_mode() {
        let c = Config::from_env(env(&[&[
            ("REGELHET_MODE", "test"),
            ("REGELHET_STORE", "postgres"),
            ("DATABASE_SERVER_FULL", "postgres://zad/regelrecht"),
        ]]))
        .unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(c.store, StoreChoice::postgres("postgres://zad/regelrecht"));
        assert_eq!(
            error(&[&[("REGELHET_MODE", "test"), ("REGELHET_STORE", "file")]]),
            ConfigError::UnknownStore("file".into())
        );
    }

    #[test]
    fn stable_mode_keeps_its_chronicles_in_postgres() {
        let c = Config::from_env(env(&[STABLE, OIDC])).unwrap();
        assert_eq!(c.store.name(), "postgres");
        assert_eq!(
            error(&[STABLE, OIDC, &[("REGELHET_STORE", "memory")]]),
            ConfigError::StableNeedsPostgres
        );
    }

    #[test]
    fn the_postgres_store_needs_a_database() {
        for pairs in [
            &[("REGELHET_MODE", "stable"), ("REGELHET_SURFACE", "burger")][..],
            &[("REGELHET_MODE", "test"), ("REGELHET_STORE", "postgres")][..],
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
        assert_eq!(oidc.required_role, "regelhet-toegang");
        assert_eq!(oidc.extra_claims, [DEFAULT_SUBJECT_CLAIM, USERNAME_CLAIM]);
        assert_eq!(base_url.as_deref(), Some("https://regelhet.example.org"));
        let c = Config::from_env(env(&[
            STABLE,
            OIDC,
            &[
                ("REGELHET_SUBJECT_CLAIM", "kvk_nummer"),
                ("OIDC_EXTRA_CLAIMS", "kvk_nummer,naam"),
            ],
        ]))
        .unwrap();
        let LoginChoice::Oidc { oidc, .. } = &c.login else {
            panic!("stable mode logs in through OIDC");
        };
        assert_eq!(oidc.extra_claims, ["kvk_nummer", "naam", USERNAME_CLAIM]);
        assert_eq!(c.subject_claim.as_deref(), Some("kvk_nummer"));
        assert_eq!(
            c.subject_claim_for(Some("bsn")),
            "kvk_nummer",
            "the setting wins over the case"
        );
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
            "regelhet-",
            "regelhet-burger",
            "regelhet-lezer",
            "regelhet-admin",
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
        let test: Pairs = &[("REGELHET_MODE", "test")];
        let c = Config::from_env(env(&[test, OIDC, &[("BASE_URL", "")]])).unwrap();
        assert!(matches!(c.login, LoginChoice::Oidc { base_url: None, .. }));
        assert_eq!(
            error(&[test, OIDC, &[("OIDC_REQUIRED_ROLE", "")]]),
            ConfigError::NoAccessRole
        );
    }

    #[test]
    fn a_half_done_oidc_configuration_does_not_open_the_test_login() {
        let test: Pairs = &[("REGELHET_MODE", "test")];
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
    fn regelhet_login_testlogin_forces_the_test_login_in_test_mode() {
        let test: Pairs = &[("REGELHET_MODE", "test")];
        let testlogin: Pairs = &[("REGELHET_LOGIN", "testlogin")];
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
    fn regelhet_login_oidc_requires_the_oidc_variables() {
        let test: Pairs = &[("REGELHET_MODE", "test")];
        let oidc: Pairs = &[("REGELHET_LOGIN", "oidc")];
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
    fn an_unknown_regelhet_login_does_not_start() {
        assert_eq!(
            error(&[&[("REGELHET_MODE", "test"), ("REGELHET_LOGIN", "keycloak")]]),
            ConfigError::UnknownLogin("keycloak".into())
        );
    }

    const FSC4B: Pairs = &[
        ("REGELHET_FSC4B_OIN", "00000099000000000001"),
        ("REGELHET_FSC4B_SCOPE", "nl.voorbeeld.regelhet"),
        ("REGELHET_FSC4B_KEY_FILE", "/run/secrets/fsc4b.pem"),
        ("REGELHET_FSC4B_STELSELDOCUMENT_FILE", "/app/stelsel.jws"),
        ("REGELHET_FSC4B_STELSELBEHEERDER_KEY", "{}"),
        ("REGELHET_FSC4B_PUBLIC_URL", "https://regelhet.example.org/"),
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
        assert_eq!(s.public_url, "https://regelhet.example.org");
        assert!(matches!(s.key, KeySource::File(_)));
    }

    #[test]
    fn a_half_done_fsc_voor_burgers_configuration_does_not_start() {
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, &[("REGELHET_FSC4B_SCOPE", "")]]),
            ConfigError::PartialFsc4b("REGELHET_FSC4B_OIN", "REGELHET_FSC4B_SCOPE")
        );
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, &[("REGELHET_FSC4B_KEY", "pem")]]),
            ConfigError::Fsc4bTwoKeys
        );
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, &[("REGELHET_SUBJECT_CLAIM", "kvk")]]),
            ConfigError::Fsc4bNeedsBsn("kvk".into())
        );
        assert_eq!(
            error(&[
                STABLE,
                OIDC,
                FSC4B,
                &[("REGELHET_FSC4B_PUBLIC_URL", "http://regelhet.example.org")]
            ]),
            ConfigError::StableFsc4bNeedsHttps("http://regelhet.example.org".into())
        );
    }

    #[test]
    fn the_demo_stelsel_is_for_test_mode_only() {
        assert_eq!(
            error(&[STABLE, OIDC, &[("REGELHET_FSC4B_DEMO", "true")]]),
            ConfigError::StableFsc4bDemo
        );
        let test: Pairs = &[("REGELHET_MODE", "test"), ("REGELHET_FSC4B_DEMO", "true")];
        let c = Config::from_env(env(&[test])).unwrap();
        assert!(matches!(c.fsc4b, Fsc4bChoice::Demo { .. }));
        assert_eq!(
            error(&[test, &[("REGELHET_FSC4B_OIN", "1")]]),
            ConfigError::Fsc4bDemoAndSettings("REGELHET_FSC4B_OIN")
        );
    }

    #[test]
    fn the_origins_of_browser_apps_are_exact_and_need_the_channel() {
        let demo: Pairs = &[("REGELHET_MODE", "test"), ("REGELHET_FSC4B_DEMO", "true")];
        let c = Config::from_env(env(&[
            demo,
            &[(
                "REGELHET_FSC4B_ORIGINS",
                "https://burgerapp.example.org, http://localhost:7150",
            )],
        ]))
        .unwrap();
        assert_eq!(
            c.fsc4b_origins.as_slice(),
            ["https://burgerapp.example.org", "http://localhost:7150"]
        );
        assert!(
            Config::from_env(env(&[demo]))
                .unwrap()
                .fsc4b_origins
                .is_empty()
        );
        assert!(Fsc4bOrigins::parse("http://[::1]:7150").is_ok());
        for bad in [
            "*",
            "burgerapp.example.org",
            "https://burgerapp.example.org/",
            "https://burgerapp.example.org/app",
            "https://Burgerapp.example.org",
            "https://:",
            "https://a:b:c",
            "https://burgerapp.example.org:poort",
        ] {
            assert_eq!(
                Fsc4bOrigins::parse(bad),
                Err(ConfigError::BadFsc4bOrigin(bad.into()))
            );
        }
        assert_eq!(
            error(&[
                &[("REGELHET_MODE", "test")],
                &[("REGELHET_FSC4B_ORIGINS", "https://burgerapp.example.org")]
            ]),
            ConfigError::Fsc4bOriginsWithoutChannel
        );
        let origins = |list: &'static str| -> Pairs {
            Box::leak(Box::new([("REGELHET_FSC4B_ORIGINS", list)]))
        };
        assert_eq!(
            error(&[STABLE, OIDC, FSC4B, origins("http://burgerapp.example.org")]),
            ConfigError::StableFsc4bOriginNeedsHttps
        );
        let c = Config::from_env(env(&[
            STABLE,
            OIDC,
            FSC4B,
            origins("https://burgerapp.example.org,http://localhost:7150"),
        ]))
        .unwrap();
        assert_eq!(c.fsc4b_origins.as_slice().len(), 2);
    }

    #[test]
    fn the_private_key_stays_out_of_debug_output() {
        let mut env_pairs = FSC4B.to_vec();
        env_pairs.retain(|(k, _)| *k != "REGELHET_FSC4B_KEY_FILE");
        env_pairs.push(("REGELHET_FSC4B_KEY", "-----BEGIN PRIVATE KEY-----geheim"));
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
