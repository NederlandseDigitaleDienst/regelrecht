//! Tests for the BDD runner's corpus loader.
//!
//! The runner (`tests/bdd/main.rs`) is `harness = false`, so its own module
//! tree cannot carry `#[test]`s. This target includes the loader module, and
//! with it the tests it holds, and runs under plain `cargo test`.

#![allow(dead_code)]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

mod common;
#[path = "common/construct_mutation.rs"]
mod construct_mutation;
#[path = "common/reverse_actions.rs"]
mod reverse_actions;

#[path = "bdd/helpers/regulation_loader.rs"]
mod regulation_loader;

// The tests of the loader itself sit here rather than in its module: the `bdd`
// target compiles that module with cfg(test) too, but runs no `#[test]`, so a
// helper used only by them would be dead code there.

use std::fs;
use std::path::Path;

use regelrecht_engine::LawExecutionService;
use regulation_loader::load_regulations_from;

/// Write a fictional regelwerk `law_id`, version `valid_from`, whose one
/// article gives `amount`, into the `nl/` of `base`.
fn write_regelwerk(base: &Path, law_id: &str, valid_from: &str, amount: u32) {
    let dir = base.join("nl/wet").join(law_id);
    fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        r"$id: {law_id}
regulatory_layer: WET
publication_date: '{valid_from}'
valid_from: '{valid_from}'
url: https://example.com/{law_id}
articles:
  - number: '1'
    text: FICTIEF.
    url: https://example.com/{law_id}/1
    machine_readable:
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: {amount}
"
    );
    fs::write(dir.join(format!("{valid_from}.yaml")), yaml).unwrap();
}

#[test]
fn versions_of_one_regelwerk_in_one_directory_load() {
    let tmp = tempfile::tempdir().unwrap();
    write_regelwerk(tmp.path(), "test_wet", "2024-01-01", 1);
    write_regelwerk(tmp.path(), "test_wet", "2025-01-01", 2);

    let mut service = LawExecutionService::new();
    let count = load_regulations_from(&mut service, &[tmp.path().to_path_buf()]).unwrap();

    assert_eq!(count, 2);
}

#[test]
fn distinct_regelwerken_in_two_directories_load() {
    let own = tempfile::tempdir().unwrap();
    let central = tempfile::tempdir().unwrap();
    write_regelwerk(own.path(), "test_eigen_wet", "2025-01-01", 1);
    write_regelwerk(central.path(), "test_centrale_wet", "2025-01-01", 2);

    let mut service = LawExecutionService::new();
    let bases = [own.path().to_path_buf(), central.path().to_path_buf()];
    let count = load_regulations_from(&mut service, &bases).unwrap();

    assert_eq!(count, 2);
    assert!(service.has_law("test_eigen_wet"));
    assert!(service.has_law("test_centrale_wet"));
}

/// Same `$id` and `valid_from` would replace, another `valid_from` would
/// add a version: both fail when the files are in different directories.
#[test]
fn one_regelwerk_in_two_directories_fails_naming_both_files() {
    for central_valid_from in ["2025-01-01", "2026-01-01"] {
        let own = tempfile::tempdir().unwrap();
        let central = tempfile::tempdir().unwrap();
        write_regelwerk(own.path(), "test_wet", "2025-01-01", 1);
        write_regelwerk(central.path(), "test_wet", central_valid_from, 2);

        let mut service = LawExecutionService::new();
        let bases = [own.path().to_path_buf(), central.path().to_path_buf()];
        let error = load_regulations_from(&mut service, &bases)
            .unwrap_err()
            .to_string();

        assert!(error.contains("'test_wet'"), "{error}");
        assert!(error.contains(&own.path().display().to_string()), "{error}");
        assert!(
            error.contains(&central.path().display().to_string()),
            "{error}"
        );
    }
}

/// A file that does not load fails the load and names the file, so a broken
/// version can never be replaced unseen by one from another directory.
#[test]
fn a_regelwerk_that_does_not_load_fails_naming_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    write_regelwerk(tmp.path(), "test_wet", "2025-01-01", 1);
    let broken = tmp.path().join("nl/wet/test_kapot/2025-01-01.yaml");
    fs::create_dir_all(broken.parent().unwrap()).unwrap();
    fs::write(
        &broken,
        "$id: test_kapot\nvalid_from: 'geen datum'\narticles: []\n",
    )
    .unwrap();

    let mut service = LawExecutionService::new();
    let error = load_regulations_from(&mut service, &[tmp.path().to_path_buf()])
        .unwrap_err()
        .to_string();

    assert!(error.contains(&broken.display().to_string()), "{error}");
}

/// A dot-prefixed sidecar next to a version is not a regelwerk and is left out.
#[test]
fn a_sidecar_is_not_loaded() {
    let tmp = tempfile::tempdir().unwrap();
    write_regelwerk(tmp.path(), "test_wet", "2025-01-01", 1);
    let dir = tmp.path().join("nl/wet/test_wet");
    fs::write(dir.join(".enrichment.yaml"), "provider: test\n").unwrap();

    let mut service = LawExecutionService::new();
    let count = load_regulations_from(&mut service, &[tmp.path().to_path_buf()]).unwrap();

    assert_eq!(count, 1);
}
