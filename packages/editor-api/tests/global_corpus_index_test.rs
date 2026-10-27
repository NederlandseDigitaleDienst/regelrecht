//! Corpus juris (the global scope) finds every law of a GitHub source, not
//! only the few whose bodies are loaded at startup. The rest is indexed by
//! path and its body is fetched when someone opens the law. What was loaded
//! stays as it was: a law the same source supplied, a law a local source
//! provides, and a law refused for a conflict are not indexed over.
//!
//! GitHub is played by wiremock through the process-wide `GITHUB_API_BASE`
//! seam, so everything sits in one test: parallel tests with their own mock
//! servers would point each other's clients at the wrong server.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use base64::Engine;
use pretty_assertions::assert_eq;
use tokio::sync::{Mutex, RwLock};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use regelrecht_corpus::dto::PaginationParams;
use regelrecht_corpus::CorpusRegistry;
use regelrecht_editor_api::config::AppConfig;
use regelrecht_editor_api::corpus_handlers::{
    get_corpus_law, get_corpus_law_versions, list_corpus_laws,
};
use regelrecht_editor_api::state::{AppState, BackendEntry, CorpusState};
use regelrecht_editor_api::traject_corpus::TrajectCorpusCache;

// Fictional repositories: a central corpus on GitHub, a second GitHub source
// at the same priority, and a local source above both.
const REPO: &str = "example-org/central-corpus";
const MIRROR_REPO: &str = "example-org/mirror-corpus";
const BRANCH: &str = "main";
const SUBPATH: &str = "regulation/nl";
/// Its body was loaded up front (a favorite); the tree lists a newer version.
const LOADED_LAW: &str = "wet_vooraf_geladen";
/// Only in the tree: held by path, fetched when opened.
const INDEXED_LAW: &str = "wet_alleen_geindexeerd";
/// Also provided by the local source, which hides the GitHub versions.
const LOCAL_LAW: &str = "wet_ook_lokaal";
/// Listed by both GitHub sources at equal priority: a conflict.
const CONFLICT_LAW: &str = "wet_in_twee_bronnen";

fn registry_file(local_dir: &std::path::Path) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        format!(
            r#"
schema_version: "1.0"
sources:
  - id: local
    name: "Lokaal corpus"
    type: local
    local:
      path: {local}
    scopes: []
    priority: 1
  - id: central
    name: "Centraal corpus"
    type: github
    github:
      owner: example-org
      repo: central-corpus
      branch: {BRANCH}
      path: {SUBPATH}
    scopes: []
    priority: 2
    enriched: false
  - id: mirror
    name: "Tweede corpus"
    type: github
    github:
      owner: example-org
      repo: mirror-corpus
      branch: {BRANCH}
      path: {SUBPATH}
    scopes: []
    priority: 2
"#,
            local = local_dir.display(),
        ),
    )
    .unwrap();
    file
}

fn law_path(law_id: &str, date: &str) -> String {
    format!("{SUBPATH}/wet/{law_id}/{date}.yaml")
}

fn law_body(law_id: &str) -> String {
    format!("$id: {law_id}\nname: {law_id}\n")
}

fn tree_entry(law_id: &str, date: &str) -> serde_json::Value {
    serde_json::json!({ "path": law_path(law_id, date), "type": "blob", "sha": format!("blob-{law_id}-{date}") })
}

async fn mount_github(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/git/trees/{BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-central",
            "truncated": false,
            "tree": [
                tree_entry(LOADED_LAW, "2025-01-01"),
                tree_entry(LOADED_LAW, "2025-06-01"),
                tree_entry(INDEXED_LAW, "2025-01-01"),
                tree_entry(LOCAL_LAW, "2025-01-01"),
                tree_entry(CONFLICT_LAW, "2025-01-01"),
            ],
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{MIRROR_REPO}/git/trees/{BRANCH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "sha": "tree-mirror",
            "truncated": false,
            "tree": [tree_entry(CONFLICT_LAW, "2025-01-01")],
        })))
        .mount(server)
        .await;
    // Only the indexed law's body is served, and only once: the law and its
    // versions share the cached body, and a second open is a cache hit.
    let indexed = law_path(INDEXED_LAW, "2025-01-01");
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/contents/{indexed}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "2025-01-01.yaml",
            "path": indexed,
            "sha": "blob-indexed",
            "type": "file",
            "content": base64::engine::general_purpose::STANDARD.encode(law_body(INDEXED_LAW)),
            "encoding": "base64",
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn state_with(corpus: CorpusState) -> AppState {
    AppState {
        corpus: Arc::new(RwLock::new(corpus)),
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
            github_oauth: None,
            task_enrich_provider: "claude".to_string(),
        }),
        http_client: regelrecht_auth::http_client(),
        pool: None,
        pipeline_api_url: None,
        harvest_admin_url: None,
        reload_lock: Arc::new(Mutex::new(())),
        integrity: Default::default(),
        trajects: Arc::new(TrajectCorpusCache::new()),
    }
}

