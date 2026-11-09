use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use regelrecht_auth::{ConfiguredClient, OidcAppState, OidcConfig};
use regelrecht_corpus::backend::RepoBackend;
use regelrecht_corpus::SourceMap;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};

use crate::config::AppConfig;
use crate::traject_corpus::TrajectCorpusCache;
use crate::traject_integrity::IntegrityCache;

#[derive(Clone)]
pub struct AppState {
    /// Loaded corpus sources with provenance metadata. Used for
    /// browse-only reads when no traject is active and as the seed for
    /// per-traject corpus configs.
    pub corpus: Arc<RwLock<CorpusState>>,
    pub oidc_client: Option<Arc<ConfiguredClient>>,
    pub end_session_url: Option<String>,
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    /// Database connection pool (available when auth is enabled).
    pub pool: Option<PgPool>,
    /// Base URL of the pipeline-api service (e.g. "http://pipeline-api:8001").
    /// When set, `/api/harvest/*` requests are proxied to this service.
    pub pipeline_api_url: Option<String>,
    /// Base URL of the standalone harvester-admin API (e.g.
    /// "http://regelrecht-harvester-admin:8000"). When set, `/api/harvest-admin/*`
    /// requests are proxied to it, forwarding the caller's session cookie so
    /// the harvester-admin service validates the shared session and enforces
    /// its own harvester-* role gates.
    pub harvest_admin_url: Option<String>,
    /// Single-flight gate for `POST /api/corpus/reload`. The reload fans out
    /// to GitHub (one API call per law source); without this gate an
    /// authenticated user could burn through the 5000 req/hr token quota
    /// with a single `xargs -P` and block corpus reads for everyone.
    pub reload_lock: Arc<Mutex<()>>,
    /// Lazy per-traject corpus cache. When a request carries an active
    /// traject in the session, save handlers route through this cache
    /// instead of [`AppState::corpus`].
    pub trajects: Arc<TrajectCorpusCache>,
    /// Content-addressed memo behind the traject integrity report (see
    /// [`crate::traject_integrity::IntegrityCache`]). Lives here rather than
    /// on the per-traject corpus snapshot because that snapshot is rebuilt
    /// on a TTL, which would throw the memo away and re-read every law body
    /// on the next visit.
    pub integrity: Arc<IntegrityCache>,
}

impl OidcAppState for AppState {
    fn oidc_client(&self) -> Option<&Arc<ConfiguredClient>> {
        self.oidc_client.as_ref()
    }
    fn end_session_url(&self) -> Option<&str> {
        self.end_session_url.as_deref()
    }
    fn oidc_config(&self) -> Option<&OidcConfig> {
        self.config.oidc.as_ref()
    }
    fn is_auth_enabled(&self) -> bool {
        self.config.is_auth_enabled()
    }
    fn base_url(&self) -> Option<&str> {
        self.config.base_url.as_deref()
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

/// A registered backend along with its writability flag, captured at init
/// time after [`RepoBackend::ensure_ready`] (so a local source on a
/// read-only filesystem is recorded as `writable: false`).
///
/// `Clone` is shallow: the clone shares the same backend mutex, which is
/// exactly what the traject index refresh needs (in-flight saves keep
/// serialising on the same lock across a snapshot swap).
#[derive(Clone)]
pub struct BackendEntry {
    pub backend: Arc<Mutex<Box<dyn RepoBackend>>>,
    pub writable: bool,
}

/// State for the corpus subsystem.
pub struct CorpusState {
    pub registry: regelrecht_corpus::CorpusRegistry,
    pub source_map: SourceMap,
    /// Backends keyed by source ID. Read-only backends are also registered
    /// here so reads (`get_scenario`, `list_scenarios`) can route through
    /// the same abstraction as writes — preventing read/write path
    /// mismatches when a fallback writable backend is used.
    pub backends: HashMap<String, BackendEntry>,
    /// Path to corpus-auth.yaml for GitHub authentication during reload.
    pub auth_file: Option<PathBuf>,
    /// Source id → error of the last failed index scan for that source.
    /// Sources absent from this map enumerated fine. Surfaced on
    /// `GET /api/sources` (and the traject variant) as `index_error`, so a
    /// source whose scan failed is distinguishable from a genuinely empty
    /// one — a failing traject writable-own source otherwise reads as
    /// `law_count: 0` while its laws silently fall back to the seed corpus.
    pub index_failures: HashMap<String, String>,
    /// Bodies fetched on demand for laws the map holds by path only, and the
    /// budget those fetches spend. Used by the global scope, where Corpus
    /// juris indexes whole sources and loads a body when someone opens the
    /// law. A traject keeps its own cache on
    /// [`crate::traject_corpus::TrajectCorpus`], next to its post-save overlay.
    pub(crate) lazy_bodies: LazyBodies,
}

/// How many bodies the global scope may fetch from GitHub per hour. Anyone
/// can open a law there, without an account, and each uncached body costs a
/// call on the same service token that trajecten read with (5000 an hour).
/// A crawler walking the corpus would otherwise spend that quota for
/// everyone; past this budget an uncached open answers 503 until the hour is
/// over, while cached bodies keep being served.
pub(crate) const GLOBAL_FETCHES_PER_HOUR: u32 = 1000;

/// See [`CorpusState::lazy_bodies`].
#[derive(Default)]
pub(crate) struct LazyBodies {
    /// Keyed by `{source_id}/{relative_path}` (see [`global_body_key`]):
    /// one version of one law. Emptied on a reload, so upstream changes show.
    pub(crate) cache: tokio::sync::RwLock<crate::traject_corpus::BoundedCache<String>>,
    budget: std::sync::Mutex<FetchBudget>,
}

impl LazyBodies {
    /// Spend one fetch from this hour's budget; `false` when it is used up.
    pub(crate) fn take_fetch(&self) -> bool {
        // A poisoned lock only means another thread panicked mid-count; the
        // counter itself is still usable.
        let mut budget = self
            .budget
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        budget.take(std::time::Instant::now(), GLOBAL_FETCHES_PER_HOUR)
    }
}

/// A fixed one-hour window of fetches.
#[derive(Default)]
struct FetchBudget {
    window_start: Option<std::time::Instant>,
    used: u32,
}

impl FetchBudget {
    fn take(&mut self, now: std::time::Instant, per_hour: u32) -> bool {
        let hour = std::time::Duration::from_secs(3600);
        if self
            .window_start
            .is_none_or(|start| now.duration_since(start) >= hour)
        {
            self.window_start = Some(now);
            self.used = 0;
        }
        if self.used >= per_hour {
            return false;
        }
        self.used += 1;
        true
    }
}

/// The key of a body in [`LazyBodies::cache`]: one version of one law.
pub(crate) fn global_body_key(source_id: &str, relative_path: &str) -> String {
    format!("{source_id}/{relative_path}")
}

impl CorpusState {
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            registry: regelrecht_corpus::CorpusRegistry::empty(),
            source_map: SourceMap::new(regelrecht_shared::dates::today_str()),
            backends: HashMap::new(),
            auth_file: None,
            index_failures: HashMap::new(),
            lazy_bodies: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FetchBudget;
    use std::time::{Duration, Instant};

    #[test]
    fn the_fetch_budget_runs_out_and_refills_after_an_hour() {
        let start = Instant::now();
        let mut budget = FetchBudget::default();
        assert!(budget.take(start, 2));
        assert!(budget.take(start, 2));
        assert!(!budget.take(start + Duration::from_secs(10), 2));
        assert!(budget.take(start + Duration::from_secs(3600), 2));
    }
}
