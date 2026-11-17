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

/// A law that tests a location against an area, for a request that gives
/// the location as a parameter.
const OVERLAP_LAW_YAML: &str = r#"---
$id: test_bin_evaluate_overlap
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Een locatie ligt in het gebied indien zij het overlapt.
    machine_readable:
      execution:
        parameters:
          - name: locatie
            type: shape
            required: true
        output:
          - name: ligt_in_gebied
            type: boolean
        actions:
          - output: ligt_in_gebied
            value:
              operation: OVERLAPS
              subject: $locatie
              value: POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))
"#;

/// Twenty thousand nested collections in a shape overflowed the stack of the
/// WKT parser, and the process died on a signal with nothing on stdout. The
/// binary now answers with an error response and a status of its own.
#[test]
fn a_deeply_nested_shape_is_an_error_response_and_no_abort() {
    let nested = format!(
        "{}POINT(1 1){}",
        "GEOMETRYCOLLECTION(".repeat(20_000),
        ")".repeat(20_000)
    );
    let deep = format!("POLYGON{}0 0{}", "(".repeat(20_000), ")".repeat(20_000));
    for (location, named) in [
        (nested, "is a collection of geometries"),
        (deep, "nests parentheses deeper than 3"),
    ] {
        let run = run_evaluate(
            &[],
            &serde_json::json!({
                "law_yaml": OVERLAP_LAW_YAML,
                "output_names": ["ligt_in_gebied"],
                "params": {"locatie": location},
                "date": "2025-01-01",
            }),
        );
        assert!(
            !run.success,
            "expected a non-zero exit code: {}",
            run.stdout
        );
        // An abort leaves stdout empty; an error response is JSON.
        let resp: serde_json::Value =
            serde_json::from_str(&run.stdout).expect("stdout is not valid JSON");
        let error = resp["error"].as_str().expect("error message");
        assert!(
            error.contains("the subject of OVERLAPS is not a shape") && error.contains(named),
            "{error}"
        );
    }
}

/// A law whose open term is filled by the regulation of the place, the law
/// that gives the territory of a municipality, and the plan of one.
const PLACE_LAW_YAML: &str = r#"---
$id: test_bin_evaluate_place
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Verboden voor zover het omgevingsplan dat bepaalt.
    machine_readable:
      open_terms:
        - id: verboden
          type: boolean
          delegation_type: GEMEENTELIJKE_VERORDENING
          location: $locatie
          territory:
            regulation: test_bin_evaluate_boundaries
            output: grens
          combine: OR
      execution:
        parameters:
          - name: locatie
            type: shape
            required: true
        output:
          - name: vergunningplichtig
            type: boolean
        actions:
          - output: vergunningplichtig
            value: $verboden
"#;

const BOUNDARIES_LAW_YAML: &str = r#"---
$id: test_bin_evaluate_boundaries
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De grens van de gemeente is de grens op de kaart bij deze wet.
    machine_readable:
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
        output:
          - name: grens
            type: shape
        actions:
          - output: grens
            value: POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))
"#;

const PLACE_PLAN_YAML: &str = r#"---
$id: test_bin_evaluate_plan
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
gemeente_code: GM9901
articles:
  - number: '1'
    text: Het is verboden te bouwen.
    machine_readable:
      implements:
        - law: test_bin_evaluate_place
          article: '1'
          open_term: verboden
      execution:
        output:
          - name: verboden
            type: boolean
        actions:
          - output: verboden
            value: true
"#;

/// The response says how a term that is filled by the regulation of a place
/// was asked: the location, each territory it was held against and what each
/// implementation gave. The territory is in no parameter, so without this a
/// caller could not tell what the outcome rests on.
#[test]
fn the_response_says_how_a_located_term_was_asked() {
    let run = run_evaluate(
        &[],
        &serde_json::json!({
            "law_yaml": PLACE_LAW_YAML,
            "extra_laws": [BOUNDARIES_LAW_YAML, PLACE_PLAN_YAML],
            "output_names": ["vergunningplichtig"],
            "params": {"locatie": "POINT(5 5)"},
            "date": "2025-01-01",
        }),
    );
    assert!(run.success, "expected exit code 0, stdout: {}", run.stdout);
    let resp: serde_json::Value =
        serde_json::from_str(&run.stdout).expect("stdout is not valid JSON");
    assert_eq!(
        resp["outputs"]["vergunningplichtig"],
        serde_json::json!(true)
    );
    let asked = &resp["open_terms_asked"][0];
    assert_eq!(asked["term"], "verboden", "{}", run.stdout);
    assert_eq!(asked["location"], "POINT(5 5)");
    assert_eq!(asked["location_covered"], true);
    let territory = &asked["territories"][0];
    assert_eq!(territory["law_id"], "test_bin_evaluate_plan");
    assert_eq!(territory["code"], "GM9901");
    assert_eq!(territory["source"], "test_bin_evaluate_boundaries#grens");
    assert_eq!(territory["overlaps"], true);
    assert!(
        territory["territory"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:"))
    );
    assert_eq!(asked["answers"][0]["gave"], "true");

    // A law without such a term has no record to show.
    let plain = run_evaluate(&[], &request());
    let resp: serde_json::Value =
        serde_json::from_str(&plain.stdout).expect("stdout is not valid JSON");
    assert!(resp.get("open_terms_asked").is_none(), "{}", plain.stdout);
}
