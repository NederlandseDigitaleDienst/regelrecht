//! Integration tests for the federated corpus.
//!
//! Tests multi-source loading, priority-based conflict resolution,
//! and the full registry → source map → engine pipeline.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_corpus::models::{LocalSource, Source, SourceType};
use regelrecht_corpus::source_map::SourceMap;
use regelrecht_corpus::CorpusRegistry;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("federation")
}

fn make_local_source(id: &str, name: &str, path: PathBuf, priority: u32) -> Source {
    Source {
        id: id.to_string(),
        name: name.to_string(),
        source_type: SourceType::Local {
            local: LocalSource { path },
        },
        scopes: vec![],
        priority,
        auth_ref: None,
        strict_auth: false,
        enriched: None,
        load_all: false,
        library_only: false,
    }
}

// --- Scenario 1: Single source ---

#[test]
fn test_single_source_loads_all_laws() {
    let central_dir = fixtures_dir().join("central");
    let source = make_local_source("central", "Central", central_dir, 1);

    let mut map = SourceMap::new("2026-06-01");
    let count = map.load_source(&source).unwrap();

    assert_eq!(count, 1);
    assert!(map.get_law("test_wet").is_some());
    assert_eq!(map.get_law("test_wet").unwrap().source_id, "central");
}

// --- Scenario 2: Multi-source without overlap ---

#[test]
fn test_multi_source_no_overlap() {
    let central_dir = fixtures_dir().join("central");
    let gemeente_a_dir = fixtures_dir().join("gemeente-a");

    let source_central = make_local_source("central", "Central", central_dir, 1);
    let source_a = make_local_source("gemeente-a", "Gemeente A", gemeente_a_dir, 10);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source_central).unwrap();
    map.load_source(&source_a).unwrap();

    assert_eq!(map.len(), 2);

    let wet = map.get_law("test_wet").unwrap();
    assert_eq!(wet.source_id, "central");
    assert_eq!(wet.source_priority, 1);

    let verordening = map.get_law("test_verordening_a").unwrap();
    assert_eq!(verordening.source_id, "gemeente-a");
    assert_eq!(verordening.source_priority, 10);
}

// --- Scenario 3: Multi-source with multiple gemeenten ---

#[test]
fn test_multi_source_multiple_gemeenten() {
    let gemeente_a_dir = fixtures_dir().join("gemeente-a");
    let gemeente_b_dir = fixtures_dir().join("gemeente-b");

    let source_a = make_local_source("gemeente-a", "Gemeente A", gemeente_a_dir, 10);
    let source_b = make_local_source("gemeente-b", "Gemeente B", gemeente_b_dir, 10);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source_a).unwrap();
    map.load_source(&source_b).unwrap();

    assert_eq!(map.len(), 2);
    assert_eq!(
        map.get_law("test_verordening_a").unwrap().source_id,
        "gemeente-a"
    );
    assert_eq!(
        map.get_law("test_verordening_b").unwrap().source_id,
        "gemeente-b"
    );
}

// --- Scenario 4: Priority conflict (central wins) ---

#[test]
fn test_priority_conflict_central_wins() {
    let central_dir = fixtures_dir().join("central");
    let overlap_dir = fixtures_dir().join("overlap");

    let source_central = make_local_source("central", "Central", central_dir, 1);
    let source_overlap = make_local_source("overlap", "Overlap", overlap_dir, 10);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source_central).unwrap();
    map.load_source(&source_overlap).unwrap();

    // Only 1 law because overlap has same $id as central
    assert_eq!(map.len(), 1);

    let law = map.get_law("test_wet").unwrap();
    assert_eq!(law.source_id, "central"); // Priority 1 beats 10

    // Conflict was recorded
    assert_eq!(map.resolved_conflicts().len(), 1);
    let conflict = &map.resolved_conflicts()[0];
    assert_eq!(conflict.winner_source_id, "central");
    assert_eq!(conflict.loser_source_id, "overlap");
}

// --- Scenario 5: Equal priority conflict ---

#[test]
fn test_equal_priority_conflict_is_error() {
    let central_dir = fixtures_dir().join("central");
    let overlap_dir = fixtures_dir().join("overlap");

    let source_a = make_local_source("source-a", "Source A", central_dir, 5);
    let source_b = make_local_source("source-b", "Source B", overlap_dir, 5);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source_a).unwrap();
    let result = map.load_source(&source_b);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("test_wet"));
    assert!(err.contains("equal priority"));
}

// --- Scenario 6: Local override via registry ---

