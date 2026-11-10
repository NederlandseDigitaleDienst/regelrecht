//! Shared OIDC configuration, loaded from environment variables.
//!
//! Each parser takes the variable lookup as a parameter, so tests pass their
//! own values instead of changing the process environment.
//! [`parse_oidc_from_env`] and [`parse_base_url`] pass [`process_env`].

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
///
/// # Errors
///
/// When `OIDC_CLIENT_ID` is set but the secret or the issuer is missing.
pub fn parse_oidc_from_env() -> Result<Option<OidcConfig>, String> {
    parse_oidc_from(process_env)
}

/// [`parse_oidc_from_env`] with the variables read through `env`.
///
/// # Errors
///
/// As [`parse_oidc_from_env`].
pub fn parse_oidc_from(env: impl Fn(&str) -> Option<String>) -> Result<Option<OidcConfig>, String> {
    match env("OIDC_CLIENT_ID") {
        None => Ok(None),
        Some(client_id) => Ok(Some(parse_oidc_config(client_id, &env)?)),
    }
}

/// Reads a variable from the process environment; a value that is not valid
/// Unicode counts as unset.
pub fn process_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn parse_oidc_config(
    client_id: String,
    env: &impl Fn(&str) -> Option<String>,
) -> Result<OidcConfig, String> {
    let client_secret = env("OIDC_CLIENT_SECRET").unwrap_or_default();
    if client_secret.is_empty() {
        return Err("OIDC_CLIENT_ID is set but OIDC_CLIENT_SECRET is missing. \
             Refusing to start without complete OIDC configuration."
            .to_string());
    }

    let issuer_url = resolve_issuer_url(env)?;

    // Minimum realm role required to log in (the login gate). Typically
    // `<app>-reader`. Per-route `require_role` middleware layers finer-grained
    // gates on top. Defaults to `allowed-user` for backwards compatibility
    // with the pre-RBAC deployment; operators should set it explicitly to the
    // per-app reader role (`editor-reader` / `harvester-reader`) once the
    // migration in `docs/auth-and-roles.md` is complete.
    let required_role = env("OIDC_REQUIRED_ROLE")
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

fn resolve_issuer_url(env: &impl Fn(&str) -> Option<String>) -> Result<String, String> {
    // OIDC_DISCOVERY_URL takes priority (RIG-style injection)
    if let Some(discovery_url) = env("OIDC_DISCOVERY_URL")
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
    let base = env("KEYCLOAK_BASE_URL").unwrap_or_default();
    let realm = env("KEYCLOAK_REALM").unwrap_or_default();

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
///
/// # Errors
///
/// When `BASE_URL` is set but is not a valid URL.
pub fn parse_base_url() -> Result<Option<String>, String> {
    parse_base_url_from(process_env)
}

/// [`parse_base_url`] with the variable read through `env`.
///
/// # Errors
///
/// As [`parse_base_url`].
pub fn parse_base_url_from(env: impl Fn(&str) -> Option<String>) -> Result<Option<String>, String> {
    let base_url = env("BASE_URL")
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
    use std::collections::HashMap;

    /// A lookup over fixed values, standing in for the process environment.
    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |key| vars.get(key).cloned()
    }

    const COMPLETE_OIDC: &[(&str, &str)] = &[
        ("OIDC_CLIENT_ID", "test-client"),
        ("OIDC_CLIENT_SECRET", "secret"),
        ("KEYCLOAK_BASE_URL", "https://keycloak.example.com"),
        ("KEYCLOAK_REALM", "test-realm"),
        ("OIDC_REQUIRED_ROLE", "editor-reader"),
    ];

    /// [`COMPLETE_OIDC`] with `extra` added, replacing a key it already has.
    fn complete_with(extra: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let mut pairs: Vec<(&str, &str)> = COMPLETE_OIDC
            .iter()
            .filter(|(k, _)| !extra.iter().any(|(e, _)| e == k))
            .copied()
            .collect();
        pairs.extend_from_slice(extra);
        env_of(&pairs)
    }

    #[test]
    fn no_oidc_vars_returns_none() {
        let result = parse_oidc_from(env_of(&[])).expect("should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn complete_keycloak_vars_enables_auth() {
        let oidc = parse_oidc_from(env_of(COMPLETE_OIDC))
            .expect("should succeed")
            .unwrap();
        assert_eq!(oidc.client_id, "test-client");
        assert_eq!(oidc.client_secret, "secret");
        assert_eq!(
            oidc.issuer_url,
            "https://keycloak.example.com/realms/test-realm"
        );
        assert_eq!(oidc.required_role, "editor-reader");
    }

    #[test]
    fn discovery_url_takes_priority() {
        let env = complete_with(&[(
            "OIDC_DISCOVERY_URL",
            "https://idp.example.com/realms/my-realm/.well-known/openid-configuration",
        )]);
        let oidc = parse_oidc_from(env).expect("should succeed").unwrap();
        assert_eq!(oidc.issuer_url, "https://idp.example.com/realms/my-realm");
    }

    #[test]
    fn missing_secret_is_error() {
        let result = parse_oidc_from(env_of(&[("OIDC_CLIENT_ID", "test-client")]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_issuer_is_error() {
        let result = parse_oidc_from(env_of(&[
            ("OIDC_CLIENT_ID", "test-client"),
            ("OIDC_CLIENT_SECRET", "secret"),
        ]));
        assert!(result.is_err());
    }

    #[test]
    fn missing_required_role_defaults_to_allowed_user() {
        let env = env_of(&[
            ("OIDC_CLIENT_ID", "test-client"),
            ("OIDC_CLIENT_SECRET", "secret"),
            ("KEYCLOAK_BASE_URL", "https://keycloak.example.com"),
            ("KEYCLOAK_REALM", "test-realm"),
        ]);
        let oidc = parse_oidc_from(env).expect("should succeed").unwrap();
        assert_eq!(oidc.required_role, "allowed-user");
    }

    #[test]
    fn empty_required_role_defaults_to_allowed_user() {
        let env = complete_with(&[("OIDC_REQUIRED_ROLE", "")]);
        let oidc = parse_oidc_from(env).expect("should succeed").unwrap();
        assert_eq!(oidc.required_role, "allowed-user");
    }

    #[test]
    fn custom_role_from_env() {
        let env = complete_with(&[("OIDC_REQUIRED_ROLE", "harvester-admin")]);
        let oidc = parse_oidc_from(env).expect("should succeed").unwrap();
        assert_eq!(oidc.required_role, "harvester-admin");
    }

    #[test]
    fn base_url_is_trimmed_and_validated() {
        let env = env_of(&[("BASE_URL", "https://editor.example.com/")]);
        assert_eq!(
            parse_base_url_from(env).unwrap().as_deref(),
            Some("https://editor.example.com")
        );
        assert!(parse_base_url_from(env_of(&[("BASE_URL", "not a url")])).is_err());
        assert_eq!(parse_base_url_from(env_of(&[])).unwrap(), None);
    }
}
