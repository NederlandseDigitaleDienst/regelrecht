//! WASM bindings for the RegelRecht engine
//!
//! This module provides WebAssembly bindings backed by `LawExecutionService`,
//! giving full cross-law resolution and data source support in the browser.
//! It is feature-gated behind the `wasm` feature flag.
//!
//! # Key Constraints
//!
//! - **No filesystem access in WASM**: Laws must be passed as YAML strings via `load_law()`
//! - **Efficient serialization**: Uses `serde-wasm-bindgen` for Rust <-> JavaScript conversion
//!
//! # Cross-Law Resolution
//!
//! Unlike the previous `ArticleEngine`-based implementation, this version uses
//! `LawExecutionService` internally. When all referenced laws are loaded,
//! cross-law references are resolved automatically:
//!
//! ```javascript
//! const engine = new WasmEngine();
//! engine.loadLaw(wetOpDeZorgtoeslagYaml);
//! engine.loadLaw(regelingStandaardpremieYaml);
//! engine.loadLaw(awirYaml);
//!
//! // Cross-law references are resolved automatically
//! const result = engine.execute(
//!     'wet_op_de_zorgtoeslag',
//!     'heeft_recht_op_zorgtoeslag',
//!     { bsn: '999993653' },
//!     '2025-01-01'
//! );
//! ```
//!
//! # Data Sources
//!
//! Register tabular data sources (e.g., personal records) that are queried
//! during execution to resolve inputs:
//!
//! ```javascript
//! engine.registerDataSource('personal_data', 'bsn', [
//!     { bsn: '999993653', geboortedatum: '2000-01-01', land_verblijf: 'NEDERLAND' }
//! ]);
//! ```
//!
//! # Error Handling
//!
//! All methods that can fail return `Result<T, JsValue>`. In JavaScript:
//!
//! ```javascript
//! try {
//!     const result = engine.execute(...);
//! } catch (e) {
//!     console.error('Execution failed:', e);  // e is a string with error details
//! }
//! ```

use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

use crate::annotation::{self, TextQuoteSelector, law_id_from_source};
use crate::config;
use crate::engine::OutputProvenance;
use crate::error::EngineError;
use crate::service::{ExecutionOutcome, LawExecutionService, StageState};
use crate::trace::{TraceBuilder, TraceDocument};
use crate::types::{RegulatoryLayer, Value};

/// Does this annotation note target a law other than `law_id`?
///
/// A federated `annotations.yaml` sidecar may carry notes for several laws, so
/// `resolveNotes` has to leave the other laws' notes alone. A note without a
/// `target.source`, or one whose source is not a `regelrecht://` law URI, is
/// treated as belonging to the law being resolved: the sidecar was handed to
/// this law and there is nothing that says otherwise.
///
/// Split out of `resolve_notes` so it can be tested natively — everything
/// around it takes or returns a `JsValue`, which aborts outside wasm32. Phrased
/// negatively so the call site needs no `!`: a negation there would be a second
/// mutant in exactly the code that cannot be reached from a test.
fn note_targets_another_law(note: &serde_json::Value, law_id: &str) -> bool {
    let Some(source) = note
        .get("target")
        .and_then(|t| t.get("source"))
        .and_then(|s| s.as_str())
    else {
        return false;
    };
    match law_id_from_source(source) {
        Some(target_law) => target_law != law_id,
        None => false,
    }
}

/// Create a serializer that converts HashMaps to JavaScript objects (not Maps),
/// and an absence (`Value::Null`) to `null`. By default an absence crosses as
/// `undefined`, which JavaScript reads as "not computed" rather than "there is
/// none" (RFC-036): a nullable output that is absent has to arrive as `null`.
fn js_serializer() -> Serializer {
    Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true)
}

/// Helper to create consistent error JsValues.
fn wasm_error(msg: &str) -> JsValue {
    JsValue::from_str(msg)
}

/// Parse a JavaScript array of record objects into engine records, dropping
/// every property whose value is `undefined` first, at any depth.
///
/// `serde-wasm-bindgen` deserializes `undefined` and `null` both as `None`,
/// so a record `{ bsn: '1', huur: undefined }` would come out with `huur:
/// Null`. Under RFC-036 that is a different statement: `null` says the
/// register is authoritative that there is no rent (absence), a missing
/// property says nobody has the value (unknown). Stripping `undefined` before
/// deserializing keeps the second from silently turning into the first. An
/// explicit `null` property is kept and becomes `Value::Null`. The same holds
/// inside a nested record (`beschikking: { status: undefined }`) and inside
/// arrays of records, so the walk is recursive.
///
/// This cannot be unit-tested natively: every `JsValue` operation aborts
/// outside wasm32 and the crate has no `wasm-bindgen-test` setup, so the
/// contract is documented here and pinned by the JS callers' tests instead.
fn parse_records(records: JsValue) -> Result<Vec<BTreeMap<String, Value>>, JsValue> {
    let records = strip_undefined_deep(records)?;
    serde_wasm_bindgen::from_value(records)
        .map_err(|e| wasm_error(&format!("Failed to parse records: {}", e)))
}

