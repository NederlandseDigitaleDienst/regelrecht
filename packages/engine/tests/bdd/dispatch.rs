//! Generic dispatch for the canonical BDD grammar.
//!
//! The code-generated cucumber steps (see `bdd/grammar.yaml` +
//! `packages/engine/build.rs`) parse their captures into a `Vec<ArgValue>` and
//! call [`RegelrechtWorld::dispatch`]. ALL semantics for the canonical vocabulary
//! live here — codegen carries no per-action knowledge. Bodies are ported from
//! the hand-written `steps/{given,when,then,notes}.rs`.

use std::collections::BTreeSet;

use regelrecht_engine::{
    annotation, Article, OutputProvenance, SelectorHint, TextQuoteSelector, Value,
};
use rust_decimal::Decimal;

use crate::helpers::table::{rows_to_params, rows_to_records, Rows};
use crate::helpers::value_conversion::{convert_gherkin_value, values_equal_with_tolerance};
use crate::world::RegelrechtWorld;

/// One positional argument passed to [`RegelrechtWorld::dispatch`].
#[derive(Debug, Clone)]
pub enum ArgValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

impl ArgValue {
    pub fn as_str(&self) -> &str {
        match self {
            ArgValue::Str(s) => s,
            _ => panic!("expected Str arg, got {self:?}"),
        }
    }

