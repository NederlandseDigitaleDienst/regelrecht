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
mod helpers;
#[path = "../common/reverse_actions.rs"]
mod reverse_actions;
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

use cucumber::feature::Ext as _;
use cucumber::{cli, parser, World as _};
use discovery::Bucket;
use futures::stream;

/// Parser that consumes an explicit, pre-collected list of feature file paths
/// (the two buckets) instead of recursively walking a single root. This keeps
/// the runner from picking up unrelated `.feature` files and lets us combine
/// `corpus/regulation/**/scenarios` with `bdd/conformance` in one run.
struct ExplicitPaths;

impl parser::Parser<Vec<PathBuf>> for ExplicitPaths {
    type Cli = cli::Empty;
    type Output = stream::Iter<std::vec::IntoIter<Result<gherkin::Feature, parser::Error>>>;

    fn parse(self, input: Vec<PathBuf>, _cli: Self::Cli) -> Self::Output {
        let features: Vec<Result<gherkin::Feature, parser::Error>> = input
            .into_iter()
            .map(|path| {
                gherkin::Feature::parse_path(&path, gherkin::GherkinEnv::default())
                    .map_err(parser::Error::from)
                    .and_then(|f| f.expand_examples().map_err(parser::Error::from))
            })
            .collect();
        stream::iter(features)
    }
}

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
    // Initialize tracing subscriber (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .init();

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

    // Run cucumber over both buckets. `@wip` scenarios document genuine,
    // NB-annotated engine gaps (they assert the desired-but-not-yet-produced
    // outcome) — skip them so they neither run nor fail the suite.
    // `filter_run_and_exit` exits non-zero on test failures.
    //
    // `fail_on_skipped` covers the other way a scenario stops testing: cucumber
    // counts a step without a matching definition as *skipped*, and
    // `execution_has_failed()` only looks at failed steps. Without it a reworded
    // `Then` passes green while asserting nothing.
    world::RegelrechtWorld::cucumber::<&std::path::Path>()
        .with_parser::<ExplicitPaths, Vec<PathBuf>>(ExplicitPaths)
        .max_concurrent_scenarios(1) // Run scenarios sequentially for predictable state
        .fail_on_skipped()
        .with_default_cli()
        .filter_run_and_exit(features, |feature, _rule, scenario| {
            let is_wip = |tags: &[String]| tags.iter().any(|t| t == "wip");
            !is_wip(&feature.tags) && !is_wip(&scenario.tags)
        })
        .await;
}