#[test]
fn test_registry_local_override() {
    let fixtures = fixtures_dir();
    let yaml = format!(
        r#"
schema_version: "1.0"
sources:
  - id: central
    name: "Central"
    type: local
    local:
      path: {central}
    scopes: []
    priority: 1
  - id: gemeente-a
    name: "Gemeente A"
    type: local
    local:
      path: {gemeente_a}
    scopes: []
    priority: 10
"#,
        central = fixtures.join("central").display(),
        gemeente_a = fixtures.join("gemeente-a").display(),
    );

    let registry = CorpusRegistry::from_yaml(&yaml).unwrap();

    let mut map = SourceMap::new("2026-06-01");
    for source in registry.sources() {
        map.load_source(source).unwrap();
    }

    assert_eq!(map.len(), 2);
    assert!(map.get_law("test_wet").is_some());
    assert!(map.get_law("test_verordening_a").is_some());
}

// --- Scenario 7: Empty source ---

#[test]
fn test_empty_source_no_error() {
    let empty_dir = tempfile::TempDir::new().unwrap();
    let source = make_local_source("empty", "Empty", empty_dir.path().to_path_buf(), 1);

    let mut map = SourceMap::new("2026-06-01");
    let count = map.load_source(&source).unwrap();

    assert_eq!(count, 0);
    assert!(map.is_empty());
}

// --- Scenario 8: Invalid YAML file ---

#[test]
fn test_invalid_yaml_skipped() {
    // Files without $id are skipped (not errors)
    let dir = tempfile::TempDir::new().unwrap();
    let bad_file = dir.path().join("bad.yaml");
    std::fs::write(&bad_file, "this is not valid regulation yaml").unwrap();

    let source = make_local_source("bad", "Bad", dir.path().to_path_buf(), 1);
    let mut map = SourceMap::new("2026-06-01");

    // Should not error — files without $id are just skipped
    let count = map.load_source(&source).unwrap();
    assert_eq!(count, 0);
}

// --- Multi-repo integration: full execution through engine ---
//
// Two git repos with overlapping and exclusive laws:
//   repo1 (priority 1): shared_law → origin=1, only_in_repo1 → origin=1
//   repo2 (priority 10): shared_law → origin=2, only_in_repo2 → origin=2
//
// Fixture YAML files live in tests/fixtures/federation/repo{1,2}/.
// Tests copy them into temp dirs and `git init` to create real repos.

/// Copy fixture dir into an isolated temp dir.
///
/// Uses a temp dir so tests don't interfere with each other and the
/// source map sees a clean directory without `.git/` metadata.
fn make_test_source(fixture_name: &str) -> tempfile::TempDir {
    let src = fixtures_dir().join(fixture_name);
    let tmp = tempfile::TempDir::new().unwrap();

    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let rel = entry.path().strip_prefix(&src).unwrap();
        let dest = tmp.path().join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }

    tmp
}

fn execute_law(
    service: &regelrecht_engine::LawExecutionService,
    law_id: &str,
    output: &str,
) -> Option<regelrecht_engine::Value> {
    service
        .evaluate_law_output(
            law_id,
            output,
            std::collections::BTreeMap::new(),
            "2025-01-01",
        )
        .ok()
        .and_then(|r| r.outputs.get(output).cloned())
}

fn load_into_engine(map: &SourceMap) -> regelrecht_engine::LawExecutionService {
    let mut service = regelrecht_engine::LawExecutionService::new();
    for law in map.laws() {
        service
            .load_law_with_source(&law.yaml_content, &law.source_id, &law.source_name)
            .unwrap();
    }
    service
}

#[test]
fn test_multi_repo_only_repo1() {
    let repo1 = make_test_source("repo1");
    let source = make_local_source("repo1", "Repo 1", repo1.path().to_path_buf(), 1);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source).unwrap();

    let service = load_into_engine(&map);

    assert_eq!(
        execute_law(&service, "shared_law", "origin"),
        Some(regelrecht_engine::Value::Int(1))
    );
    assert_eq!(
        execute_law(&service, "only_in_repo1", "origin"),
        Some(regelrecht_engine::Value::Int(1))
    );
    assert_eq!(execute_law(&service, "only_in_repo2", "origin"), None);
}

#[test]
fn test_multi_repo_only_repo2() {
    let repo2 = make_test_source("repo2");
    let source = make_local_source("repo2", "Repo 2", repo2.path().to_path_buf(), 1);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source).unwrap();

    let service = load_into_engine(&map);

    assert_eq!(
        execute_law(&service, "shared_law", "origin"),
        Some(regelrecht_engine::Value::Int(2))
    );
    assert_eq!(
        execute_law(&service, "only_in_repo2", "origin"),
        Some(regelrecht_engine::Value::Int(2))
    );
    assert_eq!(execute_law(&service, "only_in_repo1", "origin"), None);
}

