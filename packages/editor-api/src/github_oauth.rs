//! Per-user GitHub OAuth link flow (spike).
//!
//! Instead of the editor holding an all-access GitHub App/service token and
//! policing which user may write which repo, this lets each editor user link
//! **their own** GitHub account via a GitHub *OAuth App* (Authorization Code
//! flow). Traject writes then authenticate as the user, so GitHub — not the
//! editor — enforces repo access. The editor never holds a credential that can
//! reach a repo the acting user can't.
//!
//! Endpoints (authenticated editor session, except the public relay):
//!   * `GET  /auth/github/login`      → redirect to GitHub's consent screen
//!   * `GET  /auth/github/callback`   → exchange code, seal the token into a cookie, bounce back
//!   * `GET  /auth/github/status`     → `{ connected, github_login, scopes, expired }`
//!   * `POST /auth/github/disconnect` → revoke at GitHub + clear the cookie
//!   * `GET  /auth/github/relay`      → **public** forwarder (relay mode)
//!
//! ## One OAuth App for every preview (relay mode)
//!
//! A classic OAuth App has a single registered callback host, so per-PR preview
//! deployments (each on its own host) would otherwise each need their own App.
//! Relay mode avoids that: `/login` sends GitHub a fixed `redirect_uri` of
//! `{GITHUB_OAUTH_CALLBACK_BASE}/auth/github/relay`, the origin deployment is
//! carried in `state`, and the public relay 302-forwards GitHub's response to
//! that deployment's own `/callback` (validated against an allowlist so it can't
//! become an open redirect). One App, one secret, every preview + production.
//!
//! ## No token at rest — it lives in the browser
//!
//! The editor deliberately stores **no** GitHub credential server-side (no DB
//! row, no session-store entry — the tower-sessions store is Postgres-backed,
//! so "in the session" would still mean "in the database"). Instead the
//! callback seals the token with [`crate::crypto`] into an HttpOnly,
//! session-scoped browser cookie, bound to the editor account that linked it.
//! Every write request carries the cookie; the server opens it transiently and
//! threads the token into the corpus write path via
//! `WriteContext::token_override`. Disconnect = revoke at GitHub + clear the
//! cookie. Consequences: linking is per browser (and gone when the browser
//! session ends), and the token is only available *during a request* — a
//! background job can never write as the user.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use axum::Router;
use axum::extract::{Extension, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tower_sessions::Session;
use uuid::Uuid;

use crate::accounts::AccountRecord;
use crate::crypto::TokenCipher;
use crate::state::AppState;

const SESSION_KEY_CSRF: &str = "github_oauth_csrf";
const SESSION_KEY_BASE_URL: &str = "github_oauth_base_url";
const SESSION_KEY_RETURN: &str = "github_oauth_return";
/// The exact `redirect_uri` sent to GitHub at `/login`, stashed so the code
/// exchange in `/callback` presents the identical value (GitHub requires it to
/// match). In relay mode this is the fixed relay host, not the deployment's own.
const SESSION_KEY_REDIRECT_URI: &str = "github_oauth_redirect_uri";

/// Default OAuth scopes: `repo` for read/write to the user's private repos,
/// `read:org` so org membership resolves for org-owned traject repos.
const DEFAULT_SCOPES: &str = "repo read:org";

/// Configuration + secrets for the GitHub OAuth App. Present in [`AppState`]
/// only when the feature is fully configured; `None` disables every endpoint
/// (they return 501) and leaves the write path on its existing token.
#[derive(Clone)]
pub struct GithubOAuth {
    pub client_id: String,
    client_secret: String,
    pub scopes: String,
    /// Cipher for sealing/opening the browser token cookie.
    pub cipher: Arc<TokenCipher>,
    /// Static (env: `GITHUB_USER_TOKEN_REQUIRED`) switch for routing traject
    /// writes through the acting user's own token ("editor is not in the
    /// middle" mode). This is one of TWO switches: the `github.user_oauth`
    /// feature flag enforces the same thing at runtime, so enabling the
    /// GitHub-koppeling UI also enables enforcement — linking is never
    /// offered-but-inert. See [`crate::credentials::write_requires_user_token`]
    /// for the combined decision; this env var remains as a deployment-wide
    /// override that wins regardless of the flag.
    ///
    /// * required off (both switches): writes **always** use the backend's
    ///   configured token — byte-identical to pre-spike behaviour for every
    ///   user, linked or not, so a linked user's saves to the operator-managed
    ///   central repo can't start 403-ing because their personal token lacks
    ///   access.
    /// * required on (either switch): a configured service token still takes
    ///   precedence per backend (see
    ///   [`crate::credentials::TrajectCredentials::for_write`]); only
    ///   writes to token-less, override-capable backends must carry the
    ///   acting user's token, and a save there with no linked (or an
    ///   expired) token is refused with 428.
    pub require_user_token: bool,
    /// Relay mode. When set, `/login` sends GitHub a **fixed** `redirect_uri`
    /// of `{callback_base}/auth/github/relay` instead of the deployment's own
    /// callback, so a single OAuth App (whose one registered callback URL is
    /// that relay) serves every ephemeral preview + production. The relay then
    /// 302-forwards GitHub's response to the originating deployment (carried in
    /// the signed-ish `state`). `None` = self-callback, exactly as before.
    pub callback_base: Option<String>,
    /// Host suffixes the relay may forward to (e.g. `editor.regelrecht.rijks.app`
    /// plus the preview host pattern). The relay refuses any origin whose host
    /// doesn't match — without this the relay would be an open redirect that
    /// leaks the OAuth `code`. Required (non-empty) whenever `callback_base` is
    /// set; `from_env` fails closed otherwise. Keep the deployed value as tight
    /// as the preview host pattern allows — prefer the exact prod host + the
    /// specific preview suffix over a broad shared apex like `rijksapps.nl`.
    pub allowed_origin_suffixes: Vec<String>,
    /// Base of GitHub's web OAuth endpoints. Overridable for tests.
    pub github_base: String,
    /// Base of the GitHub REST API. Overridable for tests.
    pub api_base: String,
}

impl GithubOAuth {
    /// Build from environment. Returns `Ok(None)` when unconfigured, `Err`
    /// when partially configured (so a half-set deployment fails loudly
    /// instead of silently disabling the feature).
    pub fn from_env() -> Result<Option<Self>, String> {
        let client_id = std::env::var("GITHUB_OAUTH_CLIENT_ID").ok();
        let client_secret = std::env::var("GITHUB_OAUTH_CLIENT_SECRET").ok();
        let enc_key = std::env::var("GITHUB_TOKEN_ENC_KEY").ok();

        match (client_id, client_secret, enc_key) {
            (None, None, None) => Ok(None),
            (Some(client_id), Some(client_secret), Some(enc_key)) => {
                if client_id.trim().is_empty() || client_secret.trim().is_empty() {
                    return Err(
                        "GITHUB_OAUTH_CLIENT_ID / GITHUB_OAUTH_CLIENT_SECRET must not be empty"
                            .to_string(),
                    );
                }
                let cipher = TokenCipher::from_base64_key(&enc_key)?;
                let scopes = std::env::var("GITHUB_OAUTH_SCOPES")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_SCOPES.to_string());
                let require_user_token = std::env::var("GITHUB_USER_TOKEN_REQUIRED")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                let callback_base = std::env::var("GITHUB_OAUTH_CALLBACK_BASE")
                    .ok()
                    .map(|s| s.trim().trim_end_matches('/').to_string())
                    .filter(|s| !s.is_empty());
                let allowed_origin_suffixes: Vec<String> =
                    std::env::var("GITHUB_OAUTH_ALLOWED_ORIGIN_SUFFIXES")
                        .unwrap_or_default()
                        .split(',')
                        .map(|s| s.trim().trim_start_matches('.').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                // Relay mode without an allowlist would be an open redirect for
                // the OAuth `code` — refuse to start rather than ship that.
                if callback_base.is_some() && allowed_origin_suffixes.is_empty() {
                    return Err("GITHUB_OAUTH_CALLBACK_BASE is set but \
                         GITHUB_OAUTH_ALLOWED_ORIGIN_SUFFIXES is empty; the relay \
                         needs an origin allowlist (comma-separated host suffixes)"
                        .to_string());
                }
                Ok(Some(Self {
                    client_id,
                    client_secret,
                    scopes,
                    cipher: Arc::new(cipher),
                    require_user_token,
                    callback_base,
                    allowed_origin_suffixes,
                    github_base: "https://github.com".to_string(),
                    api_base: "https://api.github.com".to_string(),
                }))
            }
            _ => Err("GitHub user-OAuth is partially configured: set all of \
                 GITHUB_OAUTH_CLIENT_ID, GITHUB_OAUTH_CLIENT_SECRET and \
                 GITHUB_TOKEN_ENC_KEY, or none of them"
                .to_string()),
        }
    }

    /// Fully-formed config with dummy GitHub credentials, for tests only.
    /// Integration tests live in a separate crate, so a `#[cfg(test)]`
    /// constructor can't reach them and `client_secret` is deliberately
    /// private. Production wiring goes through [`GithubOAuth::from_env`].
    // Only called from integration tests; the bin target (main.rs includes
    // these modules directly) would otherwise flag it as dead code, and the
    // expect on a static test key is fine outside production paths.
    #[allow(dead_code, clippy::expect_used)]
    #[doc(hidden)]
    pub fn for_tests(require_user_token: bool) -> Self {
        use base64::engine::general_purpose::STANDARD;
        let cipher = TokenCipher::from_base64_key(&STANDARD.encode([9u8; 32]))
            .expect("static 32-byte test key is valid");
        Self {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            scopes: "repo".to_string(),
            cipher: Arc::new(cipher),
            require_user_token,
            callback_base: None,
            allowed_origin_suffixes: Vec::new(),
            github_base: "https://github.invalid".to_string(),
            api_base: "https://api.github.invalid".to_string(),
        }
    }
}

/// Routes for the GitHub OAuth link flow. Mounted behind session auth +
/// `account_middleware` in `main.rs` so every handler sees an [`AccountRecord`].
pub fn github_oauth_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/github/login", get(login))
        .route("/auth/github/callback", get(callback))
        .route("/auth/github/status", get(status))
        .route("/auth/github/disconnect", post(disconnect))
}

