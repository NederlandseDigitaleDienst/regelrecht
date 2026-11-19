//! Every route sits behind the gate the table below names, checked through the
//! router the binary serves.
//!
//! The table is the routes' contract: which role each route needs, and whether
//! it loads the caller's account. Requests go through `router()` itself, behind
//! a memory session store, so a route put in the wrong group fails here. A
//! second test reads the methods each `.route(..)` call mounts from the source
//! and compares them with the table, so a route or a method added without a
//! row fails too. (Over HTTP that cannot be told: a gate, and the account
//! loading, run before the method is matched, so an unmounted method answers
//! like a mounted one.)
//!
//! No database: a gate answers before any handler runs, and a route that gets
//! past its gate and needs an account answers 503 without a pool. That proves
//! an account route loads the account. The reverse is not proven: a
//! session-only route that also answers 503 without a pool (favorites,
//! settings) would not show that it started loading the account.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Query;
use axum::http::{Method, Request, StatusCode};
use axum::routing::get;
use regelrecht_auth::config::OidcConfig;
use regelrecht_auth::handlers::{SESSION_KEY_AUTHENTICATED, SESSION_KEY_ROLES, SESSION_KEY_SUB};
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::router;
use regelrecht_editor_api::state::AppState;
use tower::ServiceExt;
use tower_sessions::{Session, SessionManagerLayer};
use tower_sessions_memory_store::MemoryStore;

/// The realm roles the editor-api gates on.
const ROLES: [&str; 4] = [
    "editor-reader",
    "editor-writer",
    "editor-admin",
    "harvester-reader",
];

