//! The core of the wetsproces app: what runs on the Rust side of a webview,
//! so that the webview never holds what proves who the citizen is.
//!
//! The app's screens are the citizen's screens of `frontend-wetsproces`; they
//! call commands ([`AppCore`]) instead of the server. The core keeps the app
//! key, the credential, the session token and the session key, speaks FSC
//! voor Burgers with the server through the client of `regelrecht-fsc4b`,
//! and hands the webview only answers and errors:
//!
//! - [`config`]: where the server is;
//! - [`keystore`]: where the app key and the credential live, behind the
//!   trait [`KeyStore`];
//! - [`credential`]: what the app reads from its own credential;
//! - [`core`]: the commands, with the session renewed when it lapses;
//! - [`error`]: the errors a command gives, as the webview reads them.
//!
//! The Tauri shell in `frontend-wetsproces/src-tauri` only registers these
//! commands; everything a test can reach is here.

pub mod config;
pub mod core;
pub mod credential;
pub mod error;
pub mod keystore;

pub use crate::config::AppConfig;
pub use crate::core::{AppCore, Status};
pub use crate::error::CommandError;
pub use crate::keystore::{KeyStorage, KeyStore, SoftwareKeyStore};

/// Install aws-lc-rs as the process-wide rustls provider, once at startup:
/// reqwest is built without a provider of its own, as in the server.
pub fn install_crypto_provider() {
    // Fails only when a provider is installed already; that one is used.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();
}
