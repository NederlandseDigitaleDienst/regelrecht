//! Integration tests for `corpus_handlers::list_traject_scenarios` and
//! `corpus_handlers::list_scenarios` (global) — verify that
//! `ScenarioEntry::target_law_ids` is populated from the feature file
//! content on both read paths.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{app_state, empty_state, local_traject, seed_account, traject_ref};

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use tokio::sync::{Mutex, RwLock};
use tower_sessions::Session;
use tower_sessions_memory_store::MemoryStore;

use regelrecht_auth::handlers::SESSION_KEY_SUB;
use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::{list_scenarios, list_traject_scenarios};
use regelrecht_editor_api::state::{BackendEntry, CorpusState};

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_op_de_zorgtoeslag";

// ---------------------------------------------------------------------------
// Helpers — the rest comes from `common`; `sub_only_session` stays local on
// purpose: a session without a verified identity.
// ---------------------------------------------------------------------------

/// Write a minimal but schema-valid law file and a `scenarios/` directory
/// under `corpus_dir/wet/{LAW_ID}/`.
fn write_corpus(corpus_dir: &std::path::Path) {
    let law_dir = corpus_dir.join("wet").join(LAW_ID);
    std::fs::create_dir_all(&law_dir).unwrap();
    std::fs::write(
        law_dir.join("2025-01-01.yaml"),
        format!("$id: {LAW_ID}\nname: Zorgtoeslagwet\n"),
    )
    .unwrap();

    let scenarios_dir = law_dir.join("scenarios");
    std::fs::create_dir_all(&scenarios_dir).unwrap();

    // A scenario that evaluates the law under test.
    std::fs::write(
        scenarios_dir.join("matching.feature"),
        format!("Feature: m\n  Scenario: a\n    When I evaluate \"recht\" of \"{LAW_ID}\"\n"),
    )
    .unwrap();

    // A scenario that evaluates a different law.
    std::fs::write(
        scenarios_dir.join("other.feature"),
        "Feature: o\n  Scenario: b\n    When I evaluate \"x\" of \"andere_wet\"\n",
    )
    .unwrap();

    // A work-in-progress scenario with no execution step.
    std::fs::write(scenarios_dir.join("wip.feature"), "Feature: wip\n").unwrap();
}

/// Build a session that carries `sub` as the authenticated user identity.
/// The active traject is passed in the URL path (traject_ref), not in the
/// session — so no `SESSION_KEY_ACTIVE_TRAJECT` insertion needed.
async fn sub_only_session(sub: &str) -> Session {
    let session = Session::new(None, Arc::new(MemoryStore::default()), None);
    session.insert(SESSION_KEY_SUB, sub).await.unwrap();
    session
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_traject_scenarios_returns_target_law_ids() {
    let db = TestDb::new().await;
    let state = empty_state(db.pool.clone());
    let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;

    let corpus = tempfile::tempdir().unwrap();
    write_corpus(corpus.path());
    let traject = local_traject(&db.pool, owner, corpus.path()).await;

    let account = AccountRecord {
        id: owner,
        person_sub: sub.clone(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    };
    let axum::Json(entries) = list_traject_scenarios(
        State(state),
        Extension(account),
        sub_only_session(&sub).await,
        Path((traject_ref(traject), LAW_ID.to_string())),
        axum::http::HeaderMap::new(),
    )
    .await
    .expect("list_traject_scenarios must succeed");

    let summary: Vec<(String, Vec<String>)> = entries
        .into_iter()
        .map(|e| (e.filename, e.target_law_ids))
        .collect();

    assert_eq!(
        summary,
        vec![
            ("matching.feature".to_string(), vec![LAW_ID.to_string()]),
            ("other.feature".to_string(), vec!["andere_wet".to_string()]),
            ("wip.feature".to_string(), Vec::new()),
        ]
    );
}

#[tokio::test]
async fn list_scenarios_global_returns_target_law_ids() {
    let corpus = tempfile::tempdir().unwrap();
    write_corpus(corpus.path());

    // Registry with a single local source rooted at the temp corpus, the
    // same shape `init_corpus` builds from corpus-registry.yaml.
    let manifest_dir = tempfile::tempdir().unwrap();
    let manifest = manifest_dir.path().join("corpus-registry.yaml");
    std::fs::write(
        &manifest,
        format!(
            "---\nschema_version: '1.0'\nsources:\n  - id: local\n    name: Local\n    \
             type: local\n    local:\n      path: {}\n    scopes: []\n    priority: 1\n",
            corpus.path().display()
        ),
    )
    .unwrap();
    let registry = regelrecht_corpus::CorpusRegistry::load(&manifest, None).unwrap();
    let source_map = registry.load_local_sources("2026-06-01").unwrap();

    let mut backends = HashMap::new();
    for source in registry.sources() {
        let mut backend = regelrecht_corpus::backend::create_backend(source, None).unwrap();
        backend.ensure_ready().await.unwrap();
        let writable = backend.is_writable();
        backends.insert(
            source.id.clone(),
            BackendEntry {
                backend: Arc::new(Mutex::new(backend)),
                writable,
            },
        );
    }

    let mut state = app_state(None, None);
    state.corpus = Arc::new(RwLock::new(CorpusState {
        registry,
        source_map,
        backends,
        auth_file: None,
        index_failures: std::collections::HashMap::new(),
    }));

    let axum::Json(entries) = list_scenarios(State(state), Path(LAW_ID.to_string()))
        .await
        .expect("list_scenarios must succeed");

    let summary: Vec<(String, Vec<String>)> = entries
        .into_iter()
        .map(|e| (e.filename, e.target_law_ids))
        .collect();

    assert_eq!(
        summary,
        vec![
            ("matching.feature".to_string(), vec![LAW_ID.to_string()]),
            ("other.feature".to_string(), vec!["andere_wet".to_string()]),
            ("wip.feature".to_string(), Vec::new()),
        ]
    );
}
