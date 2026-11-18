//! Tests for the BDD runner's corpus loader.
//!
//! The runner (`tests/bdd/main.rs`) is `harness = false`, so its own module
//! tree cannot carry `#[test]`s. This target includes the loader module, and
//! with it the tests it holds, and runs under plain `cargo test`.

#![allow(dead_code)]

mod common;
#[path = "common/reverse_actions.rs"]
mod reverse_actions;

#[path = "bdd/helpers/regulation_loader.rs"]
mod regulation_loader;