/// The relay route. **Public** (no session/account) — GitHub redirects the
/// user's browser here on the fixed callback host, and we 302 them on to the
/// deployment that started the flow. Mounted outside the auth layer in
/// `main.rs`.
pub fn github_relay_route() -> Router<AppState> {
    Router::new().route("/auth/github/relay", get(relay))
}

/// State carried through GitHub: the CSRF token (checked against the session on
/// return) plus the origin base URL the relay must bounce back to. Base64url of
/// this JSON is the `state` parameter.
#[derive(Serialize, Deserialize)]
struct OAuthState {
    c: String,
    o: String,
}

fn encode_state(csrf: &str, origin: &str) -> String {
    let payload = OAuthState {
        c: csrf.to_string(),
        o: origin.to_string(),
    };
    // Serializing two plain strings can't fail in practice; if it somehow
    // does, log it — the resulting empty `state` surfaces to the user as an
    // opaque "state mismatch" bounce, which is undebuggable without this.
    let json = serde_json::to_vec(&payload).unwrap_or_else(|e| {
        tracing::error!(error = %e, "failed to serialize OAuth state parameter");
        Vec::new()
    });
    URL_SAFE_NO_PAD.encode(json)
}

fn decode_state(s: &str) -> Option<(String, String)> {
    let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
    let st: OAuthState = serde_json::from_slice(&bytes).ok()?;
    Some((st.c, st.o))
}

