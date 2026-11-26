//! Where the app finds the server, from the environment of the shell.
//!
//! In the demo Stelsel the server itself serves the Stelseldocument, the
//! Stelselbeheerder's key and the test Activatiedienst, so one address is
//! enough. The OIN of the organization defaults to the demo's.

use regelrecht_fsc4b::wire::DEMO_ORGANIZATION_OIN;

/// The variable with the server's address, such as `https://regelhet.example`.
pub const SERVER_VAR: &str = "REGELHET_APP_SERVER";
/// The variable with the organization's OIN; the demo's when not set.
pub const OIN_VAR: &str = "REGELHET_APP_OIN";

/// Why the configuration does not read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("{SERVER_VAR} is not set")]
    NoServer,
    #[error("{SERVER_VAR} is no http:// or https:// address: {0}")]
    BadServer(String),
    #[error("{OIN_VAR} is not twenty digits: {0}")]
    BadOin(String),
}

/// A checked configuration: a server address without a trailing slash and
/// the OIN of the organization behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    server: String,
    oin: String,
}

impl AppConfig {
    /// The configuration for the server at `server`, with the organization
    /// `oin` (the demo's when `None`).
    ///
    /// # Errors
    ///
    /// When `server` is no http(s) address, or `oin` is not twenty digits.
    pub fn build(server: &str, oin: Option<&str>) -> Result<Self, ConfigError> {
        let server = server.trim().trim_end_matches('/');
        let rest = server
            .strip_prefix("https://")
            .or_else(|| server.strip_prefix("http://"));
        if rest.is_none_or(|host| host.is_empty() || host.contains(char::is_whitespace)) {
            return Err(ConfigError::BadServer(server.to_string()));
        }
        let oin = oin.map_or(DEMO_ORGANIZATION_OIN, str::trim);
        if oin.len() != 20 || !oin.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ConfigError::BadOin(oin.to_string()));
        }
        Ok(Self {
            server: server.to_string(),
            oin: oin.to_string(),
        })
    }

    /// The configuration from the variables `lookup` gives, such as
    /// `std::env::var`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoServer`] without [`SERVER_VAR`], otherwise as
    /// [`AppConfig::build`].
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let server = lookup(SERVER_VAR).ok_or(ConfigError::NoServer)?;
        Self::build(&server, lookup(OIN_VAR).as_deref())
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn oin(&self) -> &str {
        &self.oin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_address_is_trimmed_and_the_oin_defaults_to_the_demo() {
        let c = AppConfig::build(" https://regelhet.example/ ", None);
        assert_eq!(
            c.as_ref().map(AppConfig::server),
            Ok("https://regelhet.example")
        );
        assert_eq!(c.as_ref().map(AppConfig::oin), Ok(DEMO_ORGANIZATION_OIN));
    }

    #[test]
    fn what_is_no_address_or_no_oin_is_refused() {
        for bad in ["", "regelhet.example", "ftp://x", "https://", "http://a b"] {
            assert!(
                matches!(AppConfig::build(bad, None), Err(ConfigError::BadServer(_))),
                "{bad}"
            );
        }
        assert!(matches!(
            AppConfig::build("http://localhost:7190", Some("123")),
            Err(ConfigError::BadOin(_))
        ));
    }

    #[test]
    fn without_the_server_variable_there_is_no_configuration() {
        assert_eq!(AppConfig::from_lookup(|_| None), Err(ConfigError::NoServer));
        let c =
            AppConfig::from_lookup(|k| (k == SERVER_VAR).then(|| "http://10.0.2.2:7190".into()));
        assert_eq!(
            c.map(|c| c.server().to_string()),
            Ok("http://10.0.2.2:7190".into())
        );
    }
}
