//! The configuration of the server, from the environment.
//!
//! - `WETSPROCES_MODE`: `test` or `stable`, required. There is no default:
//!   the mode decides whether the clock is real and who may use the server,
//!   so a deployment says which it wants.
//! - `WETSPROCES_STORE`: where the chronicles live, `memory` or `postgres`.
//!   By default `memory` in test mode and `postgres` in stable mode, which
//!   takes no other.
//! - `DATABASE_URL`, or else `DATABASE_SERVER_FULL` as ZAD sets it: the
//!   database of the `postgres` store, required with it.
//! - `WETSPROCES_MANIFEST`: the source manifest to run, by default
//!   `packages/wetsproces/bron.yaml` of this checkout.
//! - `WETSPROCES_PORT`: the port, by default 8000.
//! - `WETSPROCES_FRONTEND`: the built frontend the server serves beside the
//!   API, by default `frontend-wetsproces/dist` of this checkout. Without an
//!   `index.html` there the server serves the API alone.
//!
//! The login ([`LoginChoice`]):
//!
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

use std::path::PathBuf;

use regelrecht_auth::OidcConfig;

use crate::access::{CELL_ROLE_PREFIX, CITIZEN, Level, ROLE_PREFIX};

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
    /// In PostgreSQL at `url`, in the schema `wetsproces`.
    Postgres { url: String },
}

impl StoreChoice {
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
            Self::Postgres { .. } => f.write_str("Postgres { url: <redacted> }"),
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
    pub store: StoreChoice,
    pub manifest: PathBuf,
    /// The directory of the built frontend, served beside the API.
    pub frontend: PathBuf,
    pub port: u16,
    pub login: LoginChoice,
    /// The claim of a citizen's account that identifies them.
    pub subject_claim: String,
}

/// Why the environment gives no configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("WETSPROCES_MODE is not set: set it to `test` or `stable`")]
    NoMode,
    #[error("WETSPROCES_MODE is `{0}`: it is `test` or `stable`")]
    UnknownMode(String),
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
         account holds, such as wetsproces-toegang, and no citizen, level or cell role"
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
}

impl Config {
    /// The configuration in the environment `env`, a lookup by name such as
    /// `|k| std::env::var(k).ok()`. An empty variable counts as not set.
    ///
    /// # Errors
    ///
    /// When the mode is missing or unknown, the port is no number, the
    /// store is unknown or does not fit the mode, or the postgres store has
    /// no database.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let set = |k: &str| env(k).filter(|v| !v.trim().is_empty());
        let mode = set("WETSPROCES_MODE").ok_or(ConfigError::NoMode)?.parse()?;
        let store = store_choice(mode, set("WETSPROCES_STORE").as_deref(), || {
            regelrecht_shared::postgres::database_url(&env)
        })?;
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
        let login = login_choice(mode, &subject_claim, &set)?;
        Ok(Self {
            mode,
            store,
            manifest,
            frontend,
            port,
            login,
            subject_claim,
        })
    }
}

/// The login the environment `set` configures for `mode`, with the claim
/// `subject_claim` among the claims it keeps.
fn login_choice(
    mode: Mode,
    subject_claim: &str,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<LoginChoice, ConfigError> {
    let Some(mut oidc) = regelrecht_auth::parse_oidc_from(set).map_err(ConfigError::Oidc)? else {
        if let Some(name) = OIDC_VARIABLES.into_iter().find(|n| set(n).is_some()) {
            return Err(ConfigError::PartialOidc(name));
        }
        return match mode {
            Mode::Stable => Err(ConfigError::StableNeedsOidc),
            Mode::Test => Ok(LoginChoice::TestLogin),
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

/// Whether `role` can be the role every account needs to log in: a role
/// of this server, and none that only some accounts hold.
fn is_access_role(role: &str) -> bool {
    role.strip_prefix(ROLE_PREFIX)
        .is_some_and(|rest| !rest.is_empty())
        && role != CITIZEN
        && !role.starts_with(CELL_ROLE_PREFIX)
        && !Level::ALL.iter().any(|l| l.role() == role)
}

/// The store `WETSPROCES_STORE` names for `mode`, or the mode's own when it
/// names none; `database_url` is asked only for the postgres store.
fn store_choice(
    mode: Mode,
    named: Option<&str>,
    database_url: impl FnOnce() -> Option<String>,
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
    Ok(StoreChoice::Postgres { url })
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
    fn the_store_is_chosen_apart_from_the_mode() {
        let c = Config::from_env(env(&[&[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_STORE", "postgres"),
            ("DATABASE_SERVER_FULL", "postgres://zad/regelrecht"),
        ]]))
        .unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(
            c.store,
            StoreChoice::Postgres {
                url: "postgres://zad/regelrecht".into()
            }
        );
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
            &[("WETSPROCES_MODE", "stable")][..],
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
            "wetsproces-cel-gemeente_wilgendaal",
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
    fn the_database_url_stays_out_of_debug_output() {
        let store = StoreChoice::Postgres {
            url: "postgres://user:secret@db/regelrecht".into(),
        };
        assert!(!format!("{store:?}").contains("secret"));
    }
}
