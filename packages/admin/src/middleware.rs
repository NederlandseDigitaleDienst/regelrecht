use std::future::Future;
use std::pin::Pin;

use axum::extract::{Request, State};
use axum::http::Method;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tower_sessions::Session;

pub use regelrecht_auth::middleware::refresh_session_token;
pub use regelrecht_auth::security_headers::{API_CSP, security_headers};
use regelrecht_auth::{RoleCheck, check_session_role};

use crate::error::ApiError;
use crate::state::AppState;

/// Methods allowed via API key authentication (no OIDC session required).
///
/// Within this set the key is an admin-equivalent for programmatic access: GET
/// reads, POST enqueues harvest/enrich jobs and triggers resets/syncs, DELETE
/// removes jobs. POST is included so scripts and services can enqueue jobs
/// without driving an interactive OIDC/SSO session. Any method outside the set
/// is rejected, so a route added with a different method does not silently
/// widen what the key can do.
const API_KEY_ALLOWED_METHODS: &[Method] = &[Method::GET, Method::POST, Method::DELETE];

type RequireAuthFuture = Pin<Box<dyn Future<Output = Result<Response, ApiError>> + Send>>;

/// Route-level auth gate for the admin (harvester) dashboard.
///
/// Two trust paths:
/// 1. A valid bearer API key — out-of-band trust, treated as regelrecht-admin
///    equivalent for the methods listed in [`API_KEY_ALLOWED_METHODS`]; other
///    methods are refused. The key holder is whoever provisioned the
///    deployment.
/// 2. An authenticated OIDC session — must carry `required_role` in
///    `SESSION_KEY_ROLES`. Composite expansion in Keycloak means higher
///    roles automatically satisfy lower-role checks.
pub fn require_auth(
    required_role: &'static str,
) -> impl Fn(State<AppState>, Session, Request, Next) -> RequireAuthFuture + Clone + Send + Sync + 'static
{
    move |State(state): State<AppState>, session: Session, request: Request, next: Next| {
        Box::pin(async move {
            // Check bearer token first (fast path for programmatic access).
            if let Some(ref key_hash) = state.config.api_key_hash
                && let Some(token) = extract_bearer_token(&request)
            {
                // Compare SHA-256 digests in constant time to prevent
                // timing leaks of both key content and length.
                let token_hash = Sha256::digest(token.as_bytes());
                let token_matches = token_hash.ct_eq(key_hash).into();
                if token_matches {
                    if !API_KEY_ALLOWED_METHODS.contains(request.method()) {
                        tracing::warn!(
                            method = %request.method(),
                            uri = %request.uri(),
                            "API key auth: method not allowed"
                        );
                        return Err(ApiError::Forbidden("method not allowed".to_string()));
                    }
                    return Ok(next.run(request).await);
                }
                // Invalid bearer token — reject immediately, don't fall
                // through to session (a wrong token is a deliberate signal).
                tracing::warn!(uri = %request.uri(), "API key auth: invalid bearer token");
                return Err(ApiError::Unauthorized("invalid bearer token".to_string()));
            }

            // Fall through to OIDC/session authentication.
            if !state.config.is_auth_enabled() {
                return Ok(next.run(request).await);
            }

            match check_session_role(&session, required_role).await {
                RoleCheck::Allowed => Ok(next.run(request).await),
                RoleCheck::NotAuthenticated => Err(ApiError::Unauthorized(
                    "authentication required".to_string(),
                )),
                RoleCheck::MissingRole { sub } => {
                    tracing::warn!(
                        required = %required_role,
                        sub = ?sub,
                        "user lacks required role for route"
                    );
                    Err(ApiError::Forbidden("forbidden".to_string()))
                }
            }
        })
    }
}

/// Guard for the `/metrics` endpoint. When `METRICS_AUTH_TOKEN` is configured,
/// only requests carrying a matching bearer token are allowed. When the env var
/// is absent the endpoint is open (backwards compatible).
pub async fn require_metrics_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(ref expected_hash) = state.config.metrics_token_hash else {
        return Ok(next.run(request).await);
    };

    if let Some(token) = extract_bearer_token(&request) {
        let token_hash = Sha256::digest(token.as_bytes());
        if token_hash.ct_eq(expected_hash).into() {
            return Ok(next.run(request).await);
        }
    }

    Err(ApiError::Unauthorized(
        "metrics auth token required".to_string(),
    ))
}

