use std::collections::{HashMap, HashSet};
use std::env;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::middleware as axum_middleware;
use axum::routing::{MethodRouter, get};
use tokio::sync::{Mutex, RwLock};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tower_sessions::ExpiredDeletion;
use tower_sessions::Expiry;
use tower_sessions::SessionManagerLayer;
use tower_sessions_memory_store::MemoryStore;
use tower_sessions_sqlx_store::PostgresStore;

use regelrecht_editor_api::state::{self, AppState, CorpusState};
use regelrecht_editor_api::{
    config, corpus_handlers, github_oauth, health, middleware, router, static_cache,
    traject_corpus, traject_integrity,
};

#[tokio::main]
async fn main() {
    // The editor is the service that wants the write/build-path latency
    // breakdown, so it opts *itself* into span-close timing logs (passing
    // `true`), while the shared subscriber leaves them off by default for
    // the hot-path workers. `LOG_SPAN_EVENTS` still overrides at runtime.
    regelrecht_shared::telemetry::init_subscriber_with_spans("info", true);
    regelrecht_auth::install_crypto_provider();

    // Before anything slow: a SIGTERM during startup exits at once, one
    // after it drains the server (see `health`).
    let readiness = health::Readiness::default();
    let stop_signal = health::stop_signal().unwrap_or_else(|e| {
        tracing::error!(error = %e, "failed to install the stop signal handlers");
        std::process::exit(1);
    });
    let stop_signal = health::watch_stop(stop_signal, readiness.clone());

    let app_config = config::AppConfig::from_env();

    // --- OIDC discovery (conditional) ---
    let (oidc_client, end_session_url) = if let Some(ref oidc_config) = app_config.oidc {
        match regelrecht_auth::discover_client(oidc_config).await {
            Ok(result) => (Some(Arc::new(result.client)), result.end_session_url),
            Err(e) => {
                tracing::error!(error = %e, "OIDC discovery failed");
                std::process::exit(1);
            }
        }
    } else {
        (None, None)
    };

    // --- HTTP client for OIDC token exchange ---
    let http_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to build HTTP client");
            std::process::exit(1);
        });

    // --- Corpus init ---
    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    let favorites = load_favorites(&static_dir);
    let corpus_state = init_corpus().await;

    let hostname = env::var("HOSTNAME").ok();
    let pipeline_api_url =
        resolve_pipeline_api_url(hostname.as_deref(), env::var("PIPELINE_API_URL").ok());
    let harvest_admin_url =
        resolve_harvest_admin_url(hostname.as_deref(), env::var("HARVEST_ADMIN_URL").ok());
    let hostname_log = hostname.as_deref().unwrap_or("<none>");
    match &pipeline_api_url {
        Some(url) => {
            tracing::info!(url = %url, hostname = %hostname_log, "pipeline-api proxy target")
        }
        None => tracing::info!(
            hostname = %hostname_log,
            "no pipeline-api URL configured, harvest proxy disabled"
        ),
    }
    match &harvest_admin_url {
        Some(url) => {
            tracing::info!(url = %url, hostname = %hostname_log, "harvester-admin proxy target")
        }
        None => tracing::info!(
            hostname = %hostname_log,
            "no harvester-admin URL configured, harvest-admin proxy disabled"
        ),
    }

    // The pool has to be created BEFORE `app_state` is built, because
    // `router()` hands each gate a snapshot of `app_state` (the
    // `route_layer(from_fn_with_state(state.clone(), …))` calls in
    // router.rs) — anything we mutate on the outer binding after that point
    // is invisible to the middleware.
    // Migrations + session-store setup still live in the auth-enabled
    // branch further down; here we only spin up the connection.
    let pool: Option<sqlx::PgPool> = if app_config.is_auth_enabled() {
        let database_url = env::var("DATABASE_URL")
            .or_else(|_| env::var("DATABASE_SERVER_FULL"))
            .unwrap_or_else(|_| {
                tracing::error!(
                    "DATABASE_URL is required when OIDC is enabled (for session storage)"
                );
                std::process::exit(1);
            });
        Some(
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!(error = %e, "failed to connect to database");
                    std::process::exit(1);
                }),
        )
    } else {
        None
    };

    let app_state = AppState {
        corpus: Arc::new(RwLock::new(corpus_state)),
        oidc_client,
        end_session_url,
        config: Arc::new(app_config),
        http_client,
        pool: pool.clone(),
        pipeline_api_url,
        harvest_admin_url,
        reload_lock: Arc::new(tokio::sync::Mutex::new(())),
        trajects: Arc::new(traject_corpus::TrajectCorpusCache::new()),
        integrity: Arc::new(traject_integrity::IntegrityCache::new()),
        readiness: readiness.clone(),
    };

    // The favourites' bodies are fetched in the background, and startup does
    // not wait for them: they took most of the startup time, and until they
    // are in, a favourite is fetched on first read like any other indexed
    // regelwerk.
    tokio::spawn(load_favorite_bodies(app_state.clone(), favorites));

    let index_file = PathBuf::from(&static_dir).join("index.html");

    // --- Serve the router behind a session layer ---
    // SessionManagerLayer is generic over the store type, so the two stores
    // get a branch each; the routes are the same `router()` in both.
    if app_state.config.is_auth_enabled() {
        // Pool was created above app_state so the route_layer middleware
        // sees a populated app_state.pool. The outer `is_auth_enabled`
        // branch above already returned `Some`, but we still take the
        // safe path here so a refactor that breaks the invariant fails
        // with a clear error instead of a panic.
        let Some(pool) = pool else {
            tracing::error!("pool was not created despite auth being enabled");
            std::process::exit(1);
        };

        // The editor shares a database with the pipeline; ensure_schema runs
        // all pipeline migrations (including 0008_user_favorites and 0014
        // trajects). If the services are ever split to separate databases
        // this should be replaced with an editor-specific migration runner.
        if let Err(e) = regelrecht_pipeline::ensure_schema(&pool).await {
            tracing::error!(error = %e, "database migration failed");
            std::process::exit(1);
        }

        let session_store = PostgresStore::new(pool);
        if let Err(e) = session_store.migrate().await {
            tracing::error!(error = %e, "failed to create session table");
            std::process::exit(1);
        }
        tracing::info!("session store ready (PostgreSQL-backed)");

        let deletion_handle = tokio::task::spawn(
            session_store
                .clone()
                .continuously_delete_expired(tokio::time::Duration::from_secs(60)),
        );

        // Drop Secure only for http localhost origins so Safari completes the
        // OIDC handshake; production (https BASE_URL) keeps it. See
        // `is_http_localhost` for the full reasoning.
        let secure_cookie = !is_http_localhost(app_state.config.base_url.as_deref());
        if !secure_cookie {
            tracing::warn!(
                "session cookie Secure flag is OFF — BASE_URL is an http localhost origin \
                 (local SSO dev). This must never happen in production."
            );
        }

        let session_layer = SessionManagerLayer::new(session_store)
            .with_expiry(Expiry::OnInactivity(time::Duration::hours(8)))
            .with_same_site(tower_sessions::cookie::SameSite::Lax)
            .with_http_only(true)
            .with_secure(secure_cookie);

        let app = router(app_state)
            .layer(session_layer)
            .layer(TraceLayer::new_for_http());
        let app = serve_static_and_secure(app, &static_dir, &index_file);

        serve(app, stop_signal, readiness, Some(deletion_handle)).await;
    } else {
        // No .with_secure(true) — this branch only runs when OIDC is disabled
        // (local development over plain HTTP). In production OIDC is always
        // enabled and the auth-enabled branch above sets secure cookies.
        let session_layer = SessionManagerLayer::new(MemoryStore::default())
            .with_same_site(tower_sessions::cookie::SameSite::Lax)
            .with_http_only(true);

        let app = router(app_state)
            .layer(session_layer)
            .layer(TraceLayer::new_for_http());
        let app = serve_static_and_secure(app, &static_dir, index_file);

        serve(app, stop_signal, readiness, None).await;
    }
}

