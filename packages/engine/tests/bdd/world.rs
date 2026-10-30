//! World struct for Cucumber BDD tests
//!
//! Contains the test state that persists across steps in a scenario.

#![allow(unused_imports)]

use cucumber::World;
use regelrecht_engine::{
    Article, ArticleResult, EngineError, LawExecutionService, MatchResult, TextQuoteSelector, Value,
};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global scenario counter for unique trace file names.
static SCENARIO_COUNTER: AtomicUsize = AtomicUsize::new(0);

use crate::helpers::regulation_loader::load_all_regulations;

/// Test world that holds state across steps in a Cucumber scenario.
#[derive(World)]
#[world(init = Self::new)]
pub struct RegelrechtWorld {
    /// Law execution service with all regulations loaded
    pub service: LawExecutionService,
    /// Calculation date for the current scenario
    pub calculation_date: String,
    /// Parameters for law execution
    pub parameters: BTreeMap<String, Value>,
    /// What an authority found for this case, apart from the parameters
    /// (RFC-045 §5)
    pub findings: BTreeMap<String, Value>,
    /// Last execution result (if successful)
    pub result: Option<ArticleResult>,
    /// Last error (if execution failed)
    pub error: Option<EngineError>,
    /// Articles set up for note-resolution scenarios (RFC-005, RFC-018)
    pub note_articles: Vec<Article>,
    /// Selector built for the current note-resolution scenario
    pub note_selector: Option<TextQuoteSelector>,
    /// Result of the last note resolution
    pub note_result: Option<MatchResult>,
    /// Generic data sources for canonical steps: source name -> (key field, rows)
    pub data_sources: BTreeMap<String, (String, Vec<BTreeMap<String, Value>>)>,
    /// Data sources bound to one law: (law id, source name, key field, rows).
    /// Registered with `register_dict_source_for_law` at evaluation time.
    pub scoped_data_sources: Vec<(String, String, String, Vec<BTreeMap<String, Value>>)>,
    /// Outputs requested by the last canonical `evaluate`/`evaluate_outputs`
    pub requested_outputs: Vec<String>,
}

impl fmt::Debug for RegelrechtWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegelrechtWorld")
            .field("calculation_date", &self.calculation_date)
            .field("parameters", &self.parameters)
            .field("result", &self.result)
            .field("error", &self.error.as_ref().map(|e| e.to_string()))
            .field(
                "service",
                &format!("<{} laws loaded>", self.service.law_count()),
            )
            .finish()
    }
}

impl Default for RegelrechtWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl RegelrechtWorld {
    /// Create a new world with all regulations loaded.
    pub fn new() -> Self {
        let mut service = LawExecutionService::new();

        // Load all regulations from the regulation directory
        if let Err(e) = load_all_regulations(&mut service) {
            panic!("Failed to load regulations: {}", e);
        }

        Self {
            service,
            calculation_date: "2024-01-01".to_string(),
            parameters: BTreeMap::new(),
            findings: BTreeMap::new(),
            result: None,
            error: None,
            note_articles: Vec::new(),
            note_selector: None,
            note_result: None,
            data_sources: BTreeMap::new(),
            scoped_data_sources: Vec::new(),
            requested_outputs: Vec::new(),
        }
    }

    /// Returns true if trace output is enabled via the `TRACE` env var.
    fn trace_enabled() -> bool {
        std::env::var("TRACE").is_ok_and(|v| !v.is_empty() && v != "0")
    }

    /// Execute a law for multiple specific outputs.
    pub fn execute_law_multi(&mut self, law_id: &str, output_names: &[&str]) {
        let trace = Self::trace_enabled();

        let outcome = if trace {
            self.service.evaluate_law_with_trace_and_findings(
                law_id,
                output_names,
                self.parameters.clone(),
                self.findings.clone(),
                &self.calculation_date,
            )
        } else {
            self.service.evaluate_law_with_findings(
                law_id,
                output_names,
                self.parameters.clone(),
                self.findings.clone(),
                &self.calculation_date,
            )
        };

        match outcome {
            Ok(result) => {
                if trace {
                    let output_label = output_names.first().copied().unwrap_or("multi");
                    self.write_trace(&result, law_id, output_label);
                }
                self.result = Some(result);
                self.error = None;
            }
            Err(e) => {
                self.result = None;
                self.error = Some(e);
            }
        }
    }

    /// Write the box-drawing trace for a successful traced execution.
    fn write_trace(&self, result: &ArticleResult, law_id: &str, output_name: &str) {
        let trace = match &result.trace {
            Some(t) => t,
            None => return,
        };

        let dir = trace_output_dir();
        std::fs::create_dir_all(&dir).expect("Failed to create trace output directory");

        let seq = SCENARIO_COUNTER.fetch_add(1, Ordering::SeqCst);
        let safe_law_id = law_id.replace('/', "_");
        let safe_output = output_name.replace('/', "_");
        let filename = format!(
            "{:03}_{}_{}_{}.txt",
            seq, safe_law_id, safe_output, self.calculation_date
        );
        let path = dir.join(&filename);

        std::fs::write(&path, trace.render_box_drawing()).expect("Failed to write trace file");

        eprintln!("  trace → {}", path.display());
    }

    /// Get an output value from the last result
    pub fn get_output(&self, name: &str) -> Option<&Value> {
        self.result.as_ref()?.outputs.get(name)
    }

    /// Check if the last execution was successful
    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }

    /// Get error message if execution failed
    pub fn error_message(&self) -> Option<String> {
        self.error.as_ref().map(|e| e.to_string())
    }
}

/// Directory where trace files are written.
fn trace_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("trace_output")
}