async fn body_text(response: impl IntoResponse) -> String {
    let bytes = axum::body::to_bytes(response.into_response().into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn corpus_juris_finds_and_opens_laws_it_did_not_load_up_front() {
    let server = MockServer::start().await;
    std::env::set_var("GITHUB_API_BASE", server.uri());
    mount_github(&server).await;

    let local_dir = tempfile::tempdir().unwrap();
    let local_law_dir = local_dir.path().join("wet").join(LOCAL_LAW);
    std::fs::create_dir_all(&local_law_dir).unwrap();
    std::fs::write(local_law_dir.join("2025-01-01.yaml"), law_body(LOCAL_LAW)).unwrap();

    let manifest = registry_file(local_dir.path());
    let registry = CorpusRegistry::load(manifest.path(), None).unwrap();
    let source = |id: &str| {
        registry
            .sources()
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .clone()
    };
    let central = source("central");

    // What startup loads before indexing: the local source, and one favorite
    // from the central corpus. Then date-first, then the rest by path, in the
    // same order as `init_corpus`.
    let mut map = regelrecht_corpus::SourceMap::new("2026-01-01");
    map.load_source(&source("local")).unwrap();
    map.load_fetched_file(
        &law_body(LOADED_LAW),
        &law_path(LOADED_LAW, "2025-01-01"),
        Some(SUBPATH),
        &central.id,
        &central.name,
        central.priority,
    )
    .unwrap();
    map.prefer_version_in_force();
    let failures = registry.index_github_sources_into(&mut map, None).await;
    assert!(failures.is_empty(), "index failed: {failures:?}");

    let version_sources = |law_id: &str| -> Vec<String> {
        map.get_law_versions(law_id)
            .unwrap()
            .iter()
            .map(|v| v.source_id.clone())
            .collect()
    };
    // A law the same source already supplied keeps its loaded body, even
    // though the tree's best version for it has a later date.
    assert_eq!(version_sources(LOADED_LAW), vec!["central"]);
    assert!(map.get_law(LOADED_LAW).unwrap().is_loaded());
    // A local copy hides the GitHub versions of that law.
    assert_eq!(version_sources(LOCAL_LAW), vec!["local"]);
    // Two sources at equal priority: the second is refused and left out.
    assert_eq!(version_sources(CONFLICT_LAW), vec!["central"]);
    // The rest is held by path.
    assert!(!map.get_law(INDEXED_LAW).unwrap().is_loaded());

    let mut corpus = CorpusState::empty();
    corpus.backends = HashMap::new();
    for id in ["local", "central"] {
        let mut backend = regelrecht_corpus::backend::create_backend(&source(id), None).unwrap();
        backend.ensure_ready().await.unwrap();
        corpus.backends.insert(
            id.to_string(),
            BackendEntry {
                backend: Arc::new(Mutex::new(backend)),
                writable: false,
            },
        );
    }
    corpus.registry = registry;
    corpus.source_map = map;
    let state = state_with(corpus);

    let list = |query: serde_json::Value| {
        let state = state.clone();
        async move {
            let params: PaginationParams = serde_json::from_value(query).unwrap();
            let mut ids: Vec<String> = list_corpus_laws(State(state), Query(params))
                .await
                .unwrap()
                .0
                .into_iter()
                .map(|l| l.law_id)
                .collect();
            ids.sort();
            ids
        }
    };
    // Search finds the indexed law.
    assert_eq!(
        list(serde_json::json!({ "q": "alleen geindexeerd" })).await,
        vec![INDEXED_LAW]
    );
    // `loaded` leaves out what is held by path only.
    assert_eq!(
        list(serde_json::json!({ "loaded": true })).await,
        vec![LOCAL_LAW, LOADED_LAW]
    );

    // Opening it fetches the body once; the second open and the versions
    // endpoint are served from the cache.
    for _ in 0..2 {
        let response = get_corpus_law(State(state.clone()), Path(INDEXED_LAW.to_string()))
            .await
            .unwrap();
        assert_eq!(body_text(response).await, law_body(INDEXED_LAW));
    }
    let versions = get_corpus_law_versions(State(state.clone()), Path(INDEXED_LAW.to_string()))
        .await
        .unwrap();
    assert_eq!(versions.0, vec![law_body(INDEXED_LAW)]);

    // The loaded law is served from memory, without a GitHub call.
    let response = get_corpus_law(State(state.clone()), Path(LOADED_LAW.to_string()))
        .await
        .unwrap();
    assert_eq!(body_text(response).await, law_body(LOADED_LAW));

    std::env::remove_var("GITHUB_API_BASE");
}
