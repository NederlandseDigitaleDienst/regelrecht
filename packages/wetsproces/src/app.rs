//! What the server is built from in each mode, and the refusals that keep it
//! from starting where it would run unsafely.
//!
//! Test mode runs on [`InMemoryStore`] and a [`ControllableClock`] that
//! starts on the case's `speeldatum`. Stable mode needs a chronicle store
//! that survives a restart and a login in front of the API; this build has
//! neither, so it refuses to start in stable mode rather than fall back to
//! memory without a word. The server has no login at all yet, which is why
//! only test mode starts.

use std::sync::Arc;

use axum::Router;

use crate::api::{AppState, router};
use crate::case::{self, CaseError};
use crate::clock::ControllableClock;
use crate::config::{Config, Mode};
use crate::service::{Service, ServiceError};
use crate::store::memory::InMemoryStore;

/// Why the server does not start.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(
        "stable mode needs a chronicle store that survives a restart (Postgres) and a login in \
         front of the API, and this build has neither yet; start with WETSPROCES_MODE=test"
    )]
    StableNotAvailable,
    #[error("the case: {0}")]
    Case(#[from] CaseError),
    #[error("the service: {0}")]
    Service(#[from] ServiceError),
}

/// The router of the server for `config`.
///
/// # Errors
///
/// [`StartupError::StableNotAvailable`] in stable mode; otherwise when the
/// manifest gives no case to run or the service does not start.
pub async fn build(config: &Config) -> Result<Router, StartupError> {
    match config.mode {
        Mode::Stable => Err(StartupError::StableNotAvailable),
        Mode::Test => {
            let (corpus, messages) = case::load(&config.manifest)?;
            for message in &messages {
                tracing::warn!(%message, "loading the case");
            }
            let clock = Arc::new(ControllableClock::new(corpus.casus.speeldatum));
            tracing::info!(
                case = %corpus.casus.id,
                manifest = %config.manifest.display(),
                today = %corpus.casus.speeldatum,
                "test mode: chronicles in memory, clock set by the API, no login"
            );
            let service = Service::start(
                corpus,
                Arc::new(InMemoryStore::new()),
                Arc::clone(&clock) as _,
            )
            .await?;
            Ok(router(AppState::new(
                Arc::new(service),
                config.mode,
                Some(clock),
            )))
        }
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
            manifest: PathBuf::from(DEFAULT_MANIFEST),
            port: DEFAULT_PORT,
        }
    }

    #[tokio::test]
    async fn stable_mode_refuses_to_start_without_its_store() {
        let e = build(&config(Mode::Stable)).await.unwrap_err();
        assert!(matches!(e, StartupError::StableNotAvailable), "{e}");
        let message = e.to_string();
        assert!(message.contains("Postgres"), "{message}");
        assert!(message.contains("WETSPROCES_MODE=test"), "{message}");
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