/// Serve `app` on port 8000 until a stop signal, then drain (see
/// [`health::drain_on`]). Exits the process when the listener cannot bind,
/// the server fails, or the session deletion task stops.
async fn serve(
    app: Router,
    stop_signal: impl Future<Output = health::StopSignal> + Send + 'static,
    readiness: health::Readiness,
    deletion_handle: Option<
        tokio::task::JoinHandle<Result<(), tower_sessions::session_store::Error>>,
    >,
) {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to bind on {addr}");
            std::process::exit(1);
        });

    if !readiness.start_serving() {
        // A stop signal came first; its task is exiting the process.
        return;
    }
    let shutdown = health::drain_on(stop_signal, health::DRAIN_DELAY);
    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown);

    let result = match deletion_handle {
        Some(deletion_handle) => tokio::select! {
            result = server => result,
            result = deletion_handle => {
                match result {
                    Ok(Ok(())) => tracing::error!("session deletion task exited unexpectedly"),
                    Ok(Err(e)) => tracing::error!(error = %e, "session deletion task failed"),
                    Err(e) => tracing::error!(error = %e, "session deletion task panicked"),
                }
                std::process::exit(1);
            }
        },
        None => server.await,
    };
    if let Err(e) = result {
        tracing::error!(error = %e, "server error");
        std::process::exit(1);
    }
    tracing::info!("server stopped");
}

