//! BDD test runner for the NAPP subsidy law.
//!
//! Runs the Gherkin scenarios in `scenarios/` against the regelrecht engine
//! with the laws from `law/` loaded.
//!
//! ```bash
//! cargo test --test bdd -- --nocapture
//! # or: just bdd
//! ```

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod steps;
mod world;

use cucumber::World;
use std::path::Path;

#[tokio::main]
async fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // De scenario's staan bij de casus-inhoud (corpus-poc/napp/scenarios), niet
    // naast de crate: in de monorepo is packages/ de buur, niet de projectwortel.
    let scenarios_dir = Path::new(manifest_dir)
        .parent() // packages/
        .and_then(Path::parent) // repo-wortel
        .map(|p| p.join("corpus-poc").join("napp").join("scenarios"))
        .expect("Could not find scenarios directory");

    if !scenarios_dir.exists() {
        panic!("Scenarios directory not found: {}", scenarios_dir.display());
    }

    // Cucumber telt een stap zonder definitie als *skipped*, en `run_and_exit`
    // faalt alleen op mislukte stappen. Zonder `fail_on_skipped` blijft een
    // herformuleerde `Then` groen terwijl hij niets meer toetst.
    world::NappWorld::cucumber()
        .max_concurrent_scenarios(1)
        .fail_on_skipped()
        .with_default_cli()
        .run_and_exit(scenarios_dir)
        .await;
}