    pub fn as_num(&self) -> f64 {
        match self {
            ArgValue::Num(n) => *n,
            _ => panic!("expected Num arg, got {self:?}"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            ArgValue::Bool(b) => *b,
            _ => panic!("expected Bool arg, got {self:?}"),
        }
    }
}

impl RegelrechtWorld {
    /// Execute one canonical grammar action. Panics (test assertion) on any
    /// mismatch — appropriate for BDD test code.
    pub async fn dispatch(&mut self, action: &str, args: Vec<ArgValue>, table: Option<Rows>) {
        match action {
            // ----- core: setup -----
            "set_calculation_date" => {
                self.calculation_date = args[0].as_str().to_string();
            }
            "load_law" => {
                // All corpus laws are preloaded in Self::new(); verify presence so
                // a typo'd law id fails loudly rather than silently later.
                let law = args[0].as_str();
                assert!(
                    self.service.has_law(law),
                    "law '{law}' is not loaded (preloaded corpus)"
                );
            }
            "set_parameter" => {
                let name = args[0].as_str().to_string();
                let value = match &args[1] {
                    ArgValue::Num(n) => bare_value(*n),
                    ArgValue::Str(s) => quoted_value(s),
                    ArgValue::Bool(b) => Value::Bool(*b),
                };
                self.parameters.insert(name, value);
            }
            "set_parameters_table" => {
                for (k, v) in rows_to_params(&table.expect("parameters table"), table_cell_value) {
                    self.parameters.insert(k, v);
                }
            }
            "set_findings_table" => {
                for (k, v) in rows_to_params(&table.expect("findings table"), table_cell_value) {
                    self.findings.insert(k, v);
                }
            }
            "set_data_source" => {
                let source = args[0].as_str().to_string();
                let key = args[1].as_str().to_string();
                let rows = rows_to_records(&table.expect("data source table"), table_cell_value);
                self.data_sources.insert(source, (key, rows));
            }
            "set_data_source_for_law" => {
                let source = args[0].as_str().to_string();
                let key = args[1].as_str().to_string();
                let law = args[2].as_str().to_string();
                let rows = rows_to_records(&table.expect("data source table"), table_cell_value);
                self.scoped_data_sources.push((law, source, key, rows));
            }
            "set_parameter_collection" => {
                let name = args[0].as_str().to_string();
                let records = rows_to_records(&table.expect("collection table"), table_cell_value);
                let items = records.into_iter().map(Value::Object).collect();
                self.parameters.insert(name, Value::Array(items));
            }

            // ----- core/provenance: execute -----
            "evaluate" => {
                let output = args[0].as_str().to_string();
                let law = args[1].as_str().to_string();
                self.run_evaluation(&law, &[output]);
            }
            "evaluate_outputs" => {
                let outputs: Vec<String> = args[0]
                    .as_str()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                let law = args[1].as_str().to_string();
                self.run_evaluation(&law, &outputs);
            }

            // ----- core: asserts -----
            "assert_succeeds" => assert!(
                self.error.is_none(),
                "expected success, got {:?}",
                self.error_message()
            ),
            "assert_fails" => assert!(
                self.error.is_some(),
                "expected failure, but succeeded with result: {:?}",
                self.result
            ),
            "assert_fails_with" => {
                let needle = args[0].as_str().to_lowercase();
                let msg = self.error_message().unwrap_or_default().to_lowercase();
                assert!(
                    msg.contains(&needle),
                    "error {msg:?} does not contain {needle:?}"
                );
            }
            "assert_boolean" => {
                let actual = self.output_value(args[0].as_str());
                assert_eq!(
                    actual,
                    Value::Bool(args[1].as_bool()),
                    "output {}",
                    args[0].as_str()
                );
            }
            "assert_equals" => {
                let expected = match &args[1] {
                    ArgValue::Num(n) => bare_value(*n),
                    ArgValue::Str(s) => quoted_value(s),
                    ArgValue::Bool(b) => Value::Bool(*b),
                };
                let actual = self.output_value(args[0].as_str());
                assert!(
                    values_equal_with_tolerance(&actual, &expected),
                    "output {} = {actual:?}, expected {expected:?}",
                    args[0].as_str()
                );
            }
            "assert_null" => {
                let actual = self.output_value(args[0].as_str());
                assert_eq!(actual, Value::Null, "output {}", args[0].as_str());
            }
            "assert_unknown" => {
                let actual = self.output_value(args[0].as_str());
                assert!(
                    actual.is_unknown(),
                    "output {} = {actual:?}, expected unknown",
                    args[0].as_str()
                );
            }
            "assert_unknown_for" => {
                let actual = self.output_value(args[0].as_str());
                let name = args[1].as_str();
                assert!(
                    actual.is_unknown(),
                    "output {} = {actual:?}, expected unknown for lack of {name:?}",
                    args[0].as_str()
                );
                let missing: Vec<&str> = actual
                    .missing_facts()
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect();
                assert!(
                    missing.contains(&name),
                    "output {} is unknown for lack of {missing:?}, not {name:?}",
                    args[0].as_str()
                );
            }
            "assert_contains" => {
                let actual = self.output_value(args[0].as_str());
                let needle = args[1].as_str();
                match actual {
                    Value::String(s) => assert!(
                        s.to_lowercase().contains(&needle.to_lowercase()),
                        "{s:?} !contains {needle:?}"
                    ),
                    other => panic!("output {} is {other:?}, not a string", args[0].as_str()),
                }
            }

            // ----- provenance tier -----
            "assert_exact_outputs" => {
                let expected: BTreeSet<String> = args[0]
                    .as_str()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                let actual = self.result_output_keys();
                assert_eq!(actual, expected, "output key set mismatch");
            }
            "assert_provenance" => {
                self.assert_provenance(args[0].as_str(), args[1].as_str());
            }

            // ----- untranslatable tier -----
            "set_untranslatable_mode" => {
                self.service
                    .set_untranslatable_mode(parse_untranslatable_mode(args[0].as_str()));
            }
            "assert_tainted" => {
                assert!(
                    self.output_is_untranslatable(args[0].as_str()),
                    "output {} not tainted untranslatable",
                    args[0].as_str()
                );
            }

            // ----- notes tier -----
            "set_note_articles" => self.note_set_articles(&table.expect("articles table")),
            "set_note_selector_exact" => self.note_selector_exact(args[0].as_str()),
            "set_note_selector_context" => {
                self.note_selector_context(args[0].as_str(), args[1].as_str(), args[2].as_str())
            }
            "set_note_hint_article" => self.note_hint_article(args[0].as_str()),
            "set_note_hint_position" => self.note_hint_position(
                args[0].as_str(),
                args[1].as_num() as usize,
                args[2].as_num() as usize,
            ),
            "resolve_note" => self.note_resolve(),
            "assert_note_resolves" => self.note_assert_resolves(args[0].as_str()),
            "assert_note_exact_match" => self.note_assert_exact(),
            "assert_note_fuzzy_match" => self.note_assert_fuzzy(),
            "assert_note_orphaned" => self.note_assert_orphaned(),
            "assert_note_ambiguous" => self.note_assert_ambiguous(),

            other => {
                if !crate::extension::dispatch(self, other, args, table) {
                    panic!("unknown action '{other}' — grammar/dispatch out of sync")
                }
            }
        }
    }

