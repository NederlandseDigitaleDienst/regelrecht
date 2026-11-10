//! Golden Tests
//!
//! These tests verify that the Rust engine produces the correct outputs
//! for a comprehensive set of test cases using pre-generated fixtures.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{LawExecutionService, Value};
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

/// Test fixture file structure
// TODO: Consider verifying version compatibility (e.g., semver check against
// a minimum supported version) to catch fixture format changes early.
#[derive(Debug, Deserialize)]
struct FixtureFile {
    /// Fixture format version (reserved for future compatibility checks)
    #[serde(rename = "version")]
    _version: String,
    /// Total test count (informational, not enforced)
    #[serde(rename = "test_count")]
    _test_count: usize,
    test_cases: Vec<TestCase>,
}

/// Individual test case
#[derive(Debug, Deserialize)]
struct TestCase {
    id: String,
    #[serde(default)]
    parameters: HashMap<String, serde_json::Value>,
    #[serde(default)]
    calculation_date: Option<String>,
    #[serde(default)]
    expected: Option<Expected>,
    #[serde(default)]
    law_yaml: Option<String>,
    #[serde(default)]
    multi_law: bool,
    #[serde(default)]
    laws: Vec<LawSpec>,
    #[serde(default)]
    law_id: Option<String>,
    #[serde(default)]
    output_name: Option<String>,
    /// If present, this test case failed to generate in Python
    #[serde(default)]
    generator_error: Option<String>,
}

/// Law specification for multi-law tests
#[derive(Debug, Deserialize)]
struct LawSpec {
    yaml: String,
    law_id: String,
}

/// Expected result
#[derive(Debug, Deserialize)]
struct Expected {
    success: bool,
    #[serde(default)]
    article_number: Option<String>,
    #[serde(default)]
    outputs: HashMap<String, serde_json::Value>,
    #[serde(default)]
    error_type: Option<String>,
}

/// Convert JSON value to engine Value using the canonical From impl.
fn json_to_value(json: &serde_json::Value) -> Value {
    Value::from(json)
}

