//! Behaviour of the `validate` binary: the corpus gate of pre-commit and CI.
//!
//! What these tests pin is the exit code, not the layout of the messages.
//! `just validate` (and so the pre-commit hook) decides on that code whether
//! a change in `corpus/regulation/` may land; a `validate` that quietly
//! returns 0 on a broken regelwerk is an open gate.
//!
//! Built only with the `validate` feature, like the binary itself (which has
//! `required-features = ["validate"]`). `just test` runs with `--all-features`.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(feature = "validate")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/validate_cli/{name}"))
}

fn run(args: &[PathBuf]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_validate"))
        .args(args)
        .output()
        .expect("the validate binary should start")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Without files nothing was validated, and that is no approval.
#[test]
fn without_arguments_it_is_a_usage_error() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("Usage"),
        "stderr: {}",
        stderr(&output)
    );
}

/// A file that is valid against the schema and clean on units passes the gate.
#[test]
fn a_valid_file_passes() {
    let output = run(&[fixture("valid.yaml")]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("OK:"),
        "stderr: {}",
        stderr(&output)
    );
}

/// A schema violation the lenient law-model does accept (a missing top-level
/// `url`) still fails.
#[test]
fn a_schema_violation_fails() {
    let output = run(&[fixture("schema_violation.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("FAIL"), "stderr: {err}");
    assert!(err.contains("schema"), "stderr: {err}");
    assert!(!err.contains("OK:"), "stderr: {err}");
}

/// A file without `$schema` cannot be validated, so it fails.
#[test]
fn a_missing_schema_field_fails() {
    let output = run(&[fixture("no_schema_field.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("missing $schema field"), "stderr: {err}");
}

/// The unit check (RFC-023) runs after a passing schema validation and counts
/// in the exit code: adding eurocent to days is a FAIL, not a WARN.
#[test]
fn a_unit_mismatch_in_a_valid_file_fails() {
    let output = run(&[fixture("unit_mismatch.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("units:"), "stderr: {err}");
    // The schema passed the file; the failure comes from the units.
    assert!(err.contains("OK:"), "stderr: {err}");
}

/// One broken file among good ones decides the exit code; the other files
/// are still checked.
#[test]
fn a_broken_file_fails_the_whole_run() {
    let output = run(&[fixture("valid.yaml"), fixture("schema_violation.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("OK:"), "stderr: {err}");
    assert!(err.contains("FAIL"), "stderr: {err}");
}

/// The type check (RFC-036/RFC-037) runs after a passing schema validation
/// and counts in the exit code: an absence test on a field that is never
/// absent is a FAIL that names the law, article, output and line.
#[test]
fn a_type_error_in_a_valid_file_fails() {
    let output = run(&[fixture("type_error.yaml")]);
    let err = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains("typecheck:"), "stderr: {err}");
    assert!(err.contains("[N1]"), "stderr: {err}");
    assert!(
        err.contains("article 1 output 'geen_huur'"),
        "stderr: {err}"
    );
    // The schema passed the file; the failure comes from the type check.
    assert!(err.contains("OK:"), "stderr: {err}");
}
