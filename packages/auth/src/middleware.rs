use std::future::Future;
use std::pin::Pin;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use openidconnect::core::CoreErrorResponseType;
use openidconnect::{OAuth2TokenResponse, RefreshToken, RequestTokenError, StandardErrorResponse};
use tower_sessions::Session;

use crate::OidcAppState;
use crate::handlers::{
    SESSION_KEY_AUTHENTICATED, SESSION_KEY_REFRESH_TOKEN, SESSION_KEY_ROLES, SESSION_KEY_SUB,
    SESSION_KEY_TOKEN_EXPIRES_AT, access_token_ttl_secs, extract_realm_roles, unix_now,
};

type RequireRoleFuture = Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>;

/// Outcome of a session-based role check. Independent of any HTTP error type
/// so that callers in different crates can map this to their own error
/// (`StatusCode` here, `ApiError` in the admin crate).
pub enum RoleCheck {
    /// Session is authenticated and carries the required role.
    Allowed,
    /// Session has no authenticated marker (or it's false).
    NotAuthenticated,
    /// Session is authenticated but lacks the required role. `sub` is the
    /// Keycloak subject if present, included so callers can log it.
    MissingRole { sub: Option<String> },
}

/// Inspect a session and decide whether it satisfies the role requirement.
///
/// Returns [`RoleCheck::Allowed`] when the session holds an authenticated
/// marker *and* `SESSION_KEY_ROLES` contains `required_role`. The role list
/// is compared verbatim against the realm roles stored at login — Keycloak
/// composite expansion means a higher role's token automatically carries the
/// lower roles, so callers never need to check "role A or role B".
///
/// This helper is shared between [`require_role`] (returns 401/403 status
/// codes) and the admin crate's `require_auth` (which wraps the API-key
/// bypass around the session path). It does **not** consider whether auth
/// is enabled on the application — callers are expected to early-return
/// before reaching this when their `is_auth_enabled()` is false.
pub async fn check_session_role(session: &Session, required_role: &str) -> RoleCheck {
    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !authenticated {
        return RoleCheck::NotAuthenticated;
    }

    // Distinguish "no roles key in session at all" (pre-RBAC session from
    // before this code shipped) from "key is present but empty" (legitimate
    // misconfiguration — Keycloak issued an empty `realm_access.roles`).
    //
    // The former must return `NotAuthenticated` so the caller maps to 401 and
    // triggers the OIDC re-login flow — that login repopulates the roles key
    // from the JWT and the session self-heals. Returning 403 here would leave
    // pre-existing sessions stuck (403 does not redirect to login), so during
    // a rolling deploy of this code every logged-in user would see "forbidden"
    // on routes they actually have permission to reach.
    //
    // An explicitly empty list still falls through to `MissingRole`, which is
    // the correct response for a user whose Keycloak roles are genuinely empty.
    let roles: Option<Vec<String>> = session.get(SESSION_KEY_ROLES).await.ok().flatten();
    match roles {
        None => RoleCheck::NotAuthenticated,
        Some(roles) => {
            if roles.iter().any(|r| r == required_role) {
                RoleCheck::Allowed
            } else {
                let sub: Option<String> = session.get(SESSION_KEY_SUB).await.ok().flatten();
                RoleCheck::MissingRole { sub }
            }
        }
    }
}

