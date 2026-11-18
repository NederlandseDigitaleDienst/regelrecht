//! Test-only helpers for seeding authenticated sessions in middleware tests.
//!
//! Exposed only under the `test-utils` feature so downstream crates (e.g.
//! `regelrecht-admin`) can reuse the same seeding contract without
//! duplicating the helpers in their own `#[cfg(test)]` modules.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::Router;
use axum::body::Body;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use tower::ServiceExt;
use tower_sessions::Session;

use crate::handlers::{SESSION_KEY_AUTHENTICATED, SESSION_KEY_ROLES};

/// Query string for the `/seed` helper route. A comma-separated list of
/// realm role names, e.g. `roles=harvester-writer,harvester-reader`.
#[derive(serde::Deserialize)]
pub struct SeedQuery {
    pub roles: String,
}

/// Mount a `/seed?roles=...` route on the given router that, when hit,
/// authenticates the current session and inserts the provided realm roles.
///
/// Returns the augmented router. Use together with [`seed_session`] to
/// obtain a session cookie that subsequent requests can present.
pub fn with_seed_route<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.route(
        "/seed",
        get(|session: Session, Query(q): Query<SeedQuery>| async move {
            session
                .insert(SESSION_KEY_AUTHENTICATED, true)
                .await
                .expect("insert auth");
            let roles: Vec<String> = q
                .roles
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            session
                .insert(SESSION_KEY_ROLES, roles)
                .await
                .expect("insert roles");
            "seeded"
        }),
    )
}

/// Hit `/seed?roles=<roles>` on the given app and return the resulting
/// `Set-Cookie` value. Panics if the seed route did not return 200 or did
/// not set a cookie — both indicate the test setup is broken.
pub async fn seed_session(app: &Router, roles: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/seed?roles={roles}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK, "seed route failed");
    response
        .headers()
        .get("set-cookie")
        .expect("set-cookie header on /seed response")
        .to_str()
        .expect("cookie str")
        .to_string()
}
