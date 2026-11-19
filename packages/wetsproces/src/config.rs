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

use std::path::PathBuf;

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

/// The port the server listens on when `WETSPROCES_PORT` is not set.
pub const DEFAULT_PORT: u16 = 8000;

/// The manifest when `WETSPROCES_MANIFEST` is not set: the one in this
/// crate, for a run from the checkout.
pub const DEFAULT_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/bron.yaml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub store: StoreChoice,
    pub manifest: PathBuf,
    pub port: u16,
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
        let port = match set("WETSPROCES_PORT") {
            Some(p) => p.trim().parse().map_err(|_| ConfigError::BadPort(p))?,
            None => DEFAULT_PORT,
        };
        Ok(Self {
            mode,
            store,
            manifest,
            port,
        })
    }
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn the_mode_is_required_and_has_no_default() {
        assert_eq!(Config::from_env(env(&[])), Err(ConfigError::NoMode));
        assert_eq!(
            Config::from_env(env(&[("WETSPROCES_MODE", " ")])),
            Err(ConfigError::NoMode)
        );
        assert_eq!(
            Config::from_env(env(&[("WETSPROCES_MODE", "prod")])),
            Err(ConfigError::UnknownMode("prod".into()))
        );
    }

    #[test]
    fn test_mode_with_the_defaults() {
        let c = Config::from_env(env(&[("WETSPROCES_MODE", "test")])).unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(c.store, StoreChoice::Memory);
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.manifest, PathBuf::from(DEFAULT_MANIFEST));
    }

    #[test]
    fn the_manifest_and_port_come_from_the_environment() {
        let c = Config::from_env(env(&[
            ("WETSPROCES_MODE", "stable"),
            ("DATABASE_URL", "postgres://db/regelrecht"),
            ("WETSPROCES_MANIFEST", "/app/bron.yaml"),
            ("WETSPROCES_PORT", "7180"),
        ]))
        .unwrap();
        assert_eq!(c.mode, Mode::Stable);
        assert_eq!(c.manifest, PathBuf::from("/app/bron.yaml"));
        assert_eq!(c.port, 7180);
        assert_eq!(
            Config::from_env(env(&[
                ("WETSPROCES_MODE", "test"),
                ("WETSPROCES_PORT", "x")
            ])),
            Err(ConfigError::BadPort("x".into()))
        );
    }

    #[test]
    fn the_store_is_chosen_apart_from_the_mode() {
        let c = Config::from_env(env(&[
            ("WETSPROCES_MODE", "test"),
            ("WETSPROCES_STORE", "postgres"),
            ("DATABASE_SERVER_FULL", "postgres://zad/regelrecht"),
        ]))
        .unwrap();
        assert_eq!(c.mode, Mode::Test);
        assert_eq!(
            c.store,
            StoreChoice::Postgres {
                url: "postgres://zad/regelrecht".into()
            }
        );
        assert_eq!(
            Config::from_env(env(&[
                ("WETSPROCES_MODE", "test"),
                ("WETSPROCES_STORE", "file")
            ])),
            Err(ConfigError::UnknownStore("file".into()))
        );
    }

    #[test]
    fn stable_mode_keeps_its_chronicles_in_postgres() {
        let c = Config::from_env(env(&[
            ("WETSPROCES_MODE", "stable"),
            ("DATABASE_URL", "postgres://db/regelrecht"),
        ]))
        .unwrap();
        assert_eq!(c.store.name(), "postgres");
        assert_eq!(
            Config::from_env(env(&[
                ("WETSPROCES_MODE", "stable"),
                ("WETSPROCES_STORE", "memory"),
                ("DATABASE_URL", "postgres://db/regelrecht"),
            ])),
            Err(ConfigError::StableNeedsPostgres)
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
            let pairs: &'static [(&str, &str)] = pairs;
            assert_eq!(Config::from_env(env(pairs)), Err(ConfigError::NoDatabase));
        }
    }

    #[test]
    fn the_database_url_stays_out_of_debug_output() {
        let store = StoreChoice::Postgres {
            url: "postgres://user:secret@db/regelrecht".into(),
        };
        assert!(!format!("{store:?}").contains("secret"));
    }
}
