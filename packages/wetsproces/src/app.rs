//! What the server is built from in each mode, and the refusals that keep it
//! from starting where it would run unsafely.
//!
//! The store and the clock are chosen apart. Test mode runs on a
//! [`ControllableClock`] that starts on the case's `speeldatum`, with the
//! store the configuration names: [`InMemoryStore`] or [`PostgresStore`].
//! Stable mode needs Postgres, the real clock and a login in front of the
//! API; this build has no login, so it refuses to start in stable mode. The
//! server has no login at all yet, which is why only test mode starts.
//!
//! When Postgres is asked for and cannot be reached or migrated, the server
//! does not start: it never falls back to memory.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;

use crate::api::{AppState, router};
use crate::case::{self, CaseError};
use crate::clock::ControllableClock;
use crate::config::{Config, Mode, StoreChoice};
use crate::service::{Service, ServiceError};
use crate::store::memory::InMemoryStore;
use crate::store::postgres::{CONNECT_TIMEOUT, PostgresStore};
use crate::store::{ChronicleStore, StoreError};

/// Why the server does not start.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(
        "stable mode needs a login in front of the API, and this build has none yet; start with \
         WETSPROCES_MODE=test"
    )]
    StableNeedsLogin,
    #[error("the chronicle store ({store}): {source}")]
    Store {
        store: &'static str,
        source: StoreError,
    },
    #[error("the case: {0}")]
    Case(#[from] CaseError),
    #[error("the service: {0}")]
    Service(#[from] ServiceError),
}

/// The router of the server for `config`.
///
/// # Errors
///
/// [`StartupError::StableNeedsLogin`] in stable mode; otherwise when the
/// manifest gives no case to run, the store does not open, or the service
/// does not start.
pub async fn build(config: &Config) -> Result<Router, StartupError> {
    match config.mode {
        Mode::Stable => Err(StartupError::StableNeedsLogin),
        Mode::Test => {
            let (corpus, messages) = case::load(&config.manifest)?;
            for message in &messages {
                tracing::warn!(%message, "loading the case");
            }
            let clock = Arc::new(ControllableClock::new(corpus.casus.speeldatum));
            let store = open_store(&config.store, CONNECT_TIMEOUT).await?;
            tracing::info!(
                case = %corpus.casus.id,
                manifest = %config.manifest.display(),
                today = %corpus.casus.speeldatum,
                store = config.store.name(),
                "test mode: clock set by the API, no login"
            );
            let service = Service::start(corpus, store, Arc::clone(&clock) as _).await?;
            Ok(router(AppState::new(
                Arc::new(service),
                config.mode,
                Some(clock),
            )))
        }
    }
}

/// The store `choice` names, opened; a connection to Postgres may take
/// `timeout`.
///
/// # Errors
///
/// [`StartupError::Store`] when Postgres cannot be reached or migrated.
async fn open_store(
    choice: &StoreChoice,
    timeout: Duration,
) -> Result<Arc<dyn ChronicleStore>, StartupError> {
    match choice {
        StoreChoice::Memory => Ok(Arc::new(InMemoryStore::new())),
        StoreChoice::Postgres { url } => match PostgresStore::connect_within(url, timeout).await {
            Ok(store) => Ok(Arc::new(store)),
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

    use super::*;
    use crate::config::{DEFAULT_MANIFEST, DEFAULT_PORT};

    fn config(mode: Mode) -> Config {
        Config {
            mode,
            store: StoreChoice::Memory,
            manifest: PathBuf::from(DEFAULT_MANIFEST),
            port: DEFAULT_PORT,
        }
    }

    #[tokio::test]
    async fn stable_mode_refuses_to_start_without_a_login() {
        let mut c = config(Mode::Stable);
        c.store = StoreChoice::Postgres {
            url: "postgres://nowhere.invalid/regelrecht".into(),
        };
        let e = build(&c).await.unwrap_err();
        assert!(matches!(e, StartupError::StableNeedsLogin), "{e}");
        let message = e.to_string();
        assert!(message.contains("login"), "{message}");
        assert!(!message.contains("Postgres"), "{message}");
        assert!(message.contains("WETSPROCES_MODE=test"), "{message}");
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
    async fn test_mode_starts_on_postgres() {
        let database = regelrecht_shared::postgres::test_utils::TestDatabase::create().await;
        let mut c = config(Mode::Test);
        c.store = StoreChoice::Postgres {
            url: database.url().to_string(),
        };
        let _router = build(&c).await.unwrap();
    }

    #[tokio::test]
    async fn test_mode_starts_on_the_manifest() {
        let _router = build(&config(Mode::Test)).await.unwrap();
    }

    #[tokio::test]
    async fn a_manifest_without_a_case_does_not_start() {
        let mut c = config(Mode::Test);
        c.manifest = PathBuf::from("/nonexistent/bron.yaml");
        let e = build(&c).await.unwrap_err();
        assert!(matches!(e, StartupError::Case(_)), "{e}");
    }
}
