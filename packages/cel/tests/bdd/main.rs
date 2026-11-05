//! The cell's own BDD suite: a cell records submissions and decisions in its
//! chronicle, plays the effects the law gives a decision on another one, and
//! a reduction reads the decisions whose consequences count.
//!
//! The features are in `packages/cel/bdd/features`, their fixtures (a
//! synthetic law and a cell with its reduction) in `packages/cel/bdd/fixtures`.
//! A feature may use the canonical steps (`bdd/grammar.yaml`) and the cell's
//! own (`packages/cel/bdd/grammar.yaml`). Those stay out of the canonical
//! grammar: that is the language every engine, Rust and JS, executes, and a
//! chronicle is behavior of this one implementation.
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
#[path = "../../../engine/tests/bdd/helpers/mod.rs"]
mod helpers;
#[path = "../../../engine/tests/common/reverse_actions.rs"]
mod reverse_actions;
#[path = "../../../engine/tests/bdd/runner.rs"]
mod runner;
#[path = "../../../engine/tests/bdd/world.rs"]
mod world;

/// Code-generated cucumber step bindings from both grammars (`build.rs`).
#[allow(unused_mut, clippy::let_and_return, clippy::vec_init_then_push)]
mod generated_steps {
    use crate::dispatch::ArgValue;
    use crate::world::RegelrechtWorld;
    include!(concat!(env!("OUT_DIR"), "/bdd_generated_steps.rs"));
}

use std::path::Path;

#[tokio::main]
async fn main() {
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
    runner::run(features).await;
}
