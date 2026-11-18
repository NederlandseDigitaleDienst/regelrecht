//! Test helpers exposed via the `test-utils` Cargo feature.
//!
//! These are used by this crate's integration tests and by downstream
//! crates (e.g. admin) that need a real Postgres container with the
//! pipeline schema applied.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Weak};

use sqlx::PgPool;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;

use crate::config::PipelineConfig;
use crate::db;

/// Where the tests get their Postgres from.
///
/// `TEST_POSTGRES_URL` points at a server that is already running — a
/// `services:` block in CI, or a container someone started by hand. Without it
/// the helper starts its own container, so a plain `cargo test` keeps working
/// without setup.
///
/// The container variant is shared per test binary, together with the
/// connection details resolved once. A container per test is what this used to
/// do, and at 185 call sites that is 185 container starts per run; resolving
/// the published port is a second daemon round-trip, and doing that per test
/// had thirty tests inspecting the daemon at once, which fails with EAGAIN and
/// reads as a test failure. Cargo runs each test binary as its own process, so
/// even shared-per-binary still pays that start about two dozen times per run.
/// An external server pays it zero times.
///
/// Held as a `Weak` so the container still stops when the last `TestDb` in the
/// binary drops; a `static` is never dropped, so an owning handle here would
/// leave the container running after the process exits.
struct SharedPostgres {
    base_url: String,
    _container: Option<ContainerAsync<Postgres>>,
}

static SHARED_CONTAINER: LazyLock<Mutex<Weak<SharedPostgres>>> =
    LazyLock::new(|| Mutex::new(Weak::new()));

/// Names the per-test database. The process id is part of the name because an
/// external server is shared by every test binary in a run, and the counter
/// alone is only unique within one process.
static DB_SEQ: AtomicU64 = AtomicU64::new(0);

#[allow(clippy::unwrap_used)]
async fn shared_postgres() -> Arc<SharedPostgres> {
    let mut shared = SHARED_CONTAINER.lock().await;
    if let Some(running) = shared.upgrade() {
        return running;
    }

    let external = std::env::var("TEST_POSTGRES_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());

    let started = Arc::new(match external {
        Some(url) => SharedPostgres {
            base_url: url.trim_end_matches('/').to_string(),
            _container: None,
        },
        None => {
            let container = Postgres::default().start().await.unwrap();
            let host_port = container.get_host_port_ipv4(5432).await.unwrap();
            // testcontainers reports the published port on the docker host.
            // From a native Linux dev environment `127.0.0.1` is correct; from
            // a dev container talking to Docker Desktop on a different host
            // (WSL2, remote docker) the docker host is reachable as
            // `host.docker.internal` instead. `TESTCONTAINERS_HOST_OVERRIDE`
            // lets those setups point at the right host without forking this
            // helper.
            let host = std::env::var("TESTCONTAINERS_HOST_OVERRIDE")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "127.0.0.1".to_string());
            SharedPostgres {
                base_url: format!("postgres://postgres:postgres@{host}:{host_port}"),
                _container: Some(container),
            }
        }
    });
    *shared = Arc::downgrade(&started);
    started
}

/// Postgres database with the pipeline schema applied and seed rows truncated,
/// ready for integration tests. Every `TestDb` gets its own database inside the
/// shared container, so tests stay isolated from each other.
pub struct TestDb {
    pub pool: PgPool,
    _container: Arc<SharedPostgres>,
}

impl TestDb {
    #[allow(clippy::unwrap_used)]
    pub async fn new() -> Self {
        let shared = shared_postgres().await;

        let db_name = format!(
            "regelrecht_test_{}_{}",
            std::process::id(),
            DB_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let admin = PgPool::connect(&format!("{}/postgres", shared.base_url))
            .await
            .unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;

        let config = PipelineConfig::new(format!("{}/{db_name}", shared.base_url));
        let pool = db::create_pool(&config).await.unwrap();
        // ensure_schema takes a pg_advisory_lock, which is scoped to the
        // database, so parallel tests do not queue behind each other.
        db::ensure_schema(&pool).await.unwrap();

        // Clear seed data from migrations so tests start with empty tables.
        sqlx::query("TRUNCATE jobs, law_entries CASCADE")
            .execute(&pool)
            .await
            .unwrap();

        Self {
            pool,
            _container: shared,
        }
    }
}
