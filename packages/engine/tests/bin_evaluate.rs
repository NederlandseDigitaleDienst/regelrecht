//! Contract-tests for the `evaluate` CLI binary.
//!
//! The binary is the process boundary that external callers use: a JSON request
//! on stdin, a JSON response on stdout, and flags that change the shape of that
//! response. These tests pin that contract, not the engine internals behind it.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;
use std::process::{Command, Stdio};

/// Self-contained law with one constant output, so the test says something about
/// the CLI and nothing about the corpus.
const LAW_YAML: &str = r#"---
$schema: >-
  https://raw.githubusercontent.com/MinBZK/regelrecht/refs/heads/main/schema/v0.5.3/schema.json
$id: test_bin_evaluate
name: Testwet bin evaluate
regulatory_layer: WET
bwb_id: BWBR9999903
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/test_bin_evaluate

articles:
  - number: '1'
    url: https://example.com/test_bin_evaluate/1
    text: >-
      Het normbedrag bedraagt 500 eurocent.
    machine_readable:
      execution:
        output:
          - name: normbedrag
            type: amount
            type_spec:
              unit: eurocent
        actions:
          - output: normbedrag
            value: 500
"#;

struct Run {
    success: bool,
    stdout: String,
}

fn run_evaluate(args: &[&str], request: &serde_json::Value) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_evaluate"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn evaluate binary");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(request.to_string().as_bytes())
        .expect("failed to write request to stdin");

    let output = child.wait_with_output().expect("failed to wait for binary");
    Run {
        success: output.status.success(),
        stdout: String::from_utf8(output.stdout).expect("stdout is not valid UTF-8"),
    }
}

fn request() -> serde_json::Value {
    serde_json::json!({
        "law_yaml": LAW_YAML,
        "output_names": ["normbedrag"],
        "params": {},
        "date": "2025-01-01",
    })
}

/// Without flags the binary answers with the plain response envelope: the
/// requested outputs, the article that produced them, and the law they came from.
#[test]
fn evaluate_writes_the_requested_outputs_to_stdout() {
    let run = run_evaluate(&[], &request());
    assert!(run.success, "expected exit code 0, stdout: {}", run.stdout);

    let resp: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("stdout is not valid JSON");

    assert_eq!(resp["outputs"]["normbedrag"], serde_json::json!(500));
    assert_eq!(resp["law_id"], "test_bin_evaluate");
    assert_eq!(resp["article_number"], "1");
    assert!(
        resp["engine_version"].is_string(),
        "engine_version is required by RFC-013, got: {}",
        run.stdout
    );
    assert!(
        resp.get("provenance").is_none(),
        "without --receipt the response must not be a receipt envelope: {}",
        run.stdout
    );
}

/// `--receipt` (RFC-013) swaps the plain response for the Execution Receipt
/// envelope, which carries the provenance and the requested outputs.
#[test]
fn receipt_flag_emits_the_execution_receipt_envelope() {
    let run = run_evaluate(&["--receipt"], &request());
    assert!(run.success, "expected exit code 0, stdout: {}", run.stdout);

    let receipt: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("stdout is not valid JSON");

    assert_eq!(receipt["provenance"]["regulation_id"], "test_bin_evaluate");
    assert_eq!(
        receipt["results"]["requested_outputs"],
        serde_json::json!(["normbedrag"])
    );
    assert_eq!(
        receipt["results"]["outputs"]["normbedrag"],
        serde_json::json!(500)
    );
    assert_eq!(receipt["execution"]["calculation_date"], "2025-01-01");
}

/// An unparsable date is rejected before any law is loaded, with a non-zero exit
/// code and the error in the response envelope rather than a panic.
#[test]
fn an_invalid_date_fails_with_an_error_response() {
    let mut req = request();
    req["date"] = serde_json::json!("01-01-2025");

    let run = run_evaluate(&[], &req);
    assert!(
        !run.success,
        "expected a non-zero exit code: {}",
        run.stdout
    );

    let resp: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("stdout is not valid JSON");
    let error = resp["error"].as_str().expect("error message");
    assert!(
        error.contains("01-01-2025") && error.contains("YYYY-MM-DD"),
        "error must name the offending date and the expected format, got: {error}"
    );
    assert!(resp.get("outputs").is_none(), "no outputs on failure");
}