/// What a route requires of its caller.
#[derive(Clone, Copy, Debug)]
enum Gate {
    /// Anyone, without a session.
    Public,
    /// A session with this role; the handlers read the session only.
    Role(&'static str),
    /// A session with this role, and the caller's account row loaded.
    RoleAndAccount(&'static str),
}

use Gate::{Public, Role, RoleAndAccount};

/// One method on one route path, as registered in the source.
struct Route {
    method: Method,
    path: &'static str,
    gate: Gate,
}

const fn route(method: Method, path: &'static str, gate: Gate) -> Route {
    Route { method, path, gate }
}

const READER: &str = "editor-reader";
const WRITER: &str = "editor-writer";
const ADMIN: &str = "editor-admin";
const HARVESTER: &str = "harvester-reader";

/// Every route the editor-api mounts (with OIDC configured), and its gate.
const ROUTES: &[Route] = &[
    route(Method::GET, "/health", Public),
    // Login and logout (regelrecht_auth::auth_routes).
    route(Method::GET, "/auth/login", Public),
    route(Method::GET, "/auth/callback", Public),
    route(Method::GET, "/auth/logout", Public),
    route(Method::GET, "/auth/status", Public),
    // Global corpus reads.
    route(Method::GET, "/api/sources", Public),
    route(Method::GET, "/api/corpus/laws", Public),
    route(Method::GET, "/api/corpus/laws/{law_id}", Public),
    route(Method::GET, "/api/corpus/laws/{law_id}/versions", Public),
    route(Method::GET, "/api/corpus/laws/{law_id}/outputs", Public),
    route(
        Method::GET,
        "/api/corpus/laws/{law_id}/implementors",
        Public,
    ),
    route(Method::GET, "/api/corpus/laws/{law_id}/scenarios", Public),
    route(
        Method::GET,
        "/api/corpus/laws/{law_id}/scenarios/{filename}",
        Public,
    ),
    route(Method::GET, "/api/corpus/laws/{law_id}/annotations", Public),
    route(Method::GET, "/api/feature-flags", Public),
    route(Method::GET, "/api/document-upload-formats", Public),
    route(Method::GET, "/api/harvest/status", Public),
    // User-scoped reads and writes, session only.
    route(Method::GET, "/api/favorites", Role(READER)),
    route(Method::PUT, "/api/favorites/{law_id}", Role(READER)),
    route(Method::DELETE, "/api/favorites/{law_id}", Role(READER)),
    route(Method::GET, "/api/user/settings", Role(READER)),
    route(Method::GET, "/api/harvest/search", Role(READER)),
    route(Method::PUT, "/api/user/settings/{key}", Role(WRITER)),
    route(Method::POST, "/api/harvest", Role(WRITER)),
    route(Method::POST, "/api/harvest/batch", Role(WRITER)),
    // Personal notes and tasks.
    route(
        Method::GET,
        "/api/user/notes/{law_id}",
        RoleAndAccount(READER),
    ),
    route(
        Method::POST,
        "/api/user/notes/{law_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PUT,
        "/api/user/notes/{law_id}/{note_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::DELETE,
        "/api/user/notes/{law_id}/{note_id}",
        RoleAndAccount(WRITER),
    ),
    route(Method::GET, "/api/tasks", RoleAndAccount(READER)),
    route(Method::GET, "/api/tasks/{task_id}", RoleAndAccount(READER)),
    route(
        Method::GET,
        "/api/tasks/jobs/{job_id}",
        RoleAndAccount(READER),
    ),
    route(
        Method::POST,
        "/api/tasks/{task_id}/resolve",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/tasks/jobs/{job_id}/apply",
        RoleAndAccount(WRITER),
    ),
    // Admin and the harvester-admin proxy.
    route(Method::POST, "/api/corpus/reload", Role(ADMIN)),
    route(Method::PUT, "/api/feature-flags/{key}", Role(ADMIN)),
    route(Method::GET, "/api/harvest-admin/{*rest}", Role(HARVESTER)),
    route(Method::POST, "/api/harvest-admin/{*rest}", Role(HARVESTER)),
    route(Method::PUT, "/api/harvest-admin/{*rest}", Role(HARVESTER)),
    route(Method::PATCH, "/api/harvest-admin/{*rest}", Role(HARVESTER)),
    route(
        Method::DELETE,
        "/api/harvest-admin/{*rest}",
        Role(HARVESTER),
    ),
    // Traject reads.
    route(Method::GET, "/api/trajects", RoleAndAccount(READER)),
    route(Method::GET, "/api/trajects/{id}", RoleAndAccount(READER)),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/sources",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/runs",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/favorites",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/changed-laws",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/versions",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/outputs",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/implementors",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios/{filename}",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/annotations",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/documents",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/documents/jobs",
        RoleAndAccount(READER),
    ),
    // Cancelling a conversion sits in the reader group, like its list.
    route(
        Method::DELETE,
        "/api/trajects/{traject_ref}/corpus/documents/jobs/{job_id}",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/corpus/documents/{*doc_path}",
        RoleAndAccount(READER),
    ),
    route(
        Method::GET,
        "/api/trajects/{traject_ref}/integrity",
        RoleAndAccount(READER),
    ),
    // Traject writes.
    route(Method::POST, "/api/trajects", RoleAndAccount(WRITER)),
    route(Method::PATCH, "/api/trajects/{id}", RoleAndAccount(WRITER)),
    route(Method::DELETE, "/api/trajects/{id}", RoleAndAccount(WRITER)),
    route(
        Method::PUT,
        "/api/trajects/{id}/repo",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PUT,
        "/api/trajects/{traject_ref}/favorites/{law_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::DELETE,
        "/api/trajects/{traject_ref}/favorites/{law_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{id}/leave",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{id}/members",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PATCH,
        "/api/trajects/{id}/members/{account_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::DELETE,
        "/api/trajects/{id}/members/{account_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::DELETE,
        "/api/trajects/{id}/invites/{email}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PUT,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios/{filename}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::DELETE,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/scenarios/{filename}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PUT,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/annotations",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PUT,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{traject_ref}/corpus/documents/upload",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{traject_ref}/corpus/laws/upload",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{traject_ref}/corpus/laws",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/enrich",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{traject_ref}/corpus/laws/{law_id}/promote",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::POST,
        "/api/trajects/{traject_ref}/corpus/harvest",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::PUT,
        "/api/trajects/{traject_ref}/corpus/documents/{*doc_path}",
        RoleAndAccount(WRITER),
    ),
    route(
        Method::DELETE,
        "/api/trajects/{traject_ref}/corpus/documents/{*doc_path}",
        RoleAndAccount(WRITER),
    ),
    // GitHub account linking, mounted only with OIDC.
    route(Method::GET, "/auth/github/login", RoleAndAccount(WRITER)),
    route(Method::GET, "/auth/github/callback", RoleAndAccount(WRITER)),
    route(Method::GET, "/auth/github/status", RoleAndAccount(WRITER)),
    route(
        Method::POST,
        "/auth/github/disconnect",
        RoleAndAccount(WRITER),
    ),
    route(Method::GET, "/auth/github/relay", Public),
];

/// A concrete path for a route pattern: every parameter becomes a segment.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with("{*") {
                "x/y"
            } else if segment.starts_with('{') {
                "x"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The state with OIDC configured, so the gates are live. Nothing is
/// discovered: the gates read the session only.
fn auth_state() -> AppState {
    let state = common::app_state(None, None);
    let mut config = AppConfig::clone(&state.config);
    config.oidc = Some(OidcConfig {
        client_id: "editor".to_string(),
        client_secret: "not-a-secret".to_string(),
        issuer_url: "https://sso.example.test/realms/test".to_string(),
        required_role: READER.to_string(),
        extra_claims: Vec::new(),
    });
    AppState {
        config: Arc::new(config),
        ..state
    }
}

#[derive(serde::Deserialize)]
struct SeedQuery {
    roles: String,
}

/// The router as the binary serves it, behind a memory session store, plus a
/// `/test-seed?roles=...` route that logs a user in with those roles, as the
/// OIDC callback would: authenticated, with a subject and the realm roles.
/// (`regelrecht_auth::test_utils::seed_session` sets no subject, and the
/// user-scoped handlers answer 401 without one, which a test would read as
/// the gate refusing.)
fn app(state: AppState) -> Router {
    router(state)
        .route(
            "/test-seed",
            get(|session: Session, Query(q): Query<SeedQuery>| async move {
                let roles: Vec<String> = q
                    .roles
                    .split(',')
                    .filter(|r| !r.is_empty())
                    .map(str::to_string)
                    .collect();
                session
                    .insert(SESSION_KEY_AUTHENTICATED, true)
                    .await
                    .unwrap();
                session.insert(SESSION_KEY_SUB, "test-user").await.unwrap();
                session.insert(SESSION_KEY_ROLES, roles).await.unwrap();
            }),
        )
        .layer(SessionManagerLayer::new(MemoryStore::default()))
}

/// A session cookie for a user with `roles` (comma-separated).
async fn seed_session(app: &Router, roles: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/test-seed?roles={roles}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "the seed route failed");
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .expect("a session cookie");
    cookie_pair(set_cookie.to_str().unwrap()).to_string()
}