/// Session-based authentication middleware.
///
/// Passes through when auth is disabled. Returns 401 when the session
/// is not authenticated. Does NOT handle API key authentication — the
/// admin dashboard wraps this with its own API key check.
pub async fn require_session_auth<S: OidcAppState>(
    State(state): State<S>,
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.is_auth_enabled() {
        return Ok(next.run(request).await);
    }

    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);

    if authenticated {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Route-level RBAC gate.
///
/// Returns a middleware function that allows the request only when the
/// session holds `role` (compared verbatim against the realm roles stored
/// at login). Role hierarchy lives in Keycloak as composite roles, so a
/// higher role (e.g. `editor-admin`) automatically carries the lower roles
/// in its token — code never needs to check "role A or role B".
///
/// Behaviour:
/// - auth disabled → passthrough (dev/test convenience).
/// - not authenticated → 401.
/// - authenticated but lacks the role → 403.
pub fn require_role<S: OidcAppState>(
    role: &'static str,
) -> impl Fn(State<S>, Session, Request, Next) -> RequireRoleFuture + Clone + Send + Sync + 'static
{
    move |State(state): State<S>, session: Session, request: Request, next: Next| {
        Box::pin(async move {
            if !state.is_auth_enabled() {
                return Ok(next.run(request).await);
            }

            match check_session_role(&session, role).await {
                RoleCheck::Allowed => Ok(next.run(request).await),
                RoleCheck::NotAuthenticated => Err(StatusCode::UNAUTHORIZED),
                RoleCheck::MissingRole { sub } => {
                    tracing::warn!(
                        required = %role,
                        sub = ?sub,
                        "user lacks required role for route"
                    );
                    Err(StatusCode::FORBIDDEN)
                }
            }
        })
    }
}

/// Renew the access token when it has at most this many seconds of life left.
/// The window absorbs clock skew and the refresh round-trip so re-validation
/// happens just before, not after, expiry.
const TOKEN_REFRESH_SKEW_SECS: i64 = 60;

/// Backoff applied to the recorded expiry after a *transient* refresh failure
/// (IdP unreachable), so we retry on a later request instead of hammering the
/// token endpoint on every request while the IdP is down.
const TOKEN_REFRESH_RETRY_BACKOFF_SECS: i64 = 30;

/// Pure refresh decision: true when an expiry is recorded and it is within the
/// skew window (or already past). A missing expiry means the session predates
/// this feature (or the IdP sent no `expires_in`) — never refresh then.
fn should_refresh(expires_at: Option<i64>, now: i64) -> bool {
    match expires_at {
        Some(expires_at) => expires_at - now <= TOKEN_REFRESH_SKEW_SECS,
        None => false,
    }
}

/// Transparently re-validate the OIDC session with the IdP.
///
/// Runs as a global layer *inside* the session layer (so the session is loaded)
/// and *outside* the per-route role gates. For an authenticated session whose
/// access token is within [`TOKEN_REFRESH_SKEW_SECS`] of expiry it uses the
/// stored refresh token to obtain a fresh access token, then updates the stored
/// expiry, the (possibly rotated) refresh token, and the realm roles — so role
/// changes at the IdP propagate without a re-login.
///
/// If the IdP *definitively* rejects the refresh token (e.g. the user was
/// logged out or disabled at Keycloak → `invalid_grant`), the authenticated
/// marker is dropped so the downstream gate returns 401 and the client
/// re-logs in. Transient failures (IdP unreachable) leave the session intact
/// and only nudge the retry window forward.
///
/// Concurrency: deliberately lock-free. With Keycloak's default "Revoke Refresh
/// Token" = off a refresh token is reusable until expiry, so concurrent
/// refreshes in the skew window are harmless. If rotation is enabled, a rare
/// concurrent double-use can fail one request's refresh; that self-heals via
/// the normal re-login redirect (silent while the IdP SSO session is alive).
/// Serializing per session would pull a `tokio` + `dashmap` runtime dependency
/// into this intentionally dependency-light shared crate — not worth it for
/// that edge case.
pub async fn refresh_session_token<S: OidcAppState>(
    State(state): State<S>,
    session: Session,
    request: Request,
    next: Next,
) -> Response {
    if state.is_auth_enabled() {
        try_refresh_token(&state, &session).await;
    }
    next.run(request).await
}

async fn try_refresh_token<S: OidcAppState>(state: &S, session: &Session) {
    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !authenticated {
        return;
    }

    let expires_at = session
        .get::<i64>(SESSION_KEY_TOKEN_EXPIRES_AT)
        .await
        .ok()
        .flatten();
    if !should_refresh(expires_at, unix_now()) {
        return;
    }

    // Need both an OIDC client and a stored refresh token to re-validate.
    let Some(client) = state.oidc_client() else {
        return;
    };
    let Some(refresh_token) = session
        .get::<String>(SESSION_KEY_REFRESH_TOKEN)
        .await
        .ok()
        .flatten()
    else {
        return;
    };

    let outcome = match client
        .exchange_refresh_token(&RefreshToken::new(refresh_token))
        .request_async(state.http_client())
        .await
    {
        Ok(token_response) => RefreshOutcome::Renewed {
            expires_at: unix_now() + access_token_ttl_secs(token_response.expires_in()),
            // Keycloak rotates the refresh token when rotation is enabled; carry
            // whatever it returned so the next refresh uses the current one.
            refresh_token: token_response.refresh_token().map(|t| t.secret().clone()),
            // Roles come from the access token (Keycloak includes `realm_access`
            // there by default). `None` (parse failure) leaves existing roles.
            roles: extract_realm_roles(token_response.access_token().secret()),
        },
        Err(e) => outcome_for_error(&e),
    };
    apply_refresh_outcome(session, outcome).await;
}

/// Result of a refresh attempt, decoupled from the oauth2/HTTP types so the
/// session-mutation logic can be unit-tested without a live IdP.
#[derive(Debug)]
enum RefreshOutcome {
    /// Fresh token material to persist (expiry, possibly-rotated refresh token,
    /// optionally re-extracted roles).
    Renewed {
        expires_at: i64,
        refresh_token: Option<String>,
        roles: Option<Vec<String>>,
    },
    /// IdP definitively rejected the refresh token → invalidate the session.
    Rejected,
    /// Transient failure (IdP unreachable / unparseable) → keep the session and
    /// back off so we retry on a later request instead of hammering the IdP.
    Transient,
}

/// Classify a token-endpoint failure.
///
/// Only `invalid_grant` is definitive: it means *this refresh token* is
/// expired/revoked (the user was logged out or disabled at the IdP), the one
/// case a re-login fixes → [`RefreshOutcome::Rejected`]. Everything else stays
/// [`RefreshOutcome::Transient`] (keep the session): `server_error` /
/// `temporarily_unavailable` (RFC 6749 §5.2) are server-side hiccups, and
/// client-config errors (`invalid_client`/`unauthorized_client`) can't be fixed
/// by the user re-logging in — flushing active sessions on any of those would
/// log everyone out on a Keycloak blip. Network/parse errors fall here too.
fn outcome_for_error<RE: std::error::Error + 'static>(
    err: &RequestTokenError<RE, StandardErrorResponse<CoreErrorResponseType>>,
) -> RefreshOutcome {
    match err {
        RequestTokenError::ServerResponse(resp) if resp.error().as_ref() == "invalid_grant" => {
            RefreshOutcome::Rejected
        }
        _ => RefreshOutcome::Transient,
    }
}