/// Whether the relay may forward to `origin`. Must be an absolute **https** URL
/// whose host equals, or is a subdomain of, a configured suffix. This is the
/// only thing standing between the relay and an open redirect of the OAuth
/// `code`, so it fails closed on any parse/scheme/host mismatch.
fn origin_allowed(oauth: &GithubOAuth, origin: &str) -> bool {
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    oauth
        .allowed_origin_suffixes
        .iter()
        .any(|sfx| host == sfx || host.ends_with(&format!(".{sfx}")))
}

impl GithubOAuth {
    /// The `redirect_uri` handed to GitHub for a login originating at `origin`.
    /// In relay mode this is always the fixed relay host (so one registered
    /// callback covers every deployment); otherwise it's the deployment's own
    /// callback.
    fn redirect_uri_for(&self, origin: &str) -> String {
        match self.callback_base.as_deref() {
            Some(cb) => format!("{cb}/auth/github/relay"),
            None => format!("{origin}/auth/github/callback"),
        }
    }
}

// --- Sealed token cookie -----------------------------------------------------
//
// The user's GitHub token is never persisted server-side. It lives in this
// HttpOnly, session-scoped cookie, sealed (encrypted + authenticated) with the
// server-held `GITHUB_TOKEN_ENC_KEY` so the browser carries an opaque blob it
// can neither read nor forge, and a leaked cookie value is useless without the
// server key.

/// Cookie holding the sealed token. Not `__Host-` prefixed because local http
/// dev needs the non-`Secure` variant (see [`is_http_localhost`]).
const TOKEN_COOKIE: &str = "github_user_token";

/// Plaintext of the sealed cookie: the token plus the metadata `status` and
/// the write path need. Bound to the editor account that linked it, so a
/// cookie replayed under another user's editor session reads as "not linked".
#[derive(Serialize, Deserialize)]
struct UserTokenCookie {
    /// Editor account id (`Uuid` string form) the token was linked under.
    account: String,
    /// The GitHub access token.
    access_token: String,
    /// GitHub login (handle) — shown in the UI so the user can confirm which
    /// account is linked, never used for authorization.
    github_login: String,
    /// Space-separated granted scopes, as reported by the token endpoint.
    scopes: String,
    /// Absolute expiry (unix seconds) when the provider reported one. Classic
    /// OAuth App `repo` tokens never expire, so usually `None`.
    expires_at: Option<u64>,
}

