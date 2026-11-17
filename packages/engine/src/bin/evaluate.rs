//! CLI binary for evaluating a law YAML via stdin.
//!
//! Usage:
//!   echo '{"law_yaml": "...", "output_names": ["a", "b"], "params": {...}, "date": "2025-01-01"}' \
//!     | cargo run --bin evaluate
//!
//! Input (JSON on stdin):
//!   - law_yaml: String — the full YAML content of the law
//!   - output_names: Optional<Vec<String>> — outputs to evaluate (preferred)
//!   - output_name: Optional<String> — single output to evaluate (backwards compat)
//!   - params: Object — key-value parameters to pass to the engine
//!   - date: String — evaluation date (YYYY-MM-DD)
//!   - extra_laws: Optional<Vec<String>> — additional YAML laws for cross-law resolution
//!
//! Output (JSON on stdout):
//!   - outputs: Object — computed output values (only requested outputs)
//!   - resolved_inputs: Object — resolved input values from cross-law references
//!   - article_number: String — the article that was executed
//!   - law_id: String — the law ID that was evaluated
//!   - law_uuid: Optional<String> — the law UUID if available
//!   - error: Optional<String> — error message if evaluation failed

use regelrecht_engine::{LawExecutionService, OutputProvenance, UntranslatableMode, Value};
use std::collections::BTreeMap;
use std::io::Read;

#[derive(serde::Deserialize)]
struct EvaluateRequest {
    law_yaml: String,
    /// Multiple outputs to evaluate (preferred).
    #[serde(default)]
    output_names: Option<Vec<String>>,
    /// Single output to evaluate (backwards compat; ignored if output_names is set).
    #[serde(default)]
    output_name: Option<String>,
    params: BTreeMap<String, serde_json::Value>,
    date: String,
    #[serde(default)]
    extra_laws: Vec<String>,
}

#[derive(serde::Serialize)]
struct EvaluateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    outputs: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_inputs: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    article_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// Engine version that produced this result (RFC-013)
    engine_version: String,
    /// Schema version of the evaluated regulation (RFC-013)
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<String>,
    /// SHA-256 hash of the regulation YAML content (RFC-013)
    #[serde(skip_serializing_if = "Option::is_none")]
    regulation_hash: Option<String>,
    /// Per-output provenance (Direct/Reactive/Override)
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    output_provenance: BTreeMap<String, OutputProvenance>,
}

fn error_response(msg: String) -> EvaluateResponse {
    EvaluateResponse {
        outputs: None,
        resolved_inputs: None,
        article_number: None,
        law_id: None,
        law_uuid: None,
        error: Some(msg),
        engine_version: regelrecht_engine::VERSION.to_string(),
        schema_version: None,
        regulation_hash: None,
        output_provenance: BTreeMap::new(),
    }
}

fn main() {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        let resp = error_response(format!("Failed to read stdin: {e}"));
        println!("{}", serde_json::to_string(&resp).unwrap_or_default());
        std::process::exit(1);
    }

    let request: EvaluateRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            let resp = error_response(format!("Failed to parse request JSON: {e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    };

    // Validate date format (YYYY-MM-DD)
    if chrono::NaiveDate::parse_from_str(&request.date, "%Y-%m-%d").is_err() {
        let resp = error_response(format!(
            "Invalid date format '{}': expected YYYY-MM-DD",
            request.date
        ));
        println!("{}", serde_json::to_string(&resp).unwrap_or_default());
        std::process::exit(1);
    }

    let mut service = LawExecutionService::new();

    // Parse CLI flags
    let mut emit_receipt = false;
    for arg in std::env::args().skip(1) {
        if let Some(mode_str) = arg.strip_prefix("--untranslatable=") {
            // RFC-012: untranslatable handling mode
            match mode_str.parse::<UntranslatableMode>() {
                Ok(mode) => service.set_untranslatable_mode(mode),
                Err(e) => {
                    let resp = error_response(e);
                    println!("{}", serde_json::to_string(&resp).unwrap_or_default());
                    std::process::exit(1);
                }
            }
        } else if arg == "--receipt" {
            // RFC-013: emit Execution Receipt envelope
            emit_receipt = true;
        }
    }

    // Load the primary law and capture its ID
    let law_id = match service.load_law(&request.law_yaml) {
        Ok(id) => id,
        Err(e) => {
            let resp = error_response(format!("Failed to load law YAML: {e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    };

    // Load additional laws for cross-law resolution
    for extra_yaml in &request.extra_laws {
        if let Err(e) = service.load_law(extra_yaml) {
            let resp = error_response(format!("Failed to load extra law YAML: {e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    }

    // Convert params
    let params: BTreeMap<String, Value> = request
        .params
        .iter()
        .map(|(k, v)| (k.clone(), Value::from(v)))
        .collect();

    // Resolve output names: output_names > output_name > error
    let output_names: Vec<String> = if let Some(names) = request.output_names {
        if names.is_empty() {
            let resp = error_response("output_names must not be empty".to_string());
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
        names
    } else if let Some(name) = request.output_name {
        vec![name]
    } else {
        let resp =
            error_response("Either 'output_names' or 'output_name' must be specified".to_string());
        println!("{}", serde_json::to_string(&resp).unwrap_or_default());
        std::process::exit(1);
    };
    let output_refs: Vec<&str> = output_names.iter().map(|s| s.as_str()).collect();

    // Evaluate
    match service.evaluate_law(&law_id, &output_refs, params.clone(), &request.date) {
        Ok(result) => {
            if emit_receipt {
                let receipt = service.build_receipt_with_outputs(
                    &result,
                    &params,
                    &request.date,
                    &output_names,
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(&receipt).unwrap_or_default()
                );
            } else {
                let outputs: BTreeMap<String, serde_json::Value> = result
                    .outputs
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::from(v)))
                    .collect();
                let resolved_inputs: BTreeMap<String, serde_json::Value> = result
                    .resolved_inputs
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::from(v)))
                    .collect();
                let resp = EvaluateResponse {
                    outputs: Some(outputs),
                    resolved_inputs: Some(resolved_inputs),
                    article_number: Some(result.article_number),
                    law_id: Some(result.law_id),
                    law_uuid: result.law_uuid,
                    error: None,
                    engine_version: result.engine_version,
                    schema_version: result.schema_version,
                    regulation_hash: result.regulation_hash,
                    output_provenance: result.output_provenance,
                };
                println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            }
        }
        Err(e) => {
            let resp = error_response(format!("{e}"));
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
            std::process::exit(1);
        }
    }
}