/// Initialize the corpus: load local sources and the GitHub sources marked
/// `load_all`, and hold the rest of each GitHub source by path. The
/// favourites' bodies follow in the background ([`load_favorite_bodies`]).
async fn init_corpus() -> CorpusState {
    let manifest_str =
        env::var("CORPUS_REGISTRY_PATH").unwrap_or_else(|_| "corpus-registry.yaml".to_string());
    let local_str = env::var("CORPUS_REGISTRY_LOCAL_PATH")
        .unwrap_or_else(|_| "corpus-registry.local.yaml".to_string());
    let auth_str = env::var("CORPUS_AUTH_FILE").unwrap_or_else(|_| "corpus-auth.yaml".to_string());
    let manifest_path = PathBuf::from(&manifest_str);
    let local_path = PathBuf::from(&local_str);
    let auth_path = PathBuf::from(&auth_str);

    let registry = if manifest_path.exists() {
        match regelrecht_corpus::CorpusRegistry::load(&manifest_path, Some(&local_path)) {
            Ok(r) => {
                tracing::info!(sources = r.sources().len(), "loaded corpus registry");
                r
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load corpus registry, using empty");
                regelrecht_corpus::CorpusRegistry::empty()
            }
        }
    } else {
        tracing::info!("no corpus-registry.yaml found, corpus endpoints will return empty results");
        regelrecht_corpus::CorpusRegistry::empty()
    };

    let github =
        regelrecht_corpus::GithubSettings::from_env(auth_path.exists().then_some(auth_path));

    let today = regelrecht_shared::dates::today_str();
    let mut source_map = match registry
        .load_favorites_async(&HashSet::new(), &github, &today)
        .await
    {
        Ok(map) => {
            tracing::info!(laws = map.len(), "loaded corpus laws");
            map
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load the GitHub sources, falling back to local-only");
            match registry.load_local_sources(&today) {
                Ok(map) => {
                    tracing::info!(laws = map.len(), "loaded corpus laws (local-only fallback)");
                    map
                }
                Err(e2) => {
                    tracing::warn!(error = %e2, "failed to load local sources");
                    regelrecht_corpus::SourceMap::new(regelrecht_shared::dates::today_str())
                }
            }
        }
    };

    // Corpus juris shows the version in force, whichever source supplies it;
    // priority only settles two sources for the same version (RFC-010).
    // Trajecten keep priority first: their own repo must stay current there.
    source_map.prefer_version_in_force();

    // The rest of each GitHub source by path only, so Corpus juris can find
    // every law in it; a body is fetched when someone opens the law (see
    // `ReadScope::law_yaml`). A source that fails to list is reported on
    // `/api/sources` instead of silently showing fewer laws. After
    // `prefer_version_in_force`, so a law refused for a conflict between two
    // sources is left out rather than kept as a version.
    let index_failures: HashMap<String, String> = registry
        .index_github_sources_into(&mut source_map, &github)
        .await
        .into_iter()
        .map(|f| (f.source_id, f.error))
        .collect();

    let backends = init_backends(&registry, &github).await;

    CorpusState::new(registry, source_map, backends, github, index_failures)
}

/// Fetch the bodies of the favourites into the global corpus, through the
/// same reload `POST /api/corpus/reload` runs. A failure leaves the corpus as
/// it was; the favourites are then fetched on first read, and are not found
/// as implementors until a reload loads them.
async fn load_favorite_bodies(state: AppState, favorites: HashSet<String>) {
    if favorites.is_empty() {
        return;
    }
    let reload_guard = state.reload_lock.lock().await;
    match corpus_handlers::reload_global_corpus(&state, &reload_guard, favorites).await {
        Ok(laws) => tracing::info!(laws, "loaded the favourites' bodies"),
        Err(e) => tracing::warn!(
            error = %e,
            "failed to load the favourites' bodies; they are fetched on first read"
        ),
    }
}

/// Create and initialize backends for each registered source.
///
/// All successfully-initialised backends are registered, including read-only
/// ones (e.g. a local source on a read-only container filesystem). Reads
/// route through the same backends as writes so the editor never has a
/// read/write path mismatch — see [`state::BackendEntry::writable`].
async fn init_backends(
    registry: &regelrecht_corpus::CorpusRegistry,
    github: &regelrecht_corpus::GithubSettings,
) -> HashMap<String, state::BackendEntry> {
    let mut backends = HashMap::new();

    for source in registry.sources() {
        // Explicit decision: resolution follows the source's `strict_auth`
        // flag (via `resolve_source`) instead of hardcoding the
        // legacy-fallback variant this call-site used before. For the global
        // registry that is behaviourally identical — manifest sources carry
        // `strict_auth: false`, so single-PAT deployments that only set
        // `CORPUS_GIT_TOKEN` keep working — but a strict source ever
        // reaching this path now resolves strictly, consistent with the
        // traject backend path in `traject_corpus.rs`.
        let token = github
            .credentials()
            .resolve_source(source)
            .map(regelrecht_corpus::auth::TokenDecision::into_token)
            .unwrap_or_else(|e| {
                tracing::warn!(source_id = %source.id, error = %e, "failed to resolve auth token");
                None
            });

        // When a push token is present, the backend will push commits to the
        // remote repo. This requires authentication on the write endpoints —
        // do NOT enable push tokens without adding auth middleware first.
        match regelrecht_corpus::backend::create_backend(source, token.as_deref(), github) {
            Ok(mut backend) => {
                if let Err(e) = backend.ensure_ready().await {
                    tracing::warn!(
                        source_id = %source.id,
                        error = %e,
                        "backend init failed, skipping registration"
                    );
                    continue;
                }
                let writable = backend.is_writable();
                tracing::info!(
                    source_id = %source.id,
                    writable,
                    "backend ready"
                );
                backends.insert(
                    source.id.clone(),
                    state::BackendEntry {
                        backend: Arc::new(Mutex::new(backend)),
                        writable,
                    },
                );
            }
            Err(e) => {
                tracing::warn!(
                    source_id = %source.id,
                    error = %e,
                    "failed to create backend"
                );
            }
        }
    }

    backends
}

