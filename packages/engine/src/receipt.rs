//! Execution Receipt (RFC-013)
//!
//! An Execution Receipt is an output envelope that records everything needed
//! to reproduce an engine execution: which engine, schema, and regulations
//! were used, the execution parameters, and the results.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::engine::{CompetentInput, OpenTermFilled, OutputProvenance};
use crate::resolver::{DeclarationNotInForce, DeclarationsFromOtherVersion, DelegationRefusal};
use crate::trace::PathNode;
use crate::types::{Connectivity, LegalStatus, UntranslatableMode, Value};

/// Top-level execution receipt (RFC-013).
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionReceipt {
    pub provenance: ReceiptProvenance,
    pub engine_config: EngineConfig,
    pub scope: ReceiptScope,
    pub execution: ReceiptExecution,
    pub results: ReceiptResults,
    pub accepted_values: Vec<AcceptedValue>,
    pub timestamp: String,
}

/// Which engine and regulation produced the result.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptProvenance {
    pub engine: String,
    pub engine_version: String,
    pub schema_version: Option<String>,
    pub regulation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation_valid_from: Option<String>,
    pub regulation_hash: Option<String>,
}

/// Engine startup configuration that affects execution behavior.
#[derive(Debug, Clone, Serialize)]
pub struct EngineConfig {
    pub connectivity: Connectivity,
    pub legal_status: LegalStatus,
    pub untranslatable_mode: UntranslatableMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<EngineIdentity>,
}

/// Engine identity for multi-org execution (RFC-009).
#[derive(Debug, Clone, Serialize)]
pub struct EngineIdentity {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organisation_id: Option<String>,
    #[serde(rename = "type")]
    pub identity_type: String,
}

/// Which regulations were loaded during execution.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptScope {
    pub sources: Vec<ReceiptSource>,
    pub loaded_regulations: Vec<LoadedRegulation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<ScopeEntry>,
}

/// A corpus source that was loaded.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptSource {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A regulation that was loaded during execution.
#[derive(Debug, Clone, Serialize)]
pub struct LoadedRegulation {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    /// Instrument end date (inclusive), recording the full validity window (RFC-019 §4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// An active jurisdiction scope.
#[derive(Debug, Clone, Serialize)]
pub struct ScopeEntry {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub value: String,
}

/// Execution parameters and context.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptExecution {
    pub calculation_date: String,
    pub parameters: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_date: Option<String>,
}

/// Execution outputs and optional trace.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptResults {
    /// Which outputs were explicitly requested (privacy-by-design audit trail).
    /// Omitted from JSON when empty (legacy callers that don't specify outputs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requested_outputs: Vec<String>,
    pub outputs: BTreeMap<String, Value>,
    /// Per-output provenance: which mechanism produced each output.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub output_provenance: BTreeMap<String, OutputProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<PathNode>,
    /// Implementations refused by the delegation gate: regulations that
    /// declared they fill an open term but sit at a layer the declaring
    /// article does not delegate to. Recorded independently of the trace, so
    /// the refusal is on the receipt even when no trace was requested.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub delegation_refusals: Vec<DelegationRefusal>,
    /// Laws whose hooks, overrides and procedures were read from a version
    /// other than the one in force on the reference date, so that "none found"
    /// on this receipt is not read as "none exist".
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub declaration_version_notes: Vec<DeclarationsFromOtherVersion>,
    /// Hooks, overrides and implementations that were offered for this
    /// execution but did not apply on the reference date: no version in force,
    /// or the version in force lacks the indexed article.
    /// They did not run; the receipt says so, rather than letting the outcome
    /// read as though nothing was ever declared.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub declarations_not_in_force: Vec<DeclarationNotInForce>,
    /// The open terms a finding or a regulation filled, and which of the
    /// two filled each, so the receipt shows what the outcome rests on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub open_terms: Vec<OpenTermFilled>,
    /// The inputs whose source names their competent authority, and whether
    /// each was supplied, answered by a cell or neither (RFC-056), so the
    /// receipt says who answered without a trace.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub competent_inputs: Vec<CompetentInput>,
}

/// A value accepted from another organisation's engine (RFC-009).
#[derive(Debug, Clone, Serialize)]
pub struct AcceptedValue {
    pub output: String,
    pub value: Value,
    pub authority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed: Option<bool>,
}
