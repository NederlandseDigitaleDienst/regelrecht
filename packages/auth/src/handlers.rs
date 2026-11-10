use std::borrow::Cow;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openidconnect::core::CoreResponseType;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, CsrfToken, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;
use serde::Serialize;
use subtle::ConstantTimeEq;
use tower_sessions::Session;

use crate::OidcAppState;

pub const SESSION_KEY_CSRF: &str = "oidc_csrf";
pub const SESSION_KEY_NONCE: &str = "oidc_nonce";
pub const SESSION_KEY_PKCE_VERIFIER: &str = "oidc_pkce_verifier";
pub const SESSION_KEY_AUTHENTICATED: &str = "authenticated";
pub const SESSION_KEY_SUB: &str = "person_sub";
pub const SESSION_KEY_EMAIL: &str = "person_email";
/// Whether the upstream IdP claims the email is verified
/// (`email_verified=true` claim). Stored as a `bool`; absent or `false`
/// means the email failed verification (or no claim was sent) and
/// downstream code that attributes actions to the user should refuse
/// rather than fall back to a service identity (see
/// `editor_user_from_session` in editor-api).
pub const SESSION_KEY_EMAIL_VERIFIED: &str = "person_email_verified";
pub const SESSION_KEY_NAME: &str = "person_name";
pub const SESSION_KEY_ID_TOKEN: &str = "id_token_hint";
pub const SESSION_KEY_ROLES: &str = "person_roles";
/// OIDC refresh token, kept server-side so the refresh middleware can
/// re-validate with the IdP (and pick up role/revocation changes) without the
/// browser ever holding a token. Rotated in place when the IdP issues a new
/// refresh token. See [`crate::middleware::refresh_session_token`].
pub const SESSION_KEY_REFRESH_TOKEN: &str = "oidc_refresh_token";
/// Absolute access-token expiry (unix seconds, `i64`). Drives *when* the
/// refresh middleware re-validates with the IdP; it does not gate requests on
/// its own (the session's own inactivity window does that).
pub const SESSION_KEY_TOKEN_EXPIRES_AT: &str = "oidc_token_expires_at";
const SESSION_KEY_BASE_URL: &str = "oidc_base_url";
const SESSION_KEY_RETURN_URL: &str = "oidc_return_url";

/// Derive the base URL from `BASE_URL` env or request headers.
///
/// Used only during `/auth/login` to construct the initial `redirect_uri`.
/// The result is stored in the session so that `/auth/callback` and
/// `/auth/logout` don't need to trust request headers again — Keycloak's
/// "Valid redirect URIs" check validates the URL at login time.
fn base_url_from_config_or_request<S: OidcAppState>(state: &S, headers: &HeaderMap) -> String {
    if let Some(base_url) = state.base_url() {
        return base_url.to_string();
    }

    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("https");

    format!("{scheme}://{host}")
}

#[derive(Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub oidc_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<PersonInfo>,
}

#[derive(Serialize)]
pub struct PersonInfo {
    pub sub: String,
    pub email: String,
    pub name: String,
    /// Keycloak realm roles the user holds (e.g. `editor-reader`,
    /// `harvester-writer`). Surfaced so the frontend can gate role-specific
    /// UI (menu items, sections). Authorization itself remains enforced
    /// server-side per route — this is purely for display.
    pub roles: Vec<String>,
}

#[derive(Deserialize)]
struct RealmAccess {
    roles: Vec<String>,
}

#[derive(Deserialize)]
pub struct LoginQuery {
    pub return_url: Option<String>,
}

/// Validate that a return URL is a safe relative path (starts with `/`,
/// no protocol or host). Returns `None` for invalid or missing values.
fn validate_return_url(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() || url == "/" {
        return None;
    }
    // Must be a relative path — reject absolute URLs and protocol-relative URLs.
    // Also reject backslashes: some browsers normalise `\` to `/`, so `/\evil.com`
    // could be interpreted as the protocol-relative `//evil.com`.
    // Reject control characters (CR, LF, etc.) — Axum's Redirect::temporary panics
    // on header values containing them, which would DoS the OIDC callback.
    if !url.starts_with('/')
        || url.starts_with("//")
        || url.contains('\\')
        || url.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return None;
    }
    Some(url.to_string())
}

