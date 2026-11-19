//! Shared helpers for integration tests.

use std::path::PathBuf;

/// The default regulation base path: the fixture corpus under `corpus/`.
fn default_regulation_base_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("regulation")
}

/// Get the regulation base path from `REGULATION_PATH` env var or default relative path.
#[allow(dead_code)] // not every test target that includes this module reads one base
pub fn regulation_base_path() -> PathBuf {
    std::env::var("REGULATION_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_regulation_base_path())
}

/// The regulation base paths in `REGULATION_PATH`, which may name several
/// directories separated like `PATH` (`:` on Unix), or the default. The BDD
/// runner loads the laws of all of them and runs their scenarios, so one
/// corpus may lie in several directories.
#[allow(dead_code)] // only the BDD runner and its loader read several bases
pub fn regulation_base_paths() -> Vec<PathBuf> {
    paths_from_env("REGULATION_PATH").unwrap_or_else(|| vec![default_regulation_base_path()])
}

/// The directories in the environment variable `var`, separated like `PATH`
/// (`:` on Unix). `None` when it is unset or blank.
#[allow(dead_code)] // only the BDD runners read a list of directories
pub fn paths_from_env(var: &str) -> Option<Vec<PathBuf>> {
    let value = std::env::var_os(var)?;
    if value.to_string_lossy().trim().is_empty() {
        return None;
    }
    Some(std::env::split_paths(&value).collect())
}
