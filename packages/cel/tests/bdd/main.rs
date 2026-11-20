//! The cell's own BDD suite: a cell records submissions, decisions and facts
//! in its chronicle, and a reduction plays the chronicle forward
//! (`projection`), with the engine rewinding where the law undoes a record.
//!
//! The features are in `packages/cel/bdd/features`, their fixtures (synthetic
//! laws, a mini general administrative act and a cell with its reductions) in
//! `packages/cel/bdd/fixtures`; they run against those alone, not the corpus.
//! Beside them, each construct under `packages/cel/bdd/constructs/` is a
//! scenario set (`sets.rs`): its features run against its own fictional
//! regelwerken and nothing else. `BDD_CONSTRUCT_SETS` names sets from outside
//! this crate, separated by `:`; when it is set, the runner plays those sets
//! and not its own suite. `BDD_JUNIT` writes a JUnit report.
//! A feature may use the canonical steps (`bdd/grammar.yaml`) and the cell's
//! own (`packages/cel/bdd/grammar.yaml`). Those stay out of the canonical
//! grammar: that is the language every engine, Rust and JS, executes, while
//! recording a chronicle is the cell's job and only the Rust engine plays one
//! forward.
//!
//! The bindings come from the engine's generator (`build.rs`); the world,
//! dispatch, helpers and the cucumber run are the engine runner's, included by
//! `#[path]`. The cell's actions are in `extension.rs`. The runner sits here
//! and not in the engine because the engine cannot depend on the cell, not
//! even for a test: the Docker builds of the WASM module, the editor and admin
//! drop `cel` from the workspace.
//!
//! ```bash
//! cargo test -p regelrecht-cel --test bdd -- --nocapture
//! ```

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "../../../engine/tests/common/mod.rs"]
mod common;
#[path = "../../../engine/tests/bdd/dispatch.rs"]
mod dispatch;
mod extension;
// The cell's own features load only their fixtures, never the corpus, so
// the corpus loader among the engine's helpers goes unused here.
#[allow(dead_code)]
#[path = "../../../engine/tests/bdd/helpers/mod.rs"]
mod helpers;
#[path = "../../../engine/tests/common/reverse_actions.rs"]
mod reverse_actions;
#[path = "../../../engine/tests/bdd/runner.rs"]
mod runner;
mod sets;
#[path = "../../../engine/tests/bdd/world.rs"]
mod world;

/// Code-generated cucumber step bindings from both grammars (`build.rs`).
#[allow(unused_mut, clippy::let_and_return, clippy::vec_init_then_push)]
mod generated_steps {
    use crate::dispatch::ArgValue;
    use crate::world::RegelrechtWorld;
    include!(concat!(env!("OUT_DIR"), "/bdd_generated_steps.rs"));
}

use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() {
    // Sets from outside the crate run on their own: the caller wants a report
    // on its own scenarios, and the cell's suite runs in regelrecht's CI.
    let external = sets::external();
    let features = if external.is_empty() {
        own_features()
    } else {
        external
            .iter()
            .flat_map(sets::ScenarioSet::features)
            .collect()
    };
    runner::run(features).await;
}

/// The cell's own suite: `bdd/features`, then each construct in
/// `bdd/constructs`.
fn own_features() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/features");
    let mut features: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|x| x == "feature"))
        .collect();
    features.sort();
    assert!(
        !features.is_empty(),
        "no feature files in {}",
        dir.display()
    );
    let constructs = Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/constructs");
    for set in sets::constructs(&constructs) {
        features.extend(set.features());
    }
    features
}
