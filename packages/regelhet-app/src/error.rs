//! The errors of a command, as the webview reads them: a `kind` the screens
//! branch on, a `detail` for the developer, and for an answer of the
//! service its status and body. The text a citizen reads is the screens'
//! work (`frontend-regelhet/src/burgeromgeving/tauri/`), in Dutch.
//!
//! No variant carries a token, a key or a credential.

use regelrecht_fsc4b::client::ClientError;
use serde::Serialize;
use serde_json::Value;

use crate::config::ConfigError;
use crate::credential::CredentialError;
use crate::keystore::KeyStoreError;

/// Why a command did not succeed.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandError {
    /// The shell has no server address.
    #[error("the app is not configured: {detail}")]
    NotConfigured { detail: String },
    /// There is no credential: the app has to be activated first.
    #[error("the app is not activated")]
    NotActivated,
    /// The credential has lapsed: activate again.
    #[error("the credential has lapsed")]
    CredentialExpired,
    /// The server refused the credential while starting a session: activate
    /// again.
    #[error("the server refused the credential")]
    CredentialRefused,
    /// The server offers no demo Stelsel or no FSC voor Burgers channel.
    #[error("the server offers no demo Activatiedienst: {detail}")]
    NoDemo { detail: String },
    /// The server could not be reached.
    #[error("the server could not be reached: {detail}")]
    Network { detail: String },
    /// The service answered with an error: its status and its body.
    #[error("the server answered {status}")]
    Service { status: u16, body: Value },
    /// The answer broke the protocol, or a check of the app failed on it.
    #[error("protocol: {detail}")]
    Protocol { detail: String },
    /// The key store failed.
    #[error("the key store: {detail}")]
    KeyStore { detail: String },
    /// The command was given something it does not take.
    #[error("{detail}")]
    Input { detail: String },
}

impl CommandError {
    pub(crate) fn input(detail: impl Into<String>) -> Self {
        Self::Input {
            detail: detail.into(),
        }
    }
}

impl From<ConfigError> for CommandError {
    fn from(e: ConfigError) -> Self {
        Self::NotConfigured {
            detail: e.to_string(),
        }
    }
}

impl From<KeyStoreError> for CommandError {
    fn from(e: KeyStoreError) -> Self {
        Self::KeyStore {
            detail: e.to_string(),
        }
    }
}

impl From<CredentialError> for CommandError {
    fn from(e: CredentialError) -> Self {
        Self::Protocol {
            detail: e.to_string(),
        }
    }
}

impl From<ClientError> for CommandError {
    fn from(e: ClientError) -> Self {
        match e {
            ClientError::Http(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
                Self::Network {
                    detail: e.to_string(),
                }
            }
            ClientError::Status { status, body } => Self::Service {
                status: status.as_u16(),
                body: serde_json::from_str(&body).unwrap_or(Value::String(body)),
            },
            other => Self::Protocol {
                detail: other.to_string(),
            },
        }
    }
}
