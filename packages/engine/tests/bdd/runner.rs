//! What the engine's BDD runner shares with the cell's (`packages/cel/tests/bdd`):
//! the parser for an explicit list of feature files and the cucumber run. The
//! cell's runner includes this module, like the world and the dispatch, by
//! `#[path]`, and gives it its own features.

use std::path::{Path, PathBuf};

use cucumber::feature::Ext as _;
use cucumber::{cli, parser, World as _};
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

/// Run `features` under the world and the generated steps of the runner that
/// calls it.
pub async fn run(features: Vec<PathBuf>) {
    // Initialize tracing subscriber (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .init();

    // `@wip` scenarios document genuine, NB-annotated engine gaps (they assert
    // the desired-but-not-yet-produced outcome) — skip them so they neither
    // run nor fail the suite. `filter_run_and_exit` exits non-zero on test
    // failures.
    //
    // `fail_on_skipped` covers the other way a scenario stops testing: cucumber
    // counts a step without a matching definition as *skipped*, and
    // `execution_has_failed()` only looks at failed steps. Without it a reworded
    // `Then` passes green while asserting nothing.
    crate::world::RegelrechtWorld::cucumber::<&Path>()
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