/// Close the router: hang the SPA off the fallback, then wrap everything in
/// the security headers.
///
/// The order is the point. `Router::layer` covers the routes and fallback
/// registered before it, so a `fallback_service` added afterwards answers
/// without any of them — and the fallback is what serves `index.html` and
/// the bundle, the only responses a browser renders as a document. Doing
/// both steps here keeps the two calls from drifting apart in the two
/// (auth-enabled / auth-disabled) router branches.
fn serve_static_and_secure(
    app: Router,
    static_dir: &str,
    index_file: impl AsRef<std::path::Path>,
) -> Router {
    app.fallback_service(static_service(static_dir, index_file))
        .layer(axum_middleware::from_fn(middleware::security_headers(
            middleware::EDITOR_CSP,
        )))
}

/// Static-file service for the built editor frontend.
///
/// The editor image has no nginx in front of it — this binary is the web
/// server for `frontend/dist` as well as the API — so compression has to
/// happen here or not at all.
///
/// `precompressed_br` / `precompressed_gzip` make `ServeDir` look for
/// `foo.js.br` and `foo.js.gz` beside `foo.js` and serve one of those when
/// the client's `Accept-Encoding` allows it. The variants are written at
/// build time by `frontend/scripts/precompress.mjs`, so the container spends
/// no CPU compressing per request, and a client that accepts neither still
/// gets the plain file. `ServeDir` sets `Vary: accept-encoding` itself once a
/// precompressed variant is configured, so shared caches stay correct.
///
/// Which encoding wins is the client's call, not ours: `ServeDir` picks the
/// highest q-value from `Accept-Encoding` and breaks ties in favour of
/// brotli. Builder order here is cosmetic. Note that a future
/// `precompressed_zstd()` would outrank brotli in that tie-break — worth
/// knowing before adding one.
///
/// Compression stops at the static files on purpose: the JSON API keeps
/// serving uncompressed bodies. See the SPA fallback below for why the
/// index gets the same treatment.
///
/// Compression makes the bytes smaller; the `Cache-Control` added on the
/// way out ([`static_cache`]) stops most of them being sent a second time
/// at all. That header has to be set per request, because the policy
/// depends on the URL — hence the wrapping handler rather than a plain
/// `SetResponseHeaderLayer`. `ServeDir` sets no `Cache-Control` of its
/// own, and the ETag and `Vary` it does set are left untouched: they are
/// what makes the `no-cache` half cheap.
fn static_service(static_dir: &str, index_file: impl AsRef<std::path::Path>) -> MethodRouter {
    // The SPA fallback (`/library/...`, `/editor/...` deep links) returns
    // index.html, which is itself precompressed — hence the same flags on the
    // not-found service.
    let index = ServeFile::new(index_file)
        .precompressed_br()
        .precompressed_gzip();
    let files = ServeDir::new(static_dir)
        .precompressed_br()
        .precompressed_gzip()
        .not_found_service(index);

    get(move |request: axum::extract::Request| {
        let files = files.clone();
        async move {
            let uri = request.uri().clone();
            // `ServeDir`'s error type is `Infallible` once a not-found
            // service is set, so this cannot fail.
            let mut response = files
                .oneshot(request)
                .await
                .unwrap_or_else(|e| match e {})
                .map(axum::body::Body::new);
            static_cache::apply(&uri, &mut response);
            response
        }
    })
}

/// Read favorites.json from the static directory.
fn load_favorites(static_dir: &str) -> HashSet<String> {
    let path = PathBuf::from(static_dir).join("favorites.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Vec<String>>(&content) {
            Ok(ids) => {
                tracing::info!(count = ids.len(), "loaded favorites");
                ids.into_iter().collect()
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to parse favorites.json");
                HashSet::new()
            }
        },
        Err(_) => {
            tracing::info!("no favorites.json found");
            HashSet::new()
        }
    }
}