/// Apply a [`RefreshOutcome`] to the session.
async fn apply_refresh_outcome(session: &Session, outcome: RefreshOutcome) {
    match outcome {
        RefreshOutcome::Renewed {
            expires_at,
            refresh_token,
            roles,
        } => {
            let _ = session
                .insert(SESSION_KEY_TOKEN_EXPIRES_AT, expires_at)
                .await;
            if let Some(refresh_token) = refresh_token {
                let _ = session
                    .insert(SESSION_KEY_REFRESH_TOKEN, refresh_token)
                    .await;
            }
            if let Some(roles) = roles {
                let _ = session.insert(SESSION_KEY_ROLES, roles).await;
            }
            tracing::debug!("refreshed OIDC access token");
        }
        RefreshOutcome::Rejected => {
            tracing::info!("refresh token rejected by IdP — invalidating session");
            invalidate_session(session).await;
        }
        RefreshOutcome::Transient => {
            tracing::warn!("token refresh failed transiently — will retry");
            // Push the recorded expiry past the skew window so the NEXT refresh
            // only fires after the backoff elapses. Storing just `now + backoff`
            // would stay inside the skew band (backoff < skew) and refresh on
            // every request — defeating the backoff.
            let _ = session
                .insert(
                    SESSION_KEY_TOKEN_EXPIRES_AT,
                    unix_now() + TOKEN_REFRESH_SKEW_SECS + TOKEN_REFRESH_RETRY_BACKOFF_SECS,
                )
                .await;
        }
    }
}

