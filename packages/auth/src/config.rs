//! Shared OIDC configuration, loaded from environment variables.

use std::env;

#[derive(Clone)]
pub struct OidcConfig {
    pub client_id: String,
    pub client_secret: String,
    pub issuer_url: String,
    pub required_role: String,
}

impl std::fmt::Debug for OidcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("issuer_url", &self.issuer_url)
            .field("required_role", &self.required_role)
            .finish()
    }
}

/// Parse OIDC configuration from environment variables.
///
/// Returns `None` when `OIDC_CLIENT_ID` is not set, `Some(config)` when
/// all required variables are present, or an error string when the
/// configuration is incomplete.
pub fn parse_oidc_from_env() -> Result<Option<OidcConfig>, String> {
    match env::var("OIDC_CLIENT_ID").ok() {
        None => Ok(None),
        Some(client_id) => Ok(Some(parse_oidc_config(client_id)?)),
    }
}

fn parse_oidc_config(client_id: String) -> Result<OidcConfig, String> {
    let client_secret = env::var("OIDC_CLIENT_SECRET").unwrap_or_default();
    if client_secret.is_empty() {
        return Err("OIDC_CLIENT_ID is set but OIDC_CLIENT_SECRET is missing. \
             Refusing to start without complete OIDC configuration."
            .to_string());
    }

    let issuer_url = resolve_issuer_url()?;

    // Minimum realm role required to log in (the login gate). Typically
    // `<app>-reader`. Per-route `require_role` middleware layers finer-grained
    // gates on top. Defaults to `allowed-user` for backwards compatibility
    // with the pre-RBAC deployment; operators should set it explicitly to the
    // per-app reader role (`editor-reader` / `harvester-reader`) once the
    // migration in `docs/auth-and-roles.md` is complete.
    let required_role = env::var("OIDC_REQUIRED_ROLE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            tracing::warn!(
                "OIDC_REQUIRED_ROLE not set; defaulting to `allowed-user`. \
                 Set this explicitly to the per-app reader role once the \
                 Keycloak migration is complete."
            );
            "allowed-user".to_string()
        });

    Ok(OidcConfig {
        client_id,
        client_secret,
        issuer_url,
        required_role,
    })
}

fn resolve_issuer_url() -> Result<String, String> {
    // OIDC_DISCOVERY_URL takes priority (RIG-style injection)
    if let Ok(discovery_url) = env::var("OIDC_DISCOVERY_URL")
        && !discovery_url.is_empty()
    {
        // Strip /.well-known/openid-configuration suffix if present
        let issuer = discovery_url
            .strip_suffix("/.well-known/openid-configuration")
            .unwrap_or(&discovery_url);
        tracing::info!("using OIDC_DISCOVERY_URL for issuer: {issuer}");
        return Ok(issuer.to_string());
    }

    // Fallback: construct from KEYCLOAK_BASE_URL + KEYCLOAK_REALM
    let base = env::var("KEYCLOAK_BASE_URL").unwrap_or_default();
    let realm = env::var("KEYCLOAK_REALM").unwrap_or_default();

    if !base.is_empty() && !realm.is_empty() {
        let issuer = format!("{}/realms/{}", base.trim_end_matches('/'), realm);
        tracing::info!("using KEYCLOAK_BASE_URL + KEYCLOAK_REALM for issuer: {issuer}");
        return Ok(issuer);
    }

    Err("OIDC_CLIENT_ID is set but no issuer could be determined. \
         Set OIDC_DISCOVERY_URL, or both KEYCLOAK_BASE_URL and KEYCLOAK_REALM."
        .to_string())
}

