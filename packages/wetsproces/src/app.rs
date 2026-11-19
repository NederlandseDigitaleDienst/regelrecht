//! What the server is built from in each mode, and the refusals that keep it
//! from starting where it would run unsafely.
//!
//! The store, the clock and the login are chosen apart:
//!
//! - test mode runs on a [`ControllableClock`] that starts on the case's
//!   `speeldatum`, with the store the configuration names ([`InMemoryStore`]
//!   or [`PostgresStore`]), and logs in through Keycloak when OIDC is
//!   configured, or else through the test login;
//! - stable mode runs on the [`RealClock`], with Postgres and the login
//!   through Keycloak. The configuration already refuses it without them
//!   ([`crate::config`]); [`build_with`] refuses once more, so no caller can
//!   hand it the test login. The same holds for the surface: stable mode
//!   serves one side ([`crate::surface::Surface`]), never both.
//!
//! The surface decides the routes ([`crate::api::router`]), the session
//! cookie and the mark on each session ([`SurfaceSessions`]), and which
//! build of the screens is served: the directory of the surface's name
//! under `WETSPROCES_FRONTEND`.
//!
//! When Postgres is asked for and cannot be reached or migrated, the server
//! does not start: it never falls back to memory. The sessions live beside
//! the chronicles ([`crate::login`]), and so do the challenges and sessions
//! of FSC voor Burgers ([`crate::fsc4b`]) when that channel is configured.

use std::sync::Arc;
use std::time::Duration;

use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use sqlx::PgPool;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{ExpiredDeletion, Expiry, SessionManagerLayer, SessionStore};
use tower_sessions_memory_store::MemoryStore;

use crate::api::{ApiError, AppState, router};
use crate::case::{self, CaseError};
use crate::clock::{Clock, ControllableClock, RealClock};
use crate::config::{Config, Fsc4bChoice, Mode, StoreChoice};
use crate::fsc4b::{Fsc4b, Fsc4bError};
use crate::login::{Login, SESSION_TABLE, SurfaceSessions};
use crate::service::{Service, ServiceError};
use crate::store::memory::InMemoryStore;
use crate::store::postgres::{CONNECT_TIMEOUT, MIGRATION_LOCK_KEY, PostgresStore, SCHEMA};
use crate::store::{ChronicleStore, StoreError};
use crate::surface::Surface;

/// How long a session lasts without a request, as the editor's.
const SESSION_INACTIVITY_HOURS: i64 = 8;

