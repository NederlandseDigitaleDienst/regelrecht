//! BDD Test Runner for RegelRecht Engine
//!
//! Runs Cucumber/Gherkin tests for the canonical, engine-agnostic BDD feature
//! language. Feature files live in two buckets:
//!
//! - **Bucket A — law validation**: `corpus/regulation/**/scenarios/*.feature`
//!   (run against the live laws, any file under a `scenarios/` directory).
//! - **Bucket B — engine conformance**: `bdd/conformance/*.feature`.
//!
//! The Rust engine supports every capability tier, so all features run except
//! scenarios tagged `@wip` (genuine, NB-documented engine gaps that assert the
//! desired-but-not-yet-produced outcome).
//!
//! The cell's own suite (`packages/cel/tests/bdd`) reuses this runner's world,
//! dispatch and helpers by `#[path]`; `extension.rs` is where a runner adds
//! actions of its own, and this one adds none.
//!
//! # Usage
//!
//! ```bash
//! cargo test --test bdd -- --nocapture
//! ```
//!
//! Or via just:
//!
//! ```bash
//! just bdd
//! ```
//!
//! # Bucket selection
//!
//! `BDD_BUCKET` picks which bucket runs: `all` (default), `conformance`
//! (bucket B) or `corpus` (bucket A). CI runs the conformance bucket as a
//! blocking check — it is deterministic and depends only on the synthetic
//! `test_*` laws. Bucket A stays out of CI: it asserts what the live laws
//! currently produce, so a failure there is a question for a human, not a
//! broken build.

// Allow panic/expect in test code - these are appropriate for test setup
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

#[path = "../common/mod.rs"]
mod common;
mod discovery;
mod dispatch;
mod extension;
mod helpers;
#[path = "../common/reverse_actions.rs"]
mod reverse_actions;
mod runner;
mod world;

/// Code-generated cucumber step bindings from `bdd/grammar.yaml`.
/// The generated fns reference `RegelrechtWorld` and `ArgValue`; bring them into
/// scope here. `cucumber::gherkin::Step` is referenced fully-qualified.
#[allow(unused_mut, clippy::let_and_return, clippy::vec_init_then_push)]
mod generated_steps {
    use crate::dispatch::ArgValue;
    use crate::world::RegelrechtWorld;
    include!(concat!(env!("OUT_DIR"), "/bdd_generated_steps.rs"));
}

use std::path::{Path, PathBuf};

use discovery::Bucket;

impl Bucket {
    /// `BDD_BUCKET`: `all` (default), `corpus` (bucket A) or `conformance`
    /// (bucket B).
    fn from_env() -> Self {
        match std::env::var("BDD_BUCKET").as_deref().map(str::trim) {
            Ok("") | Err(_) | Ok("all") => Self::All,
            Ok("corpus") | Ok("a") => Self::Corpus,
            Ok("conformance") | Ok("b") => Self::Conformance,
            Ok(other) => panic!("BDD_BUCKET={other}: expected all, corpus or conformance"),
        }
    }
}

/// The corpus whose scenarios bucket A runs: `REGULATION_PATH` when set (the
/// same variable the engine loads its laws from), else the fixture corpus.
/// `just bdd-demo` points it at `corpus/demo/regulation`, `just bdd-chrono`
/// at the directories of the chronolexografie PoC.
fn corpus_roots() -> Vec<PathBuf> {
    common::regulation_base_paths()
}

#[tokio::main]
async fn main() {
    // Project root = two levels up from the engine package manifest.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let root = Path::new(manifest_dir)
        .parent() // packages/
        .and_then(|p| p.parent()) // project root
        .expect("Could not resolve project root")
        .to_path_buf();

    let bucket = Bucket::from_env();
    let features = match discovery::collect_feature_paths(&root, &corpus_roots(), bucket) {
        Ok(features) => features,
        Err(e) => panic!("BDD feature discovery failed under {}: {e}", root.display()),
    };
    runner::run(features).await;
}