/// Parse `BASE_URL` from the environment. Returns validated URL or error.
pub fn parse_base_url() -> Result<Option<String>, String> {
    let base_url = env::var("BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| {
            let trimmed = s.trim_end_matches('/').to_string();
            url::Url::parse(&trimmed).map_err(|e| format!("BASE_URL is not a valid URL: {e}"))?;
            tracing::info!("BASE_URL configured: {trimmed}");
            Ok::<String, String>(trimmed)
        })
        .transpose()?;
    Ok(base_url)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, value: &str) {
        // SAFETY: std locks its own environment access, so the hazard is C code
        // reading the environment on another thread. No unit test in this crate
        // resolves a host name or calls libc time functions, and every test
        // that changes these variables holds `ENV_LOCK`.
        unsafe { env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: as in `set_env`.
        unsafe { env::remove_var(key) };
    }

    const OIDC_VARS: &[&str] = &[
        "OIDC_CLIENT_ID",
        "OIDC_CLIENT_SECRET",
        "OIDC_DISCOVERY_URL",
        "KEYCLOAK_BASE_URL",
        "KEYCLOAK_REALM",
        "OIDC_REQUIRED_ROLE",
    ];

    fn clear_oidc_env() {
        for var in OIDC_VARS {
            remove_env(var);
        }
    }

    fn set_complete_oidc_env() {
        set_env("OIDC_CLIENT_ID", "test-client");
        set_env("OIDC_CLIENT_SECRET", "secret");
        set_env("KEYCLOAK_BASE_URL", "https://keycloak.example.com");
        set_env("KEYCLOAK_REALM", "test-realm");
        set_env("OIDC_REQUIRED_ROLE", "editor-reader");
    }

    #[test]
    fn no_oidc_vars_returns_none() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();

        let result = parse_oidc_from_env().expect("should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn complete_keycloak_vars_enables_auth() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();

        let oidc = parse_oidc_from_env().expect("should succeed").unwrap();
        assert_eq!(oidc.client_id, "test-client");
        assert_eq!(oidc.client_secret, "secret");
        assert_eq!(
            oidc.issuer_url,
            "https://keycloak.example.com/realms/test-realm"
        );
        assert_eq!(oidc.required_role, "editor-reader");

        clear_oidc_env();
    }

    #[test]
    fn discovery_url_takes_priority() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();
        set_env(
            "OIDC_DISCOVERY_URL",
            "https://idp.example.com/realms/my-realm/.well-known/openid-configuration",
        );

        let oidc = parse_oidc_from_env().expect("should succeed").unwrap();
        assert_eq!(oidc.issuer_url, "https://idp.example.com/realms/my-realm");

        clear_oidc_env();
    }

    #[test]
    fn missing_secret_is_error() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_env("OIDC_CLIENT_ID", "test-client");

        let result = parse_oidc_from_env();
        assert!(result.is_err());

        clear_oidc_env();
    }

    #[test]
    fn missing_issuer_is_error() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_env("OIDC_CLIENT_ID", "test-client");
        set_env("OIDC_CLIENT_SECRET", "secret");

        let result = parse_oidc_from_env();
        assert!(result.is_err());

        clear_oidc_env();
    }

    #[test]
    fn missing_required_role_defaults_to_allowed_user() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_env("OIDC_CLIENT_ID", "test-client");
        set_env("OIDC_CLIENT_SECRET", "secret");
        set_env("KEYCLOAK_BASE_URL", "https://keycloak.example.com");
        set_env("KEYCLOAK_REALM", "test-realm");
        // No OIDC_REQUIRED_ROLE — falls back to `allowed-user`.

        let oidc = parse_oidc_from_env().expect("should succeed").unwrap();
        assert_eq!(oidc.required_role, "allowed-user");

        clear_oidc_env();
    }

    #[test]
    fn empty_required_role_defaults_to_allowed_user() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_env("OIDC_CLIENT_ID", "test-client");
        set_env("OIDC_CLIENT_SECRET", "secret");
        set_env("KEYCLOAK_BASE_URL", "https://keycloak.example.com");
        set_env("KEYCLOAK_REALM", "test-realm");
        set_env("OIDC_REQUIRED_ROLE", "");

        let oidc = parse_oidc_from_env().expect("should succeed").unwrap();
        assert_eq!(oidc.required_role, "allowed-user");

        clear_oidc_env();
    }

    #[test]
    fn custom_role_from_env() {
        let _lock = ENV_LOCK.lock();
        clear_oidc_env();
        set_complete_oidc_env();
        set_env("OIDC_REQUIRED_ROLE", "harvester-admin");

        let oidc = parse_oidc_from_env().expect("should succeed").unwrap();
        assert_eq!(oidc.required_role, "harvester-admin");

        clear_oidc_env();
    }
}