impl UserTokenCookie {
    fn expired(&self) -> bool {
        self.expires_at.is_some_and(|e| now_unix() >= e)
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Seal a token payload into the base64url cookie value.
fn seal_token_cookie(oauth: &GithubOAuth, payload: &UserTokenCookie) -> Result<String, String> {
    let json = serde_json::to_string(payload)
        .map_err(|e| format!("failed to serialize token cookie: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(oauth.cipher.encrypt(&json)?))
}

/// `Cookie`-header value (`name=value`) with a validly sealed token, for
/// tests only — integration tests live in a separate crate and need a linked
/// state without walking the OAuth dance. See [`GithubOAuth::for_tests`].
#[allow(dead_code, clippy::expect_used)]
#[doc(hidden)]
pub fn seal_token_cookie_for_tests(
    oauth: &GithubOAuth,
    account_id: Uuid,
    access_token: &str,
) -> String {
    let payload = UserTokenCookie {
        account: account_id.to_string(),
        access_token: access_token.to_string(),
        github_login: "test-user".to_string(),
        scopes: "repo".to_string(),
        expires_at: None,
    };
    let sealed =
        seal_token_cookie(oauth, &payload).expect("sealing a static test payload cannot fail");
    format!("{TOKEN_COOKIE}={sealed}")
}

/// Read and open the sealed token cookie for `account_id`. Returns `None` on
/// *any* failure — an absent, undecodable, tampered, wrong-key or
/// foreign-account cookie all simply mean "not linked". The account check is a
/// plain compare: both sides are server-derived (session account vs sealed
/// payload), not attacker-supplied secrets, so no constant-time is needed.
fn open_token_cookie(
    oauth: &GithubOAuth,
    headers: &HeaderMap,
    account_id: Uuid,
) -> Option<UserTokenCookie> {
    let value = cookie_value(headers, TOKEN_COOKIE)?;
    let blob = URL_SAFE_NO_PAD.decode(value.as_bytes()).ok()?;
    let json = oauth.cipher.decrypt(&blob).ok()?;
    let payload: UserTokenCookie = serde_json::from_str(&json).ok()?;
    if payload.account != account_id.to_string() {
        tracing::warn!("github token cookie is bound to a different account — ignoring");
        return None;
    }
    Some(payload)
}

/// The sealed per-user GitHub link, opened for one request. The cookie
/// internals (encryption, account binding, the `expired` computation) stay
/// private to this module; [`crate::credentials`] consumes this narrow view to
/// apply the requiredness / 428 policy.
pub(crate) struct OpenedLink {
    /// The GitHub access token to authenticate as the linked user.
    pub access_token: String,
    /// Whether the provider-reported expiry has passed.
    pub expired: bool,
    /// GitHub login (handle) — for the authorizing-as log line only.
    pub github_login: String,
}

/// Open the sealed per-user token cookie for `account_id`, exposing only what
/// the credential policy needs. `None` on *any* cookie failure (absent,
/// undecodable, tampered, wrong-key, foreign-account) — all of which mean "not
/// linked" and fail closed into the 428 connect-flow at the policy layer.
pub(crate) fn open_link(
    oauth: &GithubOAuth,
    headers: &HeaderMap,
    account_id: Uuid,
) -> Option<OpenedLink> {
    open_token_cookie(oauth, headers, account_id).map(|cookie| OpenedLink {
        expired: cookie.expired(),
        github_login: cookie.github_login,
        access_token: cookie.access_token,
    })
}

/// Extract a cookie value from the request's `Cookie` header(s).
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    for header in headers.get_all(axum::http::header::COOKIE) {
        let Ok(s) = header.to_str() else { continue };
        for pair in s.split(';') {
            if let Some(value) = pair
                .trim()
                .strip_prefix(name)
                .and_then(|v| v.strip_prefix('='))
            {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// `Set-Cookie` storing the sealed token. Session-scoped on purpose (no
/// `Max-Age`/`Expires`): the link lives exactly as long as the browser
/// session, and nothing outlives it anywhere.
fn token_cookie_header(sealed: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{TOKEN_COOKIE}={sealed}; Path=/; HttpOnly; SameSite=Lax{secure}")
}

/// `Set-Cookie` deleting the token cookie (disconnect).
fn clear_token_cookie_header(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{TOKEN_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

/// Append a `Set-Cookie` header to a response.
fn append_set_cookie(response: &mut Response, header: &str) {
    match HeaderValue::from_str(header) {
        Ok(value) => {
            response
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, value);
        }
        // Unreachable in practice (the value is base64url + fixed attributes),
        // but never fail the response over a cookie.
        Err(e) => tracing::error!(error = %e, "failed to build Set-Cookie header"),
    }
}

/// True when `base_url` is an http (not https) origin pointing at the local
/// machine. Used to drop the `Secure` flag on the session cookie (`main.rs`)
/// and the token cookie (here) for local SSO dev (`just editor-sso` over
/// http://localhost) so Safari — which, unlike Chrome and Firefox, refuses
/// Secure cookies over http://localhost — completes the OIDC handshake.
/// Production always serves over an https BASE_URL, so this is false there and
/// cookies stay Secure. A missing or unparseable BASE_URL is treated as
/// non-local (Secure stays on) — the safe default.
///
/// Parses with `url::Url` (the same crate that validates `BASE_URL` at startup)
/// so the scheme/host extraction matches WHATWG rules: the host is exact (a
/// look-alike like `http://localhost.attacker.example` or userinfo like
/// `http://localhost@evil.com` resolves to a non-loopback host and is rejected)
/// and IPv6 loopback is handled via the typed `Host` enum. Exhaustive tests
/// live in `main.rs` (`http_localhost_tests`).
pub(crate) fn is_http_localhost(base_url: Option<&str>) -> bool {
    let Some(url) = base_url.and_then(|u| url::Url::parse(u).ok()) else {
        return false;
    };
    if url.scheme() != "http" {
        return false;
    }
    matches!(
        url.host(),
        Some(url::Host::Domain("localhost"))
            | Some(url::Host::Ipv4(std::net::Ipv4Addr::LOCALHOST))
            | Some(url::Host::Ipv6(std::net::Ipv6Addr::LOCALHOST))
    )
}

/// Derive the externally-visible base URL, preferring configured `BASE_URL`
/// and falling back to forwarded/host headers (same approach as the OIDC flow).
fn base_url_from_config_or_request(state: &AppState, headers: &HeaderMap) -> String {
    if let Some(base) = state.config.base_url.as_deref() {
        return base.to_string();
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

/// Validate a return path is a safe relative URL (mirrors the OIDC guard).
fn validate_return_url(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() || url == "/" {
        return None;
    }
    if !url.starts_with('/')
        || url.starts_with("//")
        || url.contains('\\')
        || url.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return None;
    }
    Some(url.to_string())
}

/// Append a `github=<marker>` query flag on the bounce-back so a consumer *can*
/// react (e.g. a toast). The SPA re-fetches `/auth/github/status` on load, so the
/// connected/expired state is already reflected without reading this flag; wiring
/// a toast off it is a spike follow-up.
///
/// `validate_return_url` permits a `#fragment` (the frontend includes
/// `window.location.hash`), so split it off and insert the marker into the
/// query *before* the fragment — otherwise `?github=…` lands inside the
/// fragment and is invisible to query-string parsers.
fn with_marker(base_url: &str, path: &str, marker: &str) -> String {
    let (path, fragment) = match path.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (path, None),
    };
    let sep = if path.contains('?') { '&' } else { '?' };
    match fragment {
        Some(f) => format!("{base_url}{path}{sep}github={marker}#{f}"),
        None => format!("{base_url}{path}{sep}github={marker}"),
    }
}

#[derive(Deserialize)]
pub struct LoginQuery {
    pub return_url: Option<String>,
}

/// `GET /auth/github/login` — start the Authorization Code flow.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Query(params): Query<LoginQuery>,
) -> Result<Response, StatusCode> {
    let oauth = state
        .config
        .github_oauth
        .as_ref()
        .ok_or(StatusCode::NOT_IMPLEMENTED)?;

    let base_url = base_url_from_config_or_request(&state, &headers);
    // Defense in depth: in relay mode `base_url` becomes the origin the relay
    // will bounce a `code` back to, and (absent a configured BASE_URL) it's
    // derived from the `X-Forwarded-Host`/`Host` request headers. The relay
    // already refuses a non-allowlisted origin, but reject it here too so a
    // spoofed/misconfigured host fails immediately at login rather than after a
    // GitHub round-trip. Only meaningful in relay mode (where an allowlist is
    // guaranteed non-empty by `from_env`).
    if oauth.callback_base.is_some() && !origin_allowed(oauth, &base_url) {
        tracing::warn!(base_url = %base_url, "github login: origin not allowlisted (relay mode)");
        return Err(StatusCode::BAD_REQUEST);
    }
    // The `redirect_uri` GitHub validates: the fixed relay host in relay mode,
    // else this deployment's own callback. The browser ultimately lands back on
    // `base_url` (directly, or via the relay's 302).
    let redirect_uri = oauth.redirect_uri_for(&base_url);
    // 122-bit random, single-use CSRF token. The `state` we hand GitHub carries
    // this token *and* our origin, so the relay knows where to bounce back to.
    let csrf = Uuid::new_v4().simple().to_string();
    let state_param = encode_state(&csrf, &base_url);

    let mut authorize = url::Url::parse(&format!("{}/login/oauth/authorize", oauth.github_base))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    authorize
        .query_pairs_mut()
        .append_pair("client_id", &oauth.client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", &oauth.scopes)
        .append_pair("state", &state_param)
        .append_pair("allow_signup", "false");

    session_insert(&session, SESSION_KEY_CSRF, csrf).await?;
    session_insert(&session, SESSION_KEY_REDIRECT_URI, redirect_uri).await?;
    session_insert(&session, SESSION_KEY_BASE_URL, base_url).await?;
    if let Some(return_url) = validate_return_url(params.return_url.as_deref()) {
        session_insert(&session, SESSION_KEY_RETURN, return_url).await?;
    }

    Ok(Redirect::temporary(authorize.as_str()).into_response())
}

async fn session_insert(session: &Session, key: &str, value: String) -> Result<(), StatusCode> {
    session.insert(key, value).await.map_err(|e| {
        tracing::error!(key, error = %e, "failed to insert into session");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// `GET /auth/github/callback` — verify state, exchange code, store the token.
pub async fn callback(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Query(params): Query<CallbackQuery>,
) -> Result<Response, StatusCode> {
    let oauth = state
        .config
        .github_oauth
        .as_ref()
        .ok_or(StatusCode::NOT_IMPLEMENTED)?;

    // Base URL + return path were stored at /login; use them for the bounce
    // back regardless of how the request arrived.
    let base_url: String = session
        .get(SESSION_KEY_BASE_URL)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| state.config.base_url.clone().unwrap_or_default());
    let return_path: String = session
        .get::<String>(SESSION_KEY_RETURN)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "/".to_string());
    let _ = session.remove::<String>(SESSION_KEY_RETURN).await;

    let stored_csrf: Option<String> = session.get(SESSION_KEY_CSRF).await.ok().flatten();
    let _ = session.remove::<String>(SESSION_KEY_CSRF).await;
    // The exact redirect_uri sent at /login — GitHub requires the exchange to
    // present the identical value (the relay host in relay mode).
    let stored_redirect_uri: Option<String> =
        session.get(SESSION_KEY_REDIRECT_URI).await.ok().flatten();
    let _ = session.remove::<String>(SESSION_KEY_REDIRECT_URI).await;

    // GitHub reported a consent error (e.g. user clicked "Cancel").
    if let Some(err) = params.error.as_deref() {
        tracing::warn!(
            error = err,
            description = params.error_description.as_deref().unwrap_or("unknown"),
            "GitHub OAuth consent returned an error"
        );
        return Ok(
            Redirect::temporary(&with_marker(&base_url, &return_path, "denied")).into_response(),
        );
    }

    let (code, req_state, stored_csrf, redirect_uri) =
        match (params.code, params.state, stored_csrf, stored_redirect_uri) {
            (Some(code), Some(req_state), Some(stored_csrf), Some(redirect_uri)) => {
                (code, req_state, stored_csrf, redirect_uri)
            }
            _ => {
                tracing::warn!("GitHub OAuth callback missing code/state or session expired");
                return Ok(
                    Redirect::temporary(&with_marker(&base_url, &return_path, "error"))
                        .into_response(),
                );
            }
        };

    // The `state` carries {csrf, origin}; only the csrf is security-relevant
    // here (constant-time compare against the session). A malformed state that
    // doesn't decode is treated as a mismatch.
    let req_csrf = decode_state(&req_state).map(|(c, _)| c).unwrap_or_default();
    let matches: bool = req_csrf.as_bytes().ct_eq(stored_csrf.as_bytes()).into();
    if !matches || req_csrf.is_empty() {
        tracing::warn!("GitHub OAuth state mismatch");
        return Ok(
            Redirect::temporary(&with_marker(&base_url, &return_path, "error")).into_response(),
        );
    }

    let token = match exchange_code(&state.http_client, oauth, &code, &redirect_uri).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "GitHub OAuth code exchange failed");
            return Ok(
                Redirect::temporary(&with_marker(&base_url, &return_path, "error")).into_response(),
            );
        }
    };

    let login = match fetch_login(&state.http_client, oauth, &token.access_token).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch GitHub user after token exchange");
            return Ok(
                Redirect::temporary(&with_marker(&base_url, &return_path, "error")).into_response(),
            );
        }
    };

    // Seal the token into the browser cookie — the editor persists nothing
    // server-side. Bound to the acting editor account so the cookie is inert
    // under any other account's session.
    let payload = UserTokenCookie {
        account: account.id.to_string(),
        access_token: token.access_token,
        github_login: login.clone(),
        scopes: token.scope,
        expires_at: token
            .expires_in
            .map(|secs| now_unix().saturating_add(secs.max(0) as u64)),
    };
    let sealed = match seal_token_cookie(oauth, &payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to seal GitHub token cookie");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    tracing::info!(github_login = %login, "linked GitHub account for editor user");
    let mut response =
        Redirect::temporary(&with_marker(&base_url, &return_path, "connected")).into_response();
    // `Secure` comes from static config ONLY — never from the header-derived
    // `base_url` above (fine for the redirect target, not for a security
    // attribute): with BASE_URL unset a client-supplied
    // `X-Forwarded-Host: localhost` + `X-Forwarded-Proto: http` would strip
    // `Secure` from the sealed-token cookie. Same decision as the session
    // cookie in `main.rs`: `None` = non-local = Secure stays on.
    let secure = !is_http_localhost(state.config.base_url.as_deref());
    append_set_cookie(&mut response, &token_cookie_header(&sealed, secure));
    Ok(response)
}

/// `GET /auth/github/relay` — **public** forwarder for relay mode.
///
/// GitHub validates one fixed callback host (this relay). We decode the origin
/// deployment from `state`, check it against the allowlist, and 302 the browser
/// on to that deployment's real `/auth/github/callback` with GitHub's params
/// intact. The forwarded `code` is single-use and only exchangeable with the
/// App's `client_secret` (held by the origin), and the origin must be
/// allowlisted — so this never becomes an open redirect or code-leak.
pub async fn relay(
    State(state): State<AppState>,
    Query(params): Query<CallbackQuery>,
) -> Result<Response, StatusCode> {
    let oauth = state
        .config
        .github_oauth
        .as_ref()
        .ok_or(StatusCode::NOT_IMPLEMENTED)?;

    let Some(req_state) = params.state.as_deref() else {
        tracing::warn!("github relay: missing state");
        return Err(StatusCode::BAD_REQUEST);
    };
    let Some((_csrf, origin)) = decode_state(req_state) else {
        tracing::warn!("github relay: undecodable state");
        return Err(StatusCode::BAD_REQUEST);
    };
    if !origin_allowed(oauth, &origin) {
        tracing::warn!(origin = %origin, "github relay: origin not allowlisted — refusing");
        return Err(StatusCode::BAD_REQUEST);
    }

    let Ok(mut target) = url::Url::parse(&origin) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    target.set_path("/auth/github/callback");
    {
        let mut qp = target.query_pairs_mut();
        if let Some(code) = params.code.as_deref() {
            qp.append_pair("code", code);
        }
        qp.append_pair("state", req_state);
        if let Some(err) = params.error.as_deref() {
            qp.append_pair("error", err);
        }
        if let Some(desc) = params.error_description.as_deref() {
            qp.append_pair("error_description", desc);
        }
    }

    Ok(Redirect::temporary(target.as_str()).into_response())
}

#[derive(Serialize)]
pub struct GithubStatus {
    pub connected: bool,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<String>,
    /// `true` when a linked token exists but has expired (reconnect needed).
    pub expired: bool,
    /// Whether writes require a linked user token in this deployment.
    pub required: bool,
}

/// `GET /auth/github/status` — non-secret link state for the frontend, read
/// from the sealed cookie on this very request (nothing is stored elsewhere).
pub async fn status(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    headers: HeaderMap,
) -> Result<Json<GithubStatus>, StatusCode> {
    let Some(oauth) = state.config.github_oauth.as_ref() else {
        return Ok(Json(GithubStatus {
            connected: false,
            configured: false,
            github_login: None,
            scopes: None,
            expired: false,
            required: false,
        }));
    };
    // Effective requiredness (env var OR feature flag), so the frontend's
    // `required` mirrors what the write path will actually enforce.
    let required = crate::credentials::write_requires_user_token(&state, oauth)
        .await
        .map_err(|(code, _)| code)?;
    Ok(Json(match open_token_cookie(oauth, &headers, account.id) {
        Some(link) => GithubStatus {
            connected: true,
            configured: true,
            expired: link.expired(),
            github_login: Some(link.github_login),
            scopes: Some(link.scopes),
            required,
        },
        None => GithubStatus {
            connected: false,
            configured: true,
            github_login: None,
            scopes: None,
            expired: false,
            required,
        },
    }))
}

/// `POST /auth/github/disconnect` — best-effort revoke at GitHub, then clear
/// the cookie (our only copy of the token rides on this request).
pub async fn disconnect(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let Some(oauth) = state.config.github_oauth.as_ref() else {
        return Err(StatusCode::NOT_IMPLEMENTED);
    };

    // Best-effort: revoke at GitHub so the token dies immediately. A revoke
    // failure (e.g. token already gone) must not block the disconnect —
    // clearing the cookie is what the user asked for.
    if let Some(link) = open_token_cookie(oauth, &headers, account.id)
        && let Err(e) = revoke(&state.http_client, oauth, &link.access_token).await
    {
        tracing::warn!(error = %e, "GitHub token revoke failed; clearing cookie anyway");
    }

    // Static config only for `Secure` — see the matching comment in
    // `callback` (spoofable forwarded headers must not downgrade the cookie).
    let secure = !is_http_localhost(state.config.base_url.as_deref());
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_set_cookie(&mut response, &clear_token_cookie_header(secure));
    Ok(response)
}

// --- GitHub HTTP calls -----------------------------------------------------

/// Minimal shape we consume from GitHub's token endpoint. No `refresh_token`:
/// the classic OAuth `repo` tokens this spike uses don't expire, and a refresh
/// flow would anyway have to live in the cookie alongside the access token.
struct ExchangedToken {
    access_token: String,
    expires_in: Option<i64>,
    scope: String,
}

#[derive(Deserialize)]
struct TokenEndpointResponse {
    access_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Exchange an authorization `code` for a user access token.
async fn exchange_code(
    client: &reqwest::Client,
    oauth: &GithubOAuth,
    code: &str,
    redirect_uri: &str,
) -> Result<ExchangedToken, String> {
    let resp = client
        .post(format!("{}/login/oauth/access_token", oauth.github_base))
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("client_id", oauth.client_id.as_str()),
            ("client_secret", oauth.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| format!("token endpoint request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("token endpoint returned HTTP {}", resp.status()));
    }
    let body: TokenEndpointResponse = resp
        .json()
        .await
        .map_err(|e| format!("token endpoint returned invalid JSON: {e}"))?;

    if let Some(err) = body.error {
        return Err(format!(
            "token endpoint error: {err} ({})",
            body.error_description.unwrap_or_default()
        ));
    }
    let access_token = body
        .access_token
        .ok_or_else(|| "token endpoint returned no access_token".to_string())?;
    Ok(ExchangedToken {
        access_token,
        expires_in: body.expires_in,
        scope: body.scope.unwrap_or_default(),
    })
}

#[derive(Deserialize)]
struct GithubUser {
    login: String,
}

/// Fetch the authenticated user's login for the given token.
async fn fetch_login(
    client: &reqwest::Client,
    oauth: &GithubOAuth,
    access_token: &str,
) -> Result<String, String> {
    let resp = client
        .get(format!("{}/user", oauth.api_base))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {access_token}"),
        )
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, "regelrecht-editor")
        .send()
        .await
        .map_err(|e| format!("/user request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("/user returned HTTP {}", resp.status()));
    }
    let user: GithubUser = resp
        .json()
        .await
        .map_err(|e| format!("/user returned invalid JSON: {e}"))?;
    Ok(user.login)
}

/// Revoke a user token (delete the app grant) — best effort on disconnect.
async fn revoke(
    client: &reqwest::Client,
    oauth: &GithubOAuth,
    access_token: &str,
) -> Result<(), String> {
    let resp = client
        .delete(format!(
            "{}/applications/{}/grant",
            oauth.api_base, oauth.client_id
        ))
        .basic_auth(&oauth.client_id, Some(&oauth.client_secret))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, "regelrecht-editor")
        .json(&serde_json::json!({ "access_token": access_token }))
        .send()
        .await
        .map_err(|e| format!("revoke request failed: {e}"))?;
    // 204 = revoked, 404 = already gone; both are fine.
    if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
        Ok(())
    } else {
        Err(format!("revoke returned HTTP {}", resp.status()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn return_url_rejects_absolute_and_control() {
        assert_eq!(validate_return_url(Some("https://evil.com")), None);
        assert_eq!(validate_return_url(Some("//evil.com")), None);
        assert_eq!(
            validate_return_url(Some("/editor/abc")),
            Some("/editor/abc".to_string())
        );
        assert_eq!(validate_return_url(Some("/lib\r\nX")), None);
        assert_eq!(validate_return_url(Some("/")), None);
    }

    #[test]
    fn with_marker_picks_separator() {
        assert_eq!(
            with_marker("https://h", "/", "connected"),
            "https://h/?github=connected"
        );
        assert_eq!(
            with_marker("https://h", "/editor/x?tab=y", "error"),
            "https://h/editor/x?tab=y&github=error"
        );
    }

    #[test]
    fn with_marker_inserts_before_fragment() {
        // Fragment present: marker goes into the query, before the '#'.
        assert_eq!(
            with_marker("https://h", "/editor/x#section", "connected"),
            "https://h/editor/x?github=connected#section"
        );
        // Existing query + fragment: marker appended with '&', still before '#'.
        assert_eq!(
            with_marker("https://h", "/editor/x?tab=y#section", "denied"),
            "https://h/editor/x?tab=y&github=denied#section"
        );
    }

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_oauth(github_base: &str, api_base: &str) -> GithubOAuth {
        let cipher =
            TokenCipher::from_base64_key(&STANDARD.encode([9u8; 32])).expect("valid test key");
        GithubOAuth {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            scopes: "repo".to_string(),
            cipher: Arc::new(cipher),
            require_user_token: false,
            callback_base: None,
            allowed_origin_suffixes: Vec::new(),
            github_base: github_base.to_string(),
            api_base: api_base.to_string(),
        }
    }

    fn headers_with_cookie(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("other=x; {TOKEN_COOKIE}={value}; theme=dark"))
                .expect("valid header"),
        );
        headers
    }

    fn test_payload(account: Uuid) -> UserTokenCookie {
        UserTokenCookie {
            account: account.to_string(),
            access_token: "gho_exampletoken".to_string(),
            github_login: "octocat".to_string(),
            scopes: "repo".to_string(),
            expires_at: None,
        }
    }

    #[test]
    fn token_cookie_roundtrip() {
        let oauth = test_oauth("https://gh", "https://api");
        let account = Uuid::new_v4();
        let sealed = seal_token_cookie(&oauth, &test_payload(account)).expect("seals");
        // The cookie value must be opaque: no token material in the clear.
        assert!(!sealed.contains("gho_exampletoken"));

        let opened = open_token_cookie(&oauth, &headers_with_cookie(&sealed), account)
            .expect("opens for the linking account");
        assert_eq!(opened.access_token, "gho_exampletoken");
        assert_eq!(opened.github_login, "octocat");
        assert!(!opened.expired());
    }

    #[test]
    fn token_cookie_bound_to_account() {
        let oauth = test_oauth("https://gh", "https://api");
        let sealed = seal_token_cookie(&oauth, &test_payload(Uuid::new_v4())).expect("seals");
        // Same sealed cookie under a different editor account: not linked.
        assert!(open_token_cookie(&oauth, &headers_with_cookie(&sealed), Uuid::new_v4()).is_none());
    }

    #[test]
    fn token_cookie_rejects_garbage_and_absence() {
        let oauth = test_oauth("https://gh", "https://api");
        let account = Uuid::new_v4();
        // Tampered/garbage value → not linked, never an error.
        assert!(open_token_cookie(&oauth, &headers_with_cookie("AAAAaaaa"), account).is_none());
        // No cookie header at all.
        assert!(open_token_cookie(&oauth, &HeaderMap::new(), account).is_none());
    }

    #[test]
    fn token_cookie_expiry() {
        let mut payload = test_payload(Uuid::new_v4());
        assert!(!payload.expired(), "no provider expiry = never expires");
        payload.expires_at = Some(now_unix() - 10);
        assert!(payload.expired());
        payload.expires_at = Some(now_unix() + 3600);
        assert!(!payload.expired());
    }

    #[test]
    fn cookie_headers_have_expected_attributes() {
        let set = token_cookie_header("abc", true);
        assert!(set.contains("HttpOnly") && set.contains("Secure"));
        assert!(
            !set.contains("Max-Age") && !set.contains("Expires"),
            "token cookie must be session-scoped"
        );
        // Local http dev: Secure dropped, mirroring the session cookie.
        assert!(!token_cookie_header("abc", false).contains("Secure"));
        assert!(clear_token_cookie_header(true).contains("Max-Age=0"));
    }

    #[test]
    fn state_roundtrip() {
        let s = encode_state("csrf-123", "https://editor-pr9.example.rijksapps.nl");
        let (csrf, origin) = decode_state(&s).expect("decodes");
        assert_eq!(csrf, "csrf-123");
        assert_eq!(origin, "https://editor-pr9.example.rijksapps.nl");
        assert!(decode_state("not-base64!!").is_none());
    }

    #[test]
    fn redirect_uri_relay_vs_self() {
        let mut o = test_oauth("https://gh", "https://api");
        // Self-callback when no relay configured.
        assert_eq!(
            o.redirect_uri_for("https://editor-pr9.rijksapps.nl"),
            "https://editor-pr9.rijksapps.nl/auth/github/callback"
        );
        // Fixed relay host in relay mode.
        o.callback_base = Some("https://editor.regelrecht.rijks.app".to_string());
        assert_eq!(
            o.redirect_uri_for("https://editor-pr9.rijksapps.nl"),
            "https://editor.regelrecht.rijks.app/auth/github/relay"
        );
    }

    #[test]
    fn origin_allowlist_enforced() {
        let mut o = test_oauth("https://gh", "https://api");
        o.allowed_origin_suffixes = vec![
            "rijksapps.nl".to_string(),
            "editor.regelrecht.rijks.app".to_string(),
        ];
        // subdomain of an allowed suffix
        assert!(origin_allowed(
            &o,
            "https://editor-pr887-regel-k4c.rig.quattro.rijksapps.nl"
        ));
        // exact allowed host
        assert!(origin_allowed(&o, "https://editor.regelrecht.rijks.app"));
        // not allowlisted
        assert!(!origin_allowed(&o, "https://evil.com"));
        // suffix must be a dot-boundary, not a substring
        assert!(!origin_allowed(&o, "https://notrijksapps.nl"));
        // http is refused (relay is https-only)
        assert!(!origin_allowed(&o, "http://editor.regelrecht.rijks.app"));
        // garbage
        assert!(!origin_allowed(&o, "not a url"));
    }

    #[tokio::test]
    async fn exchange_code_parses_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "gho_exampletoken",
                "token_type": "bearer",
                "scope": "repo,read:org"
            })))
            .mount(&server)
            .await;

        let oauth = test_oauth(&server.uri(), &server.uri());
        let client = regelrecht_auth::http_client();
        let token = exchange_code(
            &client,
            &oauth,
            "code-123",
            "https://app/auth/github/callback",
        )
        .await
        .expect("exchange should succeed");
        assert_eq!(token.access_token, "gho_exampletoken");
        assert_eq!(token.scope, "repo,read:org");
        assert!(token.expires_in.is_none());
    }

    #[tokio::test]
    async fn exchange_code_surfaces_provider_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "bad_verification_code",
                "error_description": "The code passed is incorrect or expired."
            })))
            .mount(&server)
            .await;

        let oauth = test_oauth(&server.uri(), &server.uri());
        let client = regelrecht_auth::http_client();
        let err = match exchange_code(&client, &oauth, "bad", "https://app/cb").await {
            Ok(_) => panic!("provider error must propagate"),
            Err(e) => e,
        };
        assert!(err.contains("bad_verification_code"), "got: {err}");
    }

    #[tokio::test]
    async fn fetch_login_reads_handle() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", "Bearer gho_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "login": "octocat" })),
            )
            .mount(&server)
            .await;

        let oauth = test_oauth(&server.uri(), &server.uri());
        let client = regelrecht_auth::http_client();
        let login = fetch_login(&client, &oauth, "gho_token")
            .await
            .expect("user fetch should succeed");
        assert_eq!(login, "octocat");
    }
}