fn extract_bearer_token(request: &Request) -> Option<String> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    // RFC 7235: auth-scheme is case-insensitive.
    if value.len() > 7 && value[..7].eq_ignore_ascii_case("bearer ") {
        Some(value[7..].to_string())
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::state::AppState;
    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::middleware as axum_middleware;
    use axum::routing::get;
    use regelrecht_auth::{SESSION_KEY_AUTHENTICATED, SESSION_KEY_ROLES};
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use tower::ServiceExt;
    use tower_sessions::SessionManagerLayer;
    use tower_sessions_memory_store::MemoryStore;

    fn test_state_with_api_key(auth_enabled: bool, api_key: Option<&str>) -> AppState {
        let config = AppConfig {
            oidc: if auth_enabled {
                Some(crate::config::OidcConfig {
                    client_id: "test".into(),
                    client_secret: "test".into(),
                    issuer_url: "https://example.com".into(),
                    required_role: "harvester-reader".into(),
                })
            } else {
                None
            },
            base_url: None,
            api_key: api_key.map(String::from),
            api_key_hash: api_key.map(|k| {
                use sha2::{Digest, Sha256};
                Sha256::digest(k.as_bytes()).into()
            }),
            metrics_token_hash: None,
        };

        #[allow(clippy::expect_used)]
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost/test")
            .expect("lazy pool");

        AppState {
            pool,
            oidc_client: None,
            end_session_url: None,
            config: Arc::new(config),
            metrics_cache: Arc::new(crate::metrics::new_cache()),
            http_client: regelrecht_auth::http_client(),
            corpus: Arc::new(tokio::sync::RwLock::new(crate::state::CorpusState::empty())),
        }
    }

    fn test_state(auth_enabled: bool) -> AppState {
        test_state_with_api_key(auth_enabled, None)
    }

    fn test_app(state: AppState) -> Router {
        test_app_with_role(state, "harvester-reader")
    }

    fn test_app_with_role(state: AppState, role: &'static str) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        // PUT is registered only so the API-key method gate can be exercised
        // on a method outside `API_KEY_ALLOWED_METHODS`; the service itself
        // serves no PUT route. Without a matching handler axum answers 405
        // before `route_layer` runs, so the gate would never be reached.
        Router::new()
            .route(
                "/test",
                get(|| async { "ok" })
                    .post(|| async { "ok" })
                    .delete(|| async { "ok" })
                    .put(|| async { "ok" }),
            )
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_auth(role),
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
        assert_eq!(
            response.headers().get("www-authenticate").unwrap(),
            "Bearer"
        );
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
                require_auth("harvester-reader"),
            ))
            .route(
                "/set-auth",
                get(|session: Session| async move {
                    session
                        .insert(SESSION_KEY_AUTHENTICATED, true)
                        .await
                        .expect("insert");
                    session
                        .insert(SESSION_KEY_ROLES, vec!["harvester-reader".to_string()])
                        .await
                        .expect("insert roles");
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

    #[tokio::test]
    async fn api_key_valid_get_passes() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_valid_delete_passes() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_valid_post_passes() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        // POST is admin-equivalent via API key so scripts can enqueue jobs
        // without an OIDC session.
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_method_outside_allowlist_returns_403() {
        // The allowlist is a deliberate subset, not "every method": a valid
        // key on a method that is not listed is refused before any role check,
        // so a route added on a new method does not silently widen the key.
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("PUT")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn api_key_invalid_returns_401() {
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_key_not_configured_ignores_bearer() {
        let state = test_state_with_api_key(false, None);
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer some-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_without_oidc_rejects_invalid_token() {
        let state = test_state_with_api_key(false, Some("test-key"));
        let app = test_app(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer wrong-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_key_without_oidc_no_bearer_falls_through() {
        let state = test_state_with_api_key(false, Some("test-key"));
        let app = test_app(state);

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

    // -- require_metrics_auth tests --

    fn metrics_state(token: Option<&str>) -> AppState {
        let config = AppConfig {
            oidc: None,
            base_url: None,
            api_key: None,
            api_key_hash: None,
            metrics_token_hash: token.map(|k| {
                use sha2::{Digest, Sha256};
                Sha256::digest(k.as_bytes()).into()
            }),
        };

        #[allow(clippy::expect_used)]
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://test@localhost/test")
            .expect("lazy pool");

        AppState {
            pool,
            oidc_client: None,
            end_session_url: None,
            config: Arc::new(config),
            metrics_cache: Arc::new(crate::metrics::new_cache()),
            http_client: regelrecht_auth::http_client(),
            corpus: Arc::new(tokio::sync::RwLock::new(crate::state::CorpusState::empty())),
        }
    }

    fn metrics_app(state: AppState) -> Router {
        Router::new()
            .route("/metrics", get(|| async { "metrics" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_metrics_auth,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn metrics_no_token_configured_allows_all() {
        let app = metrics_app(metrics_state(None));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_valid_token_passes() {
        let app = metrics_app(metrics_state(Some("prom-secret")));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/metrics")
                    .header("authorization", "Bearer prom-secret")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_invalid_token_rejects() {
        let app = metrics_app(metrics_state(Some("prom-secret")));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/metrics")
                    .header("authorization", "Bearer wrong")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn metrics_no_token_sent_rejects() {
        let app = metrics_app(metrics_state(Some("prom-secret")));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // --- Role-based gating ---

    /// Build an app gated on `role` with a `/seed?roles=` helper that
    /// authenticates the session and inserts a roles list. The `/test` route
    /// also accepts DELETE so we can exercise the API-key bypass path on a
    /// destructive method against an admin-tier role.
    fn role_app(state: AppState, role: &'static str) -> Router {
        let store = MemoryStore::default();
        let session_layer = SessionManagerLayer::new(store);

        let gated = Router::new()
            .route(
                "/test",
                get(|| async { "ok" })
                    .post(|| async { "ok" })
                    .delete(|| async { "ok" }),
            )
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_auth(role),
            ));

        regelrecht_auth::test_utils::with_seed_route(gated)
            .with_state(state)
            .layer(session_layer)
    }

    use regelrecht_auth::test_utils::seed_session as seed_and_get_cookie;

    #[tokio::test]
    async fn reader_can_access_reader_route() {
        let app = role_app(test_state(true), "harvester-reader");
        let cookie = seed_and_get_cookie(&app, "harvester-reader").await;
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

    #[tokio::test]
    async fn reader_cannot_access_writer_route() {
        let app = role_app(test_state(true), "harvester-writer");
        let cookie = seed_and_get_cookie(&app, "harvester-reader").await;
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
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn writer_can_access_reader_route() {
        // Keycloak composite expansion: a writer's token contains the reader role.
        let app = role_app(test_state(true), "harvester-reader");
        let cookie = seed_and_get_cookie(&app, "harvester-writer,harvester-reader").await;
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

    #[tokio::test]
    async fn api_key_bypasses_role_check_on_get_reader_route() {
        // Reader-tier baseline: a GET request with a valid API key reaches a
        // reader-tier route without a session. Today reader_routes already
        // contains GETs (e.g. `GET /api/jobs`), so this case is exercised in
        // production — this test locks the invariant. Pairs with the writer-
        // and admin-tier variants below to cover all three tiers explicitly.
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = role_app(state, "harvester-reader");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_bypasses_role_check_on_get_admin_route() {
        // Locks in the documented invariant: a valid API key is treated as
        // regelrecht-admin-equivalent for the allowed methods, so GET on an
        // admin-tier route succeeds even though the bearer path carries no
        // role. Today writer_routes and admin_routes contain no GETs, but if
        // a future GET is added on either tier, this test ensures it's
        // obvious that route is reachable via the API key without a session.
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = role_app(state, "harvester-admin");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_bypasses_role_check_on_get_writer_route() {
        // Same invariant as the admin-tier variant above, but for the
        // writer tier. Writer routes currently contain no GETs either; this
        // test makes the API-key reachability of any future writer-tier GET
        // explicit and forces a deliberate change here if the invariant
        // ever shifts.
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = role_app(state, "harvester-writer");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_key_bypasses_role_check_on_delete_admin_route() {
        // Locks in the documented invariant: a valid API key is treated as
        // regelrecht-admin-equivalent for the allowed methods, so DELETE on
        // an admin-tier route succeeds even though the bearer path carries
        // no role. If this invariant changes (e.g. API key gains a role
        // restriction), this test must change deliberately — protecting any
        // future admin-tier DELETE from a silent permission shift.
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = role_app(state, "harvester-admin");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn merged_split_path_preserves_per_method_role_gate() {
        // Regression guard for the split-path setup in `main.rs`, where
        // `GET /api/jobs` lives behind `harvester-reader` (in `reader_routes`)
        // and `DELETE /api/jobs` behind `harvester-admin` (in `admin_routes`),
        // wired together via `.merge()`. Axum bakes `route_layer` into each
        // method's `MethodRouter` before the merge, so the per-method
        // middleware must survive — but if axum ever changes that semantic,
        // a reader session would silently gain the admin DELETE. This test
        // locks that invariant explicitly.
        //
        // Builds two single-method sub-routers on the same path with
        // different role gates, merges them, and asserts a reader session
        // gets 200 on GET but 403 on DELETE.
        let store = MemoryStore::default();
        let state = test_state(true);
        let session_layer = SessionManagerLayer::new(store);

        let reader_branch = Router::new()
            .route("/test", get(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_auth("harvester-reader"),
            ));

        let admin_branch = Router::new()
            .route("/test", axum::routing::delete(|| async { "ok" }))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_auth("harvester-admin"),
            ));

        let merged = reader_branch.merge(admin_branch);

        let app = regelrecht_auth::test_utils::with_seed_route(merged)
            .with_state(state)
            .layer(session_layer);

        let cookie = seed_and_get_cookie(&app, "harvester-reader").await;

        // GET passes (reader has the role).
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/test")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "GET should succeed for harvester-reader"
        );

        // DELETE is gated on harvester-admin — reader must be rejected.
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("DELETE")
                    .uri("/test")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "DELETE must reject harvester-reader — per-method middleware did not survive merge"
        );
    }

    #[tokio::test]
    async fn api_key_post_passes_regardless_of_role() {
        // POST is in API_KEY_ALLOWED_METHODS — the bearer path accepts it
        // without consulting the session role, so a valid key can enqueue jobs
        // on any role-gated route. A non-admin gate (harvester-writer) is used
        // here so the pass is evidently a method-based bypass, not a role match;
        // together with `api_key_valid_post_passes` (a reader-gated route) the
        // two cases span the "regardless of role" claim.
        let state = test_state_with_api_key(true, Some("test-key"));
        let app = role_app(state, "harvester-writer");
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/test")
                    .header("authorization", "Bearer test-key")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }
}