/// How often expired sessions are removed from Postgres.
const SESSION_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Why the server does not start.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(
        "stable mode needs the login through Keycloak, the chronicles in Postgres and the real \
         clock; it was given the test login or the store in memory"
    )]
    StableNeedsLogin,
    #[error("stable mode serves citizens or employees, not both on one address")]
    StableNeedsOneSurface,
    #[error("FSC voor Burgers serves citizens, and this instance serves the back office")]
    Fsc4bOnBackoffice,
    #[error("the chronicle store ({store}): {source}")]
    Store {
        store: &'static str,
        source: StoreError,
    },
    #[error("the session store: {0}")]
    Sessions(String),
    #[error("the login: {0}")]
    Login(String),
    #[error("the case: {0}")]
    Case(#[from] CaseError),
    #[error("the service: {0}")]
    Service(#[from] ServiceError),
    #[error("FSC voor Burgers: {0}")]
    Fsc4b(#[from] Fsc4bError),
}

/// The router of the server for `config`, with its login discovered.
///
/// # Errors
///
/// When OIDC discovery fails, and as [`build_with`].
pub async fn build(config: &Config) -> Result<Router, StartupError> {
    let login = Login::discover(&config.login)
        .await
        .map_err(StartupError::Login)?;
    build_with(config, login).await
}

/// The router of the server for `config` with the login `login`.
///
/// # Errors
///
/// [`StartupError::StableNeedsLogin`] in stable mode with the test login or
/// the store in memory, [`StartupError::StableNeedsOneSurface`] in stable
/// mode on [`Surface::All`]; otherwise when the manifest gives no case to
/// run, the stores do not open, or the service does not start.
pub async fn build_with(config: &Config, login: Login) -> Result<Router, StartupError> {
    let stable = config.mode == Mode::Stable;
    if stable && (matches!(login, Login::TestLogin) || config.store == StoreChoice::Memory) {
        return Err(StartupError::StableNeedsLogin);
    }
    if stable && config.surface == Surface::All {
        return Err(StartupError::StableNeedsOneSurface);
    }
    if !config.surface.serves_citizens() && !matches!(config.fsc4b, Fsc4bChoice::Off) {
        return Err(StartupError::Fsc4bOnBackoffice);
    }
    let (corpus, messages) = case::load(&config.manifest)?;
    for message in &messages {
        tracing::warn!(%message, "loading the case");
    }
    let stores = open_store(&config.store, CONNECT_TIMEOUT).await?;
    // The test clock keeps its day in the store, so instances on one store
    // tell the same date.
    let controllable = (!stable).then(|| {
        Arc::new(ControllableClock::new(
            Arc::clone(&stores.chronicles),
            corpus.casus.speeldatum,
        ))
    });
    let clock: Arc<dyn Clock> = match &controllable {
        Some(c) => Arc::clone(c) as _,
        None => Arc::new(RealClock),
    };
    let fsc4b = Fsc4b::open(&config.fsc4b, config.mode, stores.sessions.clone())?;
    let today = clock.today().await.map_err(|source| StartupError::Store {
        store: config.store.name(),
        source,
    })?;
    tracing::info!(
        mode = %config.mode,
        surface = %config.surface,
        case = %corpus.casus.id,
        manifest = %config.manifest.display(),
        today = %today,
        store = config.store.name(),
        login = ?login,
        fsc4b = ?config.fsc4b,
        "starting"
    );
    let service = Service::start(corpus, stores.chronicles, clock).await?;
    let secure = login.secure_cookie();
    let mut state = AppState::new(
        Arc::new(service),
        config.mode,
        config.surface,
        controllable,
        login,
        &config.subject_claim,
    );
    if let Some(fsc4b) = fsc4b {
        state = state.with_fsc4b(Arc::new(fsc4b));
    }
    let frontend = config.frontend.join(config.surface.name());
    let app = with_frontend(router(state.clone()), &frontend).layer(
        middleware::from_fn_with_state(state, regelrecht_auth::refresh_session_token::<AppState>),
    );
    let surface = config.surface;
    match stores.sessions {
        None => Ok(with_sessions(app, MemoryStore::default(), surface, secure)),
        Some(pool) => Ok(with_sessions(
            app,
            postgres_sessions(pool).await?,
            surface,
            secure,
        )),
    }
}

/// `app` with the built frontend in `dir` behind it, when `dir` holds an
/// `index.html`: a path that is no route of the API is a file of the
/// frontend, or else its `index.html`, so a deep link opens the app. A path
/// under `/api/` or `/auth/` that is no route stays a `404` of the API, so
/// a mistyped call never gets a page back. Without the frontend the server
/// serves the API alone.
fn with_frontend(app: Router, dir: &Path) -> Router {
    let index = dir.join("index.html");
    if !index.is_file() {
        tracing::info!(frontend = %dir.display(), "no built frontend; serving the API alone");
        return app;
    }
    tracing::info!(frontend = %dir.display(), "serving the frontend");
    let files = ServeDir::new(dir).fallback(ServeFile::new(index));
    app.fallback(move |request: Request| {
        let files = files.clone();
        async move {
            let path = request.uri().path();
            // `/fsc4b/` too: a route of FSC voor Burgers that does not exist,
            // or a channel that is off, answers 404 to an app, not the screens.
            if path.starts_with("/api/")
                || path.starts_with("/auth/")
                || path.starts_with("/fsc4b/")
            {
                return ApiError::new(StatusCode::NOT_FOUND, "there is no such route")
                    .into_response();
            }
            match files.oneshot(request).await {
                Ok(response) => response.map(Body::new),
                Err(e) => match e {},
            }
        }
    })
}

/// `app` with its sessions in `store`, under the cookie of `surface` and
/// marked with it.
fn with_sessions<St: SessionStore + Clone>(
    app: Router,
    store: St,
    surface: Surface,
    secure: bool,
) -> Router {
    let layer = SessionManagerLayer::new(SurfaceSessions::new(store, surface))
        .with_name(surface.session_cookie())
        .with_expiry(Expiry::OnInactivity(CookieDuration::hours(
            SESSION_INACTIVITY_HOURS,
        )))
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_http_only(true)
        .with_secure(secure);
    app.layer(layer)
}

/// The session store in the table [`SESSION_TABLE`] of the schema
/// [`SCHEMA`], made if it is not there, with a task that removes expired
/// sessions. The table is made under the advisory lock of the schema's
/// migrations, so two instances that start at once on a new database do not
/// both make it.
///
/// # Errors
///
/// [`StartupError::Sessions`] when the table cannot be made.
async fn postgres_sessions(
    pool: PgPool,
) -> Result<tower_sessions_sqlx_store::PostgresStore, StartupError> {
    let store = tower_sessions_sqlx_store::PostgresStore::new(pool.clone())
        .with_schema_name(SCHEMA)
        .and_then(|s| s.with_table_name(SESSION_TABLE))
        .map_err(StartupError::Sessions)?;
    migrate_sessions(&pool, &store)
        .await
        .map_err(StartupError::Sessions)?;
    let cleaner = store.clone();
    tokio::spawn(async move {
        if let Err(e) = cleaner
            .continuously_delete_expired(SESSION_CLEANUP_INTERVAL)
            .await
        {
            tracing::error!(error = %e, "removing expired sessions stopped");
        }
    });
    Ok(store)
}

/// Make the sessions table of `store`, holding the migrations' advisory
/// lock on a connection of `pool` while it does.
async fn migrate_sessions(
    pool: &PgPool,
    store: &tower_sessions_sqlx_store::PostgresStore,
) -> Result<(), String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let migrated = store.migrate().await.map_err(|e| e.to_string());
    let unlocked = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_KEY)
        .execute(&mut *conn)
        .await;
    if let Err(e) = unlocked {
        // A connection that may still hold the lock goes back to no pool:
        // closing it ends its session, and the lock with it.
        if let Err(close) = conn.close().await {
            tracing::warn!(error = %close, "closing the connection that held the migration lock");
        }
        return Err(e.to_string());
    }
    migrated
}

