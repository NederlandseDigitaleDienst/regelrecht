//! The configuration of the server, from the environment.
//!
//! - `WETSPROCES_MODE`: `test` or `stable`, required. There is no default:
//!   the mode decides whether chronicles survive a restart, so a deployment
//!   says which it wants.
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

/// The port the server listens on when `WETSPROCES_PORT` is not set.
pub const DEFAULT_PORT: u16 = 8000;

/// The manifest when `WETSPROCES_MANIFEST` is not set: the one in this
/// crate, for a run from the checkout.
pub const DEFAULT_MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/bron.yaml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
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
}

impl Config {
    /// The configuration in the environment `env`, a lookup by name such as
    /// `|k| std::env::var(k).ok()`. An empty variable counts as not set.
    ///
    /// # Errors
    ///
    /// When the mode is missing or unknown, or the port is no number.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let set = |k: &str| env(k).filter(|v| !v.trim().is_empty());
        let mode = set("WETSPROCES_MODE").ok_or(ConfigError::NoMode)?.parse()?;
        let manifest = set("WETSPROCES_MANIFEST")
            .map_or_else(|| PathBuf::from(DEFAULT_MANIFEST), PathBuf::from);
        let port = match set("WETSPROCES_PORT") {
            Some(p) => p.trim().parse().map_err(|_| ConfigError::BadPort(p))?,
            None => DEFAULT_PORT,
        };
        Ok(Self {
            mode,
            manifest,
            port,
        })
    }
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
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.manifest, PathBuf::from(DEFAULT_MANIFEST));
    }

    #[test]
    fn the_manifest_and_port_come_from_the_environment() {
        let c = Config::from_env(env(&[
            ("WETSPROCES_MODE", "stable"),
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
}