async fn status(app: &Router, method: &Method, path: &str, cookie: Option<&str>) -> StatusCode {
    let mut request = Request::builder().method(method.clone()).uri(path);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

/// The `name=value` part of a `Set-Cookie` header.
fn cookie_pair(set_cookie: &str) -> &str {
    set_cookie.split(';').next().unwrap_or(set_cookie)
}

#[tokio::test]
async fn every_route_sits_behind_its_gate() {
    let app = app(auth_state());
    let mut failures = Vec::new();

    for route in ROUTES {
        let path = concrete(route.path);
        let (role, needs_account) = match route.gate {
            Public => {
                let got = status(&app, &route.method, &path, None).await;
                if matches!(got, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                    failures.push(format!(
                        "{} {}: public, but answered {got} without a session",
                        route.method, route.path
                    ));
                }
                continue;
            }
            Role(role) => (role, false),
            RoleAndAccount(role) => (role, true),
        };

        let anonymous = status(&app, &route.method, &path, None).await;
        if anonymous != StatusCode::UNAUTHORIZED {
            failures.push(format!(
                "{} {}: without a session {anonymous}, want 401",
                route.method, route.path
            ));
        }

        // Every role but the one required: a writer route must refuse a
        // reader, and the admin role does not stand in for a writer.
        let others = ROLES
            .iter()
            .filter(|r| **r != role)
            .copied()
            .collect::<Vec<_>>()
            .join(",");
        let other_cookie = seed_session(&app, &others).await;
        let wrong_role = status(&app, &route.method, &path, Some(&other_cookie)).await;
        if wrong_role != StatusCode::FORBIDDEN {
            failures.push(format!(
                "{} {}: without {role} {wrong_role}, want 403",
                route.method, route.path
            ));
        }

        let cookie = seed_session(&app, role).await;
        let allowed = status(&app, &route.method, &path, Some(&cookie)).await;
        if matches!(allowed, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            failures.push(format!(
                "{} {}: with {role} still {allowed}",
                route.method, route.path
            ));
        }
        // Past the gate, a route that loads the account finds no database
        // here and stops there.
        if needs_account && allowed != StatusCode::SERVICE_UNAVAILABLE {
            failures.push(format!(
                "{} {}: with {role} {allowed}, want 503 from loading the account",
                route.method, route.path
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "routes behind the wrong gate:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
async fn without_oidc_the_gates_pass_and_github_linking_is_not_mounted() {
    let app = app(common::app_state(None, None));
    // A writer route answers without a session: local development only.
    let harvest = status(&app, &Method::POST, "/api/harvest", None).await;
    assert!(
        !matches!(harvest, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
        "a writer route without OIDC answered {harvest}"
    );
    assert_eq!(
        status(&app, &Method::GET, "/auth/github/status", None).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, &Method::GET, "/auth/github/relay", None).await,
        StatusCode::NOT_FOUND
    );
}

/// The methods each `.route(path, ...)` call in `source` mounts, as
/// `(METHOD, path)`: `get(..).put(..)` mounts GET and PUT, `any(..)` every
/// method the table knows. Comments are dropped first, so a word in one is
/// not read as a method.
fn registered_routes(source: &str) -> BTreeSet<(String, String)> {
    let code: String = source
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut found = BTreeSet::new();
    for call in code.split(".route(").skip(1) {
        let Some(path) = call.split('"').nth(1) else {
            continue;
        };
        // The call ends where its parentheses close.
        let mut depth = 1;
        let end = call
            .char_indices()
            .find(|&(_, c)| {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .map_or(call.len(), |(i, _)| i);
        let handlers = &call[..end];
        for (word, methods) in [
            ("get(", &["GET"][..]),
            ("post(", &["POST"][..]),
            ("put(", &["PUT"][..]),
            ("patch(", &["PATCH"][..]),
            ("delete(", &["DELETE"][..]),
            ("any(", &["GET", "POST", "PUT", "PATCH", "DELETE"][..]),
        ] {
            let mounted = handlers.match_indices(word).any(|(at, _)| {
                // A whole word: `.put(`, `routing::put(` or `(get(`, not
                // `budget(`.
                !handlers[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_')
            });
            if mounted {
                for method in methods {
                    found.insert(((*method).to_string(), path.to_string()));
                }
            }
        }
    }
    found
}

#[test]
fn the_table_lists_every_route_in_the_source() {
    let mut in_source = registered_routes(include_str!("../src/router.rs"));
    in_source.extend(registered_routes(include_str!("../src/github_oauth.rs")));
    // The login routes come from regelrecht_auth, outside this source.
    let in_table: BTreeSet<(String, String)> = ROUTES
        .iter()
        .filter(|r| !r.path.starts_with("/auth/") || r.path.starts_with("/auth/github/"))
        .map(|r| (r.method.to_string(), r.path.to_string()))
        .collect();

    let missing: Vec<_> = in_source.difference(&in_table).collect();
    let stale: Vec<_> = in_table.difference(&in_source).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "routes without a row in the table: {missing:?}\nrows for routes that are not mounted: {stale:?}"
    );
}

#[tokio::test]
async fn every_response_carries_its_server_timing() {
    // `server_timing` wraps the routed handlers, so even a request a gate
    // refuses goes through it.
    let app = app(auth_state());
    for (method, path) in [(Method::GET, "/health"), (Method::POST, "/api/harvest")] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            response.headers().contains_key("server-timing"),
            "{method} {path} answered without a Server-Timing header"
        );
    }
}