/// Parse the JavaScript parameters object of an `execute*` call, dropping
/// every `undefined`-valued property first, at any depth (see
/// [`parse_records`] for why).
///
/// A parameter that is `undefined` is a parameter the caller did not pass:
/// an optional one then resolves to unknown for lack of it, a required one is
/// the caller's error, exactly as when the key is absent. Deserialized as
/// `null` it would instead count as passed and state an absence the caller
/// never made, which an operation downstream then rejects (`AbsentOperand`)
/// or a cross-law call turns into "there is nobody".
fn parse_parameters(parameters: JsValue) -> Result<BTreeMap<String, Value>, JsValue> {
    let parameters = strip_undefined_deep(parameters)?;
    serde_wasm_bindgen::from_value(parameters)
        .map_err(|e| wasm_error(&format!("Failed to parse parameters: {}", e)))
}

/// Rebuild a JavaScript value without its `undefined`-valued object
/// properties, recursively through arrays and plain objects. An `undefined`
/// array *element* is kept (it is a position, not a property, and the
/// deserializer reports it as it always did). Anything that is not an array
/// or a plain object is passed through untouched, so a shape error is still
/// reported by the deserializer.
fn strip_undefined_deep(value: JsValue) -> Result<JsValue, JsValue> {
    if js_sys::Array::is_array(&value) {
        let stripped = js_sys::Array::new();
        for element in js_sys::Array::from(&value).iter() {
            stripped.push(&strip_undefined_deep(element)?);
        }
        return Ok(stripped.into());
    }
    if !value.is_object() {
        return Ok(value);
    }
    let clean = js_sys::Object::new();
    for entry in js_sys::Object::entries(&value.into()).iter() {
        let pair = js_sys::Array::from(&entry);
        let key = pair.get(0);
        let property = pair.get(1);
        if property.is_undefined() {
            continue;
        }
        js_sys::Reflect::set(&clean, &key, &strip_undefined_deep(property)?)?;
    }
    Ok(clean.into())
}

/// Convert internal EngineError to user-friendly WASM error.
fn engine_error_to_wasm(err: EngineError) -> JsValue {
    match err {
        EngineError::LawNotFound(ref law_id) => wasm_error(&format!(
            "Law '{}' not found. Load it first with loadLaw().",
            law_id
        )),
        other => wasm_error(&other.to_string()),
    }
}

/// Serializable result for execute()
#[derive(Serialize)]
struct WasmExecuteResult {
    outputs: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    output_provenance: BTreeMap<String, OutputProvenance>,
    resolved_inputs: BTreeMap<String, Value>,
    article_number: String,
    law_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
    engine_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regulation_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regulation_valid_from: Option<String>,
}

/// Serializable result for executeStage().
///
/// Two shapes in one object, told apart by `complete`. A finished lifecycle
/// carries the outputs of the whole run; a yielded one carries what is computed
/// so far, the state to hand back on the next call, and what it is waiting for.
/// The caller persists `state` verbatim — it is the engine's, not the caller's,
/// and RFC-008 puts the keeping of it outside the engine.
#[derive(Serialize)]
struct WasmStageResult {
    /// True when every stage is done; false when the lifecycle is waiting.
    complete: bool,
    outputs: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    output_provenance: BTreeMap<String, OutputProvenance>,
    /// The decision state to pass back in to advance. Absent when complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<StageState>,
    /// What the lifecycle needs before it can go on. Empty when complete.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pending_inputs: Vec<String>,
    /// Where the decision is now, e.g. "BEKENDMAKING". Absent when complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    current_stage: Option<String>,
}

/// Serializable result for executeWithTrace()
#[derive(Serialize)]
struct WasmExecuteResultWithTrace {
    outputs: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    output_provenance: BTreeMap<String, OutputProvenance>,
    resolved_inputs: BTreeMap<String, Value>,
    article_number: String,
    law_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    law_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<TraceDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_text: Option<String>,
    engine_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regulation_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regulation_valid_from: Option<String>,
}

/// Serializable law info for get_law_info()
#[derive(Serialize)]
struct WasmLawInfo {
    id: String,
    regulatory_layer: RegulatoryLayer,
    publication_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bwb_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    outputs: Vec<String>,
    article_count: usize,
}

/// WASM-compatible law execution engine with cross-law resolution.
///
/// Backed by `LawExecutionService`, providing automatic resolution of
/// cross-law references and data source support in the browser.
#[wasm_bindgen]
pub struct WasmEngine {
    service: LawExecutionService,
}