/// Resolve the pipeline-api URL, preferring pod HOSTNAME over environment
/// override.
///
/// Priority: `HOSTNAME` → `PIPELINE_API_URL` → `None`.
///
/// HOSTNAME wins because ZAD's alias-based env injection is resolved at
/// component-creation time and gets stuck with stale values when the
/// pipelineapi component is renamed or its port changes. The pod hostname
/// is bound by Kubernetes and also cannot be inherited via ZAD's
/// `clone-from`, so it is the only reliable "which deployment am I?" signal
/// — matching the pattern `regelrecht_corpus::deployment_from_hostname`
/// uses for corpus branch resolution (see PR #574 for the corpus rationale).
///
/// `PIPELINE_API_URL` remains as an explicit override for local dev where
/// HOSTNAME doesn't match the `{deployment}-{component}-{rs}-{pod}` shape.
///
/// Edge case: a dev machine whose HOSTNAME happens to match that shape *and*
/// starts with `regelrecht` or `pr<N>` would silently derive a cluster-internal
/// URL that won't resolve locally. `deployment_from_hostname`'s whitelist is
/// the sole guard here; the old `KUBERNETES_SERVICE_HOST` gate is gone.
fn resolve_pipeline_api_url(hostname: Option<&str>, env_url: Option<String>) -> Option<String> {
    hostname
        .and_then(regelrecht_corpus::deployment_from_hostname)
        .map(|deployment| format!("http://{deployment}-pipelineapi:8000"))
        .or(env_url)
}

/// Resolve the standalone harvester-admin API URL, preferring pod HOSTNAME over
/// an environment override — same rationale and priority as
/// [`resolve_pipeline_api_url`] (`HOSTNAME` → `HARVEST_ADMIN_URL` → `None`).
///
/// The harvester-admin service is deployed under ZAD component name
/// `harvester-admin`, so the in-cluster service is `{deployment}-harvester-admin`.
/// `HARVEST_ADMIN_URL` is the explicit override for local dev where HOSTNAME
/// doesn't match the `{deployment}-{component}-{rs}-{pod}` shape.
fn resolve_harvest_admin_url(hostname: Option<&str>, env_url: Option<String>) -> Option<String> {
    hostname
        .and_then(regelrecht_corpus::deployment_from_hostname)
        .map(|deployment| format!("http://{deployment}-harvester-admin:8000"))
        .or(env_url)
}

// `is_http_localhost` (drop the cookie `Secure` flag only for http-localhost
// local dev) lives in `github_oauth` — the token cookie there and the session
// cookie here must make the same call. Its exhaustive tests stay below.
use github_oauth::is_http_localhost;

#[cfg(test)]
mod pipeline_api_url_tests {
    use super::resolve_pipeline_api_url;

    #[test]
    fn prod_pod_hostname_derives_regelrecht_pipelineapi() {
        let url = resolve_pipeline_api_url(Some("regelrecht-editor-abc-xyz"), None);
        assert_eq!(url.as_deref(), Some("http://regelrecht-pipelineapi:8000"));
    }

    #[test]
    fn pr_preview_pod_hostname_derives_pr_pipelineapi() {
        let url = resolve_pipeline_api_url(Some("pr123-editor-abc-xyz"), None);
        assert_eq!(url.as_deref(), Some("http://pr123-pipelineapi:8000"));
    }