pub async fn login<S: OidcAppState>(
    State(state): State<S>,
    headers: HeaderMap,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<LoginQuery>,
) -> Result<Response, StatusCode> {
    let client = state.oidc_client().ok_or(StatusCode::NOT_IMPLEMENTED)?;

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "failed to cycle session ID");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let base_url = base_url_from_config_or_request(&state, &headers);
    let redirect_url = RedirectUrl::new(format!("{base_url}/auth/callback")).map_err(|e| {
        tracing::error!(error = %e, "invalid redirect URL");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_redirect_uri(Cow::Owned(redirect_url))
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    session_insert(&session, SESSION_KEY_CSRF, csrf_token.secret().clone()).await?;
    session_insert(&session, SESSION_KEY_NONCE, nonce.secret().clone()).await?;
    session_insert(
        &session,
        SESSION_KEY_PKCE_VERIFIER,
        pkce_verifier.secret().clone(),
    )
    .await?;
    session_insert(&session, SESSION_KEY_BASE_URL, base_url).await?;

    if let Some(return_url) = validate_return_url(params.return_url.as_deref()) {
        session_insert(&session, SESSION_KEY_RETURN_URL, return_url).await?;
    }

    Ok(Redirect::temporary(auth_url.as_str()).into_response())
}

async fn session_insert(session: &Session, key: &str, value: String) -> Result<(), StatusCode> {
    session.insert(key, value).await.map_err(|e| {
        tracing::error!(key, error = %e, "failed to insert into session");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[derive(serde::Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: String,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn callback<S: OidcAppState>(
    State(app_state): State<S>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<CallbackQuery>,
) -> Result<Response, StatusCode> {
    if let Some(ref error) = params.error {
        let description = params.error_description.as_deref().unwrap_or("unknown");
        tracing::warn!(error, description, "IdP returned error on callback");
        return Err(StatusCode::FORBIDDEN);
    }

    let code = params.code.ok_or_else(|| {
        tracing::warn!("callback missing authorization code");
        StatusCode::BAD_REQUEST
    })?;

    let client = app_state.oidc_client().ok_or(StatusCode::NOT_IMPLEMENTED)?;

    // Retrieve the base_url that was stored during /auth/login.
    // This is the URL that Keycloak validated against its redirect URI allowlist,
    // so we know it's trusted — no need to re-derive from request headers.
    let base_url: String = session
        .get(SESSION_KEY_BASE_URL)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read base_url from session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::warn!("no base_url in session — session may have expired");
            StatusCode::BAD_REQUEST
        })?;

    let redirect_url = RedirectUrl::new(format!("{base_url}/auth/callback")).map_err(|e| {
        tracing::error!(error = %e, "invalid redirect URL");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let stored_csrf: String = session
        .get(SESSION_KEY_CSRF)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to read CSRF from session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            tracing::warn!("no CSRF token in session");
            StatusCode::BAD_REQUEST
        })?;

    // Constant-time comparison to prevent timing side-channels.
    let csrf_matches: bool = params.state.as_bytes().ct_eq(stored_csrf.as_bytes()).into();
    if !csrf_matches {
        tracing::warn!("CSRF token mismatch");
        return Err(StatusCode::BAD_REQUEST);
    }

    let stored_nonce: String = session
        .get(SESSION_KEY_NONCE)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let stored_pkce: String = session
        .get(SESSION_KEY_PKCE_VERIFIER)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_redirect_uri(Cow::Owned(redirect_url))
        .set_pkce_verifier(PkceCodeVerifier::new(stored_pkce))
        .request_async(app_state.http_client())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "token exchange failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let id_token = token_response.id_token().ok_or_else(|| {
        tracing::error!("no ID token in response");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let nonce_verifier = openidconnect::Nonce::new(stored_nonce);
    let claims = id_token
        .claims(&client.id_token_verifier(), &nonce_verifier)
        .map_err(|e| {
            tracing::error!(error = %e, "ID token verification failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let required_role = &app_state
        .oidc_config()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .required_role;

    // Extract realm roles, preferring the already-verified ID token.
    // Keycloak includes `realm_access` in the access token by default; to use
    // the more secure ID-token path, add a "realm roles" mapper to the client
    // scopes in Keycloak so that `realm_access` is included in the ID token.
    let id_token_jwt = id_token.to_string();
    let realm_roles = extract_realm_roles(&id_token_jwt).or_else(|| {
        tracing::info!(
            "realm_access not found in ID token — falling back to access token. \
             Configure a Keycloak client mapper to include realm_access in the ID token."
        );
        let access_token_secret = get_access_token_secret(&token_response);
        extract_realm_roles(access_token_secret)
    });
    tracing::debug!(
        sub = %claims.subject().as_str(),
        roles = ?realm_roles,
        required = %required_role,
        "checking realm roles"
    );

    let realm_roles = match realm_roles {
        Some(roles) => roles,
        None => {
            tracing::warn!(
                sub = %claims.subject().as_str(),
                "no realm_access claim in ID token or access token — \
                 check Keycloak client mappers"
            );
            Vec::new()
        }
    };
    if !realm_roles.contains(required_role) {
        tracing::warn!(role = %required_role, "user lacks required role");
        return Err(StatusCode::FORBIDDEN);
    }

    let sub = claims.subject().as_str().to_string();
    let email = claims.email().map(|e| (**e).clone()).unwrap_or_default();
    // Standard OIDC `email_verified` claim — `false`/missing is the
    // safe default. Editor commit attribution refuses to use the email
    // when this is `false`, so a misconfigured IdP fails closed instead
    // of letting a spoofed email through.
    let email_verified = claims.email_verified().unwrap_or(false);
    let name = claims
        .name()
        .and_then(|n| n.get(None).map(|v| (**v).clone()))
        .or_else(|| claims.preferred_username().map(|u| (**u).clone()))
        .unwrap_or_default();

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "failed to rotate session ID");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Err(e) = session.remove::<String>(SESSION_KEY_CSRF).await {
        tracing::warn!(error = %e, "failed to remove CSRF from session");
    }
    if let Err(e) = session.remove::<String>(SESSION_KEY_NONCE).await {
        tracing::warn!(error = %e, "failed to remove nonce from session");
    }
    if let Err(e) = session.remove::<String>(SESSION_KEY_PKCE_VERIFIER).await {
        tracing::warn!(error = %e, "failed to remove PKCE verifier from session");
    }
    // Keep SESSION_KEY_BASE_URL — logout needs it for post_logout_redirect_uri.

    session
        .insert(SESSION_KEY_AUTHENTICATED, true)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    session_insert(&session, SESSION_KEY_SUB, sub.clone()).await?;
    session_insert(&session, SESSION_KEY_EMAIL, email.clone()).await?;
    // `email_verified` is a bool, not a String — go through `insert`
    // directly. Same error mapping as `session_insert` for consistency.
    session
        .insert(SESSION_KEY_EMAIL_VERIFIED, email_verified)
        .await
        .map_err(|e| {
            tracing::error!(key = SESSION_KEY_EMAIL_VERIFIED, error = %e, "failed to insert into session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    session_insert(&session, SESSION_KEY_NAME, name.clone()).await?;
    session_insert(&session, SESSION_KEY_ID_TOKEN, id_token_jwt).await?;
    // Persist the realm roles so route-level `require_role` middleware can
    // gate per-route access without re-parsing the JWT on every request.
    session
        .insert(SESSION_KEY_ROLES, realm_roles)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to insert roles into session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Persist refresh material so the refresh middleware can re-validate with
    // the IdP before the access token's lifetime elapses. The refresh token is
    // optional (only stored when the IdP returns one); without it the session
    // simply never re-validates and falls back to its own inactivity window.
    if let Some(refresh_token) = token_response.refresh_token() {
        session_insert(
            &session,
            SESSION_KEY_REFRESH_TOKEN,
            refresh_token.secret().clone(),
        )
        .await?;
    }
    let expires_at = unix_now() + access_token_ttl_secs(token_response.expires_in());
    session
        .insert(SESSION_KEY_TOKEN_EXPIRES_AT, expires_at)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to insert token expiry into session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::debug!(email = %email, "OIDC login successful");

    let return_url: Option<String> = session.get(SESSION_KEY_RETURN_URL).await.ok().flatten();
    let _ = session.remove::<String>(SESSION_KEY_RETURN_URL).await;

    let redirect_target = match validate_return_url(return_url.as_deref()) {
        Some(path) => format!("{base_url}{path}"),
        None => format!("{base_url}/"),
    };

    Ok(Redirect::temporary(&redirect_target).into_response())
}

pub async fn logout<S: OidcAppState>(
    State(state): State<S>,
    session: Session,
) -> Result<Response, StatusCode> {
    let id_token_hint: Option<String> = session.get(SESSION_KEY_ID_TOKEN).await.ok().flatten();

    // Read the base_url stored during the OIDC login flow. Derived from
    // trusted config or request headers at login time (same origin used for
    // the Keycloak-validated redirect_uri). Stored server-side after
    // cycle_id() prevents session fixation. Falls back to BASE_URL config,
    // then to "/" for a relative redirect.
    let base_url: String = match session.get(SESSION_KEY_BASE_URL).await {
        Ok(Some(url)) => url,
        Ok(None) => state.base_url().map(|u| u.to_string()).unwrap_or_else(|| {
            tracing::warn!("no base_url in session and BASE_URL not configured — post_logout_redirect_uri will be relative");
            "/".to_string()
        }),
        Err(e) => {
            tracing::warn!(error = %e, "failed to read base_url from session, using config fallback");
            state.base_url().map(|u| u.to_string()).unwrap_or_else(|| {
                tracing::warn!("BASE_URL not configured — post_logout_redirect_uri will be relative");
                "/".to_string()
            })
        }
    };

    session.flush().await.map_err(|e| {
        tracing::error!(error = %e, "failed to flush session");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let (Some(end_session_url), Some(oidc)) = (state.end_session_url(), state.oidc_config()) {
        let mut url = url::Url::parse(end_session_url).map_err(|e| {
            tracing::error!(error = %e, "invalid end_session_endpoint URL");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        url.query_pairs_mut()
            .append_pair("post_logout_redirect_uri", &base_url)
            .append_pair("client_id", &oidc.client_id);
        if let Some(ref hint) = id_token_hint {
            url.query_pairs_mut().append_pair("id_token_hint", hint);
        }

        Ok(Redirect::temporary(url.as_str()).into_response())
    } else {
        Ok(Redirect::temporary(&format!("{base_url}/")).into_response())
    }
}

pub async fn status<S: OidcAppState>(State(state): State<S>, session: Session) -> Json<AuthStatus> {
    let oidc_configured = state.is_auth_enabled();

    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);

    let person = if authenticated {
        let sub: String = session
            .get(SESSION_KEY_SUB)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let email: String = session
            .get(SESSION_KEY_EMAIL)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let name: String = session
            .get(SESSION_KEY_NAME)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let roles: Vec<String> = session
            .get(SESSION_KEY_ROLES)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        Some(PersonInfo {
            sub,
            email,
            name,
            roles,
        })
    } else {
        None
    };

    Json(AuthStatus {
        authenticated,
        oidc_configured,
        person,
    })
}

#[derive(Deserialize)]
struct JwtPayload {
    realm_access: Option<RealmAccess>,
}

fn get_access_token_secret(resp: &impl OAuth2TokenResponse) -> &str {
    resp.access_token().secret()
}

/// Current wall-clock time in unix seconds. Saturates to 0 if the clock is
/// before the epoch (never in practice) so callers get a monotonic-ish `i64`.
pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Access-token lifetime in seconds. Falls back to 300s when the IdP omits
/// `expires_in` — comfortably above the refresh middleware's skew window so a
/// missing `expires_in` doesn't put the session straight back into the
/// refresh-now band (which would refresh on every request). Keycloak always
/// sends `expires_in`, so the fallback is belt-and-braces.
pub(crate) fn access_token_ttl_secs(expires_in: Option<std::time::Duration>) -> i64 {
    expires_in.map(|d| d.as_secs() as i64).unwrap_or(300)
}

/// Decode `realm_access.roles` from a JWT payload.
///
/// Called first on the signature-verified ID token, then as fallback on the
/// access token received directly from the trusted token endpoint over TLS.
pub fn extract_realm_roles(jwt: &str) -> Option<Vec<String>> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let payload: JwtPayload = serde_json::from_slice(&payload_bytes).ok()?;
    Some(payload.realm_access?.roles)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    fn fake_jwt(payload_json: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256"}"#);
        let payload = URL_SAFE_NO_PAD.encode(payload_json);
        format!("{header}.{payload}.fake-signature")
    }

    // --- CallbackQuery deserialization ---

    #[test]
    fn callback_query_success_response() {
        let q: CallbackQuery =
            serde_json::from_str(r#"{"code":"abc123","state":"csrf-token"}"#).unwrap();
        assert_eq!(q.code.unwrap(), "abc123");
        assert_eq!(q.state, "csrf-token");
        assert!(q.error.is_none());
        assert!(q.error_description.is_none());
    }

    #[test]
    fn callback_query_error_response() {
        let q: CallbackQuery = serde_json::from_str(
            r#"{"state":"csrf-token","error":"access_denied","error_description":"User denied"}"#,
        )
        .unwrap();
        assert!(q.code.is_none());
        assert_eq!(q.error.unwrap(), "access_denied");
        assert_eq!(q.error_description.unwrap(), "User denied");
    }

    #[test]
    fn callback_query_missing_state_fails() {
        let result: Result<CallbackQuery, _> = serde_json::from_str(r#"{"code":"abc123"}"#);
        assert!(result.is_err());
    }

    // --- AuthStatus / PersonInfo serialization (the /auth/status contract) ---

    #[test]
    fn auth_status_serializes_person_roles() {
        let status = AuthStatus {
            authenticated: true,
            oidc_configured: true,
            person: Some(PersonInfo {
                sub: "sub-123".to_string(),
                email: "user@example.org".to_string(),
                name: "Test User".to_string(),
                roles: vec!["editor-reader".to_string(), "harvester-writer".to_string()],
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&status).unwrap();
        assert_eq!(
            json["person"]["roles"],
            serde_json::json!(["editor-reader", "harvester-writer"])
        );
    }

    #[test]
    fn auth_status_unauthenticated_omits_person() {
        let status = AuthStatus {
            authenticated: false,
            oidc_configured: true,
            person: None,
        };
        let json: serde_json::Value = serde_json::to_value(&status).unwrap();
        assert!(json.get("person").is_none());
    }

    // --- extract_realm_roles ---

    #[test]
    fn extract_roles_with_valid_realm_access() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":["allowed-user","viewer"]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert_eq!(roles, vec!["allowed-user", "viewer"]);
    }

    #[test]
    fn extract_roles_missing_realm_access() {
        let jwt = fake_jwt(r#"{"sub":"user1"}"#);
        assert!(extract_realm_roles(&jwt).is_none());
    }

    #[test]
    fn extract_roles_empty_roles_array() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":[]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert!(roles.is_empty());
    }

    #[test]
    fn extract_roles_invalid_jwt() {
        assert!(extract_realm_roles("not-a-jwt").is_none());
    }

    #[test]
    fn extract_roles_contains_check() {
        let jwt = fake_jwt(r#"{"realm_access":{"roles":["allowed-user","editor"]}}"#);
        let roles = extract_realm_roles(&jwt).unwrap();
        assert!(roles.contains(&"allowed-user".to_string()));
        assert!(!roles.contains(&"admin".to_string()));
    }

    // --- validate_return_url ---

    #[test]
    fn return_url_valid_path() {
        assert_eq!(
            validate_return_url(Some("/library/some-law/article-5")),
            Some("/library/some-law/article-5".to_string())
        );
    }

    #[test]
    fn return_url_with_query() {
        assert_eq!(
            validate_return_url(Some("/library?tab=jobs")),
            Some("/library?tab=jobs".to_string())
        );
    }

    #[test]
    fn return_url_rejects_root() {
        assert_eq!(validate_return_url(Some("/")), None);
    }

    #[test]
    fn return_url_rejects_empty() {
        assert_eq!(validate_return_url(Some("")), None);
        assert_eq!(validate_return_url(None), None);
    }

    #[test]
    fn return_url_rejects_absolute_url() {
        assert_eq!(validate_return_url(Some("https://evil.com/steal")), None);
    }

    #[test]
    fn return_url_rejects_protocol_relative() {
        assert_eq!(validate_return_url(Some("//evil.com/steal")), None);
    }

    #[test]
    fn return_url_rejects_backslash() {
        assert_eq!(validate_return_url(Some("/\\evil.com")), None);
        assert_eq!(validate_return_url(Some("/path\\segment")), None);
    }

    #[test]
    fn return_url_rejects_whitespace_only() {
        assert_eq!(validate_return_url(Some("   ")), None);
        assert_eq!(validate_return_url(Some("  /  ")), None);
    }

    #[test]
    fn return_url_allows_fragment() {
        assert_eq!(
            validate_return_url(Some("/library#section")),
            Some("/library#section".to_string())
        );
    }

    #[test]
    fn return_url_rejects_control_characters() {
        // CRLF injection — would panic in Axum's Redirect::temporary
        assert_eq!(
            validate_return_url(Some("/library\r\nX-Injected: header")),
            None
        );
        assert_eq!(validate_return_url(Some("/library\nheader")), None);
        // DEL character
        assert_eq!(validate_return_url(Some("/library\x7f")), None);
        // Null byte
        assert_eq!(validate_return_url(Some("/library\0")), None);
    }
}