#[test]
fn test_multi_repo_both_repos_priority_wins() {
    let repo1 = make_test_source("repo1");
    let repo2 = make_test_source("repo2");

    let source1 = make_local_source("repo1", "Repo 1", repo1.path().to_path_buf(), 1);
    let source2 = make_local_source("repo2", "Repo 2", repo2.path().to_path_buf(), 10);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source1).unwrap();
    map.load_source(&source2).unwrap();

    let service = load_into_engine(&map);

    // shared_law: repo1 wins (priority 1 < 10)
    assert_eq!(
        execute_law(&service, "shared_law", "origin"),
        Some(regelrecht_engine::Value::Int(1))
    );
    assert_eq!(
        service.get_law_source("shared_law"),
        Some(("repo1", "Repo 1"))
    );

    // exclusive laws from both repos are available
    assert_eq!(
        execute_law(&service, "only_in_repo1", "origin"),
        Some(regelrecht_engine::Value::Int(1))
    );
    assert_eq!(
        execute_law(&service, "only_in_repo2", "origin"),
        Some(regelrecht_engine::Value::Int(2))
    );

    // conflict was recorded
    assert!(map
        .resolved_conflicts()
        .iter()
        .any(|c| c.law_id == "shared_law"
            && c.winner_source_id == "repo1"
            && c.loser_source_id == "repo2"));
}

#[test]
fn test_multi_repo_reversed_priority() {
    let repo1 = make_test_source("repo1");
    let repo2 = make_test_source("repo2");

    // Flip priorities: repo2 now has higher priority
    let source1 = make_local_source("repo1", "Repo 1", repo1.path().to_path_buf(), 10);
    let source2 = make_local_source("repo2", "Repo 2", repo2.path().to_path_buf(), 1);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source1).unwrap();
    map.load_source(&source2).unwrap();

    let service = load_into_engine(&map);

    // shared_law: repo2 wins now (priority 1 < 10)
    assert_eq!(
        execute_law(&service, "shared_law", "origin"),
        Some(regelrecht_engine::Value::Int(2))
    );
    assert_eq!(
        service.get_law_source("shared_law"),
        Some(("repo2", "Repo 2"))
    );

    // exclusive laws still both available
    assert_eq!(
        execute_law(&service, "only_in_repo1", "origin"),
        Some(regelrecht_engine::Value::Int(1))
    );
    assert_eq!(
        execute_law(&service, "only_in_repo2", "origin"),
        Some(regelrecht_engine::Value::Int(2))
    );
}

// --- Scenario 9: Source map fed into engine ---

#[test]
fn test_source_map_to_engine() {
    use regelrecht_engine::LawExecutionService;
    use std::collections::BTreeMap;

    let central_dir = fixtures_dir().join("central");
    let gemeente_a_dir = fixtures_dir().join("gemeente-a");

    let source_central = make_local_source("central", "Central", central_dir, 1);
    let source_a = make_local_source("gemeente-a", "Gemeente A", gemeente_a_dir, 10);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source_central).unwrap();
    map.load_source(&source_a).unwrap();

    // Feed source map into engine
    let mut service = LawExecutionService::new();
    for law in map.laws() {
        service
            .load_law_with_source(&law.yaml_content, &law.source_id, &law.source_name)
            .unwrap();
    }

    // Verify source tracking
    assert_eq!(
        service.get_law_source("test_wet"),
        Some(("central", "Central"))
    );
    assert_eq!(
        service.get_law_source("test_verordening_a"),
        Some(("gemeente-a", "Gemeente A"))
    );

    // Execute a law from the central source
    let result = service
        .evaluate_law_output("test_wet", "test_value", BTreeMap::new(), "2025-01-01")
        .unwrap();

    assert_eq!(
        result.outputs.get("test_value"),
        Some(&regelrecht_engine::Value::Int(200))
    );

    // Execute a law from gemeente-a
    let result = service
        .evaluate_law_output(
            "test_verordening_a",
            "local_rate",
            BTreeMap::new(),
            "2025-01-01",
        )
        .unwrap();

    assert_eq!(
        result.outputs.get("local_rate"),
        Some(&regelrecht_engine::Value::Int(42))
    );
}

// --- Scenario 10: Priority conflict with engine execution ---

#[test]
fn test_priority_conflict_correct_law_executes() {
    use regelrecht_engine::LawExecutionService;
    use std::collections::BTreeMap;

    let central_dir = fixtures_dir().join("central");
    let overlap_dir = fixtures_dir().join("overlap");

    let source_central = make_local_source("central", "Central", central_dir, 1);
    let source_overlap = make_local_source("overlap", "Overlap", overlap_dir, 10);

    let mut map = SourceMap::new("2026-06-01");
    map.load_source(&source_central).unwrap();
    map.load_source(&source_overlap).unwrap();

    let mut service = LawExecutionService::new();
    for law in map.laws() {
        service
            .load_law_with_source(&law.yaml_content, &law.source_id, &law.source_name)
            .unwrap();
    }

    // Central outputs 200, overlap outputs 999
    // Central should win (priority 1 < 10)
    let result = service
        .evaluate_law_output("test_wet", "test_value", BTreeMap::new(), "2025-01-01")
        .unwrap();

    // Should be 200 (from central), not 999 (from overlap)
    assert_eq!(
        result.outputs.get("test_value"),
        Some(&regelrecht_engine::Value::Int(200))
    );
}