    /// Register every generic data source onto the service, then evaluate the
    /// law for the requested outputs (storing `result`/`error`). Mirrors
    /// `when.rs::register_if_present` + `world.execute_law_multi`.
    pub(crate) fn run_evaluation(&mut self, law: &str, outputs: &[String]) {
        for (name, (key, records)) in &self.data_sources {
            if !records.is_empty() {
                self.service
                    .register_dict_source(name, key, records.clone())
                    .expect("Failed to register data source");
            }
        }
        for (law, name, key, records) in &self.scoped_data_sources {
            if !records.is_empty() {
                self.service
                    .register_dict_source_for_law(law, name, key, records.clone(), 10)
                    .expect("Failed to register scoped data source");
            }
        }
        self.requested_outputs = outputs.to_vec();
        let output_refs: Vec<&str> = outputs.iter().map(|s| s.as_str()).collect();
        self.execute_law_multi(law, &output_refs);
    }

    /// Read a named output from the last result, panicking with a clear message
    /// when there is no result or the output is missing. Mirrors
    /// `then.rs::assert_output_value`.
    pub(crate) fn output_value(&self, name: &str) -> Value {
        assert!(
            self.is_success(),
            "expected successful execution, got error: {:?}",
            self.error_message()
        );
        match self.get_output(name) {
            Some(value) => value.clone(),
            None => {
                let available: Vec<&String> = self
                    .result
                    .as_ref()
                    .map(|r| r.outputs.keys().collect())
                    .unwrap_or_default();
                panic!("output '{name}' not found. Available outputs: {available:?}");
            }
        }
    }

    /// Set of output keys present on the last result. From
    /// `then.rs::assert_exact_outputs`.
    fn result_output_keys(&self) -> BTreeSet<String> {
        assert!(
            self.is_success(),
            "expected successful execution, got error: {:?}",
            self.error_message()
        );
        self.result
            .as_ref()
            .expect("result")
            .outputs
            .keys()
            .cloned()
            .collect()
    }

    /// Assert the provenance kind of an output. From `then.rs`.
    fn assert_provenance(&self, output: &str, kind: &str) {
        assert!(self.is_success(), "expected successful execution");
        let result = self.result.as_ref().expect("result");
        let prov = result.output_provenance.get(output);
        let matches = match kind {
            "direct" => matches!(prov, Some(OutputProvenance::Direct { .. })),
            "reactive" => matches!(prov, Some(OutputProvenance::Reactive { .. })),
            "override" => matches!(prov, Some(OutputProvenance::Override { .. })),
            // A voided output is absent from `outputs` and present here with
            // its ground, so a scenario can assert the exclusion rather than
            // only the missing value.
            "voided" => matches!(prov, Some(OutputProvenance::Voided { .. })),
            other => panic!("unknown provenance kind '{other}'"),
        };
        assert!(
            matches,
            "expected '{output}' to have {kind} provenance, got {prov:?}"
        );
    }

    /// True when the named output is tainted untranslatable. From
    /// `then.rs::assert_output_untranslatable`.
    fn output_is_untranslatable(&self, output: &str) -> bool {
        self.output_value(output).is_untranslatable()
    }

    // ----- note helpers (ported verbatim from notes.rs) -----