/// Compare expected JSON value with actual Value
fn values_match(expected: &serde_json::Value, actual: &Value) -> bool {
    match (expected, actual) {
        (serde_json::Value::Null, Value::Null) => true,
        (serde_json::Value::Bool(e), Value::Bool(a)) => e == a,
        (serde_json::Value::String(e), Value::String(a)) => e == a,
        (serde_json::Value::Array(e), Value::Array(a)) => {
            e.len() == a.len() && e.iter().zip(a.iter()).all(|(e, a)| values_match(e, a))
        }
        // Numeric comparison (Int or exact Decimal) with a small tolerance.
        (serde_json::Value::Number(e), actual) => {
            // Known limitation, not an oversight: golden fixtures are JSON, and
            // serde_json parses every number through f64, so the expected side is
            // already f64. We therefore compare in f64 with a 1e-9 tolerance —
            // the engine keeps Decimal precision internally, but the fixture
            // format can't express more than f64 on the expected side.
            let actual_num = match actual {
                Value::Int(a) => Some(*a as f64),
                Value::Decimal(a) => a.to_f64(),
                _ => None,
            };
            match (e.as_f64(), actual_num) {
                (Some(expected), Some(actual)) => (expected - actual).abs() < 1e-9,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Run a single test case
fn run_test_case(test: &TestCase) -> Result<(), String> {
    // Skip test cases that failed to generate in Python
    if let Some(error) = &test.generator_error {
        return Err(format!("SKIPPED (generator error): {}", error));
    }

    // Verify required fields are present
    let law_id = test.law_id.as_ref().ok_or("Missing law_id")?;
    let output_name = test.output_name.as_ref().ok_or("Missing output_name")?;
    let calculation_date = test
        .calculation_date
        .as_ref()
        .ok_or("Missing calculation_date")?;
    let expected = test.expected.as_ref().ok_or("Missing expected")?;

    let mut service = LawExecutionService::new();

    // Load law(s)
    if test.multi_law {
        for law_spec in &test.laws {
            service
                .load_law(&law_spec.yaml)
                .map_err(|e| format!("Failed to load law '{}': {}", law_spec.law_id, e))?;
        }
    } else if let Some(yaml) = &test.law_yaml {
        service
            .load_law(yaml)
            .map_err(|e| format!("Failed to load law '{}': {}", law_id, e))?;
    } else {
        return Err("Test case has neither law_yaml nor multi_law".to_string());
    }

    // Convert parameters
    let params: BTreeMap<String, Value> = test
        .parameters
        .iter()
        .map(|(k, v)| (k.clone(), json_to_value(v)))
        .collect();

    // Execute. The engine computes only what is asked for (RFC-043), so ask
    // for every output the fixture checks, not just the one it names.
    let mut requested: Vec<&str> = vec![output_name.as_str()];
    if expected.success {
        for name in expected.outputs.keys() {
            if !requested.contains(&name.as_str()) {
                requested.push(name);
            }
        }
    }
    let result = service.evaluate_law(law_id, &requested, params, calculation_date);

    // Verify result
    if expected.success {
        let result = result.map_err(|e| {
            format!(
                "Expected success but got error: {} (expected outputs: {:?})",
                e, expected.outputs
            )
        })?;

        // Check outputs
        for (out_name, expected_value) in &expected.outputs {
            let actual = result.outputs.get(out_name).ok_or_else(|| {
                format!(
                    "Output '{}' not found in result. Available: {:?}",
                    out_name,
                    result.outputs.keys().collect::<Vec<_>>()
                )
            })?;

            if !values_match(expected_value, actual) {
                return Err(format!(
                    "Output '{}' mismatch:\n  expected: {:?}\n  actual: {:?}",
                    out_name, expected_value, actual
                ));
            }
        }

        // Check article number if specified
        if let Some(expected_article) = &expected.article_number
            && result.article_number != *expected_article
        {
            return Err(format!(
                "Article number mismatch: expected '{}', got '{}'",
                expected_article, result.article_number
            ));
        }

        Ok(())
    } else {
        // Expected error
        match result {
            Ok(r) => Err(format!(
                "Expected error '{}' but got success with outputs: {:?}",
                expected.error_type.as_deref().unwrap_or("unknown"),
                r.outputs
            )),
            Err(e) => {
                // Verify error type matches (if specified)
                let error_str = e.to_string();
                if let Some(expected_type) = &expected.error_type {
                    let error_type = format!("{:?}", e)
                        .split('(')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if !error_type.contains(expected_type) && !error_str.contains(expected_type) {
                        return Err(format!(
                            "Error type mismatch: expected '{}', got '{}'",
                            expected_type, error_type
                        ));
                    }
                }
                Ok(())
            }
        }
    }
}

/// Load and run all tests from a fixture file
fn run_fixture_file(path: &Path) -> Vec<(String, Result<(), String>)> {
    let content = fs::read_to_string(path).expect("Failed to read fixture file");
    let fixture: FixtureFile =
        serde_json::from_str(&content).expect("Failed to parse fixture file");

    fixture
        .test_cases
        .iter()
        .map(|test| (test.id.clone(), run_test_case(test)))
        .collect()
}

// ============================================================================
// Test modules for each category
// ============================================================================

macro_rules! golden_test_category {
    ($name:ident, $file:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn test_all() {
                let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join("fixtures")
                    .join($file);

                if !fixture_path.exists() {
                    panic!(
                        "Fixture file not found: {}. Fixture missing.",
                        fixture_path.display()
                    );
                }

                let results = run_fixture_file(&fixture_path);
                let mut failures = Vec::new();
                let mut skipped = 0;

                for (test_id, result) in &results {
                    if let Err(e) = result {
                        if e.starts_with("SKIPPED") {
                            skipped += 1;
                        } else {
                            failures.push(format!("  {}: {}", test_id, e));
                        }
                    }
                }

                let passed = results.len() - failures.len() - skipped;

                if !failures.is_empty() {
                    panic!(
                        "\n{} of {} tests failed in {}:\n{}",
                        failures.len(),
                        results.len() - skipped,
                        $file,
                        failures.join("\n")
                    );
                }

                if skipped > 0 {
                    println!(
                        "{}/{} tests passed in {} ({} skipped)",
                        passed, passed, $file, skipped
                    );
                } else {
                    println!("All {} tests passed in {}", passed, $file);
                }
            }
        }
    };
}

golden_test_category!(basic_operations, "basic_operations.json");
golden_test_category!(comparison_operations, "comparison_operations.json");
golden_test_category!(arithmetic_operations, "arithmetic_operations.json");
golden_test_category!(aggregate_operations, "aggregate_operations.json");
golden_test_category!(logical_operations, "logical_operations.json");
golden_test_category!(conditional_operations, "conditional_operations.json");
golden_test_category!(nested_operations, "nested_operations.json");
golden_test_category!(action_execution, "action_execution.json");
golden_test_category!(cross_law_references, "cross_law_references.json");
golden_test_category!(error_cases, "error_cases.json");
golden_test_category!(real_regulations, "real_regulations.json");

/// Combined test that runs all fixtures and reports summary
#[test]
fn test_all_golden_tests() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");

    if !fixtures_dir.exists() {
        panic!(
            "Fixtures directory not found: {}. Fixture missing.",
            fixtures_dir.display()
        );
    }

    let fixture_files = [
        "basic_operations.json",
        "comparison_operations.json",
        "arithmetic_operations.json",
        "aggregate_operations.json",
        "logical_operations.json",
        "conditional_operations.json",
        "nested_operations.json",
        "action_execution.json",
        "cross_law_references.json",
        "error_cases.json",
        "real_regulations.json",
    ];

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;
    let mut all_failures = Vec::new();

    for file in &fixture_files {
        let path = fixtures_dir.join(file);
        if !path.exists() {
            println!("Skipping {} (not found)", file);
            continue;
        }

        let results = run_fixture_file(&path);
        let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
        let skipped = results
            .iter()
            .filter(|(_, r)| matches!(r, Err(e) if e.starts_with("SKIPPED")))
            .count();
        let failed = results.len() - passed - skipped;

        total_passed += passed;
        total_failed += failed;
        total_skipped += skipped;

        for (test_id, result) in results {
            if let Err(e) = result
                && !e.starts_with("SKIPPED")
            {
                all_failures.push(format!("[{file}] {test_id}: {e}"));
            }
        }

        if skipped > 0 {
            println!(
                "{}: {}/{} passed ({} skipped)",
                file,
                passed,
                passed + failed,
                skipped
            );
        } else {
            println!("{}: {}/{} passed", file, passed, passed + failed);
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Total: {}/{} passed ({} skipped)",
        total_passed,
        total_passed + total_failed,
        total_skipped
    );

    if !all_failures.is_empty() {
        println!("\nFailures:");
        for failure in &all_failures {
            println!("  {}", failure);
        }
        panic!("{} tests failed", total_failed);
    }
}