#[wasm_bindgen]
impl WasmEngine {
    /// Create a new empty engine instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            service: LawExecutionService::new(),
        }
    }

    /// Load a law from a YAML string.
    ///
    /// If a law with the same ID and valid_from is already loaded, it will be replaced.
    /// Multiple versions (same ID, different valid_from) can coexist.
    ///
    /// # Arguments
    /// * `yaml` - YAML string containing the law definition (max 1 MB)
    ///
    /// # Returns
    /// * `Ok(String)` - The law ID
    /// * `Err(JsValue)` - Error message if parsing fails
    #[wasm_bindgen(js_name = loadLaw)]
    pub fn load_law(&mut self, yaml: &str) -> Result<String, JsValue> {
        if yaml.len() > config::MAX_YAML_SIZE {
            return Err(wasm_error(&format!(
                "YAML exceeds maximum size ({} bytes)",
                config::MAX_YAML_SIZE
            )));
        }

        self.service.load_law(yaml).map_err(engine_error_to_wasm)
    }

    /// Execute a law output with automatic cross-law resolution.
    ///
    /// All referenced laws must be loaded via `loadLaw()` first.
    /// Data sources registered via `registerDataSource()` are queried
    /// to resolve inputs before falling back to cross-law resolution.
    ///
    /// # Arguments
    /// * `law_id` - ID of the loaded law
    /// * `output_name` - Name of the output to calculate
    /// * `parameters` - JavaScript object with input parameters
    /// * `calculation_date` - Date string (YYYY-MM-DD) for which to calculate
    ///
    /// # Returns
    /// * `Ok(JsValue)` - JavaScript object with `outputs`, `resolved_inputs`, etc.
    /// * `Err(JsValue)` - Error message if execution fails
    #[wasm_bindgen(js_name = execute)]
    pub fn execute(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let params = parse_parameters(parameters)?;

        let result = self
            .service
            .evaluate_law_output(law_id, output_name, params, calculation_date)
            .map_err(engine_error_to_wasm)?;

        let wasm_result = WasmExecuteResult {
            outputs: result.outputs,
            output_provenance: result.output_provenance,
            resolved_inputs: result.resolved_inputs,
            article_number: result.article_number,
            law_id: result.law_id,
            law_uuid: result.law_uuid,
            engine_version: result.engine_version,
            schema_version: result.schema_version,
            regulation_hash: result.regulation_hash,
            regulation_valid_from: result.regulation_valid_from,
        };

        wasm_result.serialize(&js_serializer()).map_err(|e| {
            wasm_error(&format!(
                "Failed to serialize result for law '{}': {}",
                law_id, e
            ))
        })
    }

    /// Execute one step of a decision's lifecycle (RFC-007, RFC-008).
    ///
    /// Where `execute()` computes an article and stops, this walks the
    /// AWB-defined procedure the article's `produces` puts it in: it fires the
    /// hooks belonging to each stage, and stops at the first stage whose inputs
    /// it does not have. A law that is in no procedure simply computes and
    /// completes, so this is safe to call for anything.
    ///
    /// The engine keeps no state. `state` is null on the first call and
    /// afterwards whatever the previous call returned; the caller persists it
    /// between the moments of the lifecycle, which can be days apart (a besluit
    /// is taken, a bekendmaking follows later). That division is the point of
    /// RFC-008: the engine stays a pure function per stage, the orchestration
    /// layer owns the decision record.
    ///
    /// # Returns
    /// * `Ok(JsValue)` — `{complete, outputs, state?, pending_inputs?, current_stage?}`
    /// * `Err(JsValue)` — error message if execution fails
    #[wasm_bindgen(js_name = executeStage)]
    pub fn execute_stage(
        &self,
        law_id: &str,
        output_name: &str,
        state: JsValue,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let params = parse_parameters(parameters)?;
        // Null and undefined both mean "this decision has no history yet".
        let state: Option<StageState> = if state.is_null() || state.is_undefined() {
            None
        } else {
            Some(
                serde_wasm_bindgen::from_value(strip_undefined_deep(state)?)
                    .map_err(|e| wasm_error(&format!("Failed to parse state: {}", e)))?,
            )
        };

        let outcome = self
            .service
            .execute_stage(law_id, output_name, state, params, calculation_date)
            .map_err(engine_error_to_wasm)?;

        let wasm_result = match outcome {
            ExecutionOutcome::Complete(result) => WasmStageResult {
                complete: true,
                outputs: result.outputs,
                output_provenance: result.output_provenance,
                state: None,
                pending_inputs: Vec::new(),
                current_stage: None,
            },
            ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
            } => WasmStageResult {
                complete: false,
                outputs,
                output_provenance: BTreeMap::new(),
                current_stage: Some(state.current_stage.clone()),
                state: Some(state),
                pending_inputs,
            },
        };

        wasm_result.serialize(&js_serializer()).map_err(|e| {
            wasm_error(&format!(
                "Failed to serialize stage result for law '{}': {}",
                law_id, e
            ))
        })
    }

    /// Execute a law output with tracing enabled.
    ///
    /// Same as `execute()` but includes a full execution trace tree in the
    /// result. The trace captures every resolution step, cross-law call, and
    /// operation performed during evaluation.
    ///
    /// # Returns
    /// * `Ok(JsValue)` - JavaScript object with `outputs`, `trace` (tree), `trace_text` (box-drawing)
    /// * `Err(JsValue)` - Error message if execution fails (may include partial trace)
    #[wasm_bindgen(js_name = executeWithTrace)]
    pub fn execute_with_trace(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let params = parse_parameters(parameters)?;

        // Use untimed trace builder to avoid Instant::now() JS FFI calls
        // that cause RefCell aliasing panics in wasm-bindgen.
        match self.service.evaluate_law_output_with_trace_builder(
            law_id,
            output_name,
            params,
            calculation_date,
            TraceBuilder::new_untimed(),
        ) {
            Ok(result) => {
                let trace_text = result.trace.as_ref().map(|t| t.render_box_drawing());
                let wasm_result = WasmExecuteResultWithTrace {
                    outputs: result.outputs,
                    output_provenance: result.output_provenance,
                    resolved_inputs: result.resolved_inputs,
                    article_number: result.article_number,
                    law_id: result.law_id,
                    law_uuid: result.law_uuid,
                    trace: result.trace.map(TraceDocument::new),
                    trace_text,
                    engine_version: result.engine_version,
                    schema_version: result.schema_version,
                    regulation_hash: result.regulation_hash,
                    regulation_valid_from: result.regulation_valid_from,
                };

                wasm_result.serialize(&js_serializer()).map_err(|e| {
                    wasm_error(&format!(
                        "Failed to serialize traced result for law '{}': {}",
                        law_id, e
                    ))
                })
            }
            Err(EngineError::TracedError { source, trace }) => {
                // Return partial trace alongside the error by encoding both
                // into a structured error object
                let trace_node = trace.map(|t| *t);
                let trace_text = trace_node.as_ref().map(|t| t.render_box_drawing());

                #[derive(Serialize)]
                struct TracedErrorResult {
                    error: String,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    trace: Option<TraceDocument>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    trace_text: Option<String>,
                }

                let err_result = TracedErrorResult {
                    error: source.to_string(),
                    trace: trace_node.map(TraceDocument::new),
                    trace_text,
                };

                match err_result.serialize(&js_serializer()) {
                    Ok(js_val) => Err(js_val),
                    Err(_) => Err(wasm_error(&source.to_string())),
                }
            }
            Err(other) => Err(engine_error_to_wasm(other)),
        }
    }

    /// Execute multiple specific outputs from a law (privacy-by-design).
    ///
    /// Callers must explicitly list which outputs they need. The engine evaluates
    /// each producing article once and returns only the requested outputs.
    ///
    /// # Arguments
    /// * `law_id` - ID of the loaded law
    /// * `output_names` - JavaScript array of output name strings
    /// * `parameters` - JavaScript object with input parameters
    /// * `calculation_date` - Date string (YYYY-MM-DD)
    #[wasm_bindgen(js_name = executeMultiple)]
    pub fn execute_multiple(
        &self,
        law_id: &str,
        output_names: JsValue,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let names: Vec<String> = serde_wasm_bindgen::from_value(output_names)
            .map_err(|e| wasm_error(&format!("Failed to parse output_names: {}", e)))?;
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let params = parse_parameters(parameters)?;

        let result = self
            .service
            .evaluate_law(law_id, &name_refs, params, calculation_date)
            .map_err(engine_error_to_wasm)?;

        let wasm_result = WasmExecuteResult {
            outputs: result.outputs,
            output_provenance: result.output_provenance,
            resolved_inputs: result.resolved_inputs,
            article_number: result.article_number,
            law_id: result.law_id,
            law_uuid: result.law_uuid,
            engine_version: result.engine_version,
            schema_version: result.schema_version,
            regulation_hash: result.regulation_hash,
            regulation_valid_from: result.regulation_valid_from,
        };

        wasm_result.serialize(&js_serializer()).map_err(|e| {
            wasm_error(&format!(
                "Failed to serialize result for law '{}': {}",
                law_id, e
            ))
        })
    }

    /// Execute multiple specific outputs with tracing enabled.
    #[wasm_bindgen(js_name = executeMultipleWithTrace)]
    pub fn execute_multiple_with_trace(
        &self,
        law_id: &str,
        output_names: JsValue,
        parameters: JsValue,
        calculation_date: &str,
    ) -> Result<JsValue, JsValue> {
        let names: Vec<String> = serde_wasm_bindgen::from_value(output_names)
            .map_err(|e| wasm_error(&format!("Failed to parse output_names: {}", e)))?;
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let params = parse_parameters(parameters)?;

        match self.service.evaluate_law_with_trace_builder(
            law_id,
            &name_refs,
            params,
            calculation_date,
            TraceBuilder::new_untimed(),
        ) {
            Ok(result) => {
                let trace_text = result.trace.as_ref().map(|t| t.render_box_drawing());
                let wasm_result = WasmExecuteResultWithTrace {
                    outputs: result.outputs,
                    output_provenance: result.output_provenance,
                    resolved_inputs: result.resolved_inputs,
                    article_number: result.article_number,
                    law_id: result.law_id,
                    law_uuid: result.law_uuid,
                    trace: result.trace.map(TraceDocument::new),
                    trace_text,
                    engine_version: result.engine_version,
                    schema_version: result.schema_version,
                    regulation_hash: result.regulation_hash,
                    regulation_valid_from: result.regulation_valid_from,
                };

                wasm_result.serialize(&js_serializer()).map_err(|e| {
                    wasm_error(&format!(
                        "Failed to serialize traced result for law '{}': {}",
                        law_id, e
                    ))
                })
            }
            Err(EngineError::TracedError { source, trace }) => {
                let trace_node = trace.map(|t| *t);
                let trace_text = trace_node.as_ref().map(|t| t.render_box_drawing());

                #[derive(Serialize)]
                struct TracedErrorResult {
                    error: String,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    trace: Option<TraceDocument>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    trace_text: Option<String>,
                }

                let err_result = TracedErrorResult {
                    error: source.to_string(),
                    trace: trace_node.map(TraceDocument::new),
                    trace_text,
                };

                match err_result.serialize(&js_serializer()) {
                    Ok(js_val) => Err(js_val),
                    Err(_) => Err(wasm_error(&source.to_string())),
                }
            }
            Err(other) => Err(engine_error_to_wasm(other)),
        }
    }

    /// List all loaded law IDs (sorted alphabetically).
    #[wasm_bindgen(js_name = listLaws)]
    pub fn list_laws(&self) -> Vec<String> {
        self.service
            .list_laws()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// Get metadata about a loaded law.
    #[wasm_bindgen(js_name = getLawInfo)]
    pub fn get_law_info(&self, law_id: &str) -> Result<JsValue, JsValue> {
        let law_info = self
            .service
            .get_law_info(law_id)
            .ok_or_else(|| wasm_error(&format!("Law '{}' not found", law_id)))?;

        let info = WasmLawInfo {
            id: law_info.id,
            regulatory_layer: law_info.regulatory_layer,
            publication_date: law_info.publication_date,
            bwb_id: law_info.bwb_id,
            url: law_info.url,
            outputs: law_info.outputs,
            article_count: law_info.article_count,
        };

        info.serialize(&js_serializer()).map_err(|e| {
            wasm_error(&format!(
                "Failed to serialize law info for '{}': {}",
                law_id, e
            ))
        })
    }

    /// Remove a loaded law from the engine.
    ///
    /// # Returns
    /// * `true` if the law was removed, `false` if it wasn't loaded
    #[wasm_bindgen(js_name = unloadLaw)]
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        self.service.unload_law(law_id)
    }

    /// Check if a law is loaded.
    #[wasm_bindgen(js_name = hasLaw)]
    pub fn has_law(&self, law_id: &str) -> bool {
        self.service.has_law(law_id)
    }

    /// Get the number of loaded laws.
    #[wasm_bindgen(js_name = lawCount)]
    pub fn law_count(&self) -> usize {
        self.service.law_count()
    }

    /// Resolve a single TextQuoteSelector against a loaded law (RFC-005).
    ///
    /// The law must already be loaded via `loadLaw()`. The selector is
    /// content-addressed; `validFrom` names the *version* the caller is
    /// viewing (that version's own `valid_from` date), so the note is looked
    /// up in the text on screen rather than in whichever loaded version is
    /// newest. Omit it to resolve against the latest version.
    ///
    /// # Arguments
    /// * `law_id` - ID of the loaded law
    /// * `selector` - JS object: `{ exact, prefix?, suffix?, "regelrecht:hint"? }`
    /// * `valid_from` - Optional `YYYY-MM-DD` of the viewed version
    ///
    /// # Returns
    /// * `Ok(JsValue)` - `MatchResult`: `{ status, matches: [{ article_number,
    ///   start, end, confidence, matched_text }], skip_reason? }`. `status`
    ///   is `"found"`, `"orphaned"`, `"ambiguous"`, or `"skipped"` — the last
    ///   means the fuzzy scan hit a resource bound (`config.rs`) and the law
    ///   was not (fully) searched, so "not searched" is distinguishable from
    ///   "not found"; `skip_reason` (`"quote_too_long"` or `"search_budget"`,
    ///   present only then) names the bound, and `matches` carries any
    ///   candidates found before the cut-off without a uniqueness claim.
    ///   `start`/`end` are **`char` offsets** (Unicode scalar values), not
    ///   UTF-16 code units: JS code slicing the article text must convert
    ///   accordingly.
    /// * `Err(JsValue)` - Error if the law is not loaded, no version matches
    ///   `valid_from`, or the selector/date is invalid
    #[wasm_bindgen(js_name = resolveNote)]
    pub fn resolve_note(
        &self,
        law_id: &str,
        selector: JsValue,
        valid_from: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let selector: TextQuoteSelector = serde_wasm_bindgen::from_value(selector)
            .map_err(|e| wasm_error(&format!("Invalid selector: {e}")))?;
        let law = self
            .service
            .get_law_version(law_id, valid_from.as_deref())
            .map_err(engine_error_to_wasm)?;
        let result = annotation::resolve(&selector, &law.articles);
        result
            .serialize(&js_serializer())
            .map_err(|e| wasm_error(&format!("Serialization error: {e}")))
    }

    /// Resolve all notes in a note-sidecar YAML string against a loaded law.
    ///
    /// Parses an `annotations:` document (the format validated by
    /// `just validate-annotations`) and resolves every note's selector.
    ///
    /// Notes whose `target.source` names a different law than `law_id` are
    /// skipped (a sidecar may legitimately carry notes for several laws). A
    /// note with a missing or unparseable selector is **not** silently
    /// dropped: it appears in the result with an `error` string and a `null`
    /// `match`, so the caller can distinguish "no notes" from "notes that
    /// failed to parse".
    ///
    /// # Arguments
    /// * `law_id` - ID of the loaded law to resolve against
    /// * `annotations_yaml` - Contents of an `annotations.yaml` sidecar file
    /// * `valid_from` - Optional `YYYY-MM-DD` of the viewed version (see
    ///   `resolveNote`); omit for the latest version
    ///
    /// # Returns
    /// * `Ok(JsValue)` - Array of `{ note, match, error }` objects. `match` is
    ///   the `MatchResult` (or `null` on error); `error` is `null` on success
    ///   or a message string. Match `start`/`end` are **`char` offsets**, not
    ///   UTF-16 code units.
    /// * `Err(JsValue)` - Error if the law is not loaded, no version matches
    ///   `valid_from`, or the YAML/date is invalid
    #[wasm_bindgen(js_name = resolveNotes)]
    pub fn resolve_notes(
        &self,
        law_id: &str,
        annotations_yaml: &str,
        valid_from: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let law = self
            .service
            .get_law_version(law_id, valid_from.as_deref())
            .map_err(engine_error_to_wasm)?;

        let doc: serde_json::Value = serde_yaml_ng::from_str(annotations_yaml)
            .map_err(|e| wasm_error(&format!("Invalid annotations YAML: {e}")))?;
        let notes = doc
            .get("annotations")
            .and_then(|v| v.as_array())
            .ok_or_else(|| wasm_error("Missing 'annotations' list"))?;

        let mut resolved: Vec<serde_json::Value> = Vec::with_capacity(notes.len());
        for note in notes {
            if note_targets_another_law(note, law_id) {
                continue;
            }

            let (match_value, error) = match note.get("target").and_then(|t| t.get("selector")) {
                None => (
                    serde_json::Value::Null,
                    Some("note has no target.selector".to_string()),
                ),
                Some(selector_value) => {
                    match serde_json::from_value::<TextQuoteSelector>(selector_value.clone()) {
                        Err(e) => (
                            serde_json::Value::Null,
                            Some(format!("invalid selector: {e}")),
                        ),
                        Ok(selector) => {
                            let result = annotation::resolve(&selector, &law.articles);
                            match serde_json::to_value(&result) {
                                Ok(v) => (v, None),
                                Err(e) => (
                                    serde_json::Value::Null,
                                    Some(format!("serialization error: {e}")),
                                ),
                            }
                        }
                    }
                }
            };
            resolved.push(serde_json::json!({
                "note": note,
                "match": match_value,
                "error": error,
            }));
        }

        serde_json::Value::Array(resolved)
            .serialize(&js_serializer())
            .map_err(|e| wasm_error(&format!("Serialization error: {e}")))
    }

    /// Register a tabular data source from flat records.
    ///
    /// Data sources are queried during execution to resolve inputs before
    /// falling back to cross-law resolution.
    ///
    /// # Arguments
    /// * `name` - Data source name (e.g., "personal_data")
    /// * `key_field` - Field name used as record key (e.g., "bsn")
    /// * `records` - JavaScript array of objects, each representing a record
    ///
    /// A property that is `undefined` is dropped from the record (the fact is
    /// unknown); a property that is `null` is kept (the register says there is
    /// none). See RFC-036.
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// engine.registerDataSource('personal_data', 'bsn', [
    ///     { bsn: '999993653', geboortedatum: '2000-01-01', land_verblijf: 'NEDERLAND' }
    /// ]);
    /// ```
    #[wasm_bindgen(js_name = registerDataSource)]
    pub fn register_data_source(
        &mut self,
        name: &str,
        key_field: &str,
        records: JsValue,
    ) -> Result<(), JsValue> {
        let parsed = parse_records(records)?;

        self.service
            .register_dict_source(name, key_field, parsed)
            .map_err(engine_error_to_wasm)
    }

    /// Register a tabular data source that answers only for one law.
    ///
    /// The records carry the law's `source: {}` inputs by name, keyed by
    /// `key_field` (typically `bsn` or `kvk_nummer`). Bound to `law_id`, the
    /// source is consulted only while that law's inputs are resolved, so a raw
    /// register column can never shadow a same-named cross-law input elsewhere.
    /// `priority` orders sources for the same law (higher wins); omit it for
    /// the default of 10. Citizen corrections go in as a second, higher-priority
    /// source for the same law.
    ///
    /// A property that is `undefined` is dropped from the record (the fact is
    /// unknown); a property that is `null` is kept (the register says there is
    /// none: `partner_bsn: null` below is a person without a partner). See
    /// RFC-036.
    ///
    /// # Example (JavaScript)
    /// ```javascript
    /// engine.registerDataSourceForLaw('wet_brp', 'RvIG', 'bsn', [
    ///     { bsn: '999993653', geboortedatum: '2000-01-01', partner_bsn: null }
    /// ]);
    /// engine.registerDataSourceForLaw('wet_brp', 'claims', 'bsn', [
    ///     { bsn: '999993653', geboortedatum: '1999-12-31' }
    /// ], 100);
    /// ```
    #[wasm_bindgen(js_name = registerDataSourceForLaw)]
    pub fn register_data_source_for_law(
        &mut self,
        law_id: &str,
        name: &str,
        key_field: &str,
        records: JsValue,
        priority: Option<i32>,
    ) -> Result<(), JsValue> {
        let parsed = parse_records(records)?;

        self.service
            .register_dict_source_for_law(law_id, name, key_field, parsed, priority.unwrap_or(10))
            .map_err(engine_error_to_wasm)
    }

    /// Remove a data source by name (every law scope it was registered under).
    #[wasm_bindgen(js_name = removeDataSource)]
    pub fn remove_data_source(&mut self, name: &str) -> bool {
        self.service.remove_data_source(name)
    }

    /// Remove all registered data sources.
    #[wasm_bindgen(js_name = clearDataSources)]
    pub fn clear_data_sources(&mut self) {
        self.service.clear_data_sources();
    }

    /// Get the engine version.
    #[wasm_bindgen(js_name = version)]
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_LAW_YAML: &str = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article
    machine_readable:
      execution:
        parameters:
          - name: value
            type: number
            required: true
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $value
              - 2