    /// Regression test: even with a stale `PIPELINE_API_URL` shadowing the
    /// resolution (e.g. ZAD alias injection leftover pointing at an old
    /// `pipelineapi-pr552:8001`), HOSTNAME must still win.
    #[test]
    fn hostname_wins_over_stale_env_var() {
        let url = resolve_pipeline_api_url(
            Some("regelrecht-editor-abc-xyz"),
            Some("http://pipelineapi-pr552:8001".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://regelrecht-pipelineapi:8000"));
    }

    #[test]
    fn dev_hostname_falls_back_to_env_override() {
        let url = resolve_pipeline_api_url(
            Some("tim-laptop"),
            Some("http://localhost:8001".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://localhost:8001"));
    }

    /// A non-whitelisted pod-shaped hostname (≥3 hyphens but first segment is
    /// not `regelrecht` or `pr<N>`) must not be trusted as a deployment name —
    /// `deployment_from_hostname` returns `None` and we fall through to the
    /// env override. Documents the whitelist boundary explicitly.
    #[test]
    fn non_whitelisted_pod_hostname_falls_back_to_env() {
        let url = resolve_pipeline_api_url(
            Some("feature-editor-abc-xyz"),
            Some("http://localhost:8001".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://localhost:8001"));
    }

    #[test]
    fn no_hostname_and_no_env_returns_none() {
        assert!(resolve_pipeline_api_url(None, None).is_none());
    }

    #[test]
    fn no_hostname_uses_env_override() {
        let url = resolve_pipeline_api_url(None, Some("http://localhost:8001".to_string()));
        assert_eq!(url.as_deref(), Some("http://localhost:8001"));
    }
}

#[cfg(test)]
mod harvest_admin_url_tests {
    use super::resolve_harvest_admin_url;

    #[test]
    fn prod_pod_hostname_derives_regelrecht_harvester_admin() {
        let url = resolve_harvest_admin_url(Some("regelrecht-editor-abc-xyz"), None);
        assert_eq!(
            url.as_deref(),
            Some("http://regelrecht-harvester-admin:8000")
        );
    }

    #[test]
    fn pr_preview_pod_hostname_derives_pr_harvester_admin() {
        let url = resolve_harvest_admin_url(Some("pr123-editor-abc-xyz"), None);
        assert_eq!(url.as_deref(), Some("http://pr123-harvester-admin:8000"));
    }

    #[test]
    fn hostname_wins_over_stale_env_var() {
        let url = resolve_harvest_admin_url(
            Some("regelrecht-editor-abc-xyz"),
            Some("http://harvester-admin-pr552:8001".to_string()),
        );
        assert_eq!(
            url.as_deref(),
            Some("http://regelrecht-harvester-admin:8000")
        );
    }

    #[test]
    fn dev_hostname_falls_back_to_env_override() {
        let url = resolve_harvest_admin_url(
            Some("tim-laptop"),
            Some("http://localhost:8000".to_string()),
        );
        assert_eq!(url.as_deref(), Some("http://localhost:8000"));
    }

    #[test]
    fn no_hostname_and_no_env_returns_none() {
        assert!(resolve_harvest_admin_url(None, None).is_none());
    }
}

#[cfg(test)]
mod http_localhost_tests {
    use super::is_http_localhost;

    #[test]
    fn http_localhost_with_port_is_local() {
        assert!(is_http_localhost(Some("http://localhost:7300")));
    }

    #[test]
    fn http_localhost_bare_is_local() {
        assert!(is_http_localhost(Some("http://localhost")));
    }

    #[test]
    fn http_loopback_ipv4_is_local() {
        assert!(is_http_localhost(Some("http://127.0.0.1:7300")));
    }

    #[test]
    fn http_loopback_ipv6_is_local() {
        assert!(is_http_localhost(Some("http://[::1]:7300")));
    }

    #[test]
    fn https_localhost_is_not_local() {
        // https already lets Safari accept Secure cookies — keep them Secure.
        assert!(!is_http_localhost(Some("https://localhost:7300")));
    }

    #[test]
    fn production_https_origin_is_not_local() {
        assert!(!is_http_localhost(Some(
            "https://editor.regelrecht.rijks.app"
        )));
    }

    #[test]
    fn missing_base_url_defaults_to_secure() {
        assert!(!is_http_localhost(None));
    }

    /// A hostname that merely starts with `localhost` must not be treated as
    /// local — the host is matched exactly, not by prefix.
    #[test]
    fn localhost_lookalike_host_is_not_local() {
        assert!(!is_http_localhost(Some(
            "http://localhost.attacker.example"
        )));
    }

    /// `localhost` in the userinfo position is not the host — the real host is
    /// `evil.com`, so this must not be treated as local.
    #[test]
    fn localhost_in_userinfo_is_not_local() {
        assert!(!is_http_localhost(Some("http://localhost@evil.com:7300")));
    }

    /// An unparseable BASE_URL fails closed (Secure stays on).
    #[test]
    fn unparseable_base_url_defaults_to_secure() {
        assert!(!is_http_localhost(Some("not a url")));
    }
}

/// The editor image serves its own static files, so the only place
/// compression can happen is `static_service`. These tests pin that the
/// precompressed variants written by `frontend/scripts/precompress.mjs` are
/// actually picked up, and that a client which cannot decompress still gets
/// the plain file.
#[cfg(test)]
mod static_service_tests {
    use super::{static_cache, static_service};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const PLAIN: &[u8] = b"plain bytes";
    const GZIPPED: &[u8] = b"pretend gzip bytes";
    const BROTLIED: &[u8] = b"pretend brotli bytes";

    /// A static dir shaped like a real build: a content-hashed
    /// `assets/app-DkX9a2Bc.js` with both variants beside it, plus the
    /// `index.html` that sits at a fixed name and doubles as the SPA
    /// fallback. Both halves are present on purpose — the caching tests
    /// below are only meaningful if the fixture can tell them apart.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("assets")).expect("mkdir");
        let asset = dir.path().join("assets").join("app-DkX9a2Bc.js");
        std::fs::write(&asset, PLAIN).expect("write");
        std::fs::write(asset.with_extension("js.gz"), GZIPPED).expect("write");
        std::fs::write(asset.with_extension("js.br"), BROTLIED).expect("write");
        std::fs::write(dir.path().join("index.html"), PLAIN).expect("write");
        std::fs::write(dir.path().join("index.html.br"), BROTLIED).expect("write");
        dir
    }

    /// Path of the hashed asset in [`fixture`].
    const ASSET: &str = "/assets/app-DkX9a2Bc.js";

    async fn get(dir: &tempfile::TempDir, path: &str, accept_encoding: Option<&str>) -> Response {
        let static_dir = dir.path().to_string_lossy().to_string();
        let index = dir.path().join("index.html");
        let mut request = Request::builder().uri(path);
        if let Some(encoding) = accept_encoding {
            request = request.header("accept-encoding", encoding);
        }
        static_service(&static_dir, index)
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response")
    }

    type Response = axum::http::Response<Body>;

    async fn body(response: Response) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes()
            .to_vec()
    }

    /// Browsers send a bare `gzip, deflate, br`, and brotli wins that tie —
    /// the smaller encoding, which is what makes registering both worthwhile.
    #[tokio::test]
    async fn brotli_wins_when_the_client_accepts_both() {
        let dir = fixture();
        let response = get(&dir, ASSET, Some("gzip, deflate, br")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-encoding"], "br");
        assert_eq!(response.headers()["vary"], "accept-encoding");
        assert_eq!(body(response).await, BROTLIED);
    }

    /// q-values are the client's, not ours: a client that explicitly ranks
    /// gzip above brotli gets gzip.
    #[tokio::test]
    async fn client_q_values_decide_over_our_preference() {
        let dir = fixture();
        let response = get(&dir, ASSET, Some("br;q=0.5, gzip;q=1.0")).await;

        assert_eq!(response.headers()["content-encoding"], "gzip");
        assert_eq!(body(response).await, GZIPPED);
    }

    #[tokio::test]
    async fn gzip_is_served_when_brotli_is_not_accepted() {
        let dir = fixture();
        let response = get(&dir, ASSET, Some("gzip")).await;

        assert_eq!(response.headers()["content-encoding"], "gzip");
        assert_eq!(body(response).await, GZIPPED);
    }

    /// The ETag is taken from the variant actually opened, so it must differ
    /// per encoding — otherwise a revalidation could swap a gzip body in
    /// under a brotli client's cached entry.
    #[tokio::test]
    async fn etag_differs_per_encoding() {
        let dir = fixture();
        let br = get(&dir, ASSET, Some("br")).await;
        let gz = get(&dir, ASSET, Some("gzip")).await;

        assert_ne!(
            br.headers().get("etag").expect("etag on br"),
            gz.headers().get("etag").expect("etag on gzip"),
        );
    }

    /// A HEAD must advertise the length of the body a GET would return, or a
    /// client sizing the download gets the uncompressed number.
    #[tokio::test]
    async fn head_reports_the_compressed_length() {
        let dir = fixture();
        let static_dir = dir.path().to_string_lossy().to_string();
        let index = dir.path().join("index.html");
        let request = Request::builder()
            .method("HEAD")
            .uri(ASSET)
            .header("accept-encoding", "br")
            .body(Body::empty())
            .expect("request");
        let response = static_service(&static_dir, index)
            .oneshot(request)
            .await
            .expect("response");

        assert_eq!(response.headers()["content-encoding"], "br");
        assert_eq!(
            response.headers()["content-length"],
            BROTLIED.len().to_string()
        );
    }

    /// The uncompressed file stays on disk and stays reachable: a client
    /// without `Accept-Encoding` must not be handed a body it cannot read.
    #[tokio::test]
    async fn plain_file_is_served_without_accept_encoding() {
        let dir = fixture();
        let response = get(&dir, ASSET, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("content-encoding").is_none());
        assert_eq!(body(response).await, PLAIN);
    }

    /// `Vary` must be present even on the uncompressed answer, or a shared
    /// cache could hand the plain body to a client that asked for brotli
    /// (and vice versa).
    #[tokio::test]
    async fn vary_is_set_on_the_uncompressed_answer_too() {
        let dir = fixture();
        let response = get(&dir, ASSET, None).await;

        assert_eq!(response.headers()["vary"], "accept-encoding");
    }

    /// Deep links like `/library/foo` are SPA routes with no file behind
    /// them; they fall through to index.html, which is precompressed as well.
    ///
    /// The 404 is pre-existing and deliberately left alone here:
    /// `not_found_service` is `fallback` wrapped in `SetStatus(404)`, so the
    /// editor has always answered deep links with the index body under a 404
    /// status. The browser runs the SPA regardless. Changing that is a
    /// separate decision from compressing the body.
    #[tokio::test]
    async fn spa_fallback_serves_the_precompressed_index() {
        let dir = fixture();
        let response = get(&dir, "/library/some-law", Some("br")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["content-encoding"], "br");
        assert_eq!(body(response).await, BROTLIED);
    }

    /// A file with no precompressed variant beside it (fonts, images —
    /// deliberately skipped by the build step) is served as-is.
    #[tokio::test]
    async fn file_without_variants_is_served_as_is() {
        let dir = fixture();
        std::fs::write(dir.path().join("font.woff2"), PLAIN).expect("write");
        let response = get(&dir, "/font.woff2", Some("gzip, br")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("content-encoding").is_none());
        assert_eq!(body(response).await, PLAIN);
    }

    /// The caching half of the story, end to end through the real service.
    ///
    /// Both cases are asserted together because either one alone proves
    /// nothing: "assets get a year" is satisfied by a blanket header on
    /// everything, and "the index revalidates" is satisfied by no header
    /// at all. It is the *difference* between the two that is the policy.
    #[tokio::test]
    async fn hashed_assets_are_immutable_and_the_index_is_not() {
        let dir = fixture();

        let asset = get(&dir, ASSET, Some("br")).await;
        assert_eq!(asset.status(), StatusCode::OK);
        assert_eq!(asset.headers()["cache-control"], static_cache::IMMUTABLE);

        let index = get(&dir, "/index.html", Some("br")).await;
        assert_eq!(index.status(), StatusCode::OK);
        assert_eq!(index.headers()["cache-control"], static_cache::REVALIDATE);
    }

    /// The SPA fallback is the index under another URL, so it must carry
    /// the index's policy — otherwise a visitor caches a deep link with
    /// yesterday's HTML in it.
    #[tokio::test]
    async fn the_spa_fallback_revalidates() {
        let dir = fixture();
        let response = get(&dir, "/library/some-law", Some("br")).await;

        assert_eq!(
            response.headers()["cache-control"],
            static_cache::REVALIDATE
        );
    }

    /// A request for an asset that is not on disk is answered with the
    /// index HTML. Marking that immutable would pin a broken page under
    /// a bundle URL for a year.
    #[tokio::test]
    async fn a_missing_asset_is_not_cached_as_one() {
        let dir = fixture();
        let response = get(&dir, "/assets/gone-Zz9YxW01.js", Some("br")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()["cache-control"],
            static_cache::REVALIDATE
        );
    }

    /// Caching sits beside compression, it does not replace it: the ETag
    /// and `Vary` that make the `no-cache` half cheap must survive.
    #[tokio::test]
    async fn caching_leaves_the_compression_headers_alone() {
        let dir = fixture();
        let response = get(&dir, ASSET, Some("br")).await;

        assert_eq!(response.headers()["content-encoding"], "br");
        assert_eq!(response.headers()["vary"], "accept-encoding");
        assert!(response.headers().get("etag").is_some());
        assert_eq!(response.headers()["cache-control"], static_cache::IMMUTABLE);
    }

    /// Files at the root of `dist` that are not the index — the icon,
    /// `favorites.json` — sit at fixed names too and must revalidate.
    #[tokio::test]
    async fn fixed_name_root_files_revalidate() {
        let dir = fixture();
        std::fs::write(dir.path().join("favorites.json"), PLAIN).expect("write");
        let response = get(&dir, "/favorites.json", None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["cache-control"],
            static_cache::REVALIDATE
        );
    }
}

/// Proves the editor ships the headers on the responses that matter. A test
/// that only hits `/health` cannot: the JSON routes kept their headers the
/// whole time the rendered document had none.
#[cfg(test)]
mod security_header_tests {
    use super::{middleware, serve_static_and_secure};
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    /// The router shape both production branches end in: some API routes,
    /// then `serve_static_and_secure` over a `dist`-like directory.
    fn app(dir: &tempfile::TempDir) -> Router {
        let static_dir = dir.path().to_string_lossy().to_string();
        let index = dir.path().join("index.html");
        let routes = Router::new().route("/health", get(|| async { "OK" }));
        serve_static_and_secure(routes, &static_dir, index)
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("assets")).expect("mkdir");
        std::fs::write(dir.path().join("assets").join("app-DkX9a2Bc.js"), b"js").expect("write");
        std::fs::write(dir.path().join("index.html"), b"<!doctype html>").expect("write");
        dir
    }

    async fn get_path(dir: &tempfile::TempDir, path: &str) -> axum::http::Response<Body> {
        app(dir)
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response")
    }

    /// `/` and a deep link both resolve through the fallback, which is the
    /// half that used to escape the middleware. A deep link answers with the
    /// SPA index under a 404 (see [`static_cache`]); the headers are what
    /// this asserts, not the status.
    #[tokio::test]
    async fn every_response_carries_the_headers() {
        let dir = fixture();
        for path in [
            "/health",
            "/",
            "/library/wet_op_de_zorgtoeslag",
            "/index.html",
        ] {
            let response = get_path(&dir, path).await;
            let headers = response.headers();
            assert_eq!(
                headers["content-security-policy"],
                middleware::EDITOR_CSP,
                "no CSP on {path}"
            );
            assert_eq!(headers["x-frame-options"], "DENY", "{path}");
            assert_eq!(headers["x-content-type-options"], "nosniff", "{path}");
            assert_eq!(
                headers["referrer-policy"], "strict-origin-when-cross-origin",
                "{path}"
            );
            assert!(headers.contains_key("permissions-policy"), "{path}");
            assert!(headers.contains_key("strict-transport-security"), "{path}");
        }
    }

    /// The static layer sets `Cache-Control` from inside the fallback; the
    /// header layer wraps it from outside. Neither may erase the other.
    #[tokio::test]
    async fn the_headers_do_not_displace_the_cache_policy() {
        let dir = fixture();
        let response = get_path(&dir, "/assets/app-DkX9a2Bc.js").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["cache-control"],
            super::static_cache::IMMUTABLE
        );
        assert_eq!(
            response.headers()["content-security-policy"],
            middleware::EDITOR_CSP
        );
    }

    /// A miss inside `/assets/` falls through to the SPA index — a rendered
    /// document under an unexpected status, so it needs the headers too.
    #[tokio::test]
    async fn the_spa_fallback_response_is_covered() {
        let dir = fixture();
        let response = get_path(&dir, "/assets/gone-Zz9YxW01.js").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers()["content-security-policy"],
            middleware::EDITOR_CSP
        );
    }
}