    /// Reads the article table by header name, like `rows_to_records` does for
    /// data-source tables. It used to read `row[0]`/`row[1]` positionally while
    /// carrying a `number | text` header that nothing checked, so swapping the
    /// two columns (header included) silently changed what the scenario meant.
    /// One convention for every table in this language: the header is the
    /// contract.
    fn note_set_articles(&mut self, table: &Rows) {
        self.note_articles.clear();
        let headers: Vec<String> = table
            .first()
            .map(|row| row.iter().map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();
        let column = |name: &str| -> usize {
            headers.iter().position(|h| h == name).unwrap_or_else(|| {
                panic!("article table needs a '{name}' column; header is {headers:?}")
            })
        };
        let (number_at, text_at) = (column("number"), column("text"));
        for row in table.iter().skip(1) {
            self.note_articles.push(Article {
                number: row[number_at].trim().to_string(),
                text: row[text_at].trim().to_string(),
                url: None,
                placement: None,
                machine_readable: None,
                references: None,
            });
        }
    }

    fn note_selector_exact(&mut self, exact: &str) {
        self.note_selector = Some(TextQuoteSelector {
            exact: exact.to_string(),
            prefix: String::new(),
            suffix: String::new(),
            hint: None,
        });
    }

    fn note_selector_context(&mut self, exact: &str, prefix: &str, suffix: &str) {
        self.note_selector = Some(TextQuoteSelector {
            exact: exact.to_string(),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            hint: None,
        });
    }

    fn note_hint_article(&mut self, article: &str) {
        let selector = self
            .note_selector
            .as_mut()
            .expect("selector must be set before adding a hint");
        selector.hint = Some(SelectorHint {
            article_number: article.to_string(),
            start: None,
            end: None,
        });
    }

    fn note_hint_position(&mut self, article: &str, start: usize, end: usize) {
        let selector = self
            .note_selector
            .as_mut()
            .expect("selector must be set before adding a hint");
        selector.hint = Some(SelectorHint {
            article_number: article.to_string(),
            start: Some(start),
            end: Some(end),
        });
    }

    fn note_resolve(&mut self) {
        let selector = self
            .note_selector
            .as_ref()
            .expect("selector must be set before resolving");
        self.note_result = Some(annotation::resolve(selector, &self.note_articles));
    }

    fn note_assert_resolves(&self, article: &str) {
        let result = self.note_result.as_ref().expect("note must be resolved");
        assert!(
            result.is_found(),
            "expected Found, got {:?} ({} matches)",
            result.status,
            result.matches.len()
        );
        assert_eq!(
            result.single().expect("single match").article_number,
            article
        );
    }

    fn note_assert_exact(&self) {
        let result = self.note_result.as_ref().expect("note must be resolved");
        let m = result.single().expect("expected a single match");
        assert_eq!(m.confidence, 1.0, "expected exact (confidence 1.0)");
    }

    fn note_assert_fuzzy(&self) {
        let result = self.note_result.as_ref().expect("note must be resolved");
        let m = result.single().expect("expected a single match");
        assert!(
            m.confidence < 1.0,
            "expected fuzzy (confidence < 1.0), got {}",
            m.confidence
        );
    }

    fn note_assert_orphaned(&self) {
        let result = self.note_result.as_ref().expect("note must be resolved");
        assert!(
            result.is_orphaned(),
            "expected Orphaned, got {:?}",
            result.status
        );
    }

    fn note_assert_ambiguous(&self) {
        let result = self.note_result.as_ref().expect("note must be resolved");
        assert!(
            result.is_ambiguous(),
            "expected Ambiguous, got {:?}",
            result.status
        );
    }
}

/// Convert a captured number to an engine `Value` (Int if integral, else exact
/// `Decimal`). The engine uses `Decimal` for non-integer numbers (RFC-024), so
/// we never produce a float. Non-integral literals are routed through the
/// shortest round-trip string repr and parsed as `Decimal`, so a literal like
/// `0.5` or `3.14` becomes the exact decimal it reads as — not its f64
/// approximation.
fn num_to_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.is_finite() {
        Value::Int(n as i64)
    } else {
        Value::Decimal(
            format!("{n}")
                .parse::<Decimal>()
                .expect("numeric literal parses as Decimal"),
        )
    }
}

/// Type a quoted capture. The rule lives in `bdd/grammar.yaml`
/// (`value_typing.quoted`) and reaches both dispatchers through codegen, so
/// neither engine can hold its own opinion about what quotes mean.
fn quoted_value(raw: &str) -> Value {
    match crate::generated_steps::QUOTED_VALUE_TYPING {
        "literal" => Value::String(raw.to_string()),
        "inferred" => convert_gherkin_value(raw),
        other => panic!("bdd/grammar.yaml: unknown value_typing.quoted '{other}'"),
    }
}

/// Type a bare (unquoted) capture; see [`quoted_value`] for where the rule lives.
fn bare_value(n: f64) -> Value {
    match crate::generated_steps::BARE_VALUE_TYPING {
        "number" => num_to_value(n),
        other => panic!("bdd/grammar.yaml: unknown value_typing.bare '{other}'"),
    }
}

/// Type a data-table cell; see [`quoted_value`] for where the rule lives.
pub(crate) fn table_cell_value(raw: &str) -> Value {
    match crate::generated_steps::TABLE_CELL_VALUE_TYPING {
        "inferred" => convert_gherkin_value(raw),
        "literal" => Value::String(raw.trim().to_string()),
        other => panic!("bdd/grammar.yaml: unknown value_typing.table_cell '{other}'"),
    }
}

/// Parse the untranslatable mode string into the engine enum. From `given.rs`.
fn parse_untranslatable_mode(mode: &str) -> regelrecht_engine::UntranslatableMode {
    mode.parse()
        .unwrap_or_else(|e| panic!("Invalid untranslatable mode: {e}"))
}
