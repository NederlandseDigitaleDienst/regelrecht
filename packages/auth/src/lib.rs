//! Shared OIDC/SSO authentication for RegelRecht services.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;

pub mod config;
pub mod handlers;
pub mod middleware;
pub mod oidc;
pub mod security_headers;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub use config::{OidcConfig, parse_base_url, parse_oidc_from_env};
pub use handlers::{
    AuthStatus, PersonInfo, SESSION_KEY_AUTHENTICATED, SESSION_KEY_EMAIL,
    SESSION_KEY_EMAIL_VERIFIED, SESSION_KEY_NAME, SESSION_KEY_ROLES, SESSION_KEY_SUB,
};
pub use middleware::{
    RoleCheck, check_session_role, refresh_session_token, require_role, require_session_auth,
};
pub use oidc::{ConfiguredClient, DiscoveryResult, discover_client};
// De middleware zelf wordt bewust niet hier heruitgevoerd: hij heet net als
// zijn module, en dan wordt `regelrecht_auth::security_headers::…` een
// dubbelzinnig pad.
pub use security_headers::{API_CSP, EDITOR_CSP};

/// Install aws-lc-rs as the process-wide rustls crypto provider.
///
/// The workspace deliberately runs on a single crypto backend: sqlx, reqwest
/// 0.13 and reqwest 0.12 all take aws-lc-rs, so `ring` is compiled nowhere.
/// reqwest 0.12 is configured with `rustls-tls-webpki-roots-no-provider`,
/// which leaves the choice to rustls' process default - and rustls only picks
/// one by itself when exactly one backend is compiled in. A test build that
/// drags a second one in through a dev-dependency would otherwise panic on the
/// first HTTPS request. Installing explicitly removes that dependency on the
/// resolved feature set.
///
/// Idempotent: a second call, or a provider installed by someone else, is a
/// no-op. Call it once at the top of `main`.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

/// A default reqwest client with the crypto provider installed.
///
/// For code that has no `main` to install the provider in - test binaries,
/// mostly. A service builds its own client with its own timeouts and calls
/// [`install_crypto_provider`] at startup.
pub fn http_client() -> reqwest::Client {
    install_crypto_provider();
    reqwest::Client::new()
}

/// Trait implemented by each service's `AppState` to provide OIDC context
/// to the shared auth handlers and middleware.
pub trait OidcAppState: Clone + Send + Sync + 'static {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>>;
    fn end_session_url(&self) -> Option<&str>;
    fn oidc_config(&self) -> Option<&OidcConfig>;
    fn is_auth_enabled(&self) -> bool;
    fn base_url(&self) -> Option<&str>;
    fn http_client(&self) -> &reqwest::Client;
}

/// Build the standard auth routes (login, callback, logout, status)
/// for any `AppState` that implements [`OidcAppState`].
pub fn auth_routes<S: OidcAppState>() -> Router<S> {
    Router::new()
        .route("/auth/login", get(handlers::login::<S>))
        .route("/auth/callback", get(handlers::callback::<S>))
        .route("/auth/logout", get(handlers::logout::<S>))
        .route("/auth/status", get(handlers::status::<S>))
}

#[cfg(test)]
mod crypto_provider_tests {
    /// reqwest 0.12 runs on `rustls-tls-webpki-roots-no-provider`, so it takes
    /// whatever crypto provider the process has installed. Building a client
    /// is where rustls resolves that, and where it panics when the choice is
    /// ambiguous or missing. This proves the TLS client of every service that
    /// uses this crate still comes up.
    #[test]
    fn een_https_client_komt_op_met_de_geinstalleerde_provider() {
        super::install_crypto_provider();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());

        reqwest::Client::builder()
            .build()
            .expect("reqwest client met rustls moet te bouwen zijn");
    }

    /// The installer runs at the top of every `main`; a second call must not
    /// blow up (a library consumer may have installed one already).
    #[test]
    fn tweemaal_installeren_is_geen_fout() {
        super::install_crypto_provider();
        super::install_crypto_provider();
    }
}
