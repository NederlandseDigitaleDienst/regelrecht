//! A Postgres for tests: a fresh database per call, on one server per test
//! binary. Behind the `test-utils` feature.
//!
//! `TEST_POSTGRES_URL` points at a server that is already running (a
//! `services:` block in CI, or a container someone started by hand). Without
//! it the first call starts a container, so a plain `cargo test` keeps working
//! without setup.
//!
//! The container is shared per test binary, together with the connection
//! details resolved once. A container per test costs a container start per
//! test, and resolving the published port per test had thirty tests
//! inspecting the daemon at once, which fails with EAGAIN and reads as a test
//! failure. It is held as a `Weak`, so it stops when the last
//! [`TestDatabase`] in the binary drops; a `static` is never dropped, so an
//! owning handle there would leave the container running after the process
//! exits.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Weak};

use sqlx::PgPool;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;

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
            // lets those setups point at the right host.
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

/// An empty database of its own, on the shared test server. Keep it alive as
/// long as the test uses the database: the server stops with the last one.
pub struct TestDatabase {
    url: String,
    _server: Arc<SharedPostgres>,
}

impl TestDatabase {
    /// A new, empty database.
    ///
    /// # Panics
    ///
    /// When no server can be started or reached, or the database cannot be
    /// created: there is no test to run then.
    #[allow(clippy::unwrap_used)]
    pub async fn create() -> Self {
        let server = shared_postgres().await;
        let name = format!(
            "regelrecht_test_{}_{}",
            std::process::id(),
            DB_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let admin = PgPool::connect(&format!("{}/postgres", server.base_url))
            .await
            .unwrap();
        sqlx::query(&format!(
            "CREATE DATABASE {}",
            super::quote_identifier(&name)
        ))
        .execute(&admin)
        .await
        .unwrap();
        admin.close().await;
        Self {
            url: format!("{}/{name}", server.base_url),
            _server: server,
        }
    }

    /// The URL to connect to it with.
    pub fn url(&self) -> &str {
        &self.url
    }
}
