//! Guards the **generated** model: `arch-extract` is run on-demand (no committed
//! `model.json` exists), and its output must validate against the JSON schema
//! and match the well-known crate layer graph. These run as part of `cargo test`
//! (hence `just check`), so a malformed model — or a lost dependency edge —
//! fails CI rather than silently breaking the architecture explorer.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

fn arch_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/content/architecture")
}

/// The workspace manifest (`packages/Cargo.toml`), so the generator's
/// `cargo metadata` resolves the whole workspace regardless of the test's cwd.
fn workspace_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml")
}

fn load_schema() -> Value {
    let path = arch_dir().join("model.schema.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

/// Runs the extractor once (deep pass over every crate, the default) and returns
/// its `model.json` on stdout, parsed. Cached so the ~2s generation happens once
/// for the whole test binary rather than per test.
fn model() -> &'static Value {
    static MODEL: OnceLock<Value> = OnceLock::new();
    MODEL.get_or_init(|| {
        let out = Command::new(env!("CARGO_BIN_EXE_arch-extract"))
            .args([
                "generate",
                "--stdout",
                "--manifest-path",
                workspace_manifest().to_str().expect("utf-8 manifest path"),
            ])
            .output()
            .expect("running arch-extract");
        assert!(
            out.status.success(),
            "arch-extract failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("parsing generated model.json")
    })
}

#[test]
fn model_validates_against_schema() {
    let schema = load_schema();
    let model = model();

    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(model)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "generated model fails schema:\n{}",
        errors.join("\n")
    );
}

fn crate_short_names(model: &Value) -> std::collections::BTreeSet<String> {
    model["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter(|n| n["kind"] == "crate")
        .filter_map(|n| n["name"].as_str().map(str::to_string))
        .collect()
}

/// `from` short-name -> set of `to` short-names over `depends-on` edges.
fn depends_on(model: &Value, from_short: &str) -> std::collections::BTreeSet<String> {
    let from_id = format!("crate:{from_short}");
    model["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .filter(|e| e["kind"] == "depends-on" && e["from"] == from_id.as_str())
        .filter_map(|e| e["to"].as_str())
        .filter_map(|to| to.strip_prefix("crate:").map(str::to_string))
        .collect()
}

#[test]
fn product_crates_present() {
    let crates = crate_short_names(model());
    let expected: std::collections::BTreeSet<String> = [
        "admin",
        "auth",
        "cel",
        "corpus",
        "editor-api",
        "engine",
        "github",
        "harvester",
        "law-model",
        "pipeline",
        "poc-napp",
        "poc-portal",
        "poc-vakantieverlof",
        "shared",
        "tui",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        crates, expected,
        "crate nodes must be exactly the product crates"
    );
}

#[test]
fn layer_graph_matches_known_dependencies() {
    let model = model();

    // `shared` is the foundation: it depends on no other workspace crate.
    assert!(depends_on(model, "shared").is_empty());

    // `github` is a second foundation: the shared GitHub REST client is
    // deliberately standalone, so the crates that consolidated onto it
    // (corpus, editor-api) can depend on it without a cycle.
    assert!(depends_on(model, "github").is_empty());

    // Spot-check the documented layers.
    // The engine sits on law-model alone; it has no telemetry or config of
    // its own to get from shared.
    assert_eq!(
        depends_on(model, "engine"),
        std::collections::BTreeSet::from(["law-model".to_string()])
    );
    assert_eq!(
        depends_on(model, "law-model"),
        std::collections::BTreeSet::from(["shared".to_string()])
    );
    assert_eq!(
        depends_on(model, "admin"),
        std::collections::BTreeSet::from([
            "auth".to_string(),
            "shared".to_string(),
            "corpus".to_string(),
            "harvester".to_string(),
            "pipeline".to_string(),
        ])
    );
}

#[test]
fn source_level_extraction_ran() {
    let model = model();
    let nodes = model["nodes"].as_array().expect("nodes array");

    let count_kind = |kind: &str| nodes.iter().filter(|n| n["kind"] == kind).count();

    // The syn pass must have produced structure below the crate level.
    assert!(count_kind("module") > 0, "expected module nodes");
    assert!(count_kind("struct") > 0, "expected struct nodes");
    assert!(count_kind("method") > 0, "expected method nodes");

    // And the engine's execution service should be captured at type level.
    let has_service = nodes.iter().any(|n| {
        n["kind"] == "struct"
            && n["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("type:engine::"))
    });
    assert!(has_service, "expected engine type nodes");
}

/// The crate short-name a `prefix:crate::…` node id belongs to, e.g.
/// `type:corpus::dto::PaginationParams` -> `corpus`. Returns `None` for a bare
/// `crate:corpus` (no `::`).
fn edge_crate(id: &str) -> Option<&str> {
    let after = id.split_once(':')?.1;
    after.split_once("::").map(|(head, _)| head)
}

#[test]
fn cross_crate_use_and_impl_edges_resolved() {
    // Before this pass, `uses`/`impl` edges were resolved by leaf-matching a
    // type name against the *same crate's* types only, so cross-crate
    // relationships (the interesting ones for an architecture map) were all
    // missed. The workspace-wide symbol table plus per-file `use` resolution
    // now captures them. Assert that concrete, well-known relationships appear —
    // both proving the coverage improvement and guarding against a regression.
    let model = model();
    let edges = model["edges"].as_array().expect("edges array");

    let uses_between = |from_crate: &str, to_crate: &str| {
        edges.iter().any(|e| {
            e["kind"] == "uses"
                && e["from"].as_str().and_then(edge_crate) == Some(from_crate)
                && e["to"].as_str().and_then(edge_crate) == Some(to_crate)
                && e["to"].as_str().is_some_and(|t| t.starts_with("type:"))
        })
    };

    // editor-api and admin both build on the corpus library.
    assert!(
        uses_between("editor-api", "corpus"),
        "expected editor-api → corpus cross-crate uses edges"
    );
    assert!(
        uses_between("corpus", "github"),
        "expected corpus → github cross-crate uses edges"
    );

    // A cross-crate trait impl: `ArticleBasedLaw` (law-model) implements the
    // engine's `LawLoad` trait — resolvable now that the trait's crate is
    // identified from the `use` path.
    let has_cross_impl = edges.iter().any(|e| {
        e["kind"] == "impl"
            && e["from"] == "type:law-model::model::ArticleBasedLaw"
            && e["to"] == "type:engine::article::LawLoad"
    });
    assert!(
        has_cross_impl,
        "expected the law-model → engine LawLoad impl edge"
    );

    // The improvement should be broad, not a one-off: many cross-crate uses.
    let cross_uses = edges
        .iter()
        .filter(|e| {
            e["kind"] == "uses"
                && e["to"].as_str().is_some_and(|t| t.starts_with("type:"))
                && matches!(
                    (
                        e["from"].as_str().and_then(edge_crate),
                        e["to"].as_str().and_then(edge_crate),
                    ),
                    (Some(a), Some(b)) if a != b
                )
        })
        .count();
    assert!(
        cross_uses >= 20,
        "expected a broad set of cross-crate uses edges, got {cross_uses}"
    );
}

#[test]
fn no_edge_leaves_the_workspace() {
    // The "no false edges" guarantee at the model level: every edge endpoint is
    // a real node in the model (the canonicalizer drops dangling edges), and no
    // edge points at a node in a crate that does not exist. A leaf-match that
    // guessed a non-existent target would surface here.
    let model = model();
    let node_ids: std::collections::HashSet<&str> = model["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter_map(|n| n["id"].as_str())
        .collect();

    for e in model["edges"].as_array().expect("edges array") {
        let from = e["from"].as_str().expect("edge from");
        let to = e["to"].as_str().expect("edge to");
        assert!(
            node_ids.contains(from),
            "edge from a non-existent node: {from}"
        );
        assert!(node_ids.contains(to), "edge to a non-existent node: {to}");
        assert_ne!(from, to, "self-edge should have been dropped");
    }
}

/// Set of node ids of a given kind.
fn ids_of_kind(model: &Value, kind: &str) -> std::collections::BTreeSet<String> {
    model["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .filter(|n| n["kind"] == kind)
        .filter_map(|n| n["id"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn frontend_apps_extracted() {
    let model = model();

    // Every npm-workspace frontend appears as an `app` container.
    let apps = ids_of_kind(model, "app");
    let expected_apps: std::collections::BTreeSet<String> = [
        "app:frontend",
        "app:frontend-demo",
        "app:frontend-lawmaking",
        "app:frontend-poc-napp",
        "app:frontend-poc-nieuwkomersbekostiging",
        "app:frontend-poc-portal",
        "app:frontend-poc-terugbetaalregimes",
        "app:frontend-poc-vakantieverlof",
        "app:frontend-shared",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(apps, expected_apps, "expected every frontend app");

    // Components (.vue) and composables (useXxx) are extracted, with lang set.
    let nodes = model["nodes"].as_array().expect("nodes array");
    let vue_components = nodes
        .iter()
        .filter(|n| n["kind"] == "component" && n["lang"] == "vue")
        .count();
    assert!(vue_components > 0, "expected Vue component nodes");
    assert!(
        !ids_of_kind(model, "composable").is_empty(),
        "expected composable nodes"
    );

    // Containment: every frontend node (below the app) reaches its app via
    // `parent`, and no frontend node is left parentless.
    let below_app = nodes.iter().filter(|n| {
        matches!(n["kind"].as_str(), Some("component" | "composable" | "dir"))
            || (n["kind"] == "module" && n["lang"] != "rust")
    });
    for n in below_app {
        assert!(
            n["parent"].is_string(),
            "frontend node {} must have a parent",
            n["id"]
        );
    }
}

#[test]
fn frontend_import_edges_present() {
    let model = model();
    let edges = model["edges"].as_array().expect("edges array");

    // Both Vue apps depend on the shared package (a cross-app `depends-on`).
    let app_depends = |from: &str| {
        edges.iter().any(|e| {
            e["kind"] == "depends-on" && e["from"] == from && e["to"] == "app:frontend-shared"
        })
    };
    assert!(
        app_depends("app:frontend"),
        "frontend should depend on frontend-shared"
    );
    assert!(
        app_depends("app:frontend-lawmaking"),
        "frontend-lawmaking should depend on frontend-shared"
    );

    // Imports within a frontend become `uses` edges — in particular a component
    // that imports a composable (the acceptance criterion "which component uses
    // which composable").
    let component_uses_composable = edges.iter().any(|e| {
        e["kind"] == "uses"
            && e["from"]
                .as_str()
                .is_some_and(|f| f.starts_with("component:frontend::"))
            && e["to"]
                .as_str()
                .is_some_and(|t| t.starts_with("composable:frontend::"))
    });
    assert!(
        component_uses_composable,
        "expected a component→composable uses edge"
    );
}

/// The crate prefix of a `kind:crate::…` node id, e.g. `mod:engine::service`
/// -> `engine`. Returns `None` for a bare `crate:engine` (no `::`).
fn crate_prefix(id: &str) -> Option<&str> {
    let after = id.split_once(':')?.1;
    after.split_once("::").map(|(head, _)| head)
}

#[test]
fn deep_extraction_covers_all_crates() {
    // The deep source pass runs for every crate by default, not just
    // engine + corpus. Assert that each product crate has at least one
    // deep-extracted node below it (i.e. the syn pass actually walked its src).
    //
    // "Below it" means any non-crate node whose id resolves to the crate, not
    // specifically a `module` node: a crate whose root is a single `lib.rs`
    // (no `mod` declarations) emits its top-level types/fns parented directly
    // at the crate node (see `FileRole::CrateRoot` in `syn_pass.rs`), so it has
    // zero module nodes even though the deep pass ran. Keying on modules alone
    // would spuriously flag such a crate as un-extracted.
    let model = model();
    let nodes = model["nodes"].as_array().expect("nodes array");

    let crates = crate_short_names(model);
    let mut crates_with_deep_nodes: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for n in nodes {
        if n["kind"] == "crate" {
            continue;
        }
        if let Some(id) = n["id"].as_str() {
            if let Some(c) = crate_prefix(id) {
                crates_with_deep_nodes.insert(c.to_string());
            }
        }
    }

    let missing: Vec<String> = crates
        .difference(&crates_with_deep_nodes)
        .cloned()
        .collect();
    assert!(
        missing.is_empty(),
        "every crate should have deep (source-level) extraction; missing: {missing:?}"
    );
}

#[test]
fn generation_is_deterministic() {
    // The explorer leans on stable node identity between runs, so two
    // generations must be byte-for-byte identical (nodes sorted by id, edges
    // sorted + deduped, no timestamp).
    let first = serde_json::to_string(model()).expect("serialize");

    let out = Command::new(env!("CARGO_BIN_EXE_arch-extract"))
        .args([
            "generate",
            "--stdout",
            "--manifest-path",
            workspace_manifest().to_str().expect("utf-8 manifest path"),
        ])
        .output()
        .expect("running arch-extract");
    assert!(out.status.success(), "arch-extract failed on second run");
    let second: Value = serde_json::from_slice(&out.stdout).expect("parse second run");
    let second = serde_json::to_string(&second).expect("serialize");

    assert_eq!(first, second, "model generation must be deterministic");
}