/// The stores a configuration opens: the chronicles, and the database the
/// sessions share with them when they live in Postgres.
#[derive(Debug)]
struct Stores {
    chronicles: Arc<dyn ChronicleStore>,
    sessions: Option<PgPool>,
}

/// The store `choice` names, opened; a connection to Postgres may take
/// `timeout`.
///
/// # Errors
///
/// [`StartupError::Store`] when Postgres cannot be reached or migrated.
async fn open_store(choice: &StoreChoice, timeout: Duration) -> Result<Stores, StartupError> {
    match choice {
        StoreChoice::Memory => Ok(Stores {
            chronicles: Arc::new(InMemoryStore::new()),
            sessions: None,
        }),
        StoreChoice::Postgres { url } => match PostgresStore::connect_within(url, timeout).await {
            Ok(store) => Ok(Stores {
                sessions: Some(store.pool().clone()),
                chronicles: Arc::new(store),
            }),
            Err(source) => Err(StartupError::Store {
                store: choice.name(),
                source,
            }),
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;

    use regelrecht_shared::postgres::test_utils::TestDatabase;

    use super::*;
    use crate::config::{DEFAULT_MANIFEST, DEFAULT_PORT, DEFAULT_SUBJECT_CLAIM, LoginChoice};

    fn config(mode: Mode) -> Config {
        Config {
            mode,
            surface: Surface::All,
            store: StoreChoice::Memory,
            manifest: PathBuf::from(DEFAULT_MANIFEST),
            frontend: PathBuf::from("/nonexistent/frontend"),
            port: DEFAULT_PORT,
            login: LoginChoice::TestLogin,
            subject_claim: DEFAULT_SUBJECT_CLAIM.into(),
            fsc4b: Fsc4bChoice::Off,
        }
    }

    #[tokio::test]
    async fn stable_mode_refuses_the_test_login_and_the_store_in_memory() {
        let mut c = config(Mode::Stable);
        c.store = StoreChoice::Postgres {
            url: "postgres://nowhere.invalid/regelrecht".into(),
        };
        let e = build_with(&c, Login::TestLogin).await.unwrap_err();
        assert!(matches!(e, StartupError::StableNeedsLogin), "{e}");
        let c = config(Mode::Stable);
        let e = build_with(&c, crate::login::tests::offline_login())
            .await
            .unwrap_err();
        assert!(matches!(e, StartupError::StableNeedsLogin), "{e}");
    }

    #[tokio::test]
    async fn stable_mode_refuses_to_serve_both_sides() {
        let database = TestDatabase::create().await;
        let mut c = config(Mode::Stable);
        c.store = StoreChoice::Postgres {
            url: database.url().to_string(),
        };
        let e = build_with(&c, crate::login::tests::offline_login())
            .await
            .unwrap_err();
        assert!(matches!(e, StartupError::StableNeedsOneSurface), "{e}");
        for surface in [Surface::Citizen, Surface::Backoffice] {
            c.surface = surface;
            let _router = build_with(&c, crate::login::tests::offline_login())
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn the_back_office_refuses_fsc_voor_burgers() {
        let mut c = config(Mode::Test);
        c.surface = Surface::Backoffice;
        c.fsc4b = Fsc4bChoice::Demo {
            public_url: "http://localhost:7199".into(),
        };
        let e = build_with(&c, Login::TestLogin).await.unwrap_err();
        assert!(matches!(e, StartupError::Fsc4bOnBackoffice), "{e}");
    }

    #[tokio::test]
    async fn two_instances_starting_at_once_on_a_new_database_both_start() {
        let database = TestDatabase::create().await;
        let mut c = config(Mode::Test);
        c.store = StoreChoice::Postgres {
            url: database.url().to_string(),
        };
        let mut b = c.clone();
        c.surface = Surface::Citizen;
        b.surface = Surface::Backoffice;
        let (first, second) = tokio::join!(
            build_with(&c, Login::TestLogin),
            build_with(&b, Login::TestLogin)
        );
        let _citizens = first.unwrap();
        let _backoffice = second.unwrap();
    }

    #[tokio::test]
    async fn an_unreachable_postgres_does_not_fall_back_to_memory() {
        // Port 1 on the loopback: nothing listens.
        let choice = StoreChoice::Postgres {
            url: "postgres://wetsproces@127.0.0.1:1/regelrecht".into(),
        };
        let e = open_store(&choice, Duration::from_millis(500))
            .await
            .unwrap_err();
        assert!(
            matches!(
                e,
                StartupError::Store {
                    store: "postgres",
                    ..
                }
            ),
            "{e}"
        );
    }

    #[tokio::test]
    async fn test_mode_starts_on_postgres_with_its_sessions_beside_the_chronicles() {
        let database = TestDatabase::create().await;
        let mut c = config(Mode::Test);
        c.store = StoreChoice::Postgres {
            url: database.url().to_string(),
        };
        let _router = build_with(&c, Login::TestLogin).await.unwrap();
        let pool = PgPool::connect(database.url()).await.unwrap();
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT table_name::text FROM information_schema.tables \
             WHERE table_schema = 'wetsproces' AND table_name = 'sessions'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(tables, ["sessions"]);
    }

    #[tokio::test]
    async fn stable_mode_starts_with_keycloak_postgres_and_the_real_clock() {
        let database = TestDatabase::create().await;
        let mut c = config(Mode::Stable);
        c.surface = Surface::Citizen;
        c.store = StoreChoice::Postgres {
            url: database.url().to_string(),
        };
        let _router = build_with(&c, crate::login::tests::offline_login())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_mode_starts_on_the_manifest() {
        let _router = build_with(&config(Mode::Test), Login::TestLogin)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_manifest_without_a_case_does_not_start() {
        let mut c = config(Mode::Test);
        c.manifest = PathBuf::from("/nonexistent/bron.yaml");
        let e = build_with(&c, Login::TestLogin).await.unwrap_err();
        assert!(matches!(e, StartupError::Case(_)), "{e}");
    }
}