"#;

    fn load_law(engine: &mut WasmEngine, yaml: &str) -> String {
        engine.service.load_law(yaml).unwrap()
    }

    #[test]
    fn test_wasm_engine_new() {
        let engine = WasmEngine::new();
        assert_eq!(engine.law_count(), 0);
        assert!(engine.list_laws().is_empty());
    }

    #[test]
    fn test_wasm_engine_default() {
        let engine = WasmEngine::default();
        assert_eq!(engine.law_count(), 0);
    }

    #[test]
    fn test_wasm_engine_load_law() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        assert_eq!(engine.law_count(), 1);
        assert!(engine.has_law("test_law"));
        assert_eq!(engine.list_laws(), vec!["test_law".to_string()]);
    }

    /// `loadLaw()` is de enige weg waarlangs JavaScript een wet de engine in
    /// krijgt, en het is het contract dat de aanroeper het `$id` terugkrijgt om
    /// mee verder te werken (`execute(lawId, ...)`). De andere tests laden via
    /// de service en zouden een kapotte wrapper dus niet zien.
    #[test]
    fn test_wasm_engine_load_law_returns_law_id() {
        let mut engine = WasmEngine::new();

        let law_id = engine.load_law(MINIMAL_LAW_YAML).ok();

        assert_eq!(law_id.as_deref(), Some("test_law"));
        assert!(engine.has_law("test_law"));
    }

    /// De grens van `MAX_YAML_SIZE` is inclusief: een document van precies de
    /// maximumgrootte hoort geladen te worden, pas één byte meer wordt
    /// geweigerd. Een wet die exact op de grens valt mag niet stilletjes
    /// onbruikbaar worden.
    ///
    /// De afkeurkant van dezelfde grens is native niet te testen: die tak bouwt
    /// een `JsValue` en wasm-bindgen aborteert het proces buiten wasm32.
    #[test]
    fn test_wasm_engine_load_law_accepts_exactly_max_size() {
        let mut engine = WasmEngine::new();

        let mut yaml = String::from(MINIMAL_LAW_YAML);
        yaml.push_str("# ");
        let padding = config::MAX_YAML_SIZE - yaml.len();
        yaml.push_str(&"x".repeat(padding));
        assert_eq!(yaml.len(), config::MAX_YAML_SIZE);

        let law_id = engine.load_law(&yaml).ok();

        assert_eq!(
            law_id.as_deref(),
            Some("test_law"),
            "a law of exactly MAX_YAML_SIZE bytes should still load"
        );
    }

    /// De engine-versie reist mee in elk uitvoeringsresultaat en is daarmee
    /// onderdeel van de herleidbaarheid van een beschikking: hij moet de
    /// werkelijke crate-versie zijn, geen placeholder.
    #[test]
    fn test_wasm_engine_version() {
        let engine = WasmEngine::new();

        assert_eq!(engine.version(), env!("CARGO_PKG_VERSION"));
        assert!(!engine.version().is_empty());
    }

    #[test]
    fn test_wasm_engine_remove_data_source_reports_whether_it_existed() {
        let mut engine = WasmEngine::new();
        let mut record = BTreeMap::new();
        record.insert("bsn".to_string(), Value::String("1".to_string()));
        record.insert("inkomen".to_string(), Value::Int(100));
        engine
            .service
            .register_dict_source("bron", "bsn", vec![record])
            .unwrap();
        assert!(
            engine.remove_data_source("bron"),
            "a registered source is removed"
        );
        assert!(
            !engine.remove_data_source("bron"),
            "removing it again reports that nothing was there"
        );
        assert!(!engine.remove_data_source("onbekend"));
    }

    #[test]
    fn test_wasm_engine_unload_law() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        assert!(engine.has_law("test_law"));
        assert!(engine.unload_law("test_law"));
        assert!(!engine.has_law("test_law"));
        assert!(!engine.unload_law("nonexistent"));
    }

    #[test]
    fn test_wasm_engine_list_laws() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        let laws = engine.list_laws();
        assert_eq!(laws.len(), 1);
        assert!(laws.contains(&"test_law".to_string()));
    }

    #[test]
    fn test_wasm_engine_has_law() {
        let mut engine = WasmEngine::new();
        assert!(!engine.has_law("test_law"));

        load_law(&mut engine, MINIMAL_LAW_YAML);

        assert!(engine.has_law("test_law"));
        assert!(!engine.has_law("other_law"));
    }

    #[test]
    fn test_wasm_engine_law_count() {
        let mut engine = WasmEngine::new();
        assert_eq!(engine.law_count(), 0);

        load_law(&mut engine, MINIMAL_LAW_YAML);
        assert_eq!(engine.law_count(), 1);

        let yaml2 = r#"
$id: second_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Second test article
"#;
        load_law(&mut engine, yaml2);
        assert_eq!(engine.law_count(), 2);
    }

    #[test]
    fn test_wasm_engine_reload_law_replaces() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);
        assert_eq!(engine.law_count(), 1);

        // Loading the same law again replaces the existing version
        load_law(&mut engine, MINIMAL_LAW_YAML);
        assert_eq!(engine.law_count(), 1);
    }

    #[test]
    fn test_wasm_engine_cross_law_execution() {
        let mut engine = WasmEngine::new();

        // Law A: outputs a constant value
        let law_a = r#"
$id: law_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Provides base value
    machine_readable:
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: 100
"#;

        // Law B: references law_a's output
        let law_b = r#"
$id: law_b
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses base value from law A
    machine_readable:
      execution:
        input:
          - name: base_value
            type: number
            source:
              regulation: law_a
              output: base_value
        output:
          - name: doubled
            type: number
        actions:
          - output: doubled
            operation: MULTIPLY
            values:
              - $base_value
              - 2
"#;

        load_law(&mut engine, law_a);
        load_law(&mut engine, law_b);

        // Execute via the service directly (can't use JsValue in native tests)
        let params = BTreeMap::new();
        let result = engine
            .service
            .evaluate_law_output("law_b", "doubled", params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("doubled"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_wasm_engine_data_source() {
        let mut engine = WasmEngine::new();

        // Law that reads age from a data source via source reference
        let law = r#"
$id: data_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses data source
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: age
            type: number
            source:
              output: age
        output:
          - name: is_adult
            type: boolean
        actions:
          - output: is_adult
            operation: GREATER_THAN_OR_EQUAL
            subject: $age
            value: 18
"#;
        load_law(&mut engine, law);

        // Register data source
        let records = vec![{
            let mut r = BTreeMap::new();
            r.insert("bsn".to_string(), Value::String("123".to_string()));
            r.insert("age".to_string(), Value::Int(25));
            r
        }];
        engine
            .service
            .register_dict_source("people", "bsn", records)
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("123".to_string()));

        let result = engine
            .service
            .evaluate_law_output("data_law", "is_adult", params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("is_adult"), Some(&Value::Bool(true)));

        // Clear and verify
        engine.clear_data_sources();
        assert_eq!(engine.service.data_source_count(), 0);
    }

    #[test]
    fn test_wasm_engine_execute_multiple() {
        let mut engine = WasmEngine::new();

        // Law with two outputs in the same article
        let law = r#"
$id: multi_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Produces two outputs
    machine_readable:
      execution:
        parameters:
          - name: amount
            type: number
            required: true
        output:
          - name: doubled
            type: number
          - name: tripled
            type: number
        actions:
          - output: doubled
            operation: MULTIPLY
            values:
              - $amount
              - 2
          - output: tripled
            operation: MULTIPLY
            values:
              - $amount
              - 3
"#;
        load_law(&mut engine, law);

        let mut params = BTreeMap::new();
        params.insert("amount".to_string(), Value::Int(10));

        // Request both outputs
        let result = engine
            .service
            .evaluate_law(
                "multi_law",
                &["doubled", "tripled"],
                params.clone(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(result.outputs.get("doubled"), Some(&Value::Int(20)));
        assert_eq!(result.outputs.get("tripled"), Some(&Value::Int(30)));
        // Privacy check: only requested outputs are returned
        assert_eq!(result.outputs.len(), 2);

        // Request only one output: the other one in the same article does not
        // depend on it, so it is not computed (RFC-043).
        let result = engine
            .service
            .evaluate_law("multi_law", &["doubled"], params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("doubled"), Some(&Value::Int(20)));
        assert_eq!(result.outputs.len(), 1);
    }

    #[test]
    fn test_wasm_engine_execute_with_trace() {
        let mut engine = WasmEngine::new();
        load_law(&mut engine, MINIMAL_LAW_YAML);

        let mut params = BTreeMap::new();
        params.insert("value".to_string(), Value::Int(21));

        let result = engine
            .service
            .evaluate_law_output_with_trace("test_law", "result", params, "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("result"), Some(&Value::Int(42)));
        assert!(result.trace.is_some(), "Trace should be populated");

        let trace = result.trace.unwrap();
        assert!(!trace.children.is_empty(), "Trace should have children");

        // Verify box-drawing rendering works
        let text = trace.render_box_drawing();
        assert!(!text.is_empty(), "Box-drawing trace should not be empty");
    }

    fn note_with_source(source: &str) -> serde_json::Value {
        serde_json::json!({ "target": { "source": source } })
    }

    #[test]
    fn a_note_for_another_law_is_not_resolved_against_this_one() {
        // Een federatieve sidecar draagt notities voor meerdere wetten. Keert
        // dit filter om, dan hangt de engine de annotaties van de ene wet aan
        // de tekst van de andere.
        let note = note_with_source("regelrecht://andere_wet");
        assert!(note_targets_another_law(&note, "test_law"));
        assert!(!note_targets_another_law(&note, "andere_wet"));
    }

    #[test]
    fn a_note_for_this_law_survives_the_filter() {
        let note = note_with_source("regelrecht://test_law/artikel/1");
        assert!(!note_targets_another_law(&note, "test_law"));
    }

    #[test]
    fn a_note_without_a_law_uri_belongs_to_the_law_being_resolved() {
        // Geen target.source, of een source die geen regelrecht-wet aanwijst:
        // de sidecar is bij deze wet aangeleverd en niets zegt het tegendeel.
        assert!(!note_targets_another_law(
            &serde_json::json!({}),
            "test_law"
        ));
        assert!(!note_targets_another_law(
            &serde_json::json!({ "target": {} }),
            "test_law"
        ));
        assert!(!note_targets_another_law(
            &note_with_source("https://example.com/bron"),
            "test_law"
        ));
    }
}