/// Invalidate the session on definitive rejection. `flush()` deletes the whole
/// session record (not just the auth marker), so no personal identifiers
/// (`sub`, name, roles, id_token) linger in the store until natural expiry —
/// data minimisation for a citizen-facing platform. The dropped auth marker
/// makes the downstream gate return 401 → client re-login.
async fn invalidate_session(session: &Session) {
    // A failed flush would leave the session authenticated in the store after a
    // definitive IdP rejection (the opposite of what we want). The current
    // request is still denied — flush clears the in-memory session so the
    // downstream gate sees no auth marker — but a store-delete failure could
    // resurrect the session on a later request, so surface it loudly.
    if let Err(e) = session.flush().await {
        tracing::error!(error = %e, "failed to flush session after token rejection");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::OidcConfig;
    use crate::oidc::ConfiguredClient;
    use axum::Router;
    use axum::body::Body;
    use axum::middleware as axum_middleware;
    use axum::routing::get;
    use std::sync::Arc;
    use tower::ServiceExt;
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_memory_store::MemoryStore;

    #[derive(Clone)]
    struct TestState {
        oidc_config: Option<OidcConfig>,
    }

    impl OidcAppState for TestState {
        fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
            None
        }
        fn end_session_url(&self) -> Option<&str> {
            None
        }
        fn oidc_config(&self) -> Option<&OidcConfig> {
            self.oidc_config.as_ref()
        }
        fn is_auth_enabled(&self) -> bool {
            self.oidc_config.is_some()
        }
        fn base_url(&self) -> Option<&str> {
            None
        }
        fn http_client(&self) -> &reqwest::Client {
            // Not called in middleware tests
            unimplemented!()
        }
    }

    fn test_state(auth_enabled: bool) -> TestState {
        let oidc_config = if auth_enabled {
            Some(OidcConfig {
                client_id: "test".into(),
                client_secret: "test".into(),
                issuer_url: "https://example.com".into(),
                required_role: "user".into(),
            })
        } else {
            None
        };
        TestState { oidc_config }
    }

    fn test_app(state: TestState) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_session_auth::<TestState>,
            ))
            .with_state(state)
            .layer(session_layer)
    }

    #[tokio::test]
    async fn auth_disabled_passes_through() {
        let app = test_app(test_state(false));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unauthenticated_returns_401() {
        let app = test_app(test_state(true));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn authenticated_passes_through() {
        let store = MemoryStore::default();
        let state = test_state(true);
        let session_layer = SessionManagerLayer::new(store);

        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_session_auth::<TestState>,
            ))
            .route(
                "/set-auth",
                get(|session: Session| async move {
                    session
                        .insert(SESSION_KEY_AUTHENTICATED, true)
                        .await
                        .expect("insert");
                    "set"
                }),
            )
            .with_state(state)
            .layer(session_layer);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/set-auth")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);

        let cookie = response
            .headers()
            .get("set-cookie")
            .expect("set-cookie header")
            .to_str()
            .expect("cookie str")
            .to_string();

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- require_role factory ---

    /// Build an app that gates `/test` with `require_role(role)` and exposes
    /// `/seed?roles=a,b,c` (via `test_utils::with_seed_route`) to set up a
    /// fully authenticated session with the given realm roles.
    fn role_test_app(state: TestState, role: &'static str) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        let gated = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_role::<TestState>(role),
            ));

        crate::test_utils::with_seed_route(gated)
            .with_state(state)
            .layer(session_layer)
    }

    use crate::test_utils::seed_session;

    async fn get_test(app: Router, cookie: &str) -> StatusCode {
        app.oneshot(
            axum::http::Request::builder()
                .uri("/test")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response")
        .status()
    }

    #[tokio::test]
    async fn require_role_passthrough_when_auth_disabled() {
        let app = role_test_app(test_state(false), "editor-writer");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_role_unauthenticated_returns_401() {
        let app = role_test_app(test_state(true), "editor-writer");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn reader_can_access_reader_route() {
        let app = role_test_app(test_state(true), "editor-reader");
        let cookie = seed_session(&app, "editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn reader_cannot_access_writer_route() {
        let app = role_test_app(test_state(true), "editor-writer");
        let cookie = seed_session(&app, "editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn writer_can_access_reader_route() {
        // Composite expansion: a writer's token contains the reader role too.
        let app = role_test_app(test_state(true), "editor-reader");
        let cookie = seed_session(&app, "editor-writer,editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn app_admin_can_access_all_routes_within_app() {
        // Editor-admin composite includes writer and reader. Test the
        // top-of-ladder role hits writer, reader, and a specific-right route.
        for required in ["editor-reader", "editor-writer", "editor-publish"] {
            let app = role_test_app(test_state(true), required);
            let cookie = seed_session(
                &app,
                "editor-admin,editor-writer,editor-reader,editor-publish",
            )
            .await;
            assert_eq!(
                get_test(app, &cookie).await,
                StatusCode::OK,
                "editor-admin should access {required}"
            );
        }
    }

    #[tokio::test]
    async fn regelrecht_admin_can_access_all_apps() {
        // Regelrecht-admin's token transitively contains every sub-role across
        // both apps; a check for any leaf role passes.
        for required in [
            "editor-reader",
            "editor-writer",
            "editor-admin",
            "harvester-reader",
            "harvester-writer",
            "harvester-admin",
        ] {
            let app = role_test_app(test_state(true), required);
            let cookie = seed_session(
                &app,
                "regelrecht-admin,editor-admin,editor-writer,editor-reader,harvester-admin,harvester-writer,harvester-reader",
            )
            .await;
            assert_eq!(
                get_test(app, &cookie).await,
                StatusCode::OK,
                "regelrecht-admin should access {required}"
            );
        }
    }

    #[tokio::test]
    async fn writer_without_specific_right_gets_403() {
        // editor-writer should NOT auto-inherit editor-publish — that's an
        // orthogonal right granted explicitly or via editor-admin composite.
        let app = role_test_app(test_state(true), "editor-publish");
        let cookie = seed_session(&app, "editor-writer,editor-reader").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn writer_with_specific_right_passes() {
        let app = role_test_app(test_state(true), "editor-publish");
        let cookie = seed_session(&app, "editor-writer,editor-reader,editor-publish").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn authenticated_with_no_roles_gets_403() {
        // A session that's authenticated but has no roles stored (edge case
        // from a misconfigured Keycloak that issued an empty realm_access)
        // must be denied, not allowed.
        let app = role_test_app(test_state(true), "editor-reader");
        let cookie = seed_session(&app, "").await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn pre_rbac_session_without_roles_key_returns_401() {
        // Simulates a session created BEFORE this PR shipped: the session
        // carries `SESSION_KEY_AUTHENTICATED = true` but the
        // `SESSION_KEY_ROLES` key was never written. The check must report
        // `NotAuthenticated` (→ 401) so the caller's response triggers an
        // OIDC re-login — that login then populates `SESSION_KEY_ROLES` from
        // the JWT and the session self-heals.
        //
        // Returning 403 here (what an `unwrap_or_default()` empty-Vec would
        // produce) would break the user experience during the rolling deploy
        // because 403 does not redirect to the login flow.
        let store = MemoryStore::default();
        let state = test_state(true);
        let session_layer = SessionManagerLayer::new(store);

        let gated = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_role::<TestState>("editor-reader"),
            ));

        // Mount a `/seed-pre-rbac` route that ONLY sets
        // SESSION_KEY_AUTHENTICATED and deliberately omits SESSION_KEY_ROLES.
        let app = gated
            .route(
                "/seed-pre-rbac",
                get(|session: Session| async move {
                    session
                        .insert(SESSION_KEY_AUTHENTICATED, true)
                        .await
                        .expect("insert auth");
                    "seeded"
                }),
            )
            .with_state(state)
            .layer(session_layer);

        // Seed the pre-RBAC session.
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/seed-pre-rbac")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response
            .headers()
            .get("set-cookie")
            .expect("set-cookie header")
            .to_str()
            .expect("cookie str")
            .to_string();

        // The gated request must return 401 (NotAuthenticated), not 403.
        assert_eq!(get_test(app, &cookie).await, StatusCode::UNAUTHORIZED);
    }

    // --- refresh_session_token ---

    #[test]
    fn should_refresh_decisions() {
        let now = 1_000_000;
        // No recorded expiry → never refresh (legacy/no-refresh-token session).
        assert!(!should_refresh(None, now));
        // Comfortably in the future → no refresh.
        assert!(!should_refresh(Some(now + 3600), now));
        // Just outside the skew window → no refresh.
        assert!(!should_refresh(
            Some(now + TOKEN_REFRESH_SKEW_SECS + 1),
            now
        ));
        // Exactly at the skew boundary → refresh.
        assert!(should_refresh(Some(now + TOKEN_REFRESH_SKEW_SECS), now));
        // Already expired → refresh.
        assert!(should_refresh(Some(now - 10), now));
    }

    /// Gate `/test` with `require_session_auth` behind the refresh layer, and
    /// expose `/seed-tokens?exp=<unix>` to seed an authenticated session with a
    /// given access-token expiry. Mirrors the production layer ordering:
    /// refresh runs inside the session layer and outside the auth gate.
    fn refresh_test_app(state: TestState) -> Router {
        let session_layer = SessionManagerLayer::new(MemoryStore::default());

        #[derive(serde::Deserialize)]
        struct ExpQuery {
            exp: i64,
        }

        Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_session_auth::<TestState>,
            ))
            .route(
                "/seed-tokens",
                get(
                    |session: Session, axum::extract::Query(q): axum::extract::Query<ExpQuery>| async move {
                        session
                            .insert(SESSION_KEY_AUTHENTICATED, true)
                            .await
                            .expect("insert auth");
                        session
                            .insert(SESSION_KEY_TOKEN_EXPIRES_AT, q.exp)
                            .await
                            .expect("insert expiry");
                        "seeded"
                    },
                ),
            )
            .layer(axum_middleware::from_fn_with_state(
                state.clone(),
                refresh_session_token::<TestState>,
            ))
            .with_state(state)
            .layer(session_layer)
    }

    async fn seed_tokens(app: &Router, exp: i64) -> String {
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/seed-tokens?exp={exp}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        response
            .headers()
            .get("set-cookie")
            .expect("set-cookie header")
            .to_str()
            .expect("cookie str")
            .to_string()
    }

    #[tokio::test]
    async fn refresh_layer_passes_through_when_token_valid() {
        // Far-future expiry → no refresh attempt; the request reaches the gate
        // with its auth marker intact.
        let app = refresh_test_app(test_state(true));
        let cookie = seed_tokens(&app, unix_now() + 3600).await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn refresh_layer_does_not_invalidate_when_no_client_configured() {
        // Token is within the skew window so a refresh is wanted, but TestState
        // has no OIDC client: the layer must bail without dropping the auth
        // marker (a misconfiguration must not log everyone out). The gate still
        // sees an authenticated session.
        let app = refresh_test_app(test_state(true));
        let cookie = seed_tokens(&app, unix_now() + 5).await;
        assert_eq!(get_test(app, &cookie).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn refresh_layer_passthrough_when_auth_disabled() {
        // Auth disabled → refresh is a no-op and the gate passes everything.
        let app = refresh_test_app(test_state(false));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- apply_refresh_outcome: the three core token-exchange paths ---
    //
    // The network exchange itself is exercised end-to-end in integration; here
    // we pin the session mutations each outcome must produce, since those are
    // the regression-prone bits (wrong session key, missing flush, wrong
    // backoff). A fresh detached Session over a MemoryStore lets us seed state,
    // apply an outcome, and read the result back without a live IdP.

    /// A detached, authenticated session seeded as a logged-in user would be.
    async fn seeded_session() -> Session {
        let session = Session::new(None, Arc::new(MemoryStore::default()), None);
        session
            .insert(SESSION_KEY_AUTHENTICATED, true)
            .await
            .unwrap();
        session.insert(SESSION_KEY_SUB, "sub-123").await.unwrap();
        session
            .insert(SESSION_KEY_REFRESH_TOKEN, "old-rt")
            .await
            .unwrap();
        session
            .insert(SESSION_KEY_TOKEN_EXPIRES_AT, 1_000_i64)
            .await
            .unwrap();
        session
            .insert(SESSION_KEY_ROLES, vec!["editor-reader".to_string()])
            .await
            .unwrap();
        session
    }

    #[tokio::test]
    async fn apply_renewed_updates_expiry_refresh_token_and_roles() {
        let session = seeded_session().await;
        apply_refresh_outcome(
            &session,
            RefreshOutcome::Renewed {
                expires_at: 9_999,
                refresh_token: Some("new-rt".to_string()),
                roles: Some(vec!["editor-writer".to_string()]),
            },
        )
        .await;

        assert_eq!(
            session
                .get::<i64>(SESSION_KEY_TOKEN_EXPIRES_AT)
                .await
                .unwrap(),
            Some(9_999)
        );
        assert_eq!(
            session
                .get::<String>(SESSION_KEY_REFRESH_TOKEN)
                .await
                .unwrap(),
            Some("new-rt".to_string())
        );
        assert_eq!(
            session.get::<Vec<String>>(SESSION_KEY_ROLES).await.unwrap(),
            Some(vec!["editor-writer".to_string()])
        );
        // Still authenticated.
        assert_eq!(
            session
                .get::<bool>(SESSION_KEY_AUTHENTICATED)
                .await
                .unwrap(),
            Some(true)
        );
    }

    #[tokio::test]
    async fn apply_renewed_keeps_existing_roles_when_none_parsed() {
        // A mapper misconfiguration (roles = None) must not wipe existing roles.
        let session = seeded_session().await;
        apply_refresh_outcome(
            &session,
            RefreshOutcome::Renewed {
                expires_at: 9_999,
                refresh_token: None,
                roles: None,
            },
        )
        .await;
        assert_eq!(
            session.get::<Vec<String>>(SESSION_KEY_ROLES).await.unwrap(),
            Some(vec!["editor-reader".to_string()])
        );
        // refresh_token untouched when the IdP returned none.
        assert_eq!(
            session
                .get::<String>(SESSION_KEY_REFRESH_TOKEN)
                .await
                .unwrap(),
            Some("old-rt".to_string())
        );
    }

    #[tokio::test]
    async fn apply_rejected_flushes_session_including_pii() {
        let session = seeded_session().await;
        apply_refresh_outcome(&session, RefreshOutcome::Rejected).await;
        // Auth marker gone → downstream gate returns 401.
        assert_eq!(
            session
                .get::<bool>(SESSION_KEY_AUTHENTICATED)
                .await
                .unwrap(),
            None
        );
        // PII flushed, not left lingering.
        assert_eq!(session.get::<String>(SESSION_KEY_SUB).await.unwrap(), None);
        assert_eq!(
            session.get::<Vec<String>>(SESSION_KEY_ROLES).await.unwrap(),
            None
        );
        assert_eq!(
            session
                .get::<String>(SESSION_KEY_REFRESH_TOKEN)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn apply_transient_keeps_session_and_backs_off() {
        let session = seeded_session().await;
        apply_refresh_outcome(&session, RefreshOutcome::Transient).await;
        // Session stays authenticated.
        assert_eq!(
            session
                .get::<bool>(SESSION_KEY_AUTHENTICATED)
                .await
                .unwrap(),
            Some(true)
        );
        // Expiry bumped past the skew window so we do NOT refresh on the very
        // next request — the backoff must actually space out retries.
        let exp = session
            .get::<i64>(SESSION_KEY_TOKEN_EXPIRES_AT)
            .await
            .unwrap()
            .expect("expiry present");
        assert!(!should_refresh(Some(exp), unix_now()));
        assert!(exp >= unix_now() + TOKEN_REFRESH_SKEW_SECS + TOKEN_REFRESH_RETRY_BACKOFF_SECS - 5);
    }

    #[test]
    fn outcome_for_error_classifies_definitive_vs_transient() {
        use openidconnect::HttpClientError;

        type Err = RequestTokenError<
            HttpClientError<reqwest::Error>,
            StandardErrorResponse<CoreErrorResponseType>,
        >;

        fn server_error(code: CoreErrorResponseType) -> Err {
            RequestTokenError::ServerResponse(StandardErrorResponse::new(code, None, None))
        }

        // Only invalid_grant is definitive → Rejected.
        assert!(matches!(
            outcome_for_error(&server_error(CoreErrorResponseType::InvalidGrant)),
            RefreshOutcome::Rejected
        ));

        // Server-side / transient OAuth errors must NOT log the user out.
        assert!(matches!(
            outcome_for_error(&server_error(CoreErrorResponseType::Extension(
                "server_error".to_string()
            ))),
            RefreshOutcome::Transient
        ));
        assert!(matches!(
            outcome_for_error(&server_error(CoreErrorResponseType::Extension(
                "temporarily_unavailable".to_string()
            ))),
            RefreshOutcome::Transient
        ));
        // Client-config error: re-login can't fix it → keep the session.
        assert!(matches!(
            outcome_for_error(&server_error(CoreErrorResponseType::InvalidClient)),
            RefreshOutcome::Transient
        ));
        // Network/parse failure → transient.
        let transient: Err = RequestTokenError::Other("connection refused".to_string());
        assert!(matches!(
            outcome_for_error(&transient),
            RefreshOutcome::Transient
        ));
    }
}
