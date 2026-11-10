use sha2::{Digest, Sha256};

pub use regelrecht_auth::OidcConfig;

#[derive(Clone)]
pub struct AppConfig {
    pub oidc: Option<OidcConfig>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    /// Pre-computed SHA-256 hash of the API key (avoids re-hashing on every request).
    pub api_key_hash: Option<[u8; 32]>,
    /// Pre-computed SHA-256 hash of the metrics auth token (sent by Prometheus).
    pub metrics_token_hash: Option<[u8; 32]>,
}

impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("oidc", &self.oidc)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("api_key_hash", &self.api_key_hash.map(|_| "[REDACTED]"))
            .field(
                "metrics_token_hash",
                &self.metrics_token_hash.map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        match Self::try_from_env() {
            Ok(config) => config,
            Err(e) => {
                tracing::error!("{e}");
                std::process::exit(1);
            }
        }
    }

    fn try_from_env() -> Result<Self, String> {
        Self::try_from_lookup(regelrecht_auth::process_env)
    }

    /// Build the configuration with every variable read through `env`.
    fn try_from_lookup(env: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let oidc = regelrecht_auth::parse_oidc_from(&env)?;

        if oidc.is_some() {
            tracing::info!("OIDC authentication is enabled");
        } else {
            tracing::warn!(
                "OIDC authentication is DISABLED — admin panel is unprotected. \
                 All routes (harvester-reader/writer/admin tiers) bypass auth checks. \
                 Configure OIDC environment variables to enable authentication. \
                 Do NOT run this configuration in production."
            );
        }

        let base_url = regelrecht_auth::parse_base_url_from(&env)?;
        if base_url.is_none() && oidc.is_some() {
            tracing::info!(
                "BASE_URL is not set — OIDC redirect URLs will be derived from request headers"
            );
        }

        let api_key = env("ADMIN_API_KEY").filter(|s| !s.is_empty());
        if let Some(ref key) = api_key {
            if key.len() < 32 {
                tracing::warn!(
                    "ADMIN_API_KEY is shorter than 32 characters — consider using a longer key"
                );
            }
            tracing::info!("API key authentication is enabled (GET + POST + DELETE)");
        }

        let api_key_hash = api_key
            .as_ref()
            .map(|k| Sha256::digest(k.as_bytes()).into());

        let metrics_token = env("METRICS_AUTH_TOKEN").filter(|s| !s.is_empty());
        if metrics_token.is_some() {
            tracing::info!("Metrics endpoint authentication is enabled (METRICS_AUTH_TOKEN)");
        }
        let metrics_token_hash = metrics_token
            .as_ref()
            .map(|k| Sha256::digest(k.as_bytes()).into());

        Ok(Self {
            oidc,
            base_url,
            api_key,
            api_key_hash,
            metrics_token_hash,
        })
    }

    pub fn is_auth_enabled(&self) -> bool {
        self.oidc.is_some()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Build the configuration from fixed values instead of the process
    /// environment.
    fn config_of(pairs: &[(&str, &str)]) -> Result<AppConfig, String> {
        let vars: HashMap<&str, &str> = pairs.iter().copied().collect();
        AppConfig::try_from_lookup(|key| vars.get(key).map(|v| (*v).to_string()))
    }

    const COMPLETE_OIDC: &[(&str, &str)] = &[
        ("OIDC_CLIENT_ID", "test-client"),
        ("OIDC_CLIENT_SECRET", "secret"),
        ("KEYCLOAK_BASE_URL", "https://keycloak.example.com"),
        ("KEYCLOAK_REALM", "test-realm"),
        ("OIDC_REQUIRED_ROLE", "harvester-reader"),
    ];

    #[test]
    fn no_oidc_vars_disables_auth() {
        let config = config_of(&[]).expect("should succeed");
        assert!(config.oidc.is_none());
        assert!(!config.is_auth_enabled());
    }

    #[test]
    fn complete_keycloak_vars_enables_auth() {
        let config = config_of(COMPLETE_OIDC).expect("should succeed");
        assert!(config.is_auth_enabled());
        let oidc = config.oidc.unwrap();
        assert_eq!(oidc.client_id, "test-client");
    }

    #[test]
    fn api_key_from_env() {
        let config = config_of(&[("ADMIN_API_KEY", "test-secret-key")]).expect("should succeed");
        assert_eq!(config.api_key.as_deref(), Some("test-secret-key"));
    }

    #[test]
    fn empty_api_key_is_none() {
        let config = config_of(&[("ADMIN_API_KEY", "")]).expect("should succeed");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn no_api_key_is_none() {
        let config = config_of(&[]).expect("should succeed");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn metrics_token_from_env() {
        let config = config_of(&[("METRICS_AUTH_TOKEN", "prom-secret")]).expect("should succeed");
        assert!(config.metrics_token_hash.is_some());
    }

    #[test]
    fn no_metrics_token_is_none() {
        let config = config_of(&[]).expect("should succeed");
        assert!(config.metrics_token_hash.is_none());
    }
}
