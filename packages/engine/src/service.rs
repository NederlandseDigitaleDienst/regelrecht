//! Service layer for cross-law resolution
//!
//! Provides the `ServiceProvider` trait and `LawExecutionService` implementation
//! for resolving cross-law references and IoC open-term resolution.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{LawExecutionService, Value};
//! use std::collections::BTreeMap;
//!
//! let mut service = LawExecutionService::new();
//! service.load_law(wet_op_de_zorgtoeslag_yaml)?;
//! service.load_law(regeling_standaardpremie_yaml)?;
//!
//! let mut params = BTreeMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! // This will automatically resolve cross-law references
//! let result = service.evaluate_law_output(
//!     "wet_op_de_zorgtoeslag",
//!     "heeft_recht_op_zorgtoeslag",
//!     params,
//!     "2025-01-01",
//! )?;
//! ```

use crate::answer::{
    AnswerProvenance, AnswerRequest, Authority, Call, Chronicle, ChronicleRead, OpenTermAnswerer,
};
use crate::article::{
    Article, ArticleBasedLaw, CompetentAuthority, Execution, HookPoint, Input, MachineReadable,
    NamedAuthority, ProcedureDefinition, Stage, StageRequirement,
};
use crate::config;
use crate::context::{LazyInputs, RuleContext};
use crate::data_source::{DataSource, DataSourceRegistry, DictDataSource};
use crate::engine::{ArticleEngine, ArticleResult, FilledBy, OpenTermFilled, OutputProvenance};
use crate::error::{EngineError, Result};
use crate::operations::ValueResolver;
use crate::priority;
use crate::resolver::{
    hook_filter_admits, missing_article_reason, submission_filter_admits, DeclarationKind,
    DeclarationNotInForce, DeclarationsFromOtherVersion, DelegationRefusal, HookEntry,
    LawArticleRef, ProcedureMiss, RuleResolver, SelectionReason,
};
use crate::trace::{LegalAnchor, TraceBuilder, ValueSource};
use crate::types::{
    Connectivity, LegalStatus, MissingKind, PathNodeType, RegulatoryLayer, ResolveType, TypeSpec,
    UntranslatableMode, Value,
};
use crate::uri::RegelrechtUri;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

// =============================================================================
// Resolution Context
// =============================================================================

/// Context for tracking cross-law resolution state.
///
/// Bundles the state needed for cycle detection and depth tracking
/// during cross-law reference resolution. This reduces the number of
/// parameters passed between internal resolution functions.
///
/// Memoization cache entry storing identity fields (for collision detection)
/// alongside the cached outputs from a cross-law evaluation.
struct CacheEntry {
    law_id: String,
    output_name: String,
    outputs: BTreeMap<String, Value>,
    output_provenance: BTreeMap<String, OutputProvenance>,
    parameters: BTreeMap<String, Value>,
}

impl CacheEntry {
    /// Whether this entry was stored for exactly this law, output and parameter set.
    ///
    /// Cache keys are `u64` hashes, so two different evaluations can in theory
    /// land on the same key. For legally binding decisions a wrong hit is never
    /// acceptable, so every read re-checks the full identity: all three fields
    /// must match, a difference in any one of them means "not this entry".
    fn matches(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: &BTreeMap<String, Value>,
    ) -> bool {
        self.law_id == law_id && self.output_name == output_name && self.parameters == *parameters
    }
}

/// Uses a scoped push/pop pattern for the visited set to avoid
/// cloning the HashSet on every cross-law descent.
struct ResolutionContext<'a> {
    /// Date for calculations (YYYY-MM-DD)
    calculation_date: &'a str,
    /// Cached parsed date for version selection (parsed once at construction)
    reference_date: Option<NaiveDate>,
    /// Set of law#output keys already being resolved (cycle detection)
    visited: HashSet<String>,
    /// Current resolution depth
    depth: usize,
    /// Optional shared trace builder
    trace: Option<Rc<RefCell<TraceBuilder>>>,
    /// Per-execution memoization cache: hash key → CacheEntry
    cache: HashMap<u64, CacheEntry>,
    /// The law that initiated the current execution chain (for override scoping).
    /// Overrides only apply when declared by this law.
    contextual_law_id: Option<String>,
    /// Where the engine is right now: the innermost provision being evaluated
    /// (RFC-039). Every trace step pushed while this is set inherits it as its
    /// `anchor`, so a step can name its article without each push site having
    /// to know one. Independent of `legal_basis`, which is what the law
    /// document cites rather than where the engine was.
    anchor: Option<LegalAnchor>,
    /// Implementations refused by the delegation gate during this execution.
    /// Collected independently of tracing so the refusal reaches the receipt
    /// even when no trace was requested.
    delegation_refusals: Vec<DelegationRefusal>,
    /// Laws whose declarations were answered from another version than the one
    /// in force, recorded once per execution.
    declaration_version_notes: Vec<DeclarationsFromOtherVersion>,
    /// Hooks, overrides and implementations skipped during this execution
    /// because their law had no version in force on the reference date.
    /// Collected independently of tracing so the skip reaches the receipt.
    declarations_not_in_force: Vec<DeclarationNotInForce>,
    /// Overrides and implementations held back while the article declaring
    /// one reads the output it replaces or fills in: that read is the value
    /// it departs from, so it does not apply to it. Keyed like
    /// [`override_key`] and [`implementation_key`].
    held_back: HashSet<String>,
    /// The article executed at one stage of its procedure (RFC-008), as
    /// `(law_id, article_number)`, set by [`LawExecutionService::execute_stage`]
    /// and [`LawExecutionService::execute_stage_at`]. Only for that article do
    /// the hooks on its submission wait for their own stage (RFC-047); an
    /// article run outside a procedure fires them on every run (RFC-045).
    staged_article: Option<(String, String)>,
    /// The findings the caller gave for this execution, keyed by the id of
    /// an open term an article leaves to an authority per case
    /// (`decided_per_case_by`, RFC-050). Each applies to every
    /// article in the execution that has such a term.
    findings: BTreeMap<String, Value>,
    /// The ids of [`Self::findings`] that an article in the execution asked.
    findings_asked: BTreeSet<String>,
    /// The ids of [`Self::findings`] on the open term of a policy that fills
    /// a term of an article in the execution, noted when that article runs
    /// (RFC-048 §2). They count as asked, so a case that does not reach the
    /// policy's term does not fail on them, but they are applied only once
    /// the policy reads them ([`Self::findings_asked`]).
    findings_noted: BTreeSet<String>,
    /// The open terms a finding, a regulation or a binding filled in this
    /// execution ([`ArticleResult::open_terms`]).
    open_terms: Vec<OpenTermFilled>,
    /// The cell that runs the execution, asked for the answers a binding
    /// takes (`source.reduction`, RFC-048).
    answerer: Option<&'a dyn OpenTermAnswerer>,
    /// The other cells of the call (RFC-052), asked instead of
    /// [`Self::answerer`] where a binding names them.
    sphere: Vec<&'a dyn OpenTermAnswerer>,
    /// The chronicle a reduction in this execution is applied to: that of
    /// the cell that runs it (RFC-048).
    chronicle: Option<&'a dyn Chronicle>,
    /// The answers each binding took, in the order the engine asked them
    /// ([`Bound`]): how an open term the binding fills came to its value
    /// ([`FilledBy::Answer`]).
    bound: Vec<Bound>,
}

/// The answers one input of a binding took (`source.reduction`, RFC-048):
/// the policy article, and who answered from what. The answers that were
/// not null; where there were none, the null ones; where no cell answered,
/// none.
struct Bound {
    law_id: String,
    article: String,
    answers: Vec<AnswerProvenance>,
}

/// The outputs `article` computes at stage `stage` of its procedure
/// (RFC-047). For an article that produces several things, such as an
/// application and the decision on it: those of the submission whose kind
/// names the stage, and at every other stage those of the decision with
/// the data derived for it, the outputs it lists under `derived` (RFC-050). `None`,
/// every output, for an article that produces one thing.
fn outputs_at_stage<'a>(article: &'a Article, stage: &str) -> Option<Vec<&'a str>> {
    if !article.produces_list() {
        return None;
    }
    let produced = article.get_produced();
    if let Some(submission) = produced.iter().find(|p| p.submission_kind() == Some(stage)) {
        return Some(article.outputs_of(*submission));
    }
    let decision = article.get_decision()?;
    let mut outputs = article.outputs_of(decision);
    outputs.extend(article.derived_outputs());
    Some(outputs)
}

/// Parse the calculation date, rejecting malformed input: an unparseable date
/// must not silently bypass `valid_from`/`valid_to` version selection (RFC-019).
fn parse_calculation_date(calculation_date: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(calculation_date, "%Y-%m-%d")
        .map_err(|e| EngineError::InvalidDate(format!("{calculation_date}: {e}")))
}

/// Why an override whose law is in force still is not applied: the version
/// selected for the reference date has no article with the indexed number.
///
/// When another article of that same version declares an override of the same
/// output, the reason says so: that is the renumbering case, where the special
/// rule *is* in force under another number and the undated overrides index
/// misses it. The engine does not apply it (that would change the outcome and
/// is a separate question), but the receipt must not let it pass for a rule
/// that does not exist yet. The wording is shared with hooks and
/// implementations through [`missing_article_reason`].
fn article_missing_from_version(
    law: &ArticleBasedLaw,
    article_number: &str,
    target_law: &str,
    target_article: &str,
    output_name: &str,
) -> String {
    missing_article_reason(
        law,
        article_number,
        "an override of the same output",
        |candidate| {
            candidate.get_overrides().is_some_and(|decls| {
                decls.iter().any(|d| {
                    d.law == target_law && d.article == target_article && d.output == output_name
                })
            })
        },
    )
}

/// Map a failed version selection to an honest engine error (RFC-019 §3):
/// the message states the data fact for the reference date, never a verdict.
fn selection_error(law_id: &str, calculation_date: &str, reason: SelectionReason) -> EngineError {
    match reason {
        SelectionReason::NotFound => EngineError::LawNotFound(law_id.to_string()),
        SelectionReason::NotYetInForce => EngineError::LawNotYetInForce {
            law_id: law_id.to_string(),
            reference_date: calculation_date.to_string(),
        },
        SelectionReason::EndedOn(valid_to) => EngineError::LawEnded {
            law_id: law_id.to_string(),
            reference_date: calculation_date.to_string(),
            valid_to: valid_to.format("%Y-%m-%d").to_string(),
        },
        // Also a data fact, not a verdict: the file does not say when this
        // version commenced, so the engine cannot say it was in force.
        SelectionReason::UndeterminedStart(reference) => EngineError::ResolutionError(format!(
            "law '{law_id}': its valid_from is the internal reference '{reference}', so whether \
             it was in force on {calculation_date} cannot be determined from the regulation"
        )),
        // A different fact, and it must read differently: the file carries
        // something that is neither a date nor a reference, so there is nothing
        // to resolve elsewhere and nothing to harvest. Saying "cannot be
        // determined" here would dress corruption up as a known gap.
        SelectionReason::UnreadableStart(raw) => EngineError::ResolutionError(format!(
            "law '{law_id}': its valid_from is '{raw}', which is neither a date nor an internal \
             reference, so this version cannot be placed in time at all"
        )),
    }
}

impl<'a> ResolutionContext<'a> {
    /// Create a new resolution context.
    ///
    /// Rejects a malformed `calculation_date`: a date that fails to parse must
    /// not silently bypass `valid_from`/`valid_to` version selection (RFC-019).
    fn new(calculation_date: &'a str) -> Result<Self> {
        let reference_date = Some(parse_calculation_date(calculation_date)?);
        Ok(Self {
            calculation_date,
            reference_date,
            visited: HashSet::new(),
            depth: 0,
            trace: None,
            cache: HashMap::new(),
            contextual_law_id: None,
            anchor: None,
            delegation_refusals: Vec::new(),
            declaration_version_notes: Vec::new(),
            declarations_not_in_force: Vec::new(),
            held_back: HashSet::new(),
            staged_article: None,
            findings: BTreeMap::new(),
            findings_asked: BTreeSet::new(),
            findings_noted: BTreeSet::new(),
            open_terms: Vec::new(),
            answerer: None,
            sphere: Vec::new(),
            chronicle: None,
            bound: Vec::new(),
        })
    }

    /// Take what the caller gives apart from the parameters.
    fn take_call(&mut self, call: Call<'a>) {
        self.findings = call.findings;
        self.answerer = call.answerer;
        self.sphere = call.sphere;
        self.chronicle = call.chronicle;
    }

    /// The caller's findings (RFC-050) that apply to `article` of `law`: one
    /// for each open term it leaves to an authority per case
    /// (`decided_per_case_by`) and that no regulation fills (`filled`, the
    /// ids of the terms a regulation in force fills for this execution). A
    /// finding on a filled term is not applied and stays unasked, so the
    /// execution fails on it whether or not the case reads the term (RFC-048
    /// §2). Each applied finding is marked asked. A finding that does not
    /// fit the type of its term is an error naming it; `null` fits only a
    /// term that is not required.
    fn findings_for(
        &mut self,
        law: &ArticleBasedLaw,
        article: &Article,
        filled: &BTreeSet<String>,
    ) -> Result<BTreeMap<String, Value>> {
        let mut applied = BTreeMap::new();
        if self.findings.is_empty() {
            return Ok(applied);
        }
        for term in article.get_open_terms().into_iter().flatten() {
            if term.decided_per_case_by.is_none() || filled.contains(&term.id) {
                continue;
            }
            let Some(value) = self.findings.get(&term.id) else {
                continue;
            };
            check_finding(&law.id, &article.number, term, value)?;
            self.findings_asked.insert(term.id.clone());
            applied.insert(term.id.clone(), value.clone());
        }
        Ok(applied)
    }

    /// Every finding the caller gave was asked by an article in the
    /// execution (RFC-050), or noted for a policy's own term; otherwise an
    /// error naming those that were not.
    fn check_findings_asked(&self) -> Result<()> {
        let findings: Vec<String> = self
            .findings
            .keys()
            .filter(|id| !self.findings_asked.contains(*id) && !self.findings_noted.contains(*id))
            .cloned()
            .collect();
        if findings.is_empty() {
            return Ok(());
        }
        Err(EngineError::FindingsNotAsked { findings })
    }

    /// Record that `filled_by` filled the open term `term` of `article` of
    /// `law`, once: a term resolved again in the same way is one filling.
    fn note_open_term(&mut self, law: &str, article: &str, term: &str, filled_by: FilledBy) {
        let filled = OpenTermFilled {
            law_id: law.to_string(),
            article: article.to_string(),
            term: term.to_string(),
            filled_by,
        };
        if !self.open_terms.contains(&filled) {
            self.open_terms.push(filled);
        }
    }

    /// Put on `result` what this context collected for the whole execution,
    /// not only for the article that happened to be merged first, and
    /// independently of tracing.
    fn report_into(&self, result: &mut ArticleResult) {
        let mut report = StageReport::default();
        report.add_stage(self);
        report.report_into(result);
    }

    /// Create a new resolution context with trace builder.
    fn with_trace(calculation_date: &'a str, trace: Rc<RefCell<TraceBuilder>>) -> Result<Self> {
        let mut ctx = Self::new(calculation_date)?;
        ctx.trace = Some(trace);
        Ok(ctx)
    }

    /// Return the cached parsed date for version selection.
    fn reference_date(&self) -> Option<NaiveDate> {
        self.reference_date
    }

    /// Enter a cross-law resolution scope: mark key as visited and increment depth.
    fn enter(&mut self, key: String) {
        self.visited.insert(key);
        self.depth += 1;
    }

    /// Leave a cross-law resolution scope: unmark key and decrement depth.
    fn leave(&mut self, key: &str) {
        self.visited.remove(key);
        self.depth -= 1;
    }

    /// What is held back for a base read, as a cycle-key suffix: empty when
    /// nothing is, so ordinary keys are unchanged.
    fn held_back_suffix(&self) -> String {
        Self::suffix_of(self.held_back.iter().map(String::as_str))
    }

    /// [`Self::held_back_suffix`], as it will be once `extra` is held back too.
    fn held_back_suffix_with(&self, extra: &[String]) -> String {
        Self::suffix_of(self.held_back.iter().chain(extra).map(String::as_str))
    }

    fn suffix_of<'k>(keys: impl Iterator<Item = &'k str>) -> String {
        let mut held: Vec<&str> = keys.collect();
        held.sort_unstable();
        held.dedup();
        held.iter().map(|key| format!("\0held:{key}")).collect()
    }

    /// Check if a key is already being resolved (cycle detection).
    fn is_visited(&self, key: &str) -> bool {
        self.visited.contains(key)
    }

    /// Enter a provision, returning the previous one so the caller can restore
    /// it. Cross-law evaluation nests, so this is a stack discipline rather
    /// than an assignment (RFC-039).
    fn enter_anchor(&mut self, anchor: Option<LegalAnchor>) -> Option<LegalAnchor> {
        std::mem::replace(&mut self.anchor, anchor)
    }

    /// Push a new trace node. No-op if tracing is disabled.
    ///
    /// Every step pushed here inherits the provision the engine is currently
    /// in as its `anchor` (RFC-039), which is why no push site has to know an
    /// article: a bare arithmetic step inside an article carries that article
    /// just as a resolve does.
    fn trace_push(&self, name: impl Into<String>, node_type: PathNodeType) {
        if let Some(ref tb) = self.trace {
            let mut tb = tb.borrow_mut();
            tb.push(name, node_type);
            if let Some(ref anchor) = self.anchor {
                tb.set_anchor(anchor.clone());
            }
        }
    }

    /// Set the result on the current trace node. No-op if tracing is disabled.
    fn trace_set_result(&self, result: Value) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_result(result);
        }
    }

    /// Set a message on the current trace node. No-op if tracing is disabled.
    fn trace_set_message(&self, msg: impl Into<String>) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_message(msg);
        }
    }

    /// Set the resolve type on the current trace node. No-op if tracing is disabled.
    fn trace_set_resolve_type(&self, rt: ResolveType) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_resolve_type(rt);
        }
    }

    /// Record who answered the current node's open term (RFC-048).
    fn trace_set_answer(&self, answer: AnswerProvenance) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_answer(answer);
        }
    }

    /// Record where the current node's value came from (RFC-039).
    fn trace_set_source(&self, source: ValueSource) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_source(source);
        }
    }

    /// Record the declared unit and precision of the current node's result
    /// (RFC-023, RFC-039).
    fn trace_set_type_spec(&self, type_spec: TypeSpec) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().set_type_spec(type_spec);
        }
    }

    /// Record a hook, override or implementation that the indexes offered but
    /// that is not in force on the reference date: its law has no version for
    /// that date, or the version it has lacks the indexed article.
    ///
    /// It is skipped either way — a regulation that is not in force does not
    /// apply — but the skip leaves no mark on the outcome, so it is stated
    /// three times over: in the log, as a node in the trace under the kind it
    /// belongs to, and on the result, which carries it to the receipt even when
    /// no trace was requested.
    fn note_not_in_force(&mut self, note: DeclarationNotInForce) {
        tracing::warn!(
            kind = note.kind.as_str(),
            law_id = %note.law_id,
            article = %note.article,
            subject = %note.subject,
            reason = %note.reason,
            "Declaration not applied: not in force on the reference date"
        );
        let node_type = match note.kind {
            DeclarationKind::Hook => PathNodeType::HookResolution,
            DeclarationKind::Override => PathNodeType::OverrideResolution,
            DeclarationKind::Implementation => PathNodeType::OpenTermResolution,
        };
        let message = note.message();
        {
            let _guard = self.trace_guard(format!("{}:{}", note.law_id, note.article), node_type);
            self.trace_set_message(message);
        }
        self.declarations_not_in_force.push(note);
    }

    /// Push a trace node and return a guard that auto-pops on drop.
    ///
    /// Guarantees balanced push/pop even on early returns or errors.
    /// Use `set_message`, `set_result`, etc. on `res_ctx` as usual —
    /// the guard only handles the pop.
    fn trace_guard(&self, name: impl Into<String>, node_type: PathNodeType) -> TraceGuard {
        self.trace_push(name, node_type);
        TraceGuard {
            trace: self.trace.clone(),
        }
    }
}

/// RAII guard that pops a trace node when dropped.
///
/// Created by `ResolutionContext::trace_guard`. Ensures balanced
/// push/pop even when errors cause early returns. Holds its own
/// `Rc` to the trace builder so it doesn't borrow `ResolutionContext`,
/// avoiding conflicts with mutable methods like `enter`/`leave`.
struct TraceGuard {
    trace: Option<Rc<RefCell<TraceBuilder>>>,
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        if let Some(ref tb) = self.trace {
            tb.borrow_mut().pop();
        }
    }
}

/// The authorities `named`, for a message: "'a' or 'b'".
fn authorities(named: &[crate::article::NamedAuthority]) -> String {
    named
        .iter()
        .map(|authority| format!("'{}'", authority.name))
        .collect::<Vec<_>>()
        .join(" or ")
}

/// Build a cache key from law_id, output_name, and parameters.
///
/// The cache key includes output_name because different outputs within the same
/// law may be produced by different articles. The multi-output API (`evaluate_law`)
/// avoids redundant evaluations by grouping outputs by article before calling
/// the internal single-output method.
///
/// Uses hashing instead of String building to avoid allocations.
/// BTreeMap guarantees sorted key order for deterministic hashing.
/// Note: `DefaultHasher` is randomly seeded per process, so keys are only
/// valid for per-execution memoization (not persisted across runs).
fn cache_key(law_id: &str, output_name: &str, params: &BTreeMap<String, Value>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    law_id.hash(&mut hasher);
    output_name.hash(&mut hasher);
    // BTreeMap iterates in sorted key order — no explicit sort needed
    for (key, value) in params {
        key.hash(&mut hasher);
        hash_value(value, &mut hasher);
    }
    hasher.finish()
}

/// The first parameter `article` declares as required (`required` absent or
/// `true`) that `parameters` does not carry, if any.
///
/// Used to tell "nobody has this fact" (Unknown, RFC-036) apart from "the
/// register was never asked about anybody": a `source: {}` input cannot be
/// resolved without its lookup key, and that is the caller's omission, not a
/// missing fact.
fn required_parameter_not_passed(
    article: &Article,
    parameters: &BTreeMap<String, Value>,
) -> Option<String> {
    article
        .get_execution_spec()
        .and_then(|exec| exec.parameters.as_ref())
        .and_then(|declared| {
            declared
                .iter()
                .find(|p| p.required != Some(false) && !parameters.contains_key(&p.name))
                .map(|p| p.name.clone())
        })
}

/// The first parameter of `article` that `parameters` carries as a `null`
/// the declaration does not allow, or as an unknown while the parameter is
/// required, with the word for how it was left empty (`"null"` or
/// `"unknown"`).
///
/// At the top level that is the caller's error (RFC-036): a law cannot be
/// evaluated for nobody, and letting the register be asked about nobody would
/// report its every input as an unknown fact, or produce a `null` that a
/// downstream operation then blames on the law. Whether `null` is allowed is
/// decided by the type (`nullable`), not by `required`: a nullable parameter
/// passed as `null` is an absence the law tests for. Unknown is not governed
/// by `nullable`; there the `required` flag decides, as before. Across laws a
/// `null` required parameter is the skip rule in
/// `resolve_external_input_internal`, so this is checked at depth 0 only.
fn required_parameter_for_nobody(
    article: &Article,
    parameters: &BTreeMap<String, Value>,
) -> Option<(String, &'static str)> {
    article
        .get_parameters()
        .iter()
        .find_map(|p| match parameters.get(&p.name) {
            Some(Value::Null) if !p.is_nullable() => Some((p.name.clone(), "null")),
            Some(Value::Unknown(_)) if p.required != Some(false) => {
                Some((p.name.clone(), "unknown"))
            }
            _ => None,
        })
}

/// The keys of a decision value (schema v0.8.4, RFC-050).
const DECISION_KEYS: [&str; 5] = ["basis", "taken_on", "fields", "findings", "outputs"];

/// Every parameter of `article` of type `decision` that `parameters` carries
/// is an object that holds the keys of a decision value `{basis,
/// taken_on, fields, findings, outputs}`, and no other (RFC-050), and its
/// `basis` and `taken_on` are article references (`<law>#<article>`).
/// Otherwise an error naming the parameter and the key, or the type the
/// value has instead. Null and unknown are left to
/// `required_parameter_for_nobody`, and an untranslatable value propagates
/// (RFC-012). What `fields`, `findings` and `outputs` hold is checked where
/// the value is read, as for a decision value an input holds.
///
/// A decision given into an article was taken on a submission, and names
/// its article in `taken_on`. Missing or no article reference, a read of it
/// would fail without naming the parameter or, compared with an article
/// reference, silently differ from it; a value that is never read would
/// pass unnoticed. Only a value the engine builds through `source.article`
/// lacks `taken_on`; that is an input.
///
/// All of this holds at the top (`at_top`, depth 0). In a nested call,
/// across laws or within one, a parameter is what the calling article passes
/// on, which may be such a value without `taken_on`, so there only a
/// `taken_on` the value holds must be an article reference. `basis` is
/// checked where the call reads it.
fn check_decision_parameters(
    law: &ArticleBasedLaw,
    article: &Article,
    parameters: &BTreeMap<String, Value>,
    at_top: bool,
) -> Result<()> {
    let reference = |value: &BTreeMap<String, Value>, key: &str| {
        matches!(
            value.get(key),
            Some(Value::String(s)) if regelrecht_law_model::is_article_reference(s)
        )
    };
    for declared in article.get_parameters() {
        if declared.param_type != crate::types::ParameterType::Decision {
            continue;
        }
        let value = match parameters.get(&declared.name) {
            Some(Value::Object(value)) => value,
            // Not passed, an absence or an unknown: `required_parameter_for_nobody`
            // already decided whether the article may do without it. An
            // untranslatable value is a taint that propagates (RFC-012).
            None | Some(Value::Null | Value::Unknown(_) | Value::Untranslatable { .. }) => {
                continue;
            }
            // In a nested call the value is read where the call needs it.
            Some(_) if !at_top => continue,
            Some(other) => {
                return Err(decision_parameter_error(
                    law,
                    article,
                    &declared.name,
                    &format!("is of type {}", other.type_name()),
                ))
            }
        };
        let flaw = if at_top {
            DECISION_KEYS
                .iter()
                .find(|key| !value.contains_key(**key))
                .map(|key| format!("lacks '{key}'"))
                .or_else(|| {
                    value
                        .keys()
                        .find(|key| !DECISION_KEYS.contains(&key.as_str()))
                        .map(|key| format!("holds the unknown key '{key}'"))
                })
                .or_else(|| {
                    ["basis", "taken_on"]
                        .into_iter()
                        .find(|key| !reference(value, key))
                        .map(|key| format!("holds no article reference in '{key}'"))
                })
        } else {
            (value.contains_key("taken_on") && !reference(value, "taken_on"))
                .then(|| "holds no article reference in 'taken_on'".to_string())
        };
        if let Some(flaw) = flaw {
            return Err(decision_parameter_error(
                law,
                article,
                &declared.name,
                &flaw,
            ));
        }
    }
    Ok(())
}

/// The error for a parameter of type `decision` that holds no decision value.
fn decision_parameter_error(
    law: &ArticleBasedLaw,
    article: &Article,
    name: &str,
    flaw: &str,
) -> EngineError {
    EngineError::InvalidOperation(format!(
        "parameter '{name}' of {}#{} is no decision value {{basis, taken_on, fields, \
         findings, outputs}}: it {flaw}",
        law.id, article.number
    ))
}

/// The error for an output an executed article did not produce.
///
/// A void is an answer, not a defect: when the provenance records one, the
/// caller is told the law says the output does not arise, with the words that
/// say so. Everything else keeps reporting a missing output, which is the
/// shape of a misspelled name or a broken binding.
fn missing_output_error(
    law_id: &str,
    output_name: &str,
    provenance: &BTreeMap<String, OutputProvenance>,
) -> EngineError {
    voided_output_error(law_id, output_name, provenance).unwrap_or_else(|| {
        EngineError::OutputNotFound {
            law_id: law_id.to_string(),
            output: output_name.to_string(),
        }
    })
}

/// The void error for an output, when the provenance records one.
fn voided_output_error(
    law_id: &str,
    output_name: &str,
    provenance: &BTreeMap<String, OutputProvenance>,
) -> Option<EngineError> {
    match provenance.get(output_name) {
        Some(OutputProvenance::Voided {
            law_id: voided_by,
            article,
            grounds,
        }) => Some(EngineError::OutputVoided {
            law_id: law_id.to_string(),
            output: output_name.to_string(),
            voided_by: voided_by.clone(),
            article: article.clone(),
            // A void without a quoted ground is a law file the marking gate
            // should have refused; say so rather than printing an empty
            // sentence at whoever has to act on it.
            grounds: grounds
                .clone()
                .unwrap_or_else(|| "no ground was quoted".to_string()),
        }),
        _ => None,
    }
}

/// What an article execution needs, settled before its actions run (RFC-043).
#[derive(Debug)]
struct Demand {
    /// The outputs to compute, closed over what they read; `None` for all.
    outputs: Option<BTreeSet<String>>,
    /// Outputs an applicable override replaces, with the parameters that
    /// override declares: replaced where the action sets them, so the
    /// article's later actions read the replaced value.
    replacing: BTreeMap<String, BTreeSet<String>>,
    /// See [`OverridePlan::rule_reads`].
    rule_reads: BTreeMap<String, BTreeSet<String>>,
    /// Requested outputs a `voids` excludes: not computed, their ground is
    /// recorded by `apply_overrides`.
    voided: Vec<String>,
    /// What the pre_actions hooks declare as parameters.
    read_before_actions: BTreeSet<String>,
    /// What the post_actions hooks and replacing overrides declare.
    read_after_actions: BTreeSet<String>,
}

/// What running the override of one output came to.
enum OverrideOutcome {
    /// No override applies, or the one that applies gave no value for it.
    NotApplied,
    /// The output does not arise, on this ground.
    Voided(OutputProvenance),
    /// The output takes this value from the overriding article.
    Replaced(Value, OutputProvenance),
}

/// The one override of an output that applies (RFC-007, RFC-041).
enum SelectedOverride<'s> {
    /// In force on this date, with the declaration that addresses the output.
    InForce {
        reference: &'s LawArticleRef,
        law: &'s ArticleBasedLaw,
        article: &'s Article,
        declaration: Option<&'s crate::article::OverrideDeclaration>,
    },
    /// Addressed, but not in force on this date, and why.
    NotInForce {
        reference: &'s LawArticleRef,
        reason: String,
    },
}

impl<'s> SelectedOverride<'s> {
    fn reference(&self) -> &'s LawArticleRef {
        match self {
            Self::InForce { reference, .. } | Self::NotInForce { reference, .. } => reference,
        }
    }
}

/// Which overrides of an article's outputs apply, read before its actions
/// run (RFC-043).
#[derive(Debug, Default)]
struct OverridePlan {
    /// Outputs an applicable `voids` excludes: not computed when requested.
    voided: BTreeSet<String>,
    /// Per output with an applicable replacing override, the parameters its
    /// article declares: it receives those names from this article, when it
    /// runs, which is when its output is computed.
    replacing: BTreeMap<String, BTreeSet<String>>,
    /// Per such output, the declared parameters the override's rule for that
    /// output may read: see [`rule_reads`]. At most `replacing`.
    rule_reads: BTreeMap<String, BTreeSet<String>>,
}

/// An article's inputs and open terms, each resolved the first time it is
/// read and kept for the rest of the execution (RFC-043).
///
/// Resolving takes the resolution context mutably. What a resolution reads of
/// the same article is therefore resolved before the context is taken (the
/// inputs a cross-law call is keyed on, `bsn: $partner_bsn`), or with the
/// context in hand (the earlier terms an open term's default reads).
///
/// A name being resolved is not this article's to answer while it is: it
/// falls through to the parameters. So a source keyed on its own name reads
/// the parameter of that name, and two inputs keyed on each other find
/// neither.
struct LazyArticleInputs<'a, 'r, 'c> {
    service: &'a LawExecutionService,
    article: &'a Article,
    law: &'a ArticleBasedLaw,
    parameters: &'a BTreeMap<String, Value>,
    /// The article-level scope a source's parameters are read in, built once.
    scope: RuleContext<'static>,
    res_ctx: RefCell<&'r mut ResolutionContext<'c>>,
    memo: RefCell<BTreeMap<String, Slot>>,
}

/// What the memo knows about a name.
#[derive(Clone)]
enum Slot {
    Resolving,
    /// Resolved, with where it came from, or `None`: not this article's to
    /// resolve, or left unresolved.
    Done(Option<(Value, ResolveType)>),
}

/// What a name read in an article refers to, when the article resolves it.
enum LazyName<'a> {
    Input(&'a Input),
    /// An input the caller passed a value for: the value wins over the source.
    Passed(&'a Input),
    OpenTerm(&'a crate::article::OpenTerm),
}

/// The actions' view of an article's inputs after its pre_actions hooks ran:
/// a name a hook produced a value under is read from that value. Where the
/// article's sources read their parameters the inputs themselves count.
struct AfterPreHooks<'x> {
    inputs: &'x dyn LazyInputs,
    hook_outputs: &'x BTreeMap<String, Value>,
    /// What a replacing override makes of an output the article just set.
    replace: &'x ReplaceOutput<'x>,
}

/// Runs the replacing override of an output, if one applies, given the name,
/// its computed value and the outputs computed before it.
type ReplaceOutput<'x> =
    dyn Fn(&str, &Value, &BTreeMap<String, Value>) -> Option<Result<Value>> + 'x;

impl LazyInputs for AfterPreHooks<'_> {
    fn resolve_input(&self, name: &str) -> Option<Result<(Value, ResolveType)>> {
        if self.hook_outputs.contains_key(name) {
            return None;
        }
        self.inputs.resolve_input(name)
    }

    fn replace_output(
        &self,
        name: &str,
        value: &Value,
        outputs: &BTreeMap<String, Value>,
    ) -> Option<Result<Value>> {
        (self.replace)(name, value, outputs)
    }
}

/// Resolves an open term declared before the one being resolved, by name:
/// `None` when the name is not one.
type EarlierTerm<'x, 'c> =
    dyn FnMut(&str, &mut ResolutionContext<'c>) -> Result<Option<Value>> + 'x;

/// The open terms declared before the one whose default is being evaluated,
/// resolved when an operation of that default reads one (RFC-043).
struct EarlierTerms<'x, 'c> {
    earlier: RefCell<&'x mut EarlierTerm<'x, 'c>>,
    res_ctx: RefCell<&'x mut ResolutionContext<'c>>,
}

impl LazyInputs for EarlierTerms<'_, '_> {
    fn resolve_input(&self, name: &str) -> Option<Result<(Value, ResolveType)>> {
        let (Ok(mut earlier), Ok(mut res_ctx)) =
            (self.earlier.try_borrow_mut(), self.res_ctx.try_borrow_mut())
        else {
            return Some(Err(EngineError::InvalidOperation(format!(
                "open term '{name}' was read while another one was being resolved"
            ))));
        };
        (*earlier)(name, &mut res_ctx)
            .transpose()
            .map(|value| value.map(|value| (value, ResolveType::OpenTerm)))
    }
}

/// The key the override in `law_id` article `article` is entered under, for
/// cycle detection.
fn override_key(law_id: &str, article: &str) -> String {
    format!("override:{law_id}\0{article}")
}

/// The article of `target_law` whose `output` `article` replaces (a
/// replacing, not voiding, override), when that article produces it.
fn replaced_article<'l>(
    article: &Article,
    target_law: &'l ArticleBasedLaw,
    output: &str,
) -> Option<&'l Article> {
    article
        .get_overrides()?
        .iter()
        .filter(|d| !d.voids && d.law == target_law.id && d.output == output)
        .find_map(|d| {
            target_law
                .find_article_by_number(&d.article)
                .filter(|target| target.has_output(output))
        })
}

/// The key the implementation in `law_id` article `article` of an open term
/// of `implemented` is held back under, while that article reads that law. Per
/// implemented law: an article filling terms in two laws and reading one of
/// them still fills the other.
fn implementation_key(law_id: &str, article: &str, implemented: &str) -> String {
    format!("implementation:{law_id}\0{article}\0{implemented}")
}

/// The key the hook in `law_id` article `article` is entered under, for cycle
/// detection.
fn hook_key(law_id: &str, article: &str) -> String {
    format!("hook:{law_id}\0{article}")
}

/// A hook that fires on an article: its entry, and its law and article in
/// force on the date.
struct FireableHook<'s> {
    entry: &'s HookEntry,
    law: &'s ArticleBasedLaw,
    article: &'s Article,
}

/// The parameters an article declares: the names it receives from a caller.
fn declared_parameter_names(article: &Article) -> impl Iterator<Item = String> + '_ {
    article
        .get_execution_spec()
        .and_then(|e| e.parameters.as_ref())
        .into_iter()
        .flatten()
        .map(|p| p.name.clone())
}

/// The parameters `article`'s rule for `output` may read, kept to what the
/// article declares. Narrowed to what its actions for that output (closed
/// over the article's own outputs) refer to only when that is all it can
/// read: an article with a sourced input (a same-law reference receives all
/// its parameters, a data source is keyed on them), an open term (a default
/// or an implementation receives them) or a `produces` (its hooks receive
/// them) may read any declared parameter, so for it this is all of them; the
/// same for an article whose own outputs are overridden (`override_plan`).
/// Missing a read would let the file order decide what the override sees.
fn rule_reads(article: &Article, output: &str) -> BTreeSet<String> {
    let declared: BTreeSet<String> = declared_parameter_names(article).collect();
    let passes_parameters_on = article.get_inputs().iter().any(|i| i.source.is_some())
        || article.get_open_terms().is_some_and(|t| !t.is_empty())
        || !article.get_produced().is_empty();
    if passes_parameters_on {
        return declared;
    }
    let actions = article
        .get_execution_spec()
        .and_then(|e| e.actions.as_deref())
        .unwrap_or_default();
    let closure = crate::demand::required_outputs(actions, &[output]);
    let mut reads: BTreeSet<String> = actions
        .iter()
        .filter(|action| action.output.as_ref().is_some_and(|o| closure.contains(o)))
        .flat_map(crate::demand::referenced_names)
        .collect();
    reads.retain(|name| declared.contains(name));
    reads
}

impl<'a, 'r, 'c> LazyArticleInputs<'a, 'r, 'c> {
    fn new(
        service: &'a LawExecutionService,
        article: &'a Article,
        law: &'a ArticleBasedLaw,
        parameters: &'a BTreeMap<String, Value>,
        res_ctx: &'r mut ResolutionContext<'c>,
    ) -> Result<Self> {
        let scope = crate::engine::article_context(
            law,
            article,
            parameters,
            res_ctx.calculation_date,
            res_ctx.trace.as_ref(),
        )?;
        Ok(Self {
            service,
            article,
            law,
            parameters,
            scope,
            res_ctx: RefCell::new(res_ctx),
            memo: RefCell::new(BTreeMap::new()),
        })
    }

    /// Everything resolved so far: what this article consulted.
    fn resolved(&self) -> BTreeMap<String, Value> {
        self.memo
            .borrow()
            .iter()
            .filter_map(|(name, slot)| match slot {
                Slot::Done(Some((value, _))) => Some((name.clone(), value.clone())),
                _ => None,
            })
            .collect()
    }

    /// Lend the resolution context out for a step between resolutions (firing
    /// a hook). Nothing resolves lazily while it is lent.
    fn with_resolution_context<T>(
        &self,
        step: impl FnOnce(&mut ResolutionContext<'c>) -> Result<T>,
    ) -> Result<T> {
        let mut res_ctx = self.res_ctx.try_borrow_mut().map_err(|_| {
            EngineError::InvalidOperation(format!(
                "the resolution context of {} article {} is already in use",
                self.law.id, self.article.number
            ))
        })?;
        step(&mut res_ctx)
    }

    /// Resolve each of `names` that is an input or open term of this article.
    fn resolve_names<'n>(&self, names: impl IntoIterator<Item = &'n String>) -> Result<()> {
        for name in names {
            if let Some(Err(e)) = self.resolve_input(name) {
                return Err(e);
            }
        }
        Ok(())
    }

    /// Answer `name` from the memo, or with `resolve` and remember the
    /// answer. `None` while `name` is being resolved.
    fn memoized(
        &self,
        name: &str,
        resolve: impl FnOnce() -> Result<Option<(Value, ResolveType)>>,
    ) -> Option<Result<(Value, ResolveType)>> {
        let known = self.memo.borrow().get(name).cloned();
        match known {
            Some(Slot::Done(answer)) => return answer.map(Ok),
            Some(Slot::Resolving) => return None,
            None => {}
        }
        self.memo
            .borrow_mut()
            .insert(name.to_string(), Slot::Resolving);
        match resolve() {
            Ok(answer) => {
                self.memo
                    .borrow_mut()
                    .insert(name.to_string(), Slot::Done(answer.clone()));
                answer.map(Ok)
            }
            Err(e) => {
                self.memo.borrow_mut().remove(name);
                Some(Err(e))
            }
        }
    }

    /// What `name` refers to, if this article resolves it. An open term
    /// comes before an input and a parameter of the same name; of two inputs
    /// with one name the last declared counts; a value passed under an
    /// input's name wins over its source.
    fn owner(&self, name: &str) -> Option<LazyName<'a>> {
        let article: &'a Article = self.article;
        if let Some(term) = article
            .get_open_terms()
            .and_then(|terms| terms.iter().find(|t| t.id == name))
        {
            return Some(LazyName::OpenTerm(term));
        }
        let input = article.get_inputs().iter().rev().find(|i| i.name == name)?;
        if self.parameters.contains_key(name) {
            return Some(LazyName::Passed(input));
        }
        input.source.is_some().then_some(LazyName::Input(input))
    }

    /// The article-level scope a source's parameters are read in: the
    /// caller's parameters, the article's definitions and its other inputs,
    /// never the scope of the operation that happened to read the input (a
    /// FOREACH variable must not become a lookup key).
    fn article_context(&self) -> RuleContext<'_> {
        let mut context: RuleContext<'_> = self.scope.clone();
        context.set_lazy(self);
        context
    }

    /// Resolve what a name refers to. `Ok(None)` leaves it unresolved.
    fn resolve_owned(&self, name: LazyName<'a>) -> Result<Option<(Value, ResolveType)>> {
        match name {
            // A value handed in under the input's name bypasses its source,
            // not its declaration: a null for an input that is never absent is
            // refused like a null cell would be, whoever passed it (a caller,
            // a scenario, an editor form). Otherwise it would surface later as
            // an absent operand without an origin (RFC-036). The value itself
            // is read from the parameters.
            LazyName::Passed(input) => {
                if self.parameters.get(&input.name).is_some_and(Value::is_null)
                    && !input.is_nullable()
                {
                    return Err(null_for_non_nullable(
                        self.law,
                        input,
                        "parameter from caller".to_string(),
                    ));
                }
                Ok(None)
            }
            LazyName::Input(input) => {
                let context = self.article_context();
                // A call to another law is keyed on the source's parameters,
                // which may read other inputs of this article: those are
                // resolved first, each as its own step. No other way of
                // resolving an input reads them.
                if self
                    .service
                    .calls_other_law(self.law, input, self.parameters)
                {
                    // A call to an article also reads the value that names
                    // it and the objects it merges (RFC-050).
                    let keys = input
                        .source
                        .iter()
                        .flat_map(|source| {
                            source
                                .parameters
                                .iter()
                                .flat_map(|p| p.values())
                                .chain(source.article.iter())
                                .chain(source.parameters_from.iter().flatten())
                        })
                        .filter_map(|reference| crate::demand::reference_base(reference));
                    for key in keys {
                        if self.owner(key).is_some() {
                            context.resolve(key)?;
                        }
                    }
                }
                let value = self.with_resolution_context(|res_ctx| {
                    self.service.resolve_input(
                        self.article,
                        self.law,
                        input,
                        &context,
                        self.parameters,
                        res_ctx,
                    )
                })?;
                Ok(value.map(|value| (value, ResolveType::ResolvedInput)))
            }
            LazyName::OpenTerm(term) => {
                let value =
                    self.with_resolution_context(|res_ctx| self.resolve_term(term, res_ctx))?;
                Ok(Some((value, ResolveType::OpenTerm)))
            }
        }
    }

    /// Resolve an open term with the context in hand. Its default reads the
    /// terms declared before it through [`Self::earlier_term`].
    fn resolve_term(
        &self,
        term: &'a crate::article::OpenTerm,
        res_ctx: &mut ResolutionContext<'c>,
    ) -> Result<Value> {
        self.service.resolve_open_term(
            self.article,
            self.law,
            term,
            self.parameters,
            &mut |id, res_ctx| self.earlier_term(term, id, res_ctx),
            res_ctx,
        )
    }

    /// The value of `id` if it is an open term declared before `term`.
    fn earlier_term(
        &self,
        term: &crate::article::OpenTerm,
        id: &str,
        res_ctx: &mut ResolutionContext<'c>,
    ) -> Result<Option<Value>> {
        let article: &'a Article = self.article;
        let Some(earlier) = article.get_open_terms().and_then(|terms| {
            terms
                .iter()
                .take_while(|t| t.id != term.id)
                .find(|t| t.id == id)
        }) else {
            return Ok(None);
        };
        let answer = self.memoized(id, || {
            let value = self.resolve_term(earlier, res_ctx)?;
            Ok(Some((value, ResolveType::OpenTerm)))
        });
        answer
            .transpose()
            .map(|found| found.map(|(value, _)| value))
    }
}

impl LazyInputs for LazyArticleInputs<'_, '_, '_> {
    fn resolve_input(&self, name: &str) -> Option<Result<(Value, ResolveType)>> {
        self.memoized(name, || match self.owner(name) {
            Some(owned) => self.resolve_owned(owned),
            None => Ok(None),
        })
    }
}

/// Named values, such as parameters or findings.
type Values = BTreeMap<String, Value>;

/// The parts of the decision value `value` that `named_by` holds, for
/// `caller` to call anew (RFC-050): its `basis`, an article reference,
/// and its `fields` and `findings`, two objects. Anything else is an error.
fn decision_parts(
    mut value: BTreeMap<String, Value>,
    named_by: &str,
    caller: &str,
) -> Result<(String, Values, Values)> {
    let shown = Value::Object(value.clone()).to_string();
    let parts = (
        value.remove("basis"),
        value.remove("fields"),
        value.remove("findings"),
    );
    match parts {
        (
            Some(Value::String(basis)),
            Some(Value::Object(fields)),
            Some(Value::Object(findings)),
        ) if regelrecht_law_model::is_article_reference(&basis) => Ok((basis, fields, findings)),
        _ => Err(EngineError::TypeMismatch {
            expected: format!(
                "a decision value {{basis, fields, findings, outputs}} in {named_by} for \
                 {caller}"
            ),
            actual: shown,
        }),
    }
}

/// Whether `value` fits the type of the open term `term`, as a finding for
/// it (RFC-050): a boolean, a number for a number, a whole number for an
/// amount (in eurocent), a string for a string, a `YYYY-MM-DD` date for a
/// date, a list, or an object for an object or a decision.
/// `null` fits only a term that is not required.
fn finding_fits(term: &crate::article::OpenTerm, value: &Value) -> bool {
    use crate::types::ParameterType as T;
    match (term.term_type, value) {
        (_, Value::Null) => !term.required,
        (T::Date, Value::String(day)) => NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == *day),
        (T::Boolean, Value::Bool(_))
        | (T::Number, Value::Int(_) | Value::Decimal(_))
        | (T::Amount, Value::Int(_))
        | (T::String, Value::String(_))
        | (T::Array, Value::Array(_))
        | (T::Object | T::Decision, Value::Object(_)) => true,
        _ => false,
    }
}

/// Refuse a finding `value` on the open term `term` of `article` of `law_id`
/// that does not fit the term ([`finding_fits`]), naming it.
fn check_finding(
    law_id: &str,
    article: &str,
    term: &crate::article::OpenTerm,
    value: &Value,
) -> Result<()> {
    if finding_fits(term, value) {
        return Ok(());
    }
    Err(EngineError::TypeMismatch {
        expected: format!(
            "a {} for the finding '{}' on {law_id}#{article}{}",
            format!("{:?}", term.term_type).to_lowercase(),
            term.id,
            if term.required {
                ", which is required"
            } else {
                ""
            }
        ),
        actual: value.to_string(),
    })
}

/// `value` as the value of `input` of `law`, unless it is a `null` the input
/// is not declared to take (RFC-036); `origin` names where it came from.
fn checked_call_value(
    law: &ArticleBasedLaw,
    input: &Input,
    value: Value,
    origin: String,
) -> Result<Option<Value>> {
    if value.is_null() && !input.is_nullable() {
        return Err(null_for_non_nullable(law, input, origin));
    }
    Ok(Some(value))
}

/// The error for a `null` that reached `input` of `law` although the input is
/// not declared nullable (RFC-036), naming where the null came from.
fn null_for_non_nullable(law: &ArticleBasedLaw, input: &Input, origin: String) -> EngineError {
    EngineError::NullForNonNullable {
        law_id: law.id.clone(),
        field: input.name.clone(),
        origin,
    }
}

/// How a cross-law reference was resolved: whether the target ran, or was
/// skipped because a required parameter named nobody (RFC-036). The caller
/// needs the distinction to name the origin of a `null` it may not accept.
struct ExternalResolution {
    value: Value,
    skipped: bool,
}

/// Hash a Value for cache key purposes.
fn hash_value(value: &Value, hasher: &mut impl Hasher) {
    std::mem::discriminant(value).hash(hasher);
    match value {
        Value::Null => {}
        Value::Bool(b) => b.hash(hasher),
        Value::Int(i) => i.hash(hasher),
        Value::Decimal(d) => {
            // Normalize so equal decimals (1.0 == 1.00) hash identically, matching PartialEq.
            d.normalize().hash(hasher);
        }
        Value::String(s) => s.hash(hasher),
        Value::Array(arr) => {
            arr.len().hash(hasher);
            for v in arr {
                hash_value(v, hasher);
            }
        }
        Value::Object(map) => {
            map.len().hash(hasher);
            // BTreeMap iterates in sorted key order — no explicit sort needed
            for (key, v) in map {
                key.hash(hasher);
                hash_value(v, hasher);
            }
        }
        Value::Untranslatable { article, construct } => {
            article.hash(hasher);
            construct.hash(hasher);
        }
        Value::Unknown(missing) => {
            missing.len().hash(hasher);
            for fact in missing {
                fact.law.hash(hasher);
                fact.name.hash(hasher);
                std::mem::discriminant(&fact.kind).hash(hasher);
            }
        }
    }
}

/// One construct an article flags as beyond what it can express (RFC-012).
///
/// Two channels say this, and the engine treats them alike: `untranslatables`
/// on schema v0.5.x and `markings` from v0.7.0. Markings are read *beside*
/// untranslatables, not instead of them, because the laws in the test suite are
/// still on v0.5.x and an article that flags something must never be executed
/// as if it had flagged nothing.
struct FlaggedConstruct<'a> {
    /// Which channel this came from, used verbatim in the trace.
    label: &'static str,
    /// The construct itself, in the words the article uses.
    construct: &'a str,
    /// What has to change before the article can be translated in full.
    reason: String,
    /// Whether a human has reviewed and acknowledged this gap.
    accepted: bool,
}

impl FlaggedConstruct<'_> {
    fn to_error(&self, law_id: &str, article_number: &str) -> EngineError {
        EngineError::Untranslatable {
            law_id: law_id.to_string(),
            article: article_number.to_string(),
            construct: self.construct.to_string(),
            reason: self.reason.clone(),
        }
    }
}

/// Everything an article flags, both channels, in document order.
fn flagged_constructs(machine_readable: &MachineReadable) -> Vec<FlaggedConstruct<'_>> {
    let untranslatables = machine_readable
        .untranslatables
        .iter()
        .flatten()
        .map(|entry| FlaggedConstruct {
            label: "Untranslatable",
            construct: &entry.construct,
            reason: entry.reason.clone(),
            accepted: entry.accepted,
        });
    let markings = machine_readable
        .markings
        .iter()
        .flatten()
        .map(|marking| FlaggedConstruct {
            label: "Marking",
            construct: &marking.about,
            reason: marking_reason(marking),
            accepted: marking.accepted,
        });
    untranslatables.chain(markings).collect()
}

/// Why a marked article stopped, as one sentence.
///
/// A v0.5.x `untranslatables` entry carried a single free-text `reason`. A
/// marking says the same thing in three fields, and the error message wants
/// all of them: the diagnosis from `reason`, and behind it the layer that has
/// to move (`resolution`) with the change that would move it (`resolved_by`).
/// The diagnosis leads because it is the half that cannot be reconstructed
/// from the other two — the change follows from the reading of the provision
/// and never the other way round. `resolved_by` is required by schema v0.7.0
/// but optional on the model, which must also read files written before it
/// was; then the layer is the answer.
fn marking_reason(marking: &crate::article::Marking) -> String {
    let layer = match marking.resolution {
        crate::article::MarkingResolution::Operation => "needs a new operation",
        crate::article::MarkingResolution::Model => "needs a new shape in the format",
    };
    let change = match marking.resolved_by {
        Some(ref resolved_by) => format!("{layer}: {resolved_by}"),
        None => layer.to_string(),
    };
    match marking.reason.trim() {
        "" => change,
        diagnosis => format!("{diagnosis} ({change})"),
    }
}

/// Trait for resolving cross-law references.
///
/// Implement this trait to provide custom law loading and resolution strategies.
/// The default implementation is `LawExecutionService`.
pub trait ServiceProvider {
    /// Resolve and execute a regelrecht:// URI.
    ///
    /// # Arguments
    /// * `uri` - The URI to resolve (e.g., "regelrecht://law_id/output")
    /// * `parameters` - Parameters to pass to the target article
    /// * `calculation_date` - Date for calculations (YYYY-MM-DD)
    ///
    /// # Returns
    /// The execution result from the referenced article.
    fn evaluate_uri(
        &self,
        uri: &str,
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult>;

    /// Get a law by ID.
    fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw>;

    /// Resolve an external input source.
    ///
    /// This is the main entry point for resolving `source.regulation` references.
    ///
    /// # Arguments
    /// * `regulation` - The target regulation ID
    /// * `output` - The output name to resolve
    /// * `source_parameters` - Parameters mapping from source (e.g., {"BSN": "$BSN"})
    /// * `context` - Current execution context for parameter resolution
    /// * `calculation_date` - Date for calculations
    fn resolve_external_input(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        calculation_date: &str,
    ) -> Result<Value>;
}

/// Metadata about a loaded law.
///
/// Provides summary information about a law without requiring full article access.
#[derive(Debug, Clone)]
pub struct LawInfo {
    /// Law identifier
    pub id: String,
    /// Regulatory layer
    pub regulatory_layer: RegulatoryLayer,
    /// Publication date
    pub publication_date: String,
    /// BWB identifier (for national laws)
    pub bwb_id: Option<String>,
    /// URL to official source
    pub url: Option<String>,
    /// List of output names produced by this law
    pub outputs: Vec<String>,
    /// Number of articles in the law
    pub article_count: usize,
}

/// State of a decision progressing through an AWB-defined procedure lifecycle (RFC-008).
///
/// The engine is stateless — this struct is passed in by the caller and returned
/// with updates. The orchestration layer persists it between stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageState {
    /// Which AWB procedure this decision follows (e.g., "beschikking")
    pub procedure_id: String,
    /// The law that initiated execution (for override scoping)
    pub contextual_law: String,
    /// Current lifecycle stage (e.g., "BESLUIT", "BEKENDMAKING")
    pub current_stage: String,
    /// Outputs accumulated from all completed stages
    pub accumulated_outputs: BTreeMap<String, Value>,
    /// Original parameters from the initial execution
    pub parameters: BTreeMap<String, Value>,
    /// What the completed stages reported beside their outputs, so the
    /// final result reports every stage, also across a yield. Boxed, so a
    /// yield stays small beside a completed result.
    #[serde(default)]
    pub report: Box<StageReport>,
}

/// The lists an [`ArticleResult`] reports beside its outputs, gathered
/// over the stages of a procedure ([`StageState::report`]). Each holds an
/// entry once, in the order the stages first reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageReport {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delegation_refusals: Vec<DelegationRefusal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declaration_version_notes: Vec<DeclarationsFromOtherVersion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declarations_not_in_force: Vec<DeclarationNotInForce>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_terms: Vec<OpenTermFilled>,
}

impl StageReport {
    /// Add what `res_ctx` collected for one stage that is not yet here.
    fn add_stage(&mut self, res_ctx: &ResolutionContext<'_>) {
        fn add<T: Clone + PartialEq>(to: &mut Vec<T>, from: &[T]) {
            for item in from {
                if !to.contains(item) {
                    to.push(item.clone());
                }
            }
        }
        add(&mut self.delegation_refusals, &res_ctx.delegation_refusals);
        add(
            &mut self.declaration_version_notes,
            &res_ctx.declaration_version_notes,
        );
        add(
            &mut self.declarations_not_in_force,
            &res_ctx.declarations_not_in_force,
        );
        add(&mut self.open_terms, &res_ctx.open_terms);
    }

    fn report_into(&self, result: &mut ArticleResult) {
        result
            .delegation_refusals
            .clone_from(&self.delegation_refusals);
        result
            .declaration_version_notes
            .clone_from(&self.declaration_version_notes);
        result
            .declarations_not_in_force
            .clone_from(&self.declarations_not_in_force);
        result.open_terms.clone_from(&self.open_terms);
    }
}

/// Outcome of a stage-aware execution step.
#[derive(Debug)]
pub enum ExecutionOutcome {
    /// All stages completed — final result
    Complete(Box<ArticleResult>),
    /// Execution yielded — waiting for external input to advance
    Yielded {
        /// Updated decision state
        state: StageState,
        /// Outputs computed so far (including this stage)
        outputs: BTreeMap<String, Value>,
        /// Inputs required to advance to the next stage
        pending_inputs: Vec<String>,
    },
}

/// The articles that take part when an article that establishes a
/// submission runs (RFC-045): "Leerplichtwet 13a says there is an application;
/// the Awb says what every application also holds". Executing the article
/// fires the hooks; this names them, so a runtime that composes the form or
/// the record of the submission takes them from the engine instead of
/// collecting them itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    /// The kind of submission (`produces.submission.kind`), such as `AANVRAAG`.
    pub kind: String,
    /// The establishing article first, then every hook on the submission in
    /// the order the engine fires them (`pre_actions`, then `post_actions`).
    pub articles: Vec<SubmissionArticle>,
    /// When the establishing article follows a procedure (RFC-008): the stage
    /// at which the hooks on the submission fire, named after its kind, such
    /// as the Awb stage AANVRAAG (RFC-047). Execute that stage with
    /// [`LawExecutionService::execute_stage_at`] to run them. `None` outside a
    /// procedure: every run of the article fires them.
    pub stage: Option<String>,
}

/// One stage of the procedure a decision article follows (RFC-008), as
/// [`LawExecutionService::execute_stage_at`] runs it: what the stage requires
/// to be entered, and the articles that take part. A runtime that takes the
/// decision reads here what it must supply (RFC-047).
#[derive(Debug, Clone, PartialEq)]
pub struct ProcedureStage {
    /// The procedure the article follows.
    pub procedure_id: String,
    /// The stage, as the procedure names it.
    pub stage: String,
    /// What the stage requires to be entered, such as the date of the
    /// decision.
    pub requires: Vec<StageRequirement>,
    /// The article first, then every hook that fires on it at this stage, in
    /// the order the engine fires them (`pre_actions`, then `post_actions`).
    pub articles: Vec<SubmissionArticle>,
}

/// An open term that a decision under an article may need
/// ([`LawExecutionService::decision_terms`]): one of that article, or of an
/// article that fills one of its terms (`implements`), with whether it is
/// judged per case, who answers it and what fills it. It says what the law
/// and the policy need for the decision, and from whom; how and when a
/// process asks it is the process's.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionTerm {
    /// The law and article that declare the term.
    pub law_id: String,
    pub article: String,
    pub id: String,
    pub term_type: crate::types::ParameterType,
    pub required: bool,
    /// Whether a default fills the term when nothing else does.
    pub has_default: bool,
    /// Whether the term is judged per case (`decided_per_case_by`): where
    /// no article fills it, it takes a finding. The text says in words who
    /// judges; the engine does not read it ([`Self::answered_by`]).
    pub decided_per_case_by: Option<String>,
    /// Who answers the term: the authorities its own `competent_authority`
    /// names; where the term has none, the one the article names (RFC-002);
    /// where the article has none either, the holder of the rulework (its
    /// root `competent_authority`). Empty when the first of these that is
    /// present is a reference (`#...`), or when none is present: then
    /// whoever executes the article answers it.
    pub answered_by: Vec<NamedAuthority>,
    /// The articles that fill the term (`implements`), as
    /// `<regulation>#<article>`, in force on the day. A term they fill takes
    /// their value, null included; no finding is asked for it (RFC-048 §2).
    pub filled_by: Vec<String>,
}

impl DecisionTerm {
    /// Whether the term takes a finding at all: it is judged per case and
    /// no article fills it. Who may give it is [`Self::is_finding`].
    pub fn asks_finding(&self) -> bool {
        self.decided_per_case_by.is_some() && self.filled_by.is_empty()
    }

    /// Whether `authority` may answer the term ([`Self::answered_by`]).
    pub fn answers(&self, authority: &Authority) -> bool {
        self.answered_by.is_empty() || authority.is_named_by(&self.answered_by)
    }

    /// Whether the term asks a finding of `authority`: it takes a finding
    /// ([`Self::asks_finding`]) and `authority` answers it
    /// ([`Self::answers`]).
    pub fn is_finding(&self, authority: &Authority) -> bool {
        self.asks_finding() && self.answers(authority)
    }

    /// The article that declares it, `<regulation>#<article>`.
    pub fn declared_by(&self) -> String {
        format!("{}#{}", self.law_id, self.article)
    }
}

/// An article that takes part in a submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionArticle {
    pub law_id: String,
    pub article_number: String,
    /// `None` for the article that establishes the submission, otherwise the
    /// point at which it fires as a hook.
    pub hook_point: Option<HookPoint>,
}

/// High-level service for executing laws with automatic cross-law resolution.
///
/// `LawExecutionService` wraps a `RuleResolver` and implements `ServiceProvider`
/// to enable automatic resolution of external references and open term implementations.
/// It also supports external data sources via `DataSourceRegistry`.
pub struct LawExecutionService {
    resolver: RuleResolver,
    /// Registry for external data sources. Queried during law execution
    /// to resolve inputs before falling back to cross-law resolution.
    /// Acts as an override layer: if a data source provides a field,
    /// it's used instead of triggering cross-law/IoC resolution.
    data_registry: DataSourceRegistry,
    /// Source provenance tracking: law_id → (source_id, source_name).
    source_info: HashMap<String, (String, String)>,
    /// How to handle articles with untranslatable constructs (RFC-012)
    untranslatable_mode: UntranslatableMode,
}

impl Default for LawExecutionService {
    fn default() -> Self {
        Self::new()
    }
}

impl LawExecutionService {
    /// Create a new empty service.
    pub fn new() -> Self {
        Self {
            resolver: RuleResolver::new(),
            data_registry: DataSourceRegistry::new(),
            source_info: HashMap::new(),
            untranslatable_mode: UntranslatableMode::default(),
        }
    }

    /// Set the untranslatable handling mode (RFC-012).
    pub fn set_untranslatable_mode(&mut self, mode: UntranslatableMode) {
        self.untranslatable_mode = mode;
    }

    /// Load a law from YAML string.
    ///
    /// # Returns
    /// The law ID on success.
    pub fn load_law(&mut self, yaml: &str) -> Result<String> {
        self.resolver.load_from_yaml(yaml)
    }

    /// Load a law struct directly.
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` if the maximum number of laws would be exceeded.
    pub fn load_law_struct(&mut self, law: ArticleBasedLaw) -> Result<()> {
        self.resolver.load_law(law)
    }

    /// Load a law without the static type check, for tests of the run-time
    /// boundary checks that a well-typed set cannot reach (RFC-036 defence in
    /// depth).
    #[cfg(test)]
    fn load_law_unchecked(&mut self, yaml: &str) -> Result<String> {
        self.resolver.load_law_unchecked(yaml)
    }

    /// Load a law from YAML string and record its source provenance.
    ///
    /// # Returns
    /// The law ID on success.
    pub fn load_law_with_source(
        &mut self,
        yaml: &str,
        source_id: &str,
        source_name: &str,
    ) -> Result<String> {
        let law_id = self.resolver.load_from_yaml(yaml)?;
        self.source_info.insert(
            law_id.clone(),
            (source_id.to_string(), source_name.to_string()),
        );
        Ok(law_id)
    }

    /// Get the source provenance for a loaded law.
    ///
    /// Returns `(source_id, source_name)` if the law was loaded via
    /// [`load_law_with_source`](Self::load_law_with_source).
    pub fn get_law_source(&self, law_id: &str) -> Option<(&str, &str)> {
        self.source_info
            .get(law_id)
            .map(|(id, name)| (id.as_str(), name.as_str()))
    }

    /// Build an Execution Receipt (RFC-013) from an ArticleResult.
    ///
    /// Wraps the result with provenance, engine config, and scope metadata,
    /// and records which outputs were explicitly requested.
    pub fn build_receipt_with_outputs(
        &self,
        result: &ArticleResult,
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
        requested_outputs: &[String],
    ) -> crate::receipt::ExecutionReceipt {
        use crate::receipt::*;

        let loaded_regulations: Vec<LoadedRegulation> = self
            .resolver
            .all_law_versions()
            .map(|law| LoadedRegulation {
                id: law.id.clone(),
                valid_from: law.valid_from.clone(),
                valid_to: law.valid_to.clone(),
                hash: law.content_hash.clone(),
            })
            .collect();

        let sources: Vec<ReceiptSource> = self
            .source_info
            .values()
            .map(|(id, name)| ReceiptSource {
                id: id.clone(),
                name: Some(name.clone()),
            })
            .collect();

        ExecutionReceipt {
            provenance: ReceiptProvenance {
                engine: "regelrecht".to_string(),
                engine_version: result.engine_version.clone(),
                schema_version: result.schema_version.clone(),
                regulation_id: result.law_id.clone(),
                regulation_valid_from: result.regulation_valid_from.clone(),
                regulation_hash: result.regulation_hash.clone(),
            },
            engine_config: EngineConfig {
                connectivity: Connectivity::Solo,
                legal_status: LegalStatus::Simulation,
                untranslatable_mode: self.untranslatable_mode,
                identity: None,
            },
            scope: ReceiptScope {
                sources,
                loaded_regulations,
                scopes: Vec::new(),
            },
            execution: ReceiptExecution {
                calculation_date: calculation_date.to_string(),
                parameters: parameters.clone(),
                reference_date: None,
            },
            results: ReceiptResults {
                requested_outputs: requested_outputs.to_vec(),
                outputs: result.outputs.clone(),
                output_provenance: result.output_provenance.clone(),
                trace: result.trace.clone(),
                delegation_refusals: result.delegation_refusals.clone(),
                declaration_version_notes: result.declaration_version_notes.clone(),
                declarations_not_in_force: result.declarations_not_in_force.clone(),
                open_terms: result.open_terms.clone(),
            },
            accepted_values: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    // =========================================================================
    // Multi-output evaluation API
    // =========================================================================

    /// Evaluate multiple specific outputs from a law.
    ///
    /// This is the preferred entry point for law execution. Callers must
    /// explicitly list which outputs they need (privacy-by-design: no
    /// "return all" mode). If multiple outputs come from the same article,
    /// the article is executed only once.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier (e.g., "wet_op_de_zorgtoeslag")
    /// * `output_names` - The outputs to calculate (must be non-empty)
    /// * `parameters` - Input parameters
    /// * `calculation_date` - Date for calculations (YYYY-MM-DD)
    ///
    /// # Returns
    /// The execution result filtered to only the requested outputs.
    pub fn evaluate_law(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_with_findings(
            law_id,
            output_names,
            parameters,
            BTreeMap::new(),
            calculation_date,
        )
    }

    /// [`Self::evaluate_law`] with `findings`: what an authority found for
    /// this case, keyed by the id of an open term it decides per case
    /// (`decided_per_case_by`), apart from the parameters (RFC-050). A
    /// finding applies to every article in the execution that leaves that
    /// term to an authority per case, also one called anew through
    /// `source.article`, where it wins over the decision value's own; an
    /// implementation of the term still wins over it. A finding that no
    /// article in the execution asks, or that does not fit the type of its
    /// term, is an error that names it.
    pub fn evaluate_law_with_findings(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        findings: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_in(
            law_id,
            output_names,
            parameters,
            Call::with_findings(findings),
            calculation_date,
        )
    }

    /// [`Self::evaluate_law_with_findings`] with what `call` gives: the
    /// findings, and the cells the engine asks for the answers a binding
    /// takes (`source.reduction`, RFC-048), also in an article the execution
    /// calls.
    pub fn evaluate_law_in<'a>(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        call: Call<'a>,
        calculation_date: &'a str,
    ) -> Result<ArticleResult> {
        if output_names.is_empty() {
            return Err(EngineError::InvalidOperation(
                "output_names must not be empty".to_string(),
            ));
        }
        let mut res_ctx = ResolutionContext::new(calculation_date)?;
        res_ctx.contextual_law_id = Some(law_id.to_string());
        res_ctx.take_call(call);
        let result =
            self.evaluate_law_multi_internal(law_id, output_names, parameters, &mut res_ctx)?;
        res_ctx.check_findings_asked()?;
        // An output the law says does not arise is an answer, and the caller
        // asked for exactly this one. Returning `Ok` with it quietly missing
        // leaves a consumer to discover an absence it has no ground for; the
        // error carries the words of the article that excluded it.
        for name in output_names {
            if !result.outputs.contains_key(*name) {
                if let Some(error) = voided_output_error(law_id, name, &result.output_provenance) {
                    return Err(error);
                }
            }
        }
        Ok(result)
    }

    /// Evaluate multiple outputs with tracing enabled.
    pub fn evaluate_law_with_trace(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_with_trace_builder(
            law_id,
            output_names,
            parameters,
            calculation_date,
            TraceBuilder::new(),
        )
    }

    /// Evaluate multiple outputs with a caller-provided trace builder.
    pub fn evaluate_law_with_trace_builder(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace_builder: TraceBuilder,
    ) -> Result<ArticleResult> {
        self.evaluate_law_traced(
            law_id,
            output_names,
            parameters,
            Call::default(),
            calculation_date,
            trace_builder,
        )
    }

    /// [`Self::evaluate_law_with_findings`] with tracing enabled.
    pub fn evaluate_law_with_trace_and_findings(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        findings: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_with_trace_in(
            law_id,
            output_names,
            parameters,
            Call::with_findings(findings),
            calculation_date,
        )
    }

    /// [`Self::evaluate_law_in`] with tracing enabled. An answer a binding
    /// took is one node, with who answered and from what.
    pub fn evaluate_law_with_trace_in<'a>(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        call: Call<'a>,
        calculation_date: &'a str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_traced(
            law_id,
            output_names,
            parameters,
            call,
            calculation_date,
            TraceBuilder::new(),
        )
    }

    fn evaluate_law_traced<'a>(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        call: Call<'a>,
        calculation_date: &'a str,
        trace_builder: TraceBuilder,
    ) -> Result<ArticleResult> {
        if output_names.is_empty() {
            return Err(EngineError::InvalidOperation(
                "output_names must not be empty".to_string(),
            ));
        }

        let outputs_label = output_names.join(", ");
        let trace = Rc::new(RefCell::new(trace_builder));

        // Push the top-level article node
        {
            let mut tb = trace.borrow_mut();
            tb.push(
                format!("{} ({})", law_id, outputs_label),
                PathNodeType::Article,
            );
            let mut sorted_params: Vec<_> = parameters
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            sorted_params.sort();
            tb.set_message(format!(
                "{} ({} {{{}}} {})",
                law_id,
                calculation_date,
                sorted_params.join(", "),
                outputs_label,
            ));
        }

        let mut res_ctx = ResolutionContext::with_trace(calculation_date, Rc::clone(&trace))?;
        res_ctx.contextual_law_id = Some(law_id.to_string());
        res_ctx.take_call(call);

        // Execute: group outputs by article, evaluate each once, merge + filter
        let result = self
            .evaluate_law_multi_internal(law_id, output_names, parameters, &mut res_ctx)
            .and_then(|result| res_ctx.check_findings_asked().map(|()| result));

        // Drop res_ctx so it releases its Rc clone
        drop(res_ctx);

        match result {
            Ok(mut result) => {
                // A requested output the law says does not arise is an error
                // here too. This is the path the editor, the demo, the TUI and
                // the BDD harness take, so leaving it out would keep exactly
                // the silence this error exists to remove, on the route humans
                // actually use: a green run with an empty result panel and no
                // ground. The trace is finished first, so the failure carries
                // the path that produced it.
                let voided = output_names.iter().find_map(|name| {
                    (!result.outputs.contains_key(*name))
                        .then(|| voided_output_error(law_id, name, &result.output_provenance))
                        .flatten()
                });

                // Set the result on the top-level article node
                let mut tb = trace.borrow_mut();
                if output_names.len() == 1 {
                    if let Some(value) = result.outputs.get(output_names[0]) {
                        tb.set_result(value.clone());
                    }
                } else {
                    // Multiple outputs: the requested ones, as with a single
                    // output. A hook's outputs have their own lines already.
                    let obj: BTreeMap<String, Value> = output_names
                        .iter()
                        .filter_map(|name| {
                            let value = result.outputs.get(*name)?;
                            Some((name.to_string(), value.clone()))
                        })
                        .collect();
                    tb.set_result(Value::Object(obj));
                }
                if let Some(error) = voided {
                    tb.set_message(error.to_string());
                    let partial_trace = tb.pop();
                    return Err(EngineError::TracedError {
                        source: Box::new(error),
                        trace: partial_trace.map(Box::new),
                    });
                }
                result.trace = tb.pop();
                Ok(result)
            }
            Err(e) => {
                let mut tb = trace.borrow_mut();
                tb.set_message(format!("Execution failed: {}", e));
                let partial_trace = tb.pop();
                Err(EngineError::TracedError {
                    source: Box::new(e),
                    trace: partial_trace.map(Box::new),
                })
            }
        }
    }

    // =========================================================================
    // Single-output convenience wrappers (delegate to multi-output API)
    // =========================================================================

    /// Execute a law for a single output by name.
    ///
    /// Convenience wrapper around [`evaluate_law`](Self::evaluate_law).
    /// Returns the requested output plus any causally-entailed outputs (hooks, overrides).
    pub fn evaluate_law_output(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law(law_id, &[output_name], parameters, calculation_date)
    }

    /// Execute a single law output with tracing enabled.
    pub fn evaluate_law_output_with_trace(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_law_with_trace(law_id, &[output_name], parameters, calculation_date)
    }

    /// Execute a single law output with a caller-provided trace builder.
    pub fn evaluate_law_output_with_trace_builder(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace_builder: TraceBuilder,
    ) -> Result<ArticleResult> {
        self.evaluate_law_with_trace_builder(
            law_id,
            &[output_name],
            parameters,
            calculation_date,
            trace_builder,
        )
    }

    /// Execute a law output using an existing shared trace builder.
    ///
    /// Unlike `evaluate_law_output_with_trace` which creates its own root trace node,
    /// this method appends to an existing trace tree. Used by `execute_stage_internal`
    /// when falling through to non-procedure execution.
    fn evaluate_law_output_with_shared_trace(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: Rc<RefCell<TraceBuilder>>,
    ) -> Result<ArticleResult> {
        let mut res_ctx = ResolutionContext::with_trace(calculation_date, trace)?;
        res_ctx.contextual_law_id = Some(law_id.to_string());
        let mut result =
            self.evaluate_law_output_internal(law_id, output_name, parameters, &mut res_ctx)?;
        res_ctx.report_into(&mut result);
        Ok(result)
    }

    /// Execute a single lifecycle stage of a procedure-aware law (RFC-008).
    ///
    /// For laws that produce a `legal_character` with an associated AWB procedure,
    /// this method executes one stage at a time and yields when external input is needed.
    ///
    /// # Arguments
    /// * `law_id` - The law being executed (e.g., "vreemdelingenwet_2000")
    /// * `output_name` - The output to calculate
    /// * `state` - Current decision state (None for initial execution)
    /// * `parameters` - Input parameters (merged with accumulated outputs if resuming)
    /// * `calculation_date` - Date for calculations
    ///
    /// # Returns
    /// `ExecutionOutcome::Complete` if all stages are done, or
    /// `ExecutionOutcome::Yielded` if waiting for external input.
    pub fn execute_stage(
        &self,
        law_id: &str,
        output_name: &str,
        state: Option<StageState>,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ExecutionOutcome> {
        self.execute_stage_internal(
            law_id,
            output_name,
            state,
            parameters,
            calculation_date,
            None,
        )
    }

    /// The decision the submission of article `article_number` of `law_id`
    /// asks for (Awb 1:3 lid 3), as `<regulation>#<article>`, in the version
    /// in force on `calculation_date`: the article its list item names with
    /// `requests` (RFC-047), or the article itself in the object form when
    /// that also carries a `legal_character`. `None` when the article
    /// establishes no submission or its submission names no decision. An
    /// error when the requested article is not loaded, has no version in
    /// force on that date, or takes no decision: the loader can check a
    /// target in another law only once that law is there, so this is where
    /// a missing one shows.
    pub fn requested_decision(
        &self,
        law_id: &str,
        article_number: &str,
        calculation_date: &str,
    ) -> Result<Option<String>> {
        let (_, article) = self.article_in_force(law_id, article_number, calculation_date)?;
        let Some(submission) = article.get_submission() else {
            return Ok(None);
        };
        let target = match submission.requests() {
            Some(target) => target.to_string(),
            None if !article.produces_list() && article.get_decision().is_some() => {
                return Ok(Some(format!("{law_id}#{article_number}")));
            }
            None => return Ok(None),
        };
        let (regulation, number) = target.split_once('#').ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "{law_id}#{article_number} requests '{target}', which is no article reference"
            ))
        })?;
        let (_, requested) = self
            .article_in_force(regulation, number, calculation_date)
            .map_err(|e| {
                EngineError::ResolutionError(format!(
                    "{law_id}#{article_number} requests the decision of {target}: {e}"
                ))
            })?;
        if requested.get_decision().is_none() {
            return Err(EngineError::ResolutionError(format!(
                "{law_id}#{article_number} requests the decision of {target}, which takes no \
                 decision (no legal_character)"
            )));
        }
        Ok(Some(target))
    }

    /// The articles whose submission asks for the decision of article
    /// `article_number` of `law_id` ([`Self::requested_decision`]), as
    /// `<regulation>#<article>`, among the laws in force on
    /// `calculation_date`: those of the deciding law first, then the others
    /// in the order of their law ids.
    pub fn applications_for(
        &self,
        law_id: &str,
        article_number: &str,
        calculation_date: &str,
    ) -> Result<Vec<String>> {
        let ref_date = Some(parse_calculation_date(calculation_date)?);
        let target = format!("{law_id}#{article_number}");
        let mut laws: Vec<&str> = self.resolver.list_laws();
        laws.sort_unstable_by_key(|id| (*id != law_id, *id));
        let mut found = Vec::new();
        for id in laws {
            let Some(law) = self.resolver.get_law_for_date(id, ref_date) else {
                continue;
            };
            for article in &law.articles {
                let Some(submission) = article.get_submission() else {
                    continue;
                };
                let asks = match submission.requests() {
                    Some(requests) => requests == target,
                    None => {
                        id == law_id
                            && article.number == article_number
                            && !article.produces_list()
                            && article.get_decision().is_some()
                    }
                };
                if asks {
                    found.push(format!("{id}#{}", article.number));
                }
            }
        }
        Ok(found)
    }

    /// What executing article `article_number` of `law_id` involves when it
    /// establishes a submission (RFC-045), in the version in force on
    /// `calculation_date`: the article and the hooks on the submission, by the
    /// rule [`Self::fire_hooks`] fires them. Executing the article, through
    /// [`Self::evaluate_law_output`] or [`Self::execute_stage`], runs them and
    /// puts their outputs in the result. `None` when the article establishes
    /// no submission. A hook whose law or article has no version in force on
    /// that date does not fire, so it is left out here too (an execution
    /// records that skip on its trace and receipt).
    pub fn submission(
        &self,
        law_id: &str,
        article_number: &str,
        calculation_date: &str,
    ) -> Result<Option<Submission>> {
        let ref_date = Some(parse_calculation_date(calculation_date)?);
        let (law, article) = self.article_in_force(law_id, article_number, calculation_date)?;
        let Some(kind) = article
            .get_submission()
            .and_then(|p| p.submission_kind())
            .map(str::to_string)
        else {
            return Ok(None);
        };
        // In a procedure the hooks wait for the stage named after the kind;
        // a procedure without that stage would never fire them.
        let stage = match self.procedure_of(law_id, article)? {
            None => None,
            Some(procedure) if procedure.stages.iter().any(|s| s.name == kind) => {
                Some(kind.clone())
            }
            Some(procedure) => {
                return Err(EngineError::InvalidOperation(format!(
                    "{law_id} article {article_number} establishes an {kind} and follows \
                     procedure '{}', which has no stage {kind} for the hooks on it to fire at",
                    procedure.id
                )))
            }
        };
        let articles = self.taking_part(law, article, ref_date, |hook_point| {
            self.resolver
                .find_submission_hooks(hook_point, &kind, law_id, article_number)
        });
        Ok(Some(Submission {
            kind,
            articles,
            stage,
        }))
    }

    /// What executing article `article_number` of `law_id` at stage
    /// `stage_name` of its procedure involves, in the version in force on
    /// `calculation_date`: what the stage requires, the article, and the
    /// hooks that fire on it at that stage. What
    /// [`Self::execute_stage_at`] runs, before running it.
    ///
    /// Fails when the article follows no procedure or its procedure has no
    /// such stage.
    pub fn procedure_stage(
        &self,
        law_id: &str,
        article_number: &str,
        stage_name: &str,
        calculation_date: &str,
    ) -> Result<ProcedureStage> {
        let ref_date = Some(parse_calculation_date(calculation_date)?);
        let (law, article) = self.article_in_force(law_id, article_number, calculation_date)?;
        let (procedure, stage) = self.stage_of(law_id, article, stage_name)?;
        let articles = self.taking_part(law, article, ref_date, |hook_point| {
            self.hooks_firing_on(hook_point, article, law, &stage.name, true)
        });
        Ok(ProcedureStage {
            procedure_id: procedure.id.clone(),
            stage: stage.name.clone(),
            requires: stage.requires.clone().unwrap_or_default(),
            articles,
        })
    }

    /// The open terms a decision under article `article_number` of `law_id`
    /// may need, in the laws in force on `calculation_date`: those of the
    /// article, then for each that an article fills (`implements`) the
    /// open terms of that article, and so on, each once, in that order. The
    /// policy that fills a term may leave a case to the authority with an
    /// open term of its own (`decided_per_case_by`, RFC-048 §2); this is
    /// where it shows. Whether a case reaches such a term only an execution
    /// says. The implementations are those that apply in `scope`, as in an
    /// execution with those parameters: a regulation scoped by
    /// `gemeente_code` fills a term only for a scope with that code.
    pub fn decision_terms(
        &self,
        law_id: &str,
        article_number: &str,
        calculation_date: &str,
        scope: &HashMap<String, Value>,
    ) -> Result<Vec<DecisionTerm>> {
        let ref_date = Some(parse_calculation_date(calculation_date)?);
        let mut terms: Vec<DecisionTerm> = Vec::new();
        let mut todo = vec![(law_id.to_string(), article_number.to_string())];
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        while let Some((law_id, number)) = todo.pop() {
            if !seen.insert((law_id.clone(), number.clone())) {
                continue;
            }
            let (law, article) = self.article_in_force(&law_id, &number, calculation_date)?;
            // Who executes the article: the authority it names, else the
            // rulework's holder. A reference (`#...`) is decided per case,
            // so it names no one here: whoever executes answers.
            let executes: Vec<NamedAuthority> = article
                .machine_readable
                .as_ref()
                .and_then(|mr| mr.competent_authority.as_ref())
                .or(law.competent_authority.as_ref())
                .and_then(CompetentAuthority::named)
                .map(<[_]>::to_vec)
                .unwrap_or_default();
            let mut found = Vec::new();
            for term in article.get_open_terms().into_iter().flatten() {
                let lookup = self
                    .resolver
                    .find_implementations(&law_id, &number, term, ref_date, scope)?;
                let filled_by: Vec<String> = lookup
                    .implementations
                    .iter()
                    .map(|(l, a)| format!("{}#{}", l.id, a.number))
                    .collect();
                for (l, a) in &lookup.implementations {
                    found.push((l.id.clone(), a.number.clone()));
                }
                // A reference on the term names no one here either.
                let answered_by = match &term.competent_authority {
                    Some(own) => own.named().map(<[_]>::to_vec).unwrap_or_default(),
                    None => executes.clone(),
                };
                terms.push(DecisionTerm {
                    law_id: law_id.clone(),
                    article: number.clone(),
                    id: term.id.clone(),
                    term_type: term.term_type,
                    required: term.required,
                    has_default: term.has_default(),
                    decided_per_case_by: term.decided_per_case_by.clone(),
                    answered_by,
                    filled_by,
                });
            }
            // Depth first, in the order of the terms.
            todo.extend(found.into_iter().rev());
        }
        Ok(terms)
    }

    /// Execute one stage of the procedure that article `article_number` of
    /// `law_id` follows, in the version in force on `calculation_date`, on a
    /// fresh state (RFC-008, RFC-047): the article, with the hooks that fire
    /// at that stage, and nothing carried over from an earlier stage.
    ///
    /// Where [`Self::execute_stage`] walks the procedure and carries what each
    /// stage produced into the next, this runs exactly the stage named. It is
    /// for a caller that keeps its own record of each step, such as a cell
    /// that records an application at the stage AANVRAAG and later takes the
    /// decision at the stage BESLUIT.
    ///
    /// `outputs` are the outputs of the article to compute, as for
    /// [`Self::evaluate_law`], and an empty list none, so only the hooks run.
    /// `None` computes what the stage is about: every output, or for an
    /// article that produces several things the outputs of the submission
    /// the stage is named after, and at any other stage those of the
    /// decision (RFC-047). Fails when the article follows no
    /// procedure, its procedure has no stage `stage_name`, or a value the
    /// stage requires is not among `parameters`.
    pub fn execute_stage_at(
        &self,
        law_id: &str,
        article_number: &str,
        stage_name: &str,
        outputs: Option<&[&str]>,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.execute_stage_at_with_findings(
            law_id,
            article_number,
            stage_name,
            outputs,
            parameters,
            BTreeMap::new(),
            calculation_date,
        )
    }

    /// [`Self::execute_stage_at`] with `findings`, as for
    /// [`Self::evaluate_law_with_findings`] (RFC-050).
    #[allow(clippy::too_many_arguments)]
    pub fn execute_stage_at_with_findings(
        &self,
        law_id: &str,
        article_number: &str,
        stage_name: &str,
        outputs: Option<&[&str]>,
        parameters: BTreeMap<String, Value>,
        findings: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.execute_stage_in(
            law_id,
            article_number,
            stage_name,
            outputs,
            parameters,
            Call::with_findings(findings),
            calculation_date,
        )
    }

    /// [`Self::execute_stage_at`] with what `call` gives, as for
    /// [`Self::evaluate_law_in`].
    #[allow(clippy::too_many_arguments)]
    pub fn execute_stage_in<'a>(
        &self,
        law_id: &str,
        article_number: &str,
        stage_name: &str,
        outputs: Option<&[&str]>,
        parameters: BTreeMap<String, Value>,
        call: Call<'a>,
        calculation_date: &'a str,
    ) -> Result<ArticleResult> {
        let (law, article) = self.article_in_force(law_id, article_number, calculation_date)?;
        let (procedure, stage) = self.stage_of(law_id, article, stage_name)?;
        let missing: Vec<&str> = stage
            .requires
            .iter()
            .flatten()
            .map(|req| req.name.as_str())
            .filter(|name| !parameters.contains_key(*name))
            .collect();
        if !missing.is_empty() {
            return Err(EngineError::InvalidOperation(format!(
                "stage '{stage_name}' of procedure '{}' requires {}",
                procedure.id,
                missing.join(", ")
            )));
        }
        let mut res_ctx = ResolutionContext::new(calculation_date)?;
        res_ctx.contextual_law_id = Some(law_id.to_string());
        res_ctx.staged_article = Some((law.id.clone(), article.number.clone()));
        res_ctx.take_call(call);
        self.note_declaration_versions(&mut res_ctx);
        let outputs = outputs
            .map(<[&str]>::to_vec)
            .or_else(|| outputs_at_stage(article, &stage.name));
        let mut result = self.evaluate_article_with_service(
            article,
            law,
            parameters,
            outputs.as_deref(),
            &stage.name,
            &mut res_ctx,
        )?;
        res_ctx.check_findings_asked()?;
        res_ctx.report_into(&mut result);
        Ok(result)
    }

    /// Article `article_number` of `law_id` in the version in force on
    /// `calculation_date`, with that version.
    fn article_in_force(
        &self,
        law_id: &str,
        article_number: &str,
        calculation_date: &str,
    ) -> Result<(&ArticleBasedLaw, &Article)> {
        let ref_date = Some(parse_calculation_date(calculation_date)?);
        let law = self
            .resolver
            .get_law_for_date_reported(law_id, ref_date)
            .map_err(|reason| selection_error(law_id, calculation_date, reason))?;
        let article = law.find_article_by_number(article_number).ok_or_else(|| {
            EngineError::ResolutionError(format!(
                "the version of {law_id} in force on {calculation_date} has no article {article_number}"
            ))
        })?;
        Ok((law, article))
    }

    /// The article first, then the hooks `hooks` names per hook point
    /// (`pre_actions`, then `post_actions`) that have a version in force on
    /// `ref_date`. A hook not in force does not fire, so it is left out here
    /// too (an execution records that skip on its trace and receipt).
    fn taking_part<'s>(
        &'s self,
        law: &ArticleBasedLaw,
        article: &Article,
        ref_date: Option<NaiveDate>,
        hooks: impl Fn(HookPoint) -> Vec<&'s HookEntry>,
    ) -> Vec<SubmissionArticle> {
        let mut articles = vec![SubmissionArticle {
            law_id: law.id.clone(),
            article_number: article.number.clone(),
            hook_point: None,
        }];
        for hook_point in [HookPoint::PreActions, HookPoint::PostActions] {
            for h in hooks(hook_point) {
                let in_force = self
                    .resolver
                    .get_law_for_date(&h.law_id, ref_date)
                    .is_some_and(|l| l.find_article_by_number(&h.article_number).is_some());
                if in_force {
                    articles.push(SubmissionArticle {
                        law_id: h.law_id.clone(),
                        article_number: h.article_number.clone(),
                        hook_point: Some(hook_point),
                    });
                }
            }
        }
        articles
    }

    /// The procedure that the decision `article` of `law_id` produces
    /// follows (RFC-008), or `None` when it has none.
    ///
    /// An article that produces nothing with a legal character has no
    /// lifecycle to begin with; beyond that, only "this legal character has no
    /// procedure in the corpus" may fall through to single-stage execution. A
    /// procedure that was asked for by name and not found must not: dropping
    /// it would drop the stages it imposes — the hearing, the notification,
    /// the objection period — and the decision would look complete while the
    /// person it is about never got what the procedure owes them.
    pub fn procedure_of(
        &self,
        law_id: &str,
        article: &Article,
    ) -> Result<Option<&ProcedureDefinition>> {
        let produces = article.get_decision();
        let procedure_id = produces.and_then(|p| p.procedure_id());
        let Some(lc) = produces.and_then(|p| p.legal_character()) else {
            return Ok(None);
        };
        match self.resolver.find_procedure_reported(lc, procedure_id) {
            Ok(def) => Ok(Some(def)),
            Err(ProcedureMiss::NoneForCharacter) => Ok(None),
            Err(ProcedureMiss::NamedNotFound(id)) => Err(EngineError::ResolutionError(format!(
                "{law_id} article {} asks for procedure '{id}' for legal character \
                 '{lc}', which no loaded law defines. Executing without it would drop \
                 the stages that procedure imposes.",
                article.number
            ))),
            Err(ProcedureMiss::DefaultDangling(id)) => Err(EngineError::ResolutionError(format!(
                "legal character '{lc}' has '{id}' registered as its default procedure, \
                 but no definition of '{id}' is loaded"
            ))),
        }
    }

    /// The procedure `article` of `law_id` follows and its stage
    /// `stage_name`. Fails when the article follows no procedure or the
    /// procedure has no such stage.
    fn stage_of(
        &self,
        law_id: &str,
        article: &Article,
        stage_name: &str,
    ) -> Result<(&ProcedureDefinition, &Stage)> {
        let procedure = self.procedure_of(law_id, article)?.ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "{law_id} article {} follows no procedure, so it has no stage '{stage_name}'",
                article.number
            ))
        })?;
        let stage = procedure
            .stages
            .iter()
            .find(|s| s.name == stage_name)
            .ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "Stage '{stage_name}' not found in procedure '{}'",
                    procedure.id
                ))
            })?;
        Ok((procedure, stage))
    }

    /// Internal stage execution with optional tracing.
    fn execute_stage_internal(
        &self,
        law_id: &str,
        output_name: &str,
        state: Option<StageState>,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: Option<Rc<RefCell<TraceBuilder>>>,
    ) -> Result<ExecutionOutcome> {
        // Look up the law and article
        let ref_date = Some(parse_calculation_date(calculation_date)?);
        let law = self
            .resolver
            .get_law_for_date_reported(law_id, ref_date)
            .map_err(|reason| selection_error(law_id, calculation_date, reason))?;
        let article = self
            .resolver
            .get_article_by_output(law_id, output_name, ref_date)
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: law_id.to_string(),
                output: output_name.to_string(),
            })?;

        // If no procedure, fall through to normal single-stage execution
        let Some(procedure) = self.procedure_of(law_id, article)? else {
            let result = if let Some(tb) = trace {
                self.evaluate_law_output_with_shared_trace(
                    law_id,
                    output_name,
                    parameters,
                    calculation_date,
                    tb,
                )?
            } else {
                self.evaluate_law_output(law_id, output_name, parameters, calculation_date)?
            };
            return Ok(ExecutionOutcome::Complete(Box::new(result)));
        };

        // Determine current stage
        let mut stage_state = match state {
            Some(s) => s,
            None => {
                // Initial execution: start at the first stage
                let first_stage = procedure.stages.first().ok_or_else(|| {
                    EngineError::InvalidOperation("Procedure has no stages".to_string())
                })?;

                StageState {
                    procedure_id: procedure.id.clone(),
                    contextual_law: law_id.to_string(),
                    current_stage: first_stage.name.clone(),
                    accumulated_outputs: BTreeMap::new(),
                    parameters: parameters.clone(),
                    report: Box::default(),
                }
            }
        };

        // Find current stage in procedure
        let stage_idx = procedure
            .stages
            .iter()
            .position(|s| s.name == stage_state.current_stage)
            .ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "Stage '{}' not found in procedure '{}'",
                    stage_state.current_stage, procedure.id
                ))
            })?;

        let stage = &procedure.stages[stage_idx];

        // Check if required inputs for this stage are present
        if let Some(requires) = &stage.requires {
            let mut missing = Vec::new();
            for req in requires {
                if !parameters.contains_key(&req.name)
                    && !stage_state.accumulated_outputs.contains_key(&req.name)
                {
                    missing.push(req.name.clone());
                }
            }
            if !missing.is_empty() {
                let outputs = stage_state.accumulated_outputs.clone();
                return Ok(ExecutionOutcome::Yielded {
                    state: stage_state,
                    outputs,
                    pending_inputs: missing,
                });
            }
        }

        // Merge accumulated outputs + new parameters for this stage's execution
        let mut stage_params = stage_state.parameters.clone();
        for (k, v) in &stage_state.accumulated_outputs {
            stage_params.insert(k.clone(), v.clone());
        }
        for (k, v) in &parameters {
            stage_params.insert(k.clone(), v.clone());
        }

        // Execute the article with stage-aware hook firing
        let mut res_ctx = if let Some(ref tb) = trace {
            ResolutionContext::with_trace(calculation_date, Rc::clone(tb))?
        } else {
            ResolutionContext::new(calculation_date)?
        };
        res_ctx.contextual_law_id = Some(stage_state.contextual_law.clone());
        res_ctx.staged_article = Some((law.id.clone(), article.number.clone()));
        self.note_declaration_versions(&mut res_ctx);

        // A stage accumulates the outputs it computes for the stages after
        // it: every output, or for an article that produces several things
        // those of the thing this stage is about (RFC-047).
        let stage_outputs = outputs_at_stage(article, &stage.name);

        // Execute the article with stage-aware hook firing.
        let result = self.evaluate_article_with_service(
            article,
            law,
            stage_params,
            stage_outputs.as_deref(),
            &stage.name,
            &mut res_ctx,
        )?;

        // Merge outputs into accumulated state
        for (k, v) in &result.outputs {
            stage_state.accumulated_outputs.insert(k.clone(), v.clone());
        }
        stage_state.report.add_stage(&res_ctx);

        // Advance to next stage
        if stage_idx + 1 < procedure.stages.len() {
            let next_stage = &procedure.stages[stage_idx + 1];
            stage_state.current_stage = next_stage.name.clone();

            // Check if next stage needs external inputs
            if let Some(requires) = &next_stage.requires {
                let mut missing = Vec::new();
                for req in requires {
                    if !stage_state.accumulated_outputs.contains_key(&req.name)
                        && !stage_state.parameters.contains_key(&req.name)
                        && !parameters.contains_key(&req.name)
                    {
                        missing.push(req.name.clone());
                    }
                }

                if !missing.is_empty() {
                    return Ok(ExecutionOutcome::Yielded {
                        outputs: stage_state.accumulated_outputs.clone(),
                        state: stage_state,
                        pending_inputs: missing,
                    });
                }
            }

            // Next stage has all inputs — continue executing (recursive)
            return self.execute_stage_internal(
                law_id,
                output_name,
                Some(stage_state),
                parameters,
                calculation_date,
                trace,
            );
        }

        // All stages complete
        let mut final_result = result;
        final_result.outputs = stage_state.accumulated_outputs;
        stage_state.report.report_into(&mut final_result);
        Ok(ExecutionOutcome::Complete(Box::new(final_result)))
    }

    /// Record, once per execution, which laws had their hooks, overrides and
    /// procedures answered from a version other than the one in force.
    ///
    /// The three indexes carry only the newest version of each law and their
    /// lookups take no reference date. For a calculation on an older date that
    /// makes "no hook, no override, no procedure" an answer about the wrong
    /// version. The engine cannot silently pass that off as a finding, so it
    /// says so: loudly in the log, in the trace, and on the receipt. Whoever
    /// reads a decision that hinges on an absent motiveringsplicht can see that
    /// its absence was not established.
    fn note_declaration_versions(&self, res_ctx: &mut ResolutionContext<'_>) {
        if !res_ctx.declaration_version_notes.is_empty() {
            return; // already recorded for this execution
        }
        let notes = self
            .resolver
            .declarations_from_another_version(res_ctx.reference_date());
        for note in &notes {
            tracing::warn!(
                law_id = %note.law_id,
                indexed_version = ?note.indexed_version,
                in_force_version = ?note.in_force_version,
                "Hooks, overrides and procedures answered from a version that is not in force"
            );
            let message = note.message();
            let _guard = res_ctx.trace_guard(&note.law_id, PathNodeType::CrossLawReference);
            res_ctx.trace_set_message(message);
        }
        res_ctx.declaration_version_notes = notes;
    }

    /// Internal method for multi-output evaluation.
    ///
    /// Groups requested outputs by producing article, executes each article
    /// once, merges results, and filters to only the requested outputs
    /// (privacy-by-design).
    fn evaluate_law_multi_internal(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        // Validate that the law exists and is in force on the calculation date
        let _law = self
            .resolver
            .get_law_for_date_reported(law_id, res_ctx.reference_date())
            .map_err(|reason| selection_error(law_id, res_ctx.calculation_date, reason))?;

        self.note_declaration_versions(res_ctx);

        // Group outputs by their producing article number to avoid redundant evaluations
        let mut article_to_outputs: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for &output_name in output_names {
            let article = self
                .resolver
                .get_article_by_output(law_id, output_name, res_ctx.reference_date())
                .ok_or_else(|| EngineError::OutputNotFound {
                    law_id: law_id.to_string(),
                    output: output_name.to_string(),
                })?;
            article_to_outputs
                .entry(article.number.clone())
                .or_default()
                .push(output_name);
        }

        // Collect the article numbers for the merged result
        let article_numbers: Vec<&str> = article_to_outputs.keys().map(|s| s.as_str()).collect();

        // Execute each unique article once and merge results
        let mut merged_result: Option<ArticleResult> = None;

        for outputs in article_to_outputs.values() {
            let result =
                self.evaluate_outputs_internal(law_id, outputs, parameters.clone(), res_ctx)?;

            match &mut merged_result {
                None => {
                    merged_result = Some(result);
                }
                Some(merged) => {
                    // Merge outputs, provenance, and resolved_inputs from additional articles
                    merged.outputs.extend(result.outputs);
                    merged.output_provenance.extend(result.output_provenance);
                    merged.resolved_inputs.extend(result.resolved_inputs);
                }
            }
        }

        let mut result = merged_result.ok_or_else(|| {
            EngineError::InvalidOperation("output_names must not be empty".to_string())
        })?;

        // When outputs came from multiple articles, set article_number to the
        // comma-separated list so callers don't see a misleading single number.
        if article_numbers.len() > 1 {
            result.article_number = article_numbers.join(", ");
        }

        // The engine executes only the articles that produce the requested
        // outputs, and in each only the actions those outputs depend on
        // (RFC-043). Causally-entailed outputs (hooks, overrides) stay in.

        // Every refusal seen anywhere in this execution chain, not just in the
        // article that happened to be merged first.
        res_ctx.report_into(&mut result);

        Ok(result)
    }

    /// Internal method with cycle tracking (single-output).
    fn evaluate_law_output_internal(
        &self,
        law_id: &str,
        output_name: &str,
        parameters: BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        self.evaluate_outputs_internal(
            law_id,
            std::slice::from_ref(&output_name),
            parameters,
            res_ctx,
        )
    }

    /// Outputs of one article of `law_id` in a single execution, with cycle
    /// tracking: the actions in the union of their closures run once (RFC-043).
    /// The cache is keyed on one output, so only a single one is cached.
    fn evaluate_outputs_internal(
        &self,
        law_id: &str,
        output_names: &[&str],
        parameters: BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        let output_name = output_names.first().copied().unwrap_or_default();
        // A value computed with an override held back is not the value of
        // that output, so it neither comes from nor goes into the cache.
        let cacheable = output_names.len() == 1 && res_ctx.held_back.is_empty();
        // --- Cache check (before depth check: cached results don't increase depth) ---
        let key = cache_key(law_id, output_name, &parameters);
        if let Some(cached) = res_ctx.cache.get(&key).filter(|_| cacheable) {
            // Runtime collision check: hash keys are u64 so collisions are
            // theoretically possible. For legally binding decisions, we must
            // never silently return wrong results.
            if !cached.matches(law_id, output_name, &parameters) {
                tracing::warn!(
                    cached_law = cached.law_id,
                    cached_output = cached.output_name,
                    law_id,
                    output_name,
                    "Cache key hash collision detected, bypassing cache"
                );
                // Fall through to re-evaluate
            } else {
                tracing::debug!(law_id, output_name, "Cache hit");
                let _guard = res_ctx
                    .trace_guard(format!("{}#{}", law_id, output_name), PathNodeType::Cached);
                if let Some(val) = cached.outputs.get(output_name) {
                    res_ctx.trace_set_result(val.clone());
                }
                return Ok(ArticleResult {
                    outputs: cached.outputs.clone(),
                    output_provenance: cached.output_provenance.clone(),
                    resolved_inputs: BTreeMap::new(),
                    article_number: String::new(),
                    law_id: law_id.to_string(),
                    law_uuid: None,
                    trace: None,
                    engine_version: crate::VERSION.to_string(),
                    schema_version: None,
                    regulation_hash: None,
                    regulation_valid_from: None,
                    // Filled in by the outermost call from the resolution
                    // context, which outlives this cached partial result.
                    delegation_refusals: Vec::new(),
                    declaration_version_notes: Vec::new(),
                    declarations_not_in_force: Vec::new(),
                    open_terms: Vec::new(),
                });
            }
        }

        tracing::debug!(
            law_id = %law_id,
            output = %output_name,
            depth = res_ctx.depth,
            "Resolving cross-law reference"
        );

        // Check depth limit
        if res_ctx.depth > config::MAX_CROSS_LAW_DEPTH {
            tracing::warn!(
                law_id = %law_id,
                output = %output_name,
                depth = res_ctx.depth,
                "Cross-law resolution depth exceeded"
            );
            let _guard = res_ctx.trace_guard(
                format!("{}#{}", law_id, output_name),
                PathNodeType::CrossLawReference,
            );
            res_ctx.trace_set_message(format!(
                "Cross-law resolution depth exceeded {} levels ({}:{})",
                config::MAX_CROSS_LAW_DEPTH,
                law_id,
                output_name
            ));
            return Err(EngineError::CircularReference(format!(
                "Cross-law resolution depth exceeded {} levels. \
                 Possible circular reference involving {}:{}",
                config::MAX_CROSS_LAW_DEPTH,
                law_id,
                output_name
            )));
        }

        // Get the law (version-aware: use the same reference date as the article lookup)
        let law = self
            .resolver
            .get_law_for_date_reported(law_id, res_ctx.reference_date())
            .map_err(|reason| selection_error(law_id, res_ctx.calculation_date, reason))?;

        // Find the article
        let article = self
            .resolver
            .get_article_by_output(law_id, output_name, res_ctx.reference_date())
            .ok_or_else(|| EngineError::OutputNotFound {
                law_id: law_id.to_string(),
                output: output_name.to_string(),
            })?;

        // Clone parameters for cache storage before moving into evaluation
        let params_for_cache = parameters.clone();

        // Execute with service provider (default stage BESLUIT for cross-law
        // calls). The anchor is entered and restored by
        // `evaluate_article_with_service` itself, for every path into it.
        let result = self.evaluate_article_with_service(
            article,
            law,
            parameters,
            Some(output_names),
            "BESLUIT",
            res_ctx,
        )?;
        if !cacheable {
            return Ok(result);
        }

        // --- Cache store (only on success) ---
        // Note: on a hash collision (astronomically unlikely, ~1e-18 per pair),
        // this overwrites the collider's entry. Both keys then thrash each other,
        // degrading to re-evaluation on every access. Correctness is preserved
        // because every read validates the stored identity fields.
        res_ctx.cache.insert(
            key,
            CacheEntry {
                law_id: law_id.to_string(),
                output_name: output_name.to_string(),
                outputs: result.outputs.clone(),
                output_provenance: result.output_provenance.clone(),
                parameters: params_for_cache,
            },
        );

        Ok(result)
    }

    /// The hooks at `hook_point` on what `article` of `law` produces: on the
    /// decision (its legal character, at this stage) and on the submission it
    /// establishes (RFC-045), in that order, each article once. Index entries
    /// only; whether a hook is in force on the date is for
    /// [`Self::fireable_hooks`].
    ///
    /// `staged` says the article runs at one stage of its procedure. The
    /// hooks on its submission then fire only at the stage named after the
    /// kind of submission, such as the Awb stage AANVRAAG, so recording the
    /// application and taking the decision on it are two steps (RFC-047).
    /// Outside a procedure they fire on every run.
    fn hooks_firing_on(
        &self,
        hook_point: HookPoint,
        article: &Article,
        law: &ArticleBasedLaw,
        stage: &str,
        staged: bool,
    ) -> Vec<&HookEntry> {
        let decision = article.get_decision();
        let mut hooks = match decision.and_then(|d| d.legal_character()) {
            Some(lc) => self.resolver.find_hooks(
                hook_point,
                lc,
                decision.and_then(|d| d.decision_type()),
                stage,
            ),
            None => Vec::new(),
        };
        if let Some(kind) = article
            .get_submission()
            .and_then(|p| p.submission_kind())
            .filter(|k| !staged || *k == stage)
        {
            for h in self
                .resolver
                .find_submission_hooks(hook_point, kind, &law.id, &article.number)
            {
                // An article that hooks onto both the decision and the
                // submission of this article fires once.
                if !hooks
                    .iter()
                    .any(|e| e.law_id == h.law_id && e.article_number == h.article_number)
                {
                    hooks.push(h);
                }
            }
        }
        hooks
    }

    /// The hooks at `hook_point` that fire on `article` at this stage, as
    /// [`Self::fire_hooks`] runs them and [`Self::hook_parameter_names`] reads
    /// them, with what they attach to as the trace names it ("BESCHIKKING
    /// stage BESLUIT", "submission AANVRAAG"). A hook not in force on the date comes back as
    /// the record of why; one already executing is left out. `None` when the
    /// article produces nothing a hook attaches to.
    #[allow(clippy::type_complexity)]
    fn fireable_hooks<'s>(
        &'s self,
        hook_point: HookPoint,
        article: &Article,
        law: &ArticleBasedLaw,
        stage: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> Option<(
        String,
        Vec<std::result::Result<FireableHook<'s>, DeclarationNotInForce>>,
    )> {
        let decision = article.get_decision();
        let legal_character = decision.and_then(|p| p.legal_character());
        let decision_type = decision.and_then(|p| p.decision_type());
        let kind = article.get_submission().and_then(|p| p.submission_kind());
        // What the hooks fire on, as the trace names it ("BESCHIKKING stage
        // BESLUIT") and as the record of a hook not in force names it
        // ("... at stage BESLUIT").
        let (trace_label, on) = match (legal_character, kind) {
            (Some(lc), Some(k)) => (
                format!("{lc} stage {stage}, submission {k}"),
                format!("{lc} at stage {stage}, submission {k}"),
            ),
            (Some(lc), None) => (
                format!("{lc} stage {stage}"),
                format!("{lc} at stage {stage}"),
            ),
            (None, Some(k)) => (format!("submission {k}"), format!("submission {k}")),
            (None, None) => return None,
        };
        let staged = res_ctx
            .staged_article
            .as_ref()
            .is_some_and(|(l, a)| *l == law.id && *a == article.number);
        let matching_hooks = self.hooks_firing_on(hook_point, article, law, stage, staged);
        let subject = format!("hook point {} on {on}", hook_point.as_str());
        // The establishing article, for `established_by`: `law` and
        // `article` are rebound to the hook's own below.
        let (establishing_law, establishing_article) = (law.id.as_str(), article.number.as_str());
        let ref_date = res_ctx.reference_date();
        let hooks = matching_hooks
            .iter()
            .filter(|hook_entry| {
                // Cycle detection: don't re-enter a hook we're already executing
                let key = hook_key(&hook_entry.law_id, &hook_entry.article_number);
                let visited = res_ctx.is_visited(&key);
                if visited {
                    tracing::debug!(hook_key = %key, "Skipping hook: cycle detected");
                }
                !visited
            })
            .map(|hook_entry| {
                let not_in_force = |reason: String| DeclarationNotInForce {
                    kind: DeclarationKind::Hook,
                    law_id: hook_entry.law_id.clone(),
                    article: hook_entry.article_number.clone(),
                    subject: subject.clone(),
                    reason,
                };
                // A hook that does not fire leaves nothing behind: the decision
                // simply comes out without its motivering, its bekendmaking or
                // its bezwaartermijn, and nothing in the outcome says one was
                // owed. "Hook law not found" was moreover untrue for the common
                // case (the law is loaded, it is this date it has no version
                // for), so the reason travels along, to the trace and to the
                // receipt.
                let law = self
                    .resolver
                    .get_law_for_date_reported(&hook_entry.law_id, ref_date)
                    .map_err(|reason| not_in_force(reason.describe()))?;
                // The law is in force, but the version selected for this date
                // need not carry the article the hooks index points at: that
                // index is built from the newest version. The hook still does
                // not fire (an article inserted later is not in force yet, and
                // a renumbered one is missed by the undated index), but the
                // same safeguards go missing as above, so the skip is recorded
                // the same way.
                let article = law
                    .find_article_by_number(&hook_entry.article_number)
                    .ok_or_else(|| {
                        not_in_force(missing_article_reason(
                            law,
                            &hook_entry.article_number,
                            if hook_entry.filter().submission.is_none() {
                                "a hook that fires at the same point on this decision"
                            } else {
                                "a hook that fires at the same point on this submission"
                            },
                            |candidate| {
                                let offered = matching_hooks.iter().any(|h| {
                                    h.law_id == hook_entry.law_id
                                        && h.article_number == candidate.number
                                });
                                !offered
                                    && candidate.get_hooks().is_some_and(|decls| {
                                        decls.iter().any(|d| {
                                            let f = &d.applies_to;
                                            d.hook_point == hook_point
                                                && match (&f.legal_character, &f.submission) {
                                                    (Some(lc), _) => {
                                                        legal_character == Some(lc.as_str())
                                                            && hook_filter_admits(
                                                                f,
                                                                decision_type,
                                                                stage,
                                                            )
                                                    }
                                                    (None, Some(k)) => {
                                                        kind == Some(k.as_str())
                                                            && (!staged || k == stage)
                                                            && submission_filter_admits(
                                                                f,
                                                                establishing_law,
                                                                establishing_article,
                                                            )
                                                    }
                                                    (None, None) => false,
                                                }
                                        })
                                    })
                            },
                        ))
                    })?;
                Ok(FireableHook {
                    entry: hook_entry,
                    law,
                    article,
                })
            })
            .collect();
        Some((trace_label, hooks))
    }

    /// The parameters the hooks at `hook_point` on this article declare: the
    /// names they receive from it (RFC-043).
    fn hook_parameter_names(
        &self,
        hook_point: HookPoint,
        article: &Article,
        law: &ArticleBasedLaw,
        stage: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> BTreeSet<String> {
        self.fireable_hooks(hook_point, article, law, stage, res_ctx)
            .map(|(_, hooks)| {
                hooks
                    .into_iter()
                    .flatten()
                    .flat_map(|hook| declared_parameter_names(hook.article))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What an execution of `article` needs, before any of its actions runs
    /// (RFC-043): which actions, and which inputs a hook or a replacing
    /// override reads.
    fn plan_demand(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        requested_outputs: Option<&[&str]>,
        stage: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> Demand {
        let plan = self.override_plan(article, law, res_ctx);
        let read_before_actions =
            self.hook_parameter_names(HookPoint::PreActions, article, law, stage, res_ctx);
        let post_hooks =
            self.hook_parameter_names(HookPoint::PostActions, article, law, stage, res_ctx);
        // What the post hooks read, and what the replacing overrides of the
        // computed outputs read; `None` means every output is computed.
        let read_after = |computed: Option<&BTreeSet<String>>| -> BTreeSet<String> {
            let replacing = plan
                .replacing
                .iter()
                .filter(|(output, _)| computed.is_none_or(|c| c.contains(*output)))
                .flat_map(|(_, parameters)| parameters.iter().cloned());
            post_hooks.iter().cloned().chain(replacing).collect()
        };

        let Some(requested) = requested_outputs else {
            return Demand {
                outputs: None,
                replacing: plan.replacing.clone(),
                rule_reads: plan.rule_reads.clone(),
                voided: Vec::new(),
                read_before_actions,
                read_after_actions: read_after(None),
            };
        };
        // The requested outputs and the outputs a post hook reads, closed over
        // what they read, where computing an output also reads what its
        // replacing override declares (of this article's outputs only the
        // replaced one itself or a name that is also an input: the override
        // is refused another output). An output a void excludes is not
        // computed for a request or a hook: the law says it does not arise.
        let actions = article
            .get_execution_spec()
            .and_then(|e| e.actions.as_deref())
            .unwrap_or_default();
        let wanted: Vec<&str> = requested
            .iter()
            .copied()
            .chain(post_hooks.iter().map(String::as_str))
            .filter(|name| !plan.voided.contains(*name))
            .collect();
        let outputs = crate::demand::required_outputs_with(actions, &wanted, &plan.replacing);
        let read_after_actions = read_after(Some(&outputs));
        Demand {
            outputs: Some(outputs),
            replacing: plan.replacing.clone(),
            rule_reads: plan.rule_reads.clone(),
            voided: requested
                .iter()
                .filter(|name| plan.voided.contains(**name))
                .map(|name| name.to_string())
                .collect(),
            read_before_actions,
            read_after_actions,
        }
    }

    /// Which overrides of this article's outputs apply, read before any action
    /// runs (RFC-043), with the selection [`Self::apply_overrides`] uses.
    fn override_plan(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        res_ctx: &ResolutionContext<'_>,
    ) -> OverridePlan {
        let mut plan = OverridePlan::default();
        for output in crate::demand::action_outputs(article) {
            // Not in force, more than one, or none: nothing to plan for;
            // `apply_overrides` records or reports it.
            let Ok(Some(SelectedOverride::InForce {
                law: ovr_law,
                article: ovr_article,
                declaration,
                ..
            })) = self.select_override(law, article, output, res_ctx)
            else {
                continue;
            };
            if declaration.is_some_and(|d| d.voids) {
                plan.voided.insert(output.to_string());
            } else {
                plan.replacing.insert(
                    output.to_string(),
                    declared_parameter_names(ovr_article).collect(),
                );
                // An override of the overriding article's own outputs
                // receives all its parameters, as an input or a hook would.
                let overridden_itself = crate::demand::action_outputs(ovr_article).any(|own| {
                    !self
                        .resolver
                        .find_overrides(&ovr_law.id, &ovr_article.number, own)
                        .is_empty()
                });
                let reads = if overridden_itself {
                    declared_parameter_names(ovr_article).collect()
                } else {
                    rule_reads(ovr_article, output)
                };
                plan.rule_reads.insert(output.to_string(), reads);
            }
        }
        plan
    }

    /// Fire hooks that match the given article's `produces` annotation.
    ///
    /// For each matching hook, executes the hook article and merges its outputs.
    /// When multiple hooks produce the same output, conflicts are resolved via
    /// `compare_law_priority` (lex superior / lex posterior).
    ///
    /// # Arguments
    /// * `hook_point` - Whether to fire pre_actions or post_actions hooks
    /// * `article` - The article whose `produces` triggers hooks
    /// * `law` - The law containing the article
    /// * `stage` - The lifecycle stage (e.g., "BESLUIT", "BEKENDMAKING")
    /// * `parameters` - Parameters available to hook articles
    /// * `res_ctx` - Resolution context for cycle detection and tracing
    fn fire_hooks(
        &self,
        hook_point: HookPoint,
        article: &Article,
        law: &ArticleBasedLaw,
        stage: &str,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<(BTreeMap<String, Value>, BTreeMap<String, OutputProvenance>)> {
        let mut hook_outputs: BTreeMap<String, Value> = BTreeMap::new();
        let mut hook_provenance: BTreeMap<String, OutputProvenance> = BTreeMap::new();
        // Track which law produced each output for priority resolution
        let mut output_sources: BTreeMap<String, &ArticleBasedLaw> = BTreeMap::new();
        let hook_point_str = hook_point.as_str();

        // Only fire hooks if the article declares what it produces
        let Some((on, hooks)) = self.fireable_hooks(hook_point, article, law, stage, res_ctx)
        else {
            return Ok((hook_outputs, hook_provenance));
        };

        if hooks.is_empty() {
            return Ok((hook_outputs, hook_provenance));
        }

        tracing::debug!(
            hook_point = ?hook_point,
            on = %on,
            matches = hooks.len(),
            "Firing hooks"
        );

        for hook in hooks {
            let FireableHook {
                entry: hook_entry,
                law: hook_law,
                article: hook_article,
            } = match hook {
                Ok(hook) => hook,
                Err(not_in_force) => {
                    res_ctx.note_not_in_force(not_in_force);
                    continue;
                }
            };
            let hook_law_id = &hook_entry.law_id;
            let hook_article_number = &hook_entry.article_number;
            let hook_key = hook_key(&hook_entry.law_id, &hook_entry.article_number);

            // Filter parameters: only pass parameters declared by the hook article (least privilege)
            let hook_params = Self::filter_parameters_for_article(hook_article, parameters);

            // Trace the hook execution (guard auto-pops on all exit paths)
            let _guard = res_ctx.trace_guard(
                format!("{}:{}", hook_law_id, hook_article_number),
                PathNodeType::HookResolution,
            );
            res_ctx.trace_set_message(format!(
                "Hook {:?} on {} → {}:{}",
                hook_point, on, hook_law_id, hook_article_number
            ));

            // Enter scope for cycle detection
            res_ctx.enter(hook_key.clone());

            // Execute the hook article
            let hook_result = self.evaluate_article_with_service(
                hook_article,
                hook_law,
                hook_params,
                None,      // hooks produce all their outputs
                "BESLUIT", // hook articles themselves are not procedure-aware
                res_ctx,
            );

            // Leave scope (even on error)
            res_ctx.leave(&hook_key);

            // If a hook fires (stage matches), it must succeed.
            // A missing variable means the law cannot be applied — that's an error.
            let result = hook_result?;

            // A voided output is absent from `outputs` by construction, so
            // walking those alone drops its ground: the hook fires, the law
            // says the output does not arise, and nothing downstream can say
            // why.
            //
            // Before the loop over the values, which overwrites this entry with
            // `Reactive` (or the `Override` a lex specialis left) for every name
            // that does arrive with one. That order
            // is what keeps a produced output's provenance correct, so asking
            // here whether the name is absent would decide nothing — and a
            // condition that cannot change an outcome reads as a guard while
            // being none.
            for (name, prov) in &result.output_provenance {
                if matches!(prov, OutputProvenance::Voided { .. }) {
                    hook_provenance.insert(name.clone(), prov.clone());
                }
            }

            for (name, value) in result.outputs {
                // A lex specialis that replaced the hook's own value while the
                // hook ran is the ground for that value: Vw art. 69 sets the
                // four weeks, not Awb 6:7 (RFC-007). Keep its `Override`;
                // every other value the hook produced is `Reactive`.
                let prov = match result.output_provenance.get(&name) {
                    Some(prov @ OutputProvenance::Override { .. }) => prov.clone(),
                    _ => OutputProvenance::Reactive {
                        law_id: hook_law.id.clone(),
                        article: hook_article.number.to_string(),
                        hook_point: hook_point_str.to_string(),
                    },
                };
                if let Some(existing_law) = output_sources.get(&name) {
                    // Conflict: two hooks produce same output.
                    // Resolve via lex superior / lex posterior.
                    match priority::compare_law_priority(hook_law, existing_law)? {
                        std::cmp::Ordering::Greater => {
                            hook_outputs.insert(name.clone(), value);
                            hook_provenance.insert(name.clone(), prov);
                            output_sources.insert(name, hook_law);
                        }
                        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                            // existing wins or unreachable (Equal → Err above)
                        }
                    }
                } else {
                    output_sources.insert(name.clone(), hook_law);
                    hook_provenance.insert(name.clone(), prov);
                    hook_outputs.insert(name, value);
                }
            }
        }

        Ok((hook_outputs, hook_provenance))
    }

    /// Handle an article's flagged constructs based on the configured mode (RFC-012).
    ///
    /// Called before article execution for every construct the article flags,
    /// through `untranslatables` (v0.5.x) or `markings` (v0.7.0).
    /// Behavior depends on `self.untranslatable_mode`:
    /// - `Error`: hard error on unaccepted entries, accepted ones log to trace
    /// - `Propagate`: always log to trace (taint propagation happens at output level)
    /// - `Warn`: log warning to trace, continue
    /// - `Ignore`: only error on unaccepted entries, otherwise silent
    ///
    /// Returns a list of (article, construct) pairs that should taint outputs
    /// in propagate mode. Empty vec means no tainting.
    fn handle_flagged_constructs(
        &self,
        law_id: &str,
        article_number: &str,
        flagged: &[FlaggedConstruct<'_>],
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Vec<(String, String)>> {
        let mut taints = Vec::new();

        for entry in flagged {
            // Always record in trace regardless of mode
            let msg = format!("{}: {} — {}", entry.label, entry.construct, entry.reason);
            {
                let _guard = res_ctx.trace_guard(
                    format!("{}:{}", entry.label.to_lowercase(), entry.construct),
                    PathNodeType::Article,
                );
                res_ctx.trace_set_message(msg.clone());
            }

            match self.untranslatable_mode {
                UntranslatableMode::Error => {
                    if !entry.accepted {
                        return Err(entry.to_error(law_id, article_number));
                    }
                    tracing::info!(
                        law_id,
                        article = article_number,
                        construct = %entry.construct,
                        "Accepted flagged construct, proceeding with partial logic"
                    );
                }
                UntranslatableMode::Propagate => {
                    tracing::info!(
                        law_id,
                        article = article_number,
                        construct = %entry.construct,
                        "Flagged construct — tainting outputs (propagate mode)"
                    );
                    taints.push((article_number.to_string(), entry.construct.to_string()));
                }
                UntranslatableMode::Warn => {
                    tracing::warn!(
                        law_id,
                        article = article_number,
                        construct = %entry.construct,
                        "Flagged construct — executing partial logic"
                    );
                }
                UntranslatableMode::Ignore => {
                    if !entry.accepted {
                        return Err(entry.to_error(law_id, article_number));
                    }
                }
            }
        }
        Ok(taints)
    }

    /// The override of `output_name` that applies, if any, as both
    /// [`Self::override_plan`] and [`Self::apply_overrides`] read it. An
    /// error when more than one applies.
    fn select_override(
        &self,
        law: &ArticleBasedLaw,
        article: &Article,
        output_name: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> Result<Option<SelectedOverride<'_>>> {
        let overrides = self
            .resolver
            .find_overrides(&law.id, &article.number, output_name);

        // An override from another law applies only within the execution
        // that law started: a Vreemdelingenwet exclusion on Awb 6:7 is not
        // a Participatiewet case (RFC-007, "contextual law").
        //
        // An article overriding an output of its own law is a different
        // claim. There is no other law to be protected from, and the
        // scoping made the outcome depend on the route in: asked through
        // the law itself the exclusion applied, asked through a third law
        // reading that output it did not, and the amount the statute says
        // does not arise was handed out anyway. Zorgtoeslag article 3
        // voiding article 2 of the same law is exactly that shape, and it
        // is the case RFC-027 works out, and RFC-041 records the
        // amendment to RFC-007's contextual-law rule that this is.
        let applicable: Vec<_> = overrides
            .iter()
            .filter(|ovr| {
                ovr.law_id == law.id || Some(&ovr.law_id) == res_ctx.contextual_law_id.as_ref()
            })
            .collect();

        let [reference] = applicable.as_slice() else {
            if applicable.is_empty() {
                return Ok(None);
            }
            return Err(EngineError::InvalidOperation(format!(
                "Multiple overrides for output '{}' on '{}:{}' (from {})",
                output_name,
                law.id,
                article.number,
                applicable
                    .iter()
                    .map(|o| o.law_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        let reference = *reference;

        // An override that is already executing does not apply again, and
        // one that is reading its own base value does not apply to that read.
        let key = override_key(&reference.law_id, &reference.article_number);
        if res_ctx.is_visited(&key) || res_ctx.held_back.contains(&key) {
            tracing::debug!(
                law = %reference.law_id,
                article = %reference.article_number,
                "Skipping override: cycle detected"
            );
            return Ok(None);
        }

        // Look up overriding article
        let ref_date = res_ctx.reference_date();
        // The overrides index is built from the newest version of each law,
        // so it offers a lex specialis that may not be in force on this
        // date. Skipping it is right, staying silent about it is not: the
        // output then carries the general rule's value and neither the
        // provenance nor the trace mentions that a special rule addresses
        // exactly this output. Same ground as the `voids` branch of
        // `apply_overrides`, where an absence with a ground stays
        // distinguishable from an absence nobody asked about.
        let ovr_law = match self
            .resolver
            .get_law_for_date_reported(&reference.law_id, ref_date)
        {
            Ok(law) => law,
            Err(reason) => {
                return Ok(Some(SelectedOverride::NotInForce {
                    reference,
                    reason: reason.describe(),
                }));
            }
        };
        // The law is in force, but the version selected for this date need
        // not carry the article the index points at: the index is built
        // from the newest version. An article inserted later is genuinely
        // not in force today. A renumbering is a known limitation of that
        // undated index: the special rule is in force under another number
        // and is still not applied. Either way the general rule's value
        // stands, unchanged from before; what changes is that the skip is
        // recorded like the one above, so "no lex specialis" never reads as
        // a finding about the case (and the reason names a renumbered
        // declaration when that version has one).
        let Some(ovr_article) = ovr_law.find_article_by_number(&reference.article_number) else {
            return Ok(Some(SelectedOverride::NotInForce {
                reference,
                reason: article_missing_from_version(
                    ovr_law,
                    &reference.article_number,
                    &law.id,
                    &article.number,
                    output_name,
                ),
            }));
        };

        // The declaration itself says whether this override replaces the value
        // or removes it. Read it from the overriding article rather than from
        // the index, which carries only the addressing.
        let declaration = ovr_article.get_overrides().and_then(|decls| {
            decls
                .iter()
                .find(|d| d.law == law.id && d.article == article.number && d.output == output_name)
        });
        Ok(Some(SelectedOverride::InForce {
            reference,
            law: ovr_law,
            article: ovr_article,
            declaration,
        }))
    }

    /// Apply lex specialis overrides to an article's outputs.
    ///
    /// For each output in the result, checks if an override exists from the contextual law.
    /// If found, executes the overriding article and replaces the output value.
    /// An output in `replaced_in_actions` was already put to its override
    /// where its action set it, and is left alone.
    #[allow(clippy::too_many_arguments)]
    fn apply_overrides(
        &self,
        result: &mut ArticleResult,
        article: &Article,
        law: &ArticleBasedLaw,
        parameters: &BTreeMap<String, Value>,
        voided_requested: &[String],
        replaced_in_actions: &BTreeMap<String, Option<OutputProvenance>>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<()> {
        // A law overriding its own output needs no contextual law, so a
        // standalone call is no longer a reason to stop here: the selection
        // still drops every override from another law when there is none.
        //
        // Check each output for overrides, and each requested output a void
        // kept from being computed (RFC-043): its ground is recorded the same.
        let mut output_names: Vec<String> = result.outputs.keys().cloned().collect();
        for name in voided_requested {
            if !output_names.contains(name) {
                output_names.push(name.clone());
            }
        }
        output_names.retain(|name| !replaced_in_actions.contains_key(name));
        for output_name in output_names {
            match self.run_override(article, law, &output_name, parameters, res_ctx)? {
                OverrideOutcome::NotApplied => {}
                OverrideOutcome::Voided(provenance) => {
                    // "Bestaat geen aanspraak": the entitlement does not arise,
                    // so there is no value to compute or replace. The output
                    // leaves `outputs` and stays in the provenance, so an
                    // absence with a ground is distinguishable from an absence
                    // nobody asked about.
                    result.outputs.remove(&output_name);
                    result.output_provenance.insert(output_name, provenance);
                }
                OverrideOutcome::Replaced(value, provenance) => {
                    result.outputs.insert(output_name.clone(), value);
                    result.output_provenance.insert(output_name, provenance);
                }
            }
        }

        Ok(())
    }

    /// The replacing override of `output_name`, run for the value it gives:
    /// `None` when none applies, or when the one that applies is a void.
    fn run_replacing_override(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        output_name: &str,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Option<(Value, OutputProvenance)>> {
        Ok(
            match self.run_override(article, law, output_name, parameters, res_ctx)? {
                OverrideOutcome::Replaced(value, provenance) => Some((value, provenance)),
                OverrideOutcome::NotApplied | OverrideOutcome::Voided(_) => None,
            },
        )
    }

    /// Select the override of `output_name` and carry it out: record it when
    /// it is not in force, report a void with its ground, or execute the
    /// overriding article for the value it replaces the output with.
    fn run_override(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        output_name: &str,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<OverrideOutcome> {
        let Some(selected) = self.select_override(law, article, output_name, res_ctx)? else {
            return Ok(OverrideOutcome::NotApplied);
        };
        let ovr_law_id = &selected.reference().law_id;
        let ovr_article_number = &selected.reference().article_number;
        let ovr_key = override_key(ovr_law_id, ovr_article_number);

        let (ovr_law, ovr_article, declaration) = match selected {
            SelectedOverride::InForce {
                law,
                article,
                declaration,
                ..
            } => (law, article, declaration),
            SelectedOverride::NotInForce { reason, .. } => {
                res_ctx.note_not_in_force(DeclarationNotInForce {
                    kind: DeclarationKind::Override,
                    law_id: ovr_law_id.clone(),
                    article: ovr_article_number.clone(),
                    subject: format!(
                        "output '{}' of {} article {}",
                        output_name, law.id, article.number
                    ),
                    reason,
                });
                return Ok(OverrideOutcome::NotApplied);
            }
        };

        // Trace (guard auto-pops on all exit paths)
        let _guard = res_ctx.trace_guard(
            format!("{}:{}", ovr_law_id, ovr_article_number),
            PathNodeType::OverrideResolution,
        );
        res_ctx.trace_set_message(format!(
            "Lex specialis: {}:{} overrides {}:{}.{}",
            ovr_law_id, ovr_article_number, law.id, article.number, output_name
        ));

        if let Some(decl) = declaration.filter(|d| d.voids) {
            res_ctx.trace_set_message(format!(
                "Lex specialis: {}:{} voids {}:{}.{}",
                ovr_law_id, ovr_article_number, law.id, article.number, output_name
            ));
            return Ok(OverrideOutcome::Voided(OutputProvenance::Voided {
                law_id: ovr_law_id.to_string(),
                article: ovr_article_number.to_string(),
                grounds: decl.legal_text_excerpt.clone(),
            }));
        }

        let ovr_params = Self::filter_parameters_for_article(ovr_article, parameters);
        res_ctx.enter(ovr_key.clone());

        let ovr_result = self.evaluate_article_with_service(
            ovr_article,
            ovr_law,
            ovr_params,
            Some(&[output_name]),
            "BESLUIT", // override articles are not procedure-aware
            res_ctx,
        );

        res_ctx.leave(&ovr_key);

        let Some(value) = ovr_result?.outputs.remove(output_name) else {
            return Ok(OverrideOutcome::NotApplied);
        };
        tracing::debug!(
            output = %output_name,
            from = %law.id,
            to = %ovr_law_id,
            "Override applied"
        );
        res_ctx.trace_set_result(value.clone());
        Ok(OverrideOutcome::Replaced(
            value,
            OutputProvenance::Override {
                law_id: ovr_law_id.to_string(),
                article: ovr_article_number.to_string(),
            },
        ))
    }

    /// Execute an article with ServiceProvider support.
    ///
    /// The `stage` parameter controls which lifecycle stage hooks fire at.
    /// For direct (non-procedure) execution, pass `"BESLUIT"` as default.
    /// Evaluate an article, with the resolution context anchored to it.
    ///
    /// Five paths reach the article evaluation -- a cross-law call, a hook, an
    /// override, an open-term implementation and an intra-law reference to
    /// another article -- and only the cross-law one used to enter the anchor
    /// itself. The other four left `res_ctx.anchor` on the *calling* article,
    /// so every step pushed through the resolution context (a data-source
    /// resolve, a nested cross-law guard, a cached node) claimed a provision
    /// the engine was not in: `standaardpremie` is an open term of article 4
    /// and its resolution was anchored to article 2, `vermogen` comes from the
    /// input list of article 3 and its cross-law call said article 2 too.
    ///
    /// Entering it here, around the one body, means a sixth path cannot forget
    /// it and no early return can leave the anchor behind (RFC-039).
    fn evaluate_article_with_service(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        parameters: BTreeMap<String, Value>,
        requested_outputs: Option<&[&str]>,
        stage: &str,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        let outer_anchor = res_ctx.enter_anchor(Some(LegalAnchor::from_article(law, article)));
        let result =
            self.evaluate_article_body(article, law, parameters, requested_outputs, stage, res_ctx);
        // Restored before the `?`: an error must not leave the caller anchored
        // to the article that failed.
        res_ctx.enter_anchor(outer_anchor);
        result
    }

    fn evaluate_article_body(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        parameters: BTreeMap<String, Value>,
        requested_outputs: Option<&[&str]>,
        stage: &str,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ArticleResult> {
        // RFC-012: check what the article flags as beyond its expression —
        // `untranslatables` (v0.5.x) and `markings` (v0.7.0) alike.
        let taints = match article.machine_readable.as_ref() {
            Some(mr) => {
                let flagged = flagged_constructs(mr);
                if flagged.is_empty() {
                    Vec::new()
                } else {
                    self.handle_flagged_constructs(&law.id, &article.number, &flagged, res_ctx)?
                }
            }
            None => Vec::new(),
        };

        // The caller's findings on the terms this article leaves to an
        // authority per case (RFC-050). Across laws one wins over what the
        // call passes for the term, such as the decision value's own
        // finding; at the top it may not also be a parameter.
        let mut parameters = parameters;
        self.note_policy_findings(law, article, &parameters, res_ctx)?;
        let filled = if res_ctx.findings.is_empty() {
            BTreeSet::new()
        } else {
            self.filled_terms(law, article, &parameters, res_ctx.reference_date())?
        };
        for (id, finding) in res_ctx.findings_for(law, article, &filled)? {
            if res_ctx.depth == 0 && parameters.contains_key(&id) {
                return Err(EngineError::InvalidOperation(format!(
                    "'{id}' of {}#{} is given both as a parameter and as a finding",
                    law.id, article.number
                )));
            }
            parameters.insert(id, finding);
        }

        // A required parameter the caller passed as null or unknown names
        // nobody. At the top level (depth 0) that is the caller's error, not a
        // missing fact and not an absence to decide on (RFC-036); a cross-law
        // call in the same situation was already turned into a skip by
        // `resolve_external_input_internal`, so it never gets here with a
        // deeper depth.
        if res_ctx.depth == 0 {
            if let Some((name, value)) = required_parameter_for_nobody(article, &parameters) {
                return Err(EngineError::MissingParameter {
                    law_id: law.id.clone(),
                    name,
                    value: value.to_string(),
                });
            }
        }
        check_decision_parameters(law, article, &parameters, res_ctx.depth == 0)?;

        // What this execution needs, settled before any action runs (RFC-043).
        let demand = self.plan_demand(article, law, requested_outputs, stage, res_ctx);

        // Inputs and open terms are resolved when they are first read: by an
        // action, or by a hook or override that declares them as parameters.
        let engine = ArticleEngine::new(article, law);
        let calculation_date = res_ctx.calculation_date;
        let trace = res_ctx.trace.as_ref().map(Rc::clone);
        let lazy = LazyArticleInputs::new(self, article, law, &parameters, res_ctx)?;

        lazy.resolve_names(&demand.read_before_actions)?;
        let mut hook_params = parameters.clone();
        hook_params.extend(lazy.resolved());
        let (pre_hook_outputs, pre_hook_provenance) = lazy.with_resolution_context(|res_ctx| {
            self.fire_hooks(
                HookPoint::PreActions,
                article,
                law,
                stage,
                &hook_params,
                res_ctx,
            )
        })?;

        // A replacing override runs where the action sets its output, with
        // what an override at the end would receive: the parameters, what was
        // resolved, the pre hook outputs and the outputs computed so far.
        // Every output it was run for is recorded, applied or not, so
        // `apply_overrides` does not run it a second time.
        let replaced_in_actions: RefCell<BTreeMap<String, Option<OutputProvenance>>> =
            RefCell::new(BTreeMap::new());
        // A name that is also an input, parameter or open term of this article
        // is that input to an override, whatever an action of the same name
        // makes of it: which of the two is set when the override runs would
        // depend on the file order.
        let article_inputs: BTreeSet<&str> = article
            .get_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .chain(article.get_inputs().iter().map(|i| i.name.as_str()))
            .chain(
                article
                    .get_open_terms()
                    .into_iter()
                    .flatten()
                    .map(|term| term.id.as_str()),
            )
            .collect();
        let article_outputs: BTreeSet<String> = crate::demand::action_outputs(article)
            .filter(|name| !article_inputs.contains(name))
            .map(str::to_string)
            .collect();
        let replace = |name: &str, value: &Value, outputs: &BTreeMap<String, Value>| {
            let declared = demand.replacing.get(name)?;
            // An override that asks for another output of the article it
            // overrides is refused, wherever that output stands. Run where
            // the output is set, it would see that other output only when
            // the file happens to set it first; run after the article, the
            // actions before it would read the general rule's value. Either
            // way the file order would decide a value.
            let reads = demand.rule_reads.get(name);
            if let Some(param) = reads
                .into_iter()
                .flatten()
                .find(|param| *param != name && article_outputs.contains(*param))
            {
                return Some(Err(EngineError::InvalidOperation(format!(
                    "the override of output '{name}' of {} article {} reads parameter \
                     '{param}', which is another output of that article; an override may \
                     read the overridden article's inputs, not its other outputs",
                    law.id, article.number
                ))));
            }
            // What the override declares, of this article's inputs, is
            // resolved for it before it runs.
            if let Err(e) = lazy.resolve_names(declared) {
                return Some(Err(e));
            }
            let mut ovr_params = parameters.clone();
            ovr_params.extend(lazy.resolved());
            ovr_params.extend(pre_hook_outputs.clone());
            ovr_params.extend(
                outputs
                    .iter()
                    .filter(|(output, _)| !article_inputs.contains(output.as_str()))
                    .map(|(output, value)| (output.clone(), value.clone())),
            );
            ovr_params.insert(name.to_string(), value.clone());
            let replaced = lazy.with_resolution_context(|res_ctx| {
                self.run_replacing_override(article, law, name, &ovr_params, res_ctx)
            });
            match replaced {
                Err(e) => Some(Err(e)),
                Ok(outcome) => {
                    let (value, provenance) = outcome.unzip();
                    replaced_in_actions
                        .borrow_mut()
                        .insert(name.to_string(), provenance);
                    value.map(Ok)
                }
            }
        };

        let mut engine_params = parameters.clone();
        engine_params.extend(pre_hook_outputs.clone());
        let mut result = engine.evaluate_outputs(
            engine_params,
            calculation_date,
            demand.outputs.as_ref(),
            trace,
            Some(&AfterPreHooks {
                inputs: &lazy,
                hook_outputs: &pre_hook_outputs,
                replace: &replace,
            }),
        )?;
        let replaced_in_actions = replaced_in_actions.into_inner();
        for (name, provenance) in &replaced_in_actions {
            if let Some(provenance) = provenance {
                result
                    .output_provenance
                    .insert(name.clone(), provenance.clone());
            }
        }
        lazy.resolve_names(
            demand
                .read_after_actions
                .iter()
                .filter(|name| !pre_hook_outputs.contains_key(*name)),
        )?;

        // What the post-action steps receive: the parameters, what was
        // resolved, and over those what a pre hook produced. What was resolved
        // is also what this article consulted.
        result.resolved_inputs = lazy.resolved();
        drop(lazy);
        let mut post_params = parameters.clone();
        post_params.extend(result.resolved_inputs.clone());
        post_params.extend(pre_hook_outputs.clone());

        // Fire post_actions hooks (between action execution and result return).
        // Post-hooks receive both parameters and article outputs.
        for (name, value) in &result.outputs {
            post_params.insert(name.clone(), value.clone());
        }
        let (post_hook_outputs, post_hook_provenance) = self.fire_hooks(
            HookPoint::PostActions,
            article,
            law,
            stage,
            &post_params,
            res_ctx,
        )?;
        for (name, value) in post_hook_outputs {
            if let std::collections::btree_map::Entry::Vacant(e) =
                result.outputs.entry(name.clone())
            {
                e.insert(value);
                // Tag as reactive (only if not already a direct output)
                if let Some(prov) = post_hook_provenance.get(&name) {
                    result.output_provenance.insert(name, prov.clone());
                }
            }
        }
        // Merge pre-hook outputs into result too (they are part of the execution)
        for (name, value) in pre_hook_outputs {
            if let std::collections::btree_map::Entry::Vacant(e) =
                result.outputs.entry(name.clone())
            {
                e.insert(value);
                if let Some(prov) = pre_hook_provenance.get(&name) {
                    result.output_provenance.insert(name, prov.clone());
                }
            }
        }
        // A voided hook output has no value, so the two merges above never
        // reach it: they walk the values a hook produced, and this name is
        // absent from those by construction. Carrying it here is what lets a
        // receipt say the co-product was excluded, on this ground, rather than
        // leaving it indistinguishable from one that was never computed.
        //
        // `or_insert` and nothing more: a name that arrived with a value was
        // given its provenance by the merge above, so the entry is taken and
        // this leaves it alone. Testing for absence as well would read like a
        // second guard while deciding nothing, and a condition that cannot
        // change an outcome is a claim the code does not keep.
        for (name, prov) in post_hook_provenance
            .iter()
            .chain(pre_hook_provenance.iter())
        {
            if matches!(prov, OutputProvenance::Voided { .. }) {
                result
                    .output_provenance
                    .entry(name.clone())
                    .or_insert_with(|| prov.clone());
            }
        }

        // Apply lex specialis overrides
        self.apply_overrides(
            &mut result,
            article,
            law,
            &post_params,
            &demand.voided,
            &replaced_in_actions,
            res_ctx,
        )?;

        // RFC-024: the engine no longer rounds eurocent outputs implicitly. Rounding
        // is a law-modeled instruction — a law that must round to whole euros/cents
        // does so with an explicit ROUND/CEIL/FLOOR operation. Outputs flow
        // out as exact Decimals; `unit` is a label, never a rounding trigger (RFC-023).

        // RFC-012 Propagate mode: taint all outputs from articles that flag a construct
        if !taints.is_empty() {
            if taints.len() > 1 {
                tracing::warn!(
                    article = %taints[0].0,
                    count = taints.len(),
                    "Article flags multiple constructs; combining into single taint"
                );
            }
            let taint_article = taints[0].0.clone();
            let taint_construct = taints
                .iter()
                .map(|(_, c)| c.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            for value in result.outputs.values_mut() {
                *value = Value::Untranslatable {
                    article: taint_article.clone(),
                    construct: taint_construct.clone(),
                };
            }
        }

        Ok(result)
    }

    /// Resolve an input that takes the named answer of a cell
    /// (`source.reduction`, RFC-048): ask the answerers of the call for
    /// `answer`, with the parameters the binding passes.
    ///
    /// The binding is `article` of the policy `law`, which fills an open
    /// term (`implements`, the loader holds to that). Where a term it fills
    /// names its competent authorities (RFC-052), the answerer of the call
    /// whose [`Authority`](crate::answer::Authority) they name is asked, and
    /// none other; otherwise the one the policy names as its holder. One
    /// question goes to one cell: where the call holds more than one cell
    /// they name, the input is an error, since which of them answers is an
    /// open point. An answerer without an answer of that name, or with a
    /// null one, is silent. Without an answer the input is null: the binding
    /// then fills nothing, as an implementation that is silent for the case
    /// (RFC-036).
    #[allow(clippy::too_many_arguments)]
    fn resolve_answer<'c>(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        input: &Input,
        source: &crate::article::Source,
        answer: &str,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'c>,
    ) -> Result<Option<Value>> {
        let _guard = res_ctx.trace_guard(&input.name, PathNodeType::Resolve);
        let refuse = |res_ctx: &mut ResolutionContext<'c>, message: String| {
            res_ctx.trace_set_message(message.clone());
            Err(EngineError::InvalidOperation(message))
        };
        // The terms the binding fills, as the laws that declare them say on
        // the calculation date.
        let terms: Vec<&crate::article::OpenTerm> = article
            .get_implements()
            .into_iter()
            .flatten()
            .filter_map(|decl| {
                let (_, declaring) = self
                    .article_in_force(&decl.law, &decl.article, res_ctx.calculation_date)
                    .ok()?;
                declaring
                    .get_open_terms()?
                    .iter()
                    .find(|term| term.id == decl.open_term)
            })
            .collect();
        let competent = terms.iter().find_map(|term| {
            term.competent_authority
                .as_ref()
                .and_then(|authority| authority.named())
        });
        let Some(named) = competent.or_else(|| {
            law.competent_authority
                .as_ref()
                .and_then(|authority| authority.named())
        }) else {
            return refuse(
                res_ctx,
                format!(
                    "input '{}' of {} takes the answer '{answer}', but the policy names no \
                     holder (competent_authority) whose cell gives it (RFC-048)",
                    input.name, law.id
                ),
            );
        };
        let askees: Vec<&'c dyn OpenTermAnswerer> = res_ctx
            .answerer
            .into_iter()
            .chain(res_ctx.sphere.iter().copied())
            .filter(|answerer| answerer.authority().is_named_by(named))
            .collect();
        // One question goes to one cell. Which cell of a category answers
        // where the call holds several is open, so the engine asks none of
        // them and does not merge answers.
        if askees.len() > 1 {
            let mut names: Vec<&str> = askees.iter().map(|a| a.authority().name.as_str()).collect();
            names.sort_unstable();
            return refuse(
                res_ctx,
                format!(
                    "input '{}' of {} takes the answer '{answer}', and {} cells of the call are \
                     named by {}: {}; the engine asks one cell, and which one is open",
                    input.name,
                    law.id,
                    askees.len(),
                    authorities(named),
                    names.join(", ")
                ),
            );
        }
        // Without a cell to ask, the binding gives nothing; the parameters it
        // passes are not needed then, and may be absent (RFC-048).
        let Some(askee) = askees.first().copied() else {
            let message = format!(
                "No cell of this execution gives the answer '{answer}' for {}",
                authorities(named)
            );
            return Self::no_answer(article, law, input, message, Vec::new(), res_ctx);
        };
        let parameters = self.build_target_parameters(source.parameters.as_ref(), context)?;
        let request = AnswerRequest {
            name: answer,
            parameters: &parameters,
            calculation_date: res_ctx.calculation_date,
        };
        let given = match askee.answer(self, &request) {
            Ok(Some(given)) => given,
            Ok(None) => {
                let message = format!(
                    "No cell gives the answer '{answer}' for {}; silent: {} (no answer \
                     '{answer}')",
                    authorities(named),
                    askee.authority().name
                );
                return Self::no_answer(article, law, input, message, Vec::new(), res_ctx);
            }
            Err(e) => {
                res_ctx.trace_set_message(format!(
                    "Answer '{answer}': {} failed: {e}",
                    askee.authority().name
                ));
                return Err(e);
            }
        };
        // One node for the answer, named after it; its `answer` says who
        // gave it from what.
        {
            let _answered = res_ctx.trace_guard(answer, PathNodeType::Resolve);
            res_ctx.trace_set_result(given.value.clone());
            res_ctx.trace_set_answer(given.provenance.clone());
            res_ctx.trace_set_message(if given.value.is_null() {
                format!("{} is silent (null)", given.provenance)
            } else {
                format!("Answered by {}", given.provenance)
            });
        }
        if given.value.is_null() {
            let message = format!(
                "No cell gives the answer '{answer}' for {}; silent: {}",
                authorities(named),
                given.provenance.answerer
            );
            return Self::no_answer(
                article,
                law,
                input,
                message,
                vec![given.provenance],
                res_ctx,
            );
        }
        res_ctx.trace_set_message(format!(
            "Answer '{answer}' given by {}",
            given.provenance.answerer
        ));
        let value = given.value;
        res_ctx.trace_set_result(value.clone());
        res_ctx.bound.push(Bound {
            law_id: law.id.clone(),
            article: article.number.clone(),
            answers: vec![given.provenance],
        });
        Ok(Some(value))
    }

    /// The value of an input of `article` that got no answer
    /// (`source.reduction`): null, where the input may be absent, and the
    /// binding then fills nothing, unless nothing falls back on it. The null
    /// answers among those asked, `nulls`, are recorded for the report.
    /// Declared never absent, the input is an error: without a cell that
    /// answers, the decision fails (RFC-036, RFC-048).
    fn no_answer(
        article: &Article,
        law: &ArticleBasedLaw,
        input: &Input,
        message: String,
        nulls: Vec<AnswerProvenance>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Option<Value>> {
        if !input.is_nullable() {
            let message = format!(
                "{message}; input '{}' of {} takes it and is declared never absent",
                input.name, law.id
            );
            res_ctx.trace_set_message(message.clone());
            return Err(EngineError::InvalidOperation(message));
        }
        res_ctx.trace_set_message(message);
        res_ctx.trace_set_result(Value::Null);
        res_ctx.bound.push(Bound {
            law_id: law.id.clone(),
            article: article.number.clone(),
            answers: nulls,
        });
        Ok(Some(Value::Null))
    }

    /// The ids of the open terms of `article` of `law` judged per case
    /// (`decided_per_case_by`) that a regulation in force on `date` fills
    /// with the execution's `parameters` as scope, as when the term is
    /// resolved.
    fn filled_terms(
        &self,
        law: &ArticleBasedLaw,
        article: &Article,
        parameters: &BTreeMap<String, Value>,
        date: Option<NaiveDate>,
    ) -> Result<BTreeSet<String>> {
        let scope: HashMap<String, Value> = parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut filled = BTreeSet::new();
        for term in article.get_open_terms().into_iter().flatten() {
            if term.decided_per_case_by.is_none() {
                continue;
            }
            let lookup =
                self.resolver
                    .find_implementations(&law.id, &article.number, term, date, &scope)?;
            if !lookup.implementations.is_empty() {
                filled.insert(term.id.clone());
            }
        }
        Ok(filled)
    }

    /// Note the caller's findings on the open terms of the policy that
    /// fills a term of `article` (`implements`), and so on: such a policy
    /// leaves a case to the authority with an open term of its own (RFC-048
    /// §2). Whether this case reaches it depends on what the article reads,
    /// so the finding counts as asked by the article whose term the policy
    /// fills, as a finding on the article's own term does. It is checked
    /// against the term's type here, and applied only where the policy
    /// reads it ([`ResolutionContext::findings_noted`]). The policies are
    /// those that apply with the article's `parameters`, as when the term is
    /// resolved.
    fn note_policy_findings(
        &self,
        law: &ArticleBasedLaw,
        article: &Article,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<()> {
        if res_ctx.findings.is_empty() {
            return Ok(());
        }
        let scope: HashMap<String, Value> = parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let date = res_ctx.reference_date();
        let mut todo: Vec<(String, String)> = vec![(law.id.clone(), article.number.clone())];
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        while let Some((law_id, number)) = todo.pop() {
            if !seen.insert((law_id.clone(), number.clone())) {
                continue;
            }
            let Some(found) = self
                .resolver
                .get_law_for_date(&law_id, date)
                .and_then(|l| l.find_article_by_number(&number))
            else {
                continue;
            };
            for term in found.get_open_terms().into_iter().flatten() {
                let lookup = self
                    .resolver
                    .find_implementations(&law_id, &number, term, date, &scope)?;
                for (impl_law, impl_article) in &lookup.implementations {
                    for own in impl_article.get_open_terms().into_iter().flatten() {
                        if own.decided_per_case_by.is_none() {
                            continue;
                        }
                        let Some(value) = res_ctx.findings.get(&own.id) else {
                            continue;
                        };
                        // A term another regulation fills takes no finding.
                        if !self
                            .resolver
                            .find_implementations(
                                &impl_law.id,
                                &impl_article.number,
                                own,
                                date,
                                &scope,
                            )?
                            .implementations
                            .is_empty()
                        {
                            continue;
                        }
                        check_finding(&impl_law.id, &impl_article.number, own, value)?;
                        res_ctx.findings_noted.insert(own.id.clone());
                    }
                    todo.push((impl_law.id.clone(), impl_article.number.clone()));
                }
            }
        }
        Ok(())
    }

    /// Resolve one open term declared on an article via IoC (implements index),
    /// the first time an action reads it (RFC-043).
    ///
    /// For each open term:
    /// 1. Look up implementations in the resolver's implements_index
    /// 2. If found: execute the implementing article to get the value
    /// 3. If not found + has default: execute the default actions. The same
    ///    when the implementation yields `null` for this case: an implementing
    ///    regulation that is silent has not deviated, so the delegating law's
    ///    own rule (its default) applies (RFC-003, RFC-036). On a term without
    ///    a default the implementation's value is the term's, null included:
    ///    a term the law leaves to an authority per case gets no finding
    ///    where a regulation fills it (RFC-048 §2).
    /// 4. If not found + required + no default: error
    /// 5. If not found + not required + no default: skip
    ///
    /// A default reads the caller's parameters and, through `earlier`, the
    /// terms of this article declared before it, each resolved when the
    /// default reads it.
    fn resolve_open_term<'c>(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        term: &crate::article::OpenTerm,
        parameters: &BTreeMap<String, Value>,
        earlier: &mut EarlierTerm<'_, 'c>,
        res_ctx: &mut ResolutionContext<'c>,
    ) -> Result<Value> {
        // Cycle detection: check if we're already resolving this open term
        // Use \0 as separator to prevent key collisions when IDs contain #.
        // A base read resolves the term again with its implementation held
        // back; that is another resolution, so what is held back is part of
        // the key and a cycle within the base read is still caught.
        let ot_key = format!(
            "open_term:{}\0{}\0{}{}",
            law.id,
            article.number,
            term.id,
            res_ctx.held_back_suffix()
        );
        if res_ctx.is_visited(&ot_key) {
            tracing::warn!(
                law_id = %law.id,
                article = %article.number,
                open_term = %term.id,
                "Circular open term dependency detected"
            );
            let _guard = res_ctx.trace_guard(&term.id, PathNodeType::OpenTermResolution);
            res_ctx.trace_set_message(format!(
                "Circular dependency: open term '{}' on {}#{} is already being resolved",
                term.id, law.id, article.number
            ));
            return Err(EngineError::CircularReference(format!(
                "Circular open term dependency: '{}' on {} article {} is already being resolved",
                term.id, law.id, article.number
            )));
        }
        res_ctx.enter(ot_key.clone());

        tracing::debug!(
            law_id = %law.id,
            article = %article.number,
            open_term = %term.id,
            "Resolving open term"
        );

        // Trace the open term resolution (guard auto-pops on all exit paths)
        let _guard = res_ctx.trace_guard(&term.id, PathNodeType::OpenTermResolution);
        res_ctx.trace_set_resolve_type(ResolveType::OpenTerm);

        // Look up implementations (filtered by execution scope)
        // Convert BTreeMap to HashMap at the resolver boundary
        let scope: HashMap<String, Value> = parameters
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let lookup = match self.resolver.find_implementations(
            &law.id,
            &article.number,
            term,
            res_ctx.reference_date(),
            &scope,
        ) {
            Ok(lookup) => lookup,
            Err(e) => {
                res_ctx.trace_set_message(format!(
                    "Open term '{}': implementation lookup failed: {}",
                    term.id, e
                ));
                res_ctx.leave(&ot_key);
                return Err(e);
            }
        };

        // A regulation that arrogates a term the law delegates elsewhere is
        // skipped, so a competent regulation lower in the ranking can still
        // win. But it is a defect in the corpus, not a property of this
        // case, so it is recorded: it stands in the trace under this open
        // term and travels to the receipt even when tracing is off. Without
        // that record a citizen would read "no implementation found, using
        // default" and never learn that a filling was offered and refused.
        for refusal in &lookup.refusals {
            tracing::warn!(
                law_id = %refusal.declaring_law,
                article = %refusal.declaring_article,
                open_term = %refusal.open_term,
                required_layer = %refusal.required_layer,
                refused_law = %refusal.refused_law,
                refused_layer = %refusal.refused_layer,
                "Implementation refused: regulatory_layer is not the delegated layer"
            );
            let message = refusal.message();
            {
                let _refusal_guard =
                    res_ctx.trace_guard(&refusal.refused_law, PathNodeType::OpenTermResolution);
                res_ctx.trace_set_message(message);
            }
        }
        res_ctx
            .delegation_refusals
            .extend(lookup.refusals.iter().cloned());

        // A regulation that fills this term but is not in force on this
        // date is no candidate either. Without a record the branches below
        // report "no implementation, using the default" or "resolved as
        // null" — the same sentences they print when nobody ever wrote one.
        for note in lookup.not_in_force {
            res_ctx.note_not_in_force(note);
        }

        // Implementations that are silent for this case (null).
        let mut silent: Vec<String> = Vec::new();

        // Every candidate that survives `find_implementations` is competent:
        // the resolver drops implementations whose regulatory_layer is not
        // the layer the open term delegates to, before priority ranking.
        //
        // Whether an implementation filled the term for this case, and
        // which implementations were silent for it (null). The candidates
        // come winner first (priority resolution); the first one that
        // fills the term decides, and a silent one is passed over for
        // the next, so a filling implementation wins over a silent one
        // whatever their order.
        let mut filled: Option<(Value, FilledBy)> = None;
        for (impl_law, impl_article) in &lookup.implementations {
            // The implementing article is reading the output it fills in:
            // that read gets the law without this filling.
            if res_ctx.held_back.contains(&implementation_key(
                &impl_law.id,
                &impl_article.number,
                &law.id,
            )) {
                res_ctx.trace_set_message(format!(
                    "Open term '{}': {} article {} held back, it is reading its own base value",
                    term.id, impl_law.id, impl_article.number
                ));
                continue;
            }
            tracing::debug!(
                open_term = %term.id,
                implementing_law = %impl_law.id,
                implementing_article = %impl_article.number,
                "Found implementation for open term"
            );

            // Execute the implementing article to get the value.
            // Only forward parameters that the implementing article declares
            // in its execution.parameters — principle of least privilege.
            let impl_params = Self::filter_parameters_for_article(impl_article, parameters);
            // The answers a binding takes while it runs (`source.reduction`).
            let bound_from = res_ctx.bound.len();
            let result = match self.evaluate_article_with_service(
                impl_article,
                impl_law,
                impl_params,
                Some(&[term.id.as_str()]),
                "BESLUIT",
                res_ctx,
            ) {
                Ok(r) => r,
                Err(e) => {
                    res_ctx.trace_set_message(format!(
                        "Open term '{}': implementation execution failed: {}",
                        term.id, e
                    ));
                    res_ctx.leave(&ot_key);
                    return Err(e);
                }
            };

            if let Some(value) = result.outputs.get(&term.id) {
                if value.is_null() && term.default.is_some() {
                    // The verordening says nothing for this case ("de APV
                    // zwijgt"): no deviation was allowed, so the rule of
                    // the delegating law applies, which is its default.
                    // An implementation cannot express "there is none"
                    // for a term that has a default; RFC-036 accepts
                    // that, and the trace and the log say what happened,
                    // since a typo in the implementation looks the same.
                    silent.push(format!("{} article {}", impl_law.id, impl_article.number));
                } else {
                    // A regulation that fills a term the law leaves to an
                    // authority per case fills it, also with null: no
                    // finding takes its place. A policy that lets the
                    // authority judge a case says so with an open term of
                    // its own (RFC-048 §2). A finding given for the term is
                    // then asked by nothing.
                    if let Some(by) = term
                        .decided_per_case_by
                        .as_ref()
                        .filter(|_| parameters.contains_key(&term.id))
                    {
                        let message = format!(
                            "Open term '{}' on {}#{} is filled by {} article {}; the finding of \
                             {by} for it is asked by none (RFC-048)",
                            term.id, law.id, article.number, impl_law.id, impl_article.number
                        );
                        res_ctx.trace_set_message(message.clone());
                        res_ctx.leave(&ot_key);
                        return Err(EngineError::InvalidOperation(message));
                    }
                    res_ctx.trace_set_result(value.clone());
                    // A binding that took the answer of a cell fills the
                    // term with it: who answered is part of who filled it
                    // (RFC-048).
                    let taken: Vec<&Bound> = res_ctx.bound[bound_from..]
                        .iter()
                        .filter(|b| b.law_id == impl_law.id && b.article == impl_article.number)
                        .collect();
                    let answers: Vec<AnswerProvenance> = taken
                        .iter()
                        .flat_map(|b| b.answers.iter().cloned())
                        .collect();
                    let by = if answers.is_empty() {
                        String::new()
                    } else {
                        format!(
                            ", with the answer of {}",
                            answers
                                .iter()
                                .map(|a| a.answerer.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    res_ctx.trace_set_message(format!(
                        "Open term '{}' implemented by {} article {}{by}",
                        term.id, impl_law.id, impl_article.number
                    ));
                    let law_id = impl_law.id.clone();
                    let article = impl_article.number.clone();
                    filled = Some((
                        value.clone(),
                        if taken.is_empty() {
                            FilledBy::Regulation { law_id, article }
                        } else {
                            FilledBy::Answer {
                                law_id,
                                article,
                                answers,
                            }
                        },
                    ));
                    break;
                }
            } else {
                // An implementation that voids the very term it implements
                // did produce an answer: the law says the value does not
                // arise. Reporting that as "produced no matching output"
                // is the misdiagnosis `OutputVoided` exists to prevent,
                // and it would send a reader looking for a modelling
                // defect in a regulation that is correct.
                if let Some(error) =
                    voided_output_error(&impl_law.id, &term.id, &result.output_provenance)
                {
                    res_ctx.trace_set_message(error.to_string());
                    res_ctx.leave(&ot_key);
                    return Err(error);
                }
                // Implementation executed but didn't produce the expected output
                res_ctx.trace_set_message(format!(
                    "Open term '{}': implementation {} article {} produced no matching output",
                    term.id, impl_law.id, impl_article.number
                ));
                res_ctx.leave(&ot_key);
                return Err(EngineError::InvalidOperation(format!(
                    "Implementation {} article {} for open term '{}' did not produce output named '{}'",
                    impl_law.id, impl_article.number, term.id, term.id
                )));
            }
        }
        if let Some((value, by)) = filled {
            res_ctx.note_open_term(&law.id, &article.number, &term.id, by);
            res_ctx.leave(&ot_key);
            return Ok(value);
        }
        // A norm the law leaves to an authority per case
        // (`decided_per_case_by`) and that no regulation fills: the
        // authority's judgement for this case comes from the caller, under the
        // term's id (RFC-047), and wins over a default. A regulation that is
        // silent on a term with a default leaves it to the default, not to
        // a finding (RFC-036, RFC-048 §2).
        if let Some(by) = &term.decided_per_case_by {
            if let Some(value) = parameters.get(&term.id) {
                if !silent.is_empty() {
                    let message = format!(
                        "Open term '{}' on {}#{} is filled by its default: implementation {} \
                         is silent for this case (null); the finding of {by} for it is asked by \
                         none (RFC-048)",
                        term.id,
                        law.id,
                        article.number,
                        silent.join(", ")
                    );
                    res_ctx.trace_set_message(message.clone());
                    res_ctx.leave(&ot_key);
                    return Err(EngineError::InvalidOperation(message));
                }
                res_ctx.trace_set_result(value.clone());
                res_ctx.trace_set_message(format!(
                    "Open term '{}' decided in this case by {by}",
                    term.id
                ));
                res_ctx.note_open_term(&law.id, &article.number, &term.id, FilledBy::Finding);
                res_ctx.leave(&ot_key);
                return Ok(value.clone());
            }
        }
        let value = if let Some(ref default) = term.default {
            // No implementation found, or a silent one — execute default actions
            tracing::debug!(
                open_term = %term.id,
                "No implementation filled the term, using default"
            );

            if let Some(ref actions) = default.actions {
                // Build a synthetic article from the default actions and evaluate
                // it through ArticleEngine — this correctly handles action.output,
                // intermediate variables, and all operation patterns.
                let synthetic_article = Article {
                    number: format!("default:{}", term.id),
                    text: String::new(),
                    url: None,
                    placement: None,
                    machine_readable: Some(MachineReadable {
                        execution: Some(Execution {
                            produces: None,
                            parameters: None,
                            input: None,
                            output: None,
                            actions: Some(actions.clone()),
                        }),
                        ..Default::default()
                    }),
                    references: None,
                };

                let engine = ArticleEngine::new(&synthetic_article, law);

                // The default reads the current context parameters (variables
                // like $type_beplanting) and an earlier term when one of its
                // operations reaches it (RFC-043).
                let calculation_date = res_ctx.calculation_date;
                let evaluated = {
                    let earlier_terms = EarlierTerms {
                        earlier: RefCell::new(&mut *earlier),
                        res_ctx: RefCell::new(&mut *res_ctx),
                    };
                    let required = crate::demand::required_outputs(actions, &[term.id.as_str()]);
                    engine.evaluate_outputs(
                        parameters.clone(),
                        calculation_date,
                        Some(&required),
                        None,
                        Some(&earlier_terms),
                    )
                };

                let default_result = match evaluated {
                    Ok(r) => r,
                    Err(e) => {
                        res_ctx.trace_set_message(format!(
                            "Open term '{}': default evaluation failed: {}",
                            term.id, e
                        ));
                        res_ctx.leave(&ot_key);
                        return Err(e);
                    }
                };

                // The default must produce the term it is the default for.
                // Reading a missing output as null let two different things
                // arrive as the same value: a default that says "null" and
                // a default that produced nothing at all — an action whose
                // `output` names something else, or none. Downstream null
                // is a signal in its own right ("no municipal verordening,
                // fall back to the statutory distance"), so the second case
                // handed the citizen the general rule under a trace saying
                // the default had been applied. The implementation branch
                // above refuses the same miss; so does this one.
                let Some(default_value) = default_result.outputs.get(&term.id).cloned() else {
                    res_ctx.trace_set_message(format!(
                        "Open term '{}': default produced no output named '{}'",
                        term.id, term.id
                    ));
                    res_ctx.leave(&ot_key);
                    return Err(EngineError::InvalidOperation(format!(
                        "Default for open term '{}' on {}#{} did not produce output named \
                         '{}', so there is no default value to apply",
                        term.id, law.id, article.number, term.id
                    )));
                };

                res_ctx.trace_set_result(default_value.clone());
                if silent.is_empty() {
                    res_ctx
                        .trace_set_message(format!("Open term '{}' using default value", term.id));
                } else {
                    let implementations = silent.join(", ");
                    tracing::warn!(
                        law_id = %law.id,
                        article = %article.number,
                        open_term = %term.id,
                        implementations = %implementations,
                        "Implementation returned null for an open term with a default: \
                         the delegating law's default applies"
                    );
                    res_ctx.trace_set_resolve_type(ResolveType::OpenTermSilent);
                    res_ctx.trace_set_message(format!(
                        "Open term '{}' using default value: implementation {implementations} \
                         is silent for this case (null)",
                        term.id
                    ));
                }
                default_value
            } else {
                // Default exists but has no actions — treat as null
                res_ctx.trace_set_result(Value::Null);
                res_ctx.trace_set_message(format!("Open term '{}' using empty default", term.id));
                Value::Null
            }
        } else if term.required {
            // Required but no implementation that fills it and no default
            res_ctx.trace_set_message(format!(
                "Open term '{}' is required but {}",
                term.id,
                Self::unfilled_note(&silent, res_ctx)
            ));
            res_ctx.leave(&ot_key);
            return Err(EngineError::ResolutionError(
                match &term.decided_per_case_by {
                    Some(by) => format!(
                        "Required open term '{}' on {}#{} is decided per case by {by}, \
                     and no judgement was given for this case",
                        term.id, law.id, article.number
                    ),
                    None => format!(
                        "Required open term '{}' on {}#{} has no implementation and no default",
                        term.id, law.id, article.number
                    ),
                },
            ));
        } else {
            // Not required, no implementation, no default — resolve as null
            // so downstream actions can check for null and fall back to their
            // own defaults (e.g., BW 5:42 falls back to the statutory distance
            // when no municipal verordening overrides it).
            tracing::debug!(
                open_term = %term.id,
                "Optional open term not implemented, resolving as null"
            );

            res_ctx.trace_set_message(format!(
                "Open term '{}' not required, {}, resolved as null",
                term.id,
                Self::unfilled_note(&silent, res_ctx)
            ));
            Value::Null
        };

        res_ctx.leave(&ot_key);
        Ok(value)
    }

    /// Why no implementation filled an open term, for its trace: none was
    /// found, or the ones found are silent for this case (null). The silent
    /// case is marked as such, as the judgement and default branches of
    /// [`Self::resolve_open_term`] do.
    fn unfilled_note(silent: &[String], res_ctx: &ResolutionContext<'_>) -> String {
        if silent.is_empty() {
            "no implementation found".to_string()
        } else {
            res_ctx.trace_set_resolve_type(ResolveType::OpenTermSilent);
            format!(
                "implementation {} is silent for this case (null)",
                silent.join(", ")
            )
        }
    }

    /// Refuse a call from `caller` to the rulework `target` when `target`
    /// is a `REDUCTIE` and `caller` is not: a reduction reads the chronicle
    /// of the cell that runs it, so only a reduction calls one (RFC-048), on
    /// that same chronicle. Each in the version in force on the reference
    /// date, or else its newest. A call to the decision of an article
    /// (`source.article`) needs none: a reduction takes no decision.
    fn refuse_reduction_call(
        &self,
        caller: &str,
        target: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> Result<()> {
        let layer = |id: &str| {
            self.resolver
                .get_law_for_date(id, res_ctx.reference_date())
                .or_else(|| self.get_law(id))
                .map(|law| law.regulatory_layer)
        };
        if layer(target) == Some(RegulatoryLayer::Reductie)
            && layer(caller) != Some(RegulatoryLayer::Reductie)
        {
            return Err(EngineError::ResolutionError(format!(
                "{caller} calls the reduction {target}; only a REDUCTIE calls a REDUCTIE (RFC-048)"
            )));
        }
        Ok(())
    }

    /// Resolve an input that reads a chronicle (`source.decisions`,
    /// `source.facts`, RFC-048) from the chronicle of the call. The article
    /// of `decisions: $<parameter>` is the value of that parameter, an
    /// article reference (schema v0.8.11); any other value, null and
    /// unknown included, is an error, since a read of no article would say
    /// that no decision was taken. Without a chronicle that answers, the
    /// input is an error too.
    fn resolve_chronicle_read(
        &self,
        law: &ArticleBasedLaw,
        input: &Input,
        read: ChronicleRead<'_>,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        let _guard = res_ctx.trace_guard(&input.name, PathNodeType::Resolve);
        let named;
        let read = match read {
            ChronicleRead::Decisions(reference) => match reference.strip_prefix('$') {
                None => read,
                Some(path) => match context.resolve(path)? {
                    Value::String(s) if regelrecht_law_model::is_article_reference(&s) => {
                        named = s;
                        ChronicleRead::Decisions(&named)
                    }
                    other => {
                        let error = EngineError::TypeMismatch {
                            expected: format!(
                                "an article reference in {reference} for input '{}' of {}",
                                input.name, law.id
                            ),
                            actual: other.to_string(),
                        };
                        res_ctx.trace_set_message(error.to_string());
                        return Err(error);
                    }
                },
            },
            ChronicleRead::Facts(_) => read,
        };
        if let Some(chronicle) = res_ctx.chronicle {
            let rows = chronicle.rows(self, law, read, res_ctx.calculation_date);
            if let Some(rows) = rows.inspect_err(|e| res_ctx.trace_set_message(e.to_string()))? {
                let keeper = chronicle.keeper().to_string();
                res_ctx.trace_set_resolve_type(ResolveType::DataSource);
                res_ctx.trace_set_result(rows.clone());
                res_ctx.trace_set_source(ValueSource {
                    kind: ResolveType::DataSource,
                    provider: Some(keeper.clone()),
                    scope: Some(law.id.clone()),
                });
                res_ctx
                    .trace_set_message(format!("Resolving from the chronicle of {keeper}: {read}"));
                return Ok(rows);
            }
        }
        let message = format!(
            "input '{}' of {} reads {read}, and no data source supplies them",
            input.name, law.id
        );
        res_ctx.trace_set_message(message.clone());
        Err(EngineError::DataSourceError(message))
    }

    /// Whether resolving `input` calls another law: it names a regulation or
    /// an article (RFC-050) and no data source answers for it first, the
    /// order [`Self::resolve_input`] follows.
    fn calls_other_law(
        &self,
        law: &ArticleBasedLaw,
        input: &Input,
        parameters: &BTreeMap<String, Value>,
    ) -> bool {
        input
            .source
            .as_ref()
            .is_some_and(|source| source.regulation.is_some() || source.article.is_some())
            && self
                .data_registry
                .resolve_for_law(&input.name, parameters, Some(&law.id))
                .is_none()
    }

    /// Resolve one input from its source, the first time an action reads it
    /// (RFC-043). `Ok(None)` leaves it unresolved: it
    /// has no source, or a required lookup key was not passed, and a reference
    /// to it fails as a missing variable.
    fn resolve_input(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        input: &crate::article::Input,
        context: &RuleContext,
        parameters: &BTreeMap<String, Value>,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Option<Value>> {
        let source = match &input.source {
            Some(s) => s,
            None => return Ok(None),
        };

        // Check DataSourceRegistry before cross-law resolution.
        // An empty registry resolves to None, so no separate guard is needed.
        if let Some(data_match) =
            self.data_registry
                .resolve_for_law(&input.name, parameters, Some(&law.id))
        {
            tracing::debug!(
                input = %input.name,
                source = %data_match.source_name,
                "Resolved input from data registry"
            );

            // Trace the data source resolution
            {
                let _guard = res_ctx.trace_guard(&input.name, PathNodeType::Resolve);
                res_ctx.trace_set_resolve_type(ResolveType::DataSource);
                res_ctx.trace_set_result(data_match.value.clone());
                // Which organization supplied this fact, in a field
                // (RFC-039). The message below says the same thing for a
                // person reading a terminal; it used to be the only place
                // it was said, so consumers matched a regular expression
                // against it.
                res_ctx.trace_set_source(ValueSource {
                    kind: ResolveType::DataSource,
                    provider: Some(data_match.source_name.clone()),
                    scope: data_match.law_scope.clone(),
                });
                // The unit the law declares for this input, so a reader
                // sees an amount and not a bare count of cents (RFC-023,
                // RFC-039). Only where a declaration carries one; a value
                // computed mid-expression has none, and version 1 does not
                // infer one.
                if let Some(ref ts) = input.type_spec {
                    res_ctx.trace_set_type_spec(ts.clone());
                }
                res_ctx.trace_set_message(format!(
                    "Resolving from SOURCE {}: {}",
                    data_match.source_name, data_match.value
                ));
            }

            // An explicit null cell is the register's claim that there is
            // none. The law's declaration says whether that is a value this
            // input can take; if not, the two contradict each other and the
            // data is wrong at the boundary (RFC-036).
            if data_match.value.is_null() && !input.is_nullable() {
                return Err(null_for_non_nullable(
                    law,
                    input,
                    format!("source {}", data_match.source_name),
                ));
            }

            return Ok(Some(data_match.value));
        }

        // The decisions taken under an article (RFC-048): only a data source
        // answers them, a value passed under the input's name, or the
        // chronicle of the call, that of the cell that runs the reduction.
        // A reduction it calls reads that chronicle too. No regulation is
        // executed for them, and no source is no empty list: a reduction
        // over decisions nobody supplied would say that none were taken.
        // The recorded facts an article requires (RFC-051) likewise.
        if let Some(read) = crate::load_check::chronicle_read(source) {
            return self
                .resolve_chronicle_read(law, input, read, context, res_ctx)
                .map(Some);
        }

        // The named answer of a cell, which a binding takes (RFC-048).
        if let Some(answer) = &source.reduction {
            return self.resolve_answer(article, law, input, source, answer, context, res_ctx);
        }

        // A call to an article, named in the law or in data (RFC-050).
        if let Some(target) = &source.article {
            return self.resolve_article_call(law, input, source, target, context, res_ctx);
        }

        // For cross-law resolution, output defaults to input name
        let output_name = source.output.as_deref().unwrap_or(&input.name);

        if let Some(regulation) = &source.regulation {
            // External reference
            let base_of = self.read_as_base(article, law, regulation, output_name, res_ctx);
            let resolution = self.resolve_external_input_detailed(
                regulation,
                output_name,
                source.parameters.as_ref(),
                context,
                &base_of,
                res_ctx,
            )?;

            // The other law said "none" (its output was null), or it was
            // not run because a required parameter named nobody. Either
            // way the value is an absence, and this input has to be
            // declared able to take one (RFC-036).
            let origin = if resolution.skipped {
                format!("skipped call to {regulation}")
            } else {
                format!("{regulation}.{output_name}")
            };
            checked_call_value(law, input, resolution.value, origin)
        } else if source.output.is_some() {
            // Internal reference (same-law) with output specified.
            // Resolve through the service layer so cross-law inputs of the
            // referenced article are properly handled.
            let _guard =
                res_ctx.trace_guard(format!("{}#{}", law.id, output_name), PathNodeType::Resolve);
            res_ctx.trace_set_resolve_type(ResolveType::ResolvedInput);
            res_ctx.trace_set_message(format!("Internal reference: {}#{}", law.id, output_name));

            // Cycle detection: check if this internal output is already being
            // resolved. Use \0 as separator to prevent key collisions, and an
            // "internal:" prefix to keep keys distinct from external references.
            // What is held back for a base read is part of the key: the
            // held-back evaluation is another resolution. An article reading
            // an output of its own law that it replaces or fills in reads its
            // base value, as across laws.
            let base_of = self.read_as_base(article, law, &law.id, output_name, res_ctx);
            let internal_key = format!(
                "internal:{}\0{}{}",
                law.id,
                output_name,
                if base_of.is_empty() {
                    res_ctx.held_back_suffix()
                } else {
                    res_ctx.held_back_suffix_with(&base_of)
                }
            );
            if res_ctx.is_visited(&internal_key) {
                res_ctx.trace_set_message(format!(
                    "Circular internal reference detected: {}#{} is already being resolved",
                    law.id, output_name
                ));
                return Err(EngineError::CircularReference(format!(
                    "Circular internal reference: output '{}' in {} is already being resolved",
                    output_name, law.id
                )));
            }

            // A base read of an output this article replaces reads the
            // article the override names: with a same-law override two
            // articles produce the output, and the general one is meant.
            let base_target = if base_of.is_empty() {
                None
            } else {
                replaced_article(article, law, output_name)
            };
            let ref_article = match base_target.or_else(|| law.find_article_by_output(output_name))
            {
                Some(a) => a,
                None => {
                    res_ctx.trace_set_message(format!(
                        "Internal reference failed: output '{}' not found in {}",
                        output_name, law.id
                    ));
                    return Err(EngineError::OutputNotFound {
                        law_id: law.id.clone(),
                        output: output_name.to_string(),
                    });
                }
            };

            let ref_params = parameters.clone();

            // Enter internal resolution scope for cycle detection
            res_ctx.enter(internal_key.clone());

            // Check depth limit (mirrors evaluate_law_output_internal): the
            // visited set bounds distinct outputs, but a long non-cyclic
            // internal chain could still overflow the stack.
            if res_ctx.depth > config::MAX_CROSS_LAW_DEPTH {
                res_ctx.leave(&internal_key);
                tracing::warn!(
                    law_id = %law.id,
                    output = %output_name,
                    depth = res_ctx.depth,
                    "Cross-law resolution depth exceeded (internal reference)"
                );
                res_ctx.trace_set_message(format!(
                    "Cross-law resolution depth exceeded {} levels ({}:{})",
                    config::MAX_CROSS_LAW_DEPTH,
                    law.id,
                    output_name
                ));
                return Err(EngineError::CircularReference(format!(
                    "Cross-law resolution depth exceeded {} levels. \
                     Possible circular reference involving {}:{}",
                    config::MAX_CROSS_LAW_DEPTH,
                    law.id,
                    output_name
                )));
            }

            let newly_held: Vec<&String> = base_of
                .iter()
                .filter(|held| res_ctx.held_back.insert((*held).clone()))
                .collect();
            let eval_result = self.evaluate_article_with_service(
                ref_article,
                law,
                ref_params,
                Some(std::slice::from_ref(&output_name)),
                "BESLUIT",
                res_ctx,
            );
            for held in newly_held {
                res_ctx.held_back.remove(held);
            }

            // Leave scope (even on error, for correct cycle tracking)
            res_ctx.leave(&internal_key);

            let result = match eval_result {
                Ok(r) => r,
                Err(e) => {
                    res_ctx.trace_set_message(format!("Internal reference failed: {}", e));
                    return Err(e);
                }
            };

            if let Some(value) = result.outputs.get(output_name) {
                res_ctx.trace_set_result(value.clone());
                // Same rule as across laws: an absent output of another
                // article only fits an input that may be absent (RFC-036).
                if value.is_null() && !input.is_nullable() {
                    return Err(null_for_non_nullable(
                        law,
                        input,
                        format!("{}.{output_name}", law.id),
                    ));
                }
                Ok(Some(value.clone()))
            } else {
                // The referenced article ran but produced no such output
                // (e.g. an IF action whose branch was not taken). Surface the
                // precise cause here instead of leaving the input unresolved
                // and letting the consuming action fail later with a generic
                // VariableNotFound. Mirrors the external-reference path
                // (resolve_external_input_internal), which errors the same way.
                res_ctx.trace_set_message(format!(
                    "Internal reference: output '{}' not in result from article {}",
                    output_name, ref_article.number
                ));
                Err(missing_output_error(
                    &law.id,
                    output_name,
                    &result.output_provenance,
                ))
            }
        } else {
            // Empty source (source: {}) — resolved from DataSourceRegistry
            // only, and no source had a value: no matching row, or a row
            // without this field. The fact exists but nobody has it, so the
            // input is Unknown for lack of exactly this fact (RFC-036). An
            // explicit null cell never reaches here: the source answered
            // "none", and that is absence, a value.
            //
            // Unless the register was never asked about anybody: a required
            // parameter of this article (the lookup key, typically `bsn`)
            // that the caller did not pass. Then nothing is known about
            // the fact because the question was malformed, not because
            // nobody has the answer, and the input stays unresolved as it
            // did before RFC-036. A reference to it fails, so a forgotten
            // or misspelled required parameter never becomes an unknown
            // outcome.
            let _guard = res_ctx.trace_guard(&input.name, PathNodeType::Resolve);
            // Nor when the register could not be asked at all: a source
            // that holds this field keys on a parameter that is unknown
            // (nobody knows yet whose record to read) or null (there is
            // nobody to read about). Then the input is that same unknown,
            // so the outcome names the key and not the field, or that same
            // null, mirroring the cross-law skip rule (RFC-036).
            if let Some(blocked) =
                self.data_registry
                    .blocked_lookup_for_law(&input.name, parameters, Some(&law.id))
            {
                res_ctx.trace_set_resolve_type(ResolveType::DataSource);
                res_ctx.trace_set_result(blocked.clone());
                res_ctx.trace_set_message(if blocked.is_unknown() {
                    format!(
                        "Input '{}' cannot be looked up, the key is unknown: input is unknown for the same reason",
                        input.name
                    )
                } else {
                    format!(
                        "Input '{}' cannot be looked up, the key is null (nobody to look up): input is null",
                        input.name
                    )
                });
                // Asked about nobody, the register answers with an absence;
                // the input has to be declared able to take one (RFC-036).
                if blocked.is_null() && !input.is_nullable() {
                    return Err(null_for_non_nullable(
                        law,
                        input,
                        "lookup keyed on null".to_string(),
                    ));
                }
                return Ok(Some(blocked));
            }
            if let Some(missing) = required_parameter_not_passed(article, parameters) {
                res_ctx.trace_set_message(format!(
                    "Input '{}' has empty source and no data source match; left unresolved \
                     because required parameter '{}' was not passed",
                    input.name, missing
                ));
                return Ok(None);
            }
            let unknown = Value::unknown(&law.id, &input.name, MissingKind::NoData);
            res_ctx.trace_set_resolve_type(ResolveType::DataSource);
            res_ctx.trace_set_result(unknown.clone());
            res_ctx.trace_set_message(format!(
                "Input '{}' has no value in any data source: unknown",
                input.name
            ));
            Ok(Some(unknown))
        }
    }

    /// The keys under which `article` is held back when it reads an output of
    /// `regulation` that it changes itself, so the read is the value it
    /// departs from; both when it does both:
    ///
    /// - it replaces that very output ("in afwijking van", RFC-007), or
    /// - it fills in an open term of that law ("gelet op", RFC-003): any read
    ///   of the law gets what it gives without this filling, which is the
    ///   default or the next implementation. Law-wide, because an output of
    ///   another article can depend on the term as much as the output of the
    ///   article declaring it.
    ///
    /// Empty for every other read, and for a void: an output that does not
    /// arise has no value to depart from, and reading it yields the void with
    /// its ground (RFC-041).
    fn read_as_base(
        &self,
        article: &Article,
        law: &ArticleBasedLaw,
        regulation: &str,
        output: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> Vec<String> {
        let mut held = Vec::new();
        let fills_in = article
            .get_implements()
            .is_some_and(|decls| decls.iter().any(|d| d.law == regulation));
        if fills_in {
            held.push(implementation_key(&law.id, &article.number, regulation));
        }
        // Only an article with overrides pays for the lookup of the law
        // producing the output: this runs on every sourced input.
        if article.get_overrides().is_none() {
            return held;
        }
        let replaces = self
            .resolver
            .get_law_for_date(regulation, res_ctx.reference_date())
            .and_then(|target_law| replaced_article(article, target_law, output))
            .is_some();
        if replaces {
            held.push(override_key(&law.id, &article.number));
        }
        held
    }

    /// Internal method for external input resolution with depth tracking.
    fn resolve_external_input_internal(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Value> {
        self.resolve_external_input_detailed(
            regulation,
            output,
            source_parameters,
            context,
            &[],
            res_ctx,
        )
        .map(|resolution| resolution.value)
    }

    /// [`Self::resolve_external_input_internal`], also reporting whether the
    /// target was skipped for a required parameter that named nobody.
    ///
    /// `base_of` names the overrides and implementations (as [`override_key`]
    /// and [`implementation_key`]) whose own article is reading the output it
    /// replaces or fills in. That read is the value it departs from, not the
    /// one it produces: the target is evaluated with it held back, under its
    /// own cycle key, so it is not a cycle when that output is being resolved
    /// already.
    fn resolve_external_input_detailed(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        base_of: &[String],
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<ExternalResolution> {
        // Check for circular reference before proceeding. Every key carries
        // what is held back, so an evaluation inside a base read is not
        // mistaken for one outside it. A base read is keyed on what it holds
        // back, together with what already is: a
        // second base read of the same output inside the first one, by
        // another override or implementation, is another resolution, and a
        // base read re-entering itself is still caught.
        let key = if base_of.is_empty() {
            format!("{}#{}{}", regulation, output, res_ctx.held_back_suffix())
        } else {
            format!(
                "{}#{}{}",
                regulation,
                output,
                res_ctx.held_back_suffix_with(base_of)
            )
        };
        if res_ctx.is_visited(&key) {
            // The key is internal (a base read carries what it holds back);
            // the message names the output.
            let base = if base_of.is_empty() {
                ""
            } else {
                " (base read)"
            };
            return Err(EngineError::CircularReference(format!(
                "Circular cross-law reference detected: {regulation}#{output}{base} is \
                 already being resolved"
            )));
        }

        // Trace cross-law call (guard auto-pops on all exit paths)
        let _guard = res_ctx.trace_guard(
            format!("{}#{}", regulation, output),
            PathNodeType::CrossLawReference,
        );

        if let Err(e) = self.refuse_reduction_call(context.law_id(), regulation, res_ctx) {
            res_ctx.trace_set_message(e.to_string());
            return Err(e);
        }

        // Build parameters for the target article
        let target_params = match self.build_target_parameters(source_parameters, context) {
            Ok(p) => p,
            Err(e) => {
                res_ctx.trace_set_message(format!("Failed to build parameters: {}", e));
                return Err(e);
            }
        };

        // The target's declared parameters and whether each is required. A
        // parameter is required unless it says `required: false`; the flag is
        // what makes the two rules below safe (RFC-036). Only a law the engine
        // knows can be skipped for nobody; a call to a law that is not loaded
        // keeps its LawNotFound error further down. `target_params` holds
        // exactly what the caller passed: an optional parameter the caller
        // omits is not filled in here, the target resolves it as Unknown for
        // lack of that parameter when one of its actions asks for it.
        let law_known = self.get_law(regulation).is_some();
        let target_article = self
            .get_law(regulation)
            .and_then(|law| law.find_article_by_output(output));
        let declared: &[crate::article::Parameter] = target_article
            .map(|article| article.get_parameters())
            .unwrap_or(&[]);
        if let Some(skipped) = Self::skip_call(
            law_known,
            declared,
            &target_params,
            regulation,
            context.law_id(),
            res_ctx,
        )? {
            return Ok(skipped);
        }

        // Enter cross-law resolution scope
        res_ctx.enter(key.clone());
        if !base_of.is_empty() {
            res_ctx.trace_set_message(format!(
                "Base value of {regulation}.{output}: the reading article's own override or \
                 implementation is held back"
            ));
        }
        let newly_held: Vec<&String> = base_of
            .iter()
            .filter(|held| res_ctx.held_back.insert((*held).clone()))
            .collect();

        // Execute the target article
        let result = self.evaluate_law_output_internal(regulation, output, target_params, res_ctx);

        // Leave scope (even on error, for correct cycle tracking)
        for held in newly_held {
            res_ctx.held_back.remove(held);
        }
        res_ctx.leave(&key);

        let value = match result {
            Ok(r) => match r.outputs.get(output).cloned() {
                Some(v) => v,
                None => {
                    let error = missing_output_error(regulation, output, &r.output_provenance);
                    res_ctx.trace_set_message(error.to_string());
                    return Err(error);
                }
            },
            Err(e) => {
                res_ctx.trace_set_message(format!("Execution failed: {}", e));
                return Err(e);
            }
        };

        // Complete trace node
        res_ctx.trace_set_result(value.clone());

        Ok(ExternalResolution {
            value,
            skipped: false,
        })
    }

    /// Resolve an input that calls the decision of an article (RFC-050).
    /// `target` is an article reference, or a `$`-reference to a value that
    /// holds one or that is a decision value. A decision value is called
    /// anew (RFC-050): its `basis` with its own `fields` and `findings`.
    ///
    /// The parameters are those `fields`, then the objects of
    /// `parameters_from`, merged in order, with `parameters` on top; a name
    /// the article does not declare is dropped, and the trace names it,
    /// unless it is the id of an open term the article leaves to an
    /// authority per case (`decided_per_case_by`): that is the finding for
    /// this case, passed as a caller at the top passes it, and kept in
    /// `findings`, apart from the parameters in `fields`. A finding in the
    /// decision value comes first, a name merged on top of it next, and a
    /// finding the caller of the engine gave for this execution wins over
    /// both (RFC-050). The article runs in the version in force on the calculation
    /// date, at stage BESLUIT, as a cross-law call does, and computes the
    /// outputs of its decision item. Without `output` the input is the
    /// decision value `{basis, fields, findings, outputs}`; with it,
    /// that one output.
    ///
    /// Refused at run time: a target that is no article reference, an
    /// article not in force on the date, an article that takes no decision,
    /// an `output` that is not one of that decision, and a merged value that
    /// is no object. A null merged value adds nothing. A target that is null
    /// or unknown, or a merged value that is unknown, is no call: the input
    /// is that null or unknown, as for a call skipped for a parameter
    /// (RFC-036). Cycles are caught on the keys a cross-law call uses, one
    /// per output computed: the article runs once for all outputs of its
    /// decision, so while it runs each of them counts as being resolved.
    ///
    /// The decision value holds the numbers as the article computed them,
    /// as a `source.regulation` call does: nothing normalizes them.
    fn resolve_article_call(
        &self,
        law: &ArticleBasedLaw,
        input: &Input,
        source: &crate::article::Source,
        target: &str,
        context: &RuleContext,
        res_ctx: &mut ResolutionContext<'_>,
    ) -> Result<Option<Value>> {
        let output = source.output.as_deref();
        let caller = format!("input '{}' of {}", input.name, law.id);

        // Which article: written in the law, or read from a value. A
        // decision value is called anew with its own fields and findings.
        let named_by = target;
        let mut passed = BTreeMap::new();
        let mut given_findings = BTreeMap::new();
        let target = match named_by.strip_prefix('$') {
            None => named_by.to_string(),
            Some(path) => match context.resolve(path)? {
                Value::String(s) if regelrecht_law_model::is_article_reference(&s) => s,
                Value::Object(value) => {
                    let (basis, fields, findings) = decision_parts(value, named_by, &caller)?;
                    passed = fields;
                    given_findings = findings;
                    basis
                }
                value @ (Value::Null | Value::Unknown(_)) => {
                    let _guard = res_ctx.trace_guard(named_by, PathNodeType::CrossLawReference);
                    res_ctx.trace_set_message(format!(
                        "{named_by} is {}, so no article is called; input resolves to it",
                        if value.is_null() { "null" } else { "unknown" }
                    ));
                    res_ctx.trace_set_result(value.clone());
                    let origin = format!("skipped call to {named_by}");
                    return checked_call_value(law, input, value, origin);
                }
                other => {
                    return Err(EngineError::TypeMismatch {
                        expected: format!(
                            "an article reference or a decision value in {named_by} for {caller}"
                        ),
                        actual: other.to_string(),
                    })
                }
            },
        };
        let (regulation, number) = target.split_once('#').ok_or_else(|| {
            EngineError::ResolutionError(format!("{caller} calls '{target}', no article"))
        })?;

        let node = match output {
            Some(output) => format!("{target}.{output}"),
            None => target.clone(),
        };
        let _guard = res_ctx.trace_guard(node, PathNodeType::CrossLawReference);
        let refuse = |res_ctx: &ResolutionContext<'_>, message: String| {
            res_ctx.trace_set_message(message.clone());
            Err(EngineError::ResolutionError(message))
        };

        // What is passed: the objects merged in order on the fields of a
        // decision value, `parameters` on top.
        for reference in source.parameters_from.iter().flatten() {
            let path = reference.strip_prefix('$').ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "{caller}: '{reference}' in parameters_from is no $-reference"
                ))
            })?;
            match context.resolve(path)? {
                Value::Object(fields) => passed.extend(fields),
                Value::Null => {}
                unknown @ Value::Unknown(_) => {
                    res_ctx.trace_set_message(format!(
                        "{reference} is unknown, so {target} is not executed; input is unknown \
                         for the same reason"
                    ));
                    res_ctx.trace_set_result(unknown.clone());
                    return Ok(Some(unknown));
                }
                other => {
                    let error = EngineError::TypeMismatch {
                        expected: format!(
                            "an object in {reference}, merged into the parameters of {target} \
                             for {caller}"
                        ),
                        actual: other.to_string(),
                    };
                    res_ctx.trace_set_message(error.to_string());
                    return Err(error);
                }
            }
        }
        passed.extend(self.build_target_parameters(source.parameters.as_ref(), context)?);

        // The article in force on the date, and its decision.
        let (target_law, article) =
            match self.article_in_force(regulation, number, res_ctx.calculation_date) {
                Ok(found) => found,
                // The typed error stays, so a caller can tell a law that is
                // not loaded from one not in force; the trace says who called.
                Err(e) => {
                    res_ctx.trace_set_message(format!("{caller} calls {target}: {e}"));
                    return Err(e);
                }
            };
        let Some(decision) = article.get_decision() else {
            return refuse(
                res_ctx,
                format!("{caller} calls the decision of {target}, which takes no decision"),
            );
        };
        let decided = article.outputs_of(decision);
        let computed: Vec<&str> = match output {
            None => decided.clone(),
            Some(output) if decided.contains(&output) => vec![output],
            Some(output) => {
                return refuse(
                    res_ctx,
                    format!(
                    "{caller} takes '{output}' from {target}, which is no output of its decision"
                ),
                )
            }
        };

        // Only what the article asks; the trace names what it does not. It
        // asks its parameters, and the judgement for this case on an open
        // term it leaves to an authority per case (`decided_per_case_by`),
        // under the term's id, as a caller at the top gives one (RFC-047).
        // A term that a regulation in force on this day fills takes no
        // finding (RFC-048 §2): the decision's finding on it, from a day the
        // regulation did not fill it yet, is not passed, and the trace says
        // so.
        let declared = Self::filter_parameters_for_article(article, &passed);
        let mut findings: BTreeMap<String, Value> = BTreeMap::new();
        let mut now_filled: Vec<String> = Vec::new();
        let filled = self.filled_terms(target_law, article, &passed, res_ctx.reference_date())?;
        for term in article.get_open_terms().into_iter().flatten() {
            if term.decided_per_case_by.is_none() || declared.contains_key(&term.id) {
                continue;
            }
            let Some(value) = passed
                .get(&term.id)
                .or_else(|| given_findings.get(&term.id))
            else {
                continue;
            };
            if filled.contains(&term.id) {
                now_filled.push(term.id.clone());
            } else {
                findings.insert(term.id.clone(), value.clone());
            }
        }
        if !now_filled.is_empty() {
            let _filled = res_ctx.trace_guard(
                format!("{target} (filled by policy)"),
                PathNodeType::Resolve,
            );
            res_ctx.trace_set_message(format!(
                "{target}: a regulation in force fills {}, so the finding is not passed",
                now_filled.join(", ")
            ));
        }
        let from_caller = res_ctx.findings_for(target_law, article, &filled)?;
        findings.extend(from_caller.clone());
        // A finding of the decision value on a term of another article the
        // call runs, such as the open term a policy declares for a case its
        // cell is silent on (RFC-048 §2), applies there as a caller's
        // finding does, for this call only. A finding the caller gave wins.
        let nested: BTreeMap<String, Value> = given_findings
            .iter()
            .filter(|(name, _)| {
                !findings.contains_key(*name)
                    && !declared.contains_key(*name)
                    && !res_ctx.findings.contains_key(*name)
                    && !now_filled.contains(*name)
            })
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        let dropped: Vec<&str> = passed
            .keys()
            .chain(given_findings.keys())
            .filter(|name| {
                !declared.contains_key(*name)
                    && !findings.contains_key(*name)
                    && !nested.contains_key(*name)
                    && !now_filled.contains(*name)
            })
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if !from_caller.is_empty() {
            let _found = res_ctx.trace_guard(
                format!("{target} (found for this case)"),
                PathNodeType::Resolve,
            );
            res_ctx.trace_set_message(format!(
                "{target}: the finding given for this execution on {} wins",
                from_caller
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !findings.is_empty() {
            let _judged =
                res_ctx.trace_guard(format!("{target} (judged per case)"), PathNodeType::Resolve);
            res_ctx.trace_set_message(format!(
                "{target} leaves {} to an authority per case, so passed as its finding",
                findings
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !dropped.is_empty() {
            let _dropped =
                res_ctx.trace_guard(format!("{target} (not asked)"), PathNodeType::Resolve);
            res_ctx.trace_set_message(format!(
                "{target} does not declare {}, so not passed",
                dropped.join(", ")
            ));
        }
        // The skip rule reads the declared parameters: a judgement that is
        // null is a judgement, not a missing party, as at the top.
        if let Some(skipped) = Self::skip_call(
            true,
            article.get_parameters(),
            &declared,
            &target,
            &law.id,
            res_ctx,
        )? {
            return checked_call_value(
                law,
                input,
                skipped.value,
                format!("skipped call to {target}"),
            );
        }
        // The article runs with both; the decision value keeps them apart.
        let mut passed_on = declared.clone();
        passed_on.extend(findings.clone());

        // The keys of a cross-law call, one per output computed, and one for
        // the article itself, which also counts the depth.
        let suffix = res_ctx.held_back_suffix();
        let article_key = format!("article:{target}{suffix}");
        let output_keys: Vec<String> = computed
            .iter()
            .map(|name| format!("{regulation}#{name}{suffix}"))
            .collect();
        if let Some(cycle) = std::iter::once(&article_key)
            .chain(&output_keys)
            .find(|key| res_ctx.is_visited(key))
        {
            let what = cycle.split('\0').next().unwrap_or(cycle);
            let what = match what.strip_prefix("article:") {
                Some(article) => format!("article {article}"),
                None => what.to_string(),
            };
            let message =
                format!("Circular cross-law reference detected: {what} is already being resolved");
            res_ctx.trace_set_message(message.clone());
            return Err(EngineError::CircularReference(message));
        }
        res_ctx.enter(article_key.clone());
        for key in &output_keys {
            res_ctx.visited.insert(key.clone());
        }
        // The article runs at one stage of its procedure, as when a cell
        // takes the decision (`execute_stage_at`): the hooks on its
        // submission wait for the stage named after it (RFC-047).
        let outer_staged = res_ctx
            .staged_article
            .replace((target_law.id.clone(), article.number.clone()));
        // The decision value's findings that an article the call runs asked:
        // they are findings of this decision, as the article's own are.
        let mut applied: BTreeMap<String, Value> = BTreeMap::new();
        let result = if res_ctx.depth > config::MAX_CROSS_LAW_DEPTH {
            Err(EngineError::CircularReference(format!(
                "Cross-law resolution depth exceeded {} levels. Possible circular reference \
                 involving {target}",
                config::MAX_CROSS_LAW_DEPTH
            )))
        } else {
            for (name, value) in &nested {
                res_ctx.findings.insert(name.clone(), value.clone());
            }
            let asked_before = res_ctx.findings_asked.clone();
            let result = self.evaluate_article_with_service(
                article,
                target_law,
                passed_on,
                Some(&computed),
                "BESLUIT",
                res_ctx,
            );
            // A finding the caller gave that an article of the call read
            // first, such as one on a policy's own term, is a finding of
            // this decision too. One only noted for a policy the case did
            // not reach is not.
            // One already in `findings` or `nested` lands with the same value.
            for name in res_ctx.findings_asked.difference(&asked_before) {
                if let Some(value) = res_ctx.findings.get(name) {
                    applied.insert(name.clone(), value.clone());
                }
            }
            let mut unasked: Vec<&str> = Vec::new();
            for (name, value) in &nested {
                res_ctx.findings.remove(name);
                let noted = res_ctx.findings_noted.remove(name);
                if res_ctx.findings_asked.remove(name) {
                    applied.insert(name.clone(), value.clone());
                } else if !noted {
                    unasked.push(name);
                }
            }
            if !applied.is_empty() {
                let _nested = res_ctx.trace_guard(
                    format!("{target} (found for the articles it runs)"),
                    PathNodeType::Resolve,
                );
                res_ctx.trace_set_message(format!(
                    "{target}: the finding on {} applies to an article the call runs",
                    applied
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !unasked.is_empty() {
                let _dropped =
                    res_ctx.trace_guard(format!("{target} (not asked)"), PathNodeType::Resolve);
                res_ctx.trace_set_message(format!(
                    "{target} does not declare {}, so not passed",
                    unasked.join(", ")
                ));
            }
            result
        };
        res_ctx.staged_article = outer_staged;
        for key in &output_keys {
            res_ctx.visited.remove(key);
        }
        res_ctx.leave(&article_key);
        let result = match result {
            Ok(result) => result,
            Err(e) => {
                res_ctx.trace_set_message(format!("Execution failed: {e}"));
                return Err(e);
            }
        };

        let pick = |name: &str| {
            result.outputs.get(name).cloned().ok_or_else(|| {
                let error = missing_output_error(regulation, name, &result.output_provenance);
                res_ctx.trace_set_message(error.to_string());
                error
            })
        };
        let value = match output {
            Some(output) => pick(output)?,
            None => {
                let outputs = computed
                    .iter()
                    .map(|name| Ok((name.to_string(), pick(name)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?;
                Value::Object(BTreeMap::from([
                    ("basis".to_string(), Value::String(target.clone())),
                    ("fields".to_string(), Value::Object(declared)),
                    (
                        "findings".to_string(),
                        Value::Object(findings.into_iter().chain(applied).collect()),
                    ),
                    ("outputs".to_string(), Value::Object(outputs)),
                ]))
            }
        };
        res_ctx.trace_set_result(value.clone());
        checked_call_value(
            law,
            input,
            value,
            format!("{regulation}.{}", output.unwrap_or(number)),
        )
    }

    /// The skip rule of a call to another law (RFC-036), on the parameters
    /// `target_params` it would pass to a target that declares `declared`:
    /// `Some` with the value the input takes when the call is not made, an
    /// error for a null its declaration rules out, `None` to make the call.
    /// `law_known` says whether the target's law is loaded; a call to a law
    /// that is not keeps its own error. `caller` names the calling law.
    fn skip_call(
        law_known: bool,
        declared: &[crate::article::Parameter],
        target_params: &BTreeMap<String, Value>,
        regulation: &str,
        caller: &str,
        res_ctx: &ResolutionContext<'_>,
    ) -> Result<Option<ExternalResolution>> {
        let declared_parameter = |name: &str| declared.iter().find(|p| p.name == name);
        let is_required =
            |name: &str| declared_parameter(name).is_none_or(|p| p.required != Some(false));
        let is_nullable = |name: &str| declared_parameter(name).is_some_and(|p| p.is_nullable());

        // A null value for a required parameter means there is nobody to look
        // up: a partner's age when there is no partner, a child's data when
        // there is no child. The referenced law cannot be executed for nobody,
        // and failing here would fail the whole calculation for a person to
        // whom that branch does not apply. The input resolves to null instead
        // and the law's own absence checks decide what that means. Unless the
        // target declares that parameter nullable: then it said it can decide
        // on nobody, and it is run with the null, the same as at the top
        // level (RFC-036).
        //
        // An unknown value for a required parameter means nobody knows yet who
        // to look up: the partner's BSN is a fact the register has not
        // delivered. The referenced law is not executed either, and the input
        // is unknown for the same facts, so the outcome still names them.
        // `nullable` says nothing about unknown.
        //
        // An optional parameter that is null or unknown is passed through: the
        // target declared it knows how to do without.
        //
        // With more than one such parameter the outcome does not depend on
        // their order: if any is unknown, the input is unknown for the union
        // of their facts (a null among them adds nothing to ask for); only
        // when all are null is the input null. The trace names every one.
        let empty_required: Vec<(&String, &Value)> = target_params
            .iter()
            .filter(|(name, v)| {
                law_known
                    && is_required(name)
                    && (v.is_unknown() || (v.is_null() && !is_nullable(name)))
            })
            .collect();
        if !empty_required.is_empty() {
            let names = empty_required
                .iter()
                .map(|(name, _)| format!("'{name}'"))
                .collect::<Vec<_>>()
                .join(", ");
            let (noun, verb) = if empty_required.len() == 1 {
                ("Parameter", "is")
            } else {
                ("Parameters", "are")
            };
            if let Some(unknown) = Value::merge_unknown(empty_required.iter().map(|(_, v)| *v)) {
                res_ctx.trace_set_message(format!(
                    "{noun} {names} {verb} unknown, so {regulation} is not executed; input is unknown for the same reason",
                ));
                res_ctx.trace_set_result(unknown.clone());
                return Ok(Some(ExternalResolution {
                    value: unknown,
                    skipped: true,
                }));
            }
            res_ctx.trace_set_message(format!(
                "{noun} {names} {verb} null, so {regulation} is not executed; input resolves to null",
            ));
            res_ctx.trace_set_result(Value::Null);
            return Ok(Some(ExternalResolution {
                value: Value::Null,
                skipped: true,
            }));
        }

        // An optional parameter passed as null is passed through (the target
        // declared it knows how to do without), but only if the target also
        // declared that "none" is a value it takes. Otherwise the caller
        // hands the target an absence its type rules out, and that is caught
        // here at the boundary rather than inside the target (RFC-036).
        if let Some(parameter) = declared.iter().find(|p| {
            p.required == Some(false)
                && !p.is_nullable()
                && target_params.get(&p.name).is_some_and(Value::is_null)
        }) {
            let error = EngineError::NullForNonNullable {
                law_id: regulation.to_string(),
                field: parameter.name.clone(),
                origin: format!("parameter from {caller}"),
            };
            res_ctx.trace_set_message(format!("Failed to build parameters: {error}"));
            return Err(error);
        }

        Ok(None)
    }

    /// Filter execution parameters to only those declared by the target article.
    ///
    /// When resolving open terms, we don't want to forward all parameters from
    /// the calling context (which may include sensitive data like BSN). Instead,
    /// we only pass parameters that the implementing article declares in its
    /// execution.parameters section.
    fn filter_parameters_for_article(
        article: &Article,
        all_params: &BTreeMap<String, Value>,
    ) -> BTreeMap<String, Value> {
        let Some(exec) = article.get_execution_spec() else {
            return BTreeMap::new();
        };
        let Some(declared_params) = &exec.parameters else {
            return BTreeMap::new();
        };

        let mut filtered = BTreeMap::new();
        for param in declared_params {
            if let Some(value) = all_params.get(&param.name) {
                filtered.insert(param.name.clone(), value.clone());
            }
        }
        filtered
    }

    /// Build parameters for a target article from source parameter mapping.
    fn build_target_parameters(
        &self,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
    ) -> Result<BTreeMap<String, Value>> {
        let mut params = BTreeMap::new();

        if let Some(param_map) = source_parameters {
            for (target_name, source_ref) in param_map {
                // Source ref can be "$variable" or a literal
                let value = if let Some(var_name) = source_ref.strip_prefix('$') {
                    context.resolve(var_name)?
                } else {
                    // Literal value (as string)
                    Value::String(source_ref.clone())
                };
                params.insert(target_name.clone(), value);
            }
        }

        Ok(params)
    }

    /// List all loaded law IDs.
    pub fn list_laws(&self) -> Vec<&str> {
        self.resolver.list_laws()
    }

    /// Get the number of loaded laws.
    pub fn law_count(&self) -> usize {
        self.resolver.law_count()
    }

    /// Check if a law is loaded.
    pub fn has_law(&self, law_id: &str) -> bool {
        self.resolver.has_law(law_id)
    }

    /// Unload a law.
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        self.resolver.unload_law(law_id)
    }

    /// Get direct access to the resolver.
    pub fn resolver(&self) -> &RuleResolver {
        &self.resolver
    }

    /// Version-aware law lookup for annotation resolution (RFC-005/RFC-018).
    ///
    /// `valid_from` names the version the caller is looking at (that
    /// version's own `valid_from` date), so a note resolves against the text
    /// on screen instead of whichever version happens to be newest —
    /// [`ServiceProvider::get_law`] returns the newest loaded version,
    /// including one that is not yet in force. `None` keeps the
    /// latest-version behaviour for callers without a version context.
    ///
    /// Failing loudly beats a silent wrong answer (RFC-019), twice over: a
    /// malformed date is an error rather than a fall-back to the newest
    /// version, and the selected version must carry exactly the requested
    /// `valid_from`. The date-based selection alone would otherwise slide to
    /// the next-older *loaded* version when the viewed one is absent from the
    /// engine (the editor's dependency loader skips versions the engine
    /// cannot parse), and the returned offsets would mis-highlight the text
    /// on screen without any signal.
    pub fn get_law_version(
        &self,
        law_id: &str,
        valid_from: Option<&str>,
    ) -> Result<&ArticleBasedLaw> {
        let reference_date = valid_from.map(parse_calculation_date).transpose()?;
        let law = self
            .resolver
            .get_law_for_date_reported(law_id, reference_date)
            .map_err(|reason| selection_error(law_id, valid_from.unwrap_or(""), reason))?;
        if let Some(requested) = valid_from {
            if law.valid_from.as_deref() != Some(requested) {
                return Err(EngineError::LoadError(format!(
                    "law '{law_id}' has no loaded version with valid_from {requested}; \
                     the viewed version may have failed to load"
                )));
            }
        }
        Ok(law)
    }

    /// Get metadata about a loaded law.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    ///
    /// # Returns
    /// `LawInfo` with metadata, or `None` if the law is not loaded.
    pub fn get_law_info(&self, law_id: &str) -> Option<LawInfo> {
        let law = self.resolver.get_law(law_id)?;

        // Collect output names from all articles
        let mut outputs = Vec::new();
        for article in &law.articles {
            if let Some(exec) = article.get_execution_spec() {
                if let Some(output_specs) = &exec.output {
                    for output in output_specs {
                        outputs.push(output.name.clone());
                    }
                }
            }
        }

        Some(LawInfo {
            id: law.id.clone(),
            regulatory_layer: law.regulatory_layer,
            publication_date: law.publication_date.clone(),
            bwb_id: law.bwb_id.clone(),
            url: law.url.clone(),
            outputs,
            article_count: law.articles.len(),
        })
    }

    /// List all (law_id, output_name) pairs across all loaded laws.
    pub fn list_all_outputs(&self) -> Vec<(&str, &str)> {
        self.resolver.list_all_outputs()
    }

    /// Get the total number of outputs across all loaded laws.
    pub fn get_output_count(&self) -> usize {
        self.resolver.output_count()
    }

    // -------------------------------------------------------------------------
    // Data Source Management
    // -------------------------------------------------------------------------

    /// Add a data source to the registry.
    ///
    /// Data sources are queried in priority order (highest first) during
    /// input resolution, before falling back to cross-law resolution.
    pub fn add_data_source(&mut self, source: Box<dyn DataSource>) {
        self.data_registry.add_source(source);
    }

    /// Add a dictionary-based data source.
    ///
    /// Convenience method for adding a `DictDataSource` with the given data.
    ///
    /// # Arguments
    /// * `name` - Name identifier for this data source
    /// * `priority` - Priority for resolution order (higher = checked first)
    /// * `data` - Data as record_key -> field_name -> value
    pub fn add_dict_source(
        &mut self,
        name: impl Into<String>,
        priority: i32,
        data: BTreeMap<String, BTreeMap<String, Value>>,
    ) {
        self.data_registry
            .add_source(Box::new(DictDataSource::new(name, priority, data)));
    }

    /// Remove a data source by name.
    pub fn remove_data_source(&mut self, name: &str) -> bool {
        self.data_registry.remove_source(name)
    }

    /// Clear all data sources from the registry.
    pub fn clear_data_sources(&mut self) {
        self.data_registry.clear();
    }

    /// Register a dictionary data source from flat records.
    ///
    /// # Arguments
    /// * `name` - Name identifier for this data source
    /// * `key_field` - Field name to use as the record key (case-insensitive)
    /// * `records` - List of records as field -> value maps
    pub fn register_dict_source(
        &mut self,
        name: &str,
        key_field: &str,
        records: Vec<BTreeMap<String, Value>>,
    ) -> Result<()> {
        match DictDataSource::from_records(name, 10, key_field, records) {
            Some(source) => {
                self.data_registry.add_source(Box::new(source));
                Ok(())
            }
            None => Err(EngineError::DataSourceError(format!(
                "Key field '{}' not found in records for source '{}'",
                key_field, name
            ))),
        }
    }

    /// Register a dictionary data source that answers only for one law.
    ///
    /// The caller has resolved the external data for `law_id` outside the
    /// YAML (the `source: {}` contract) and hands it over per record key, one
    /// field per input name. Because the source is bound to `law_id`, a raw
    /// register value can never shadow a same-named cross-law input of another
    /// law. `priority` orders competing sources for the same law: a set of
    /// citizen corrections registered above the register data overrides it.
    pub fn register_dict_source_for_law(
        &mut self,
        law_id: &str,
        name: &str,
        key_field: &str,
        records: Vec<BTreeMap<String, Value>>,
        priority: i32,
    ) -> Result<()> {
        match DictDataSource::from_records(name, priority, key_field, records) {
            Some(source) => {
                self.data_registry
                    .add_source(Box::new(source.with_law_scope(law_id)));
                Ok(())
            }
            None => Err(EngineError::DataSourceError(format!(
                "Key field '{}' not found in records for source '{}' (law '{}')",
                key_field, name, law_id
            ))),
        }
    }

    /// Get the number of registered data sources.
    pub fn data_source_count(&self) -> usize {
        self.data_registry.source_count()
    }

    /// List all registered data source names.
    pub fn list_data_sources(&self) -> Vec<&str> {
        self.data_registry.list_sources()
    }

    /// Get direct access to the data registry.
    pub fn data_registry(&self) -> &DataSourceRegistry {
        &self.data_registry
    }
}

impl ServiceProvider for LawExecutionService {
    fn evaluate_uri(
        &self,
        uri: &str,
        parameters: &BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        let parsed = RegelrechtUri::parse(uri)?;
        self.evaluate_law_output(
            parsed.law_id(),
            parsed.output(),
            parameters.clone(),
            calculation_date,
        )
    }

    fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw> {
        self.resolver.get_law(law_id)
    }

    fn resolve_external_input(
        &self,
        regulation: &str,
        output: &str,
        source_parameters: Option<&BTreeMap<String, String>>,
        context: &RuleContext,
        calculation_date: &str,
    ) -> Result<Value> {
        let mut res_ctx = ResolutionContext::new(calculation_date)?;
        self.resolve_external_input_internal(
            regulation,
            output,
            source_parameters,
            context,
            &mut res_ctx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decision's stages compute the data derived for it too, the
    /// outputs it lists under `derived` (RFC-050); the stage of the
    /// submission only what the submission lists.
    #[test]
    fn the_decision_stages_compute_the_derived_outputs() {
        let law = ArticleBasedLaw::from_yaml_str(
            r#"
$id: afgeleid
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            outputs: [compleet]
          - type: BESCHIKKING
            outputs: [verleend]
            derived: [afgeleid]
        output:
          - name: compleet
            type: boolean
          - name: verleend
            type: boolean
          - name: afgeleid
            type: boolean
        actions:
          - output: compleet
            value: true
          - output: verleend
            value: true
          - output: afgeleid
            value: false
"#,
        )
        .unwrap();
        let article = &law.articles[0];
        assert_eq!(
            outputs_at_stage(article, "AANVRAAG"),
            Some(vec!["compleet"])
        );
        assert_eq!(
            outputs_at_stage(article, "BESLUIT"),
            Some(vec!["verleend", "afgeleid"])
        );
    }
    use crate::article::LawLoad;
    use crate::types::MissingFact;

    /// Cycle detection enters every hook and override under its own key: a
    /// shared one would read a second, different hook nested in a first as
    /// the first one again, and skip it.
    #[test]
    fn every_hook_and_override_has_its_own_cycle_key() {
        let keys = [
            hook_key("wet_a", "1"),
            hook_key("wet_a", "2"),
            hook_key("wet_b", "1"),
            override_key("wet_a", "1"),
            override_key("wet_a", "2"),
            override_key("wet_b", "1"),
        ];
        let distinct: std::collections::BTreeSet<&String> = keys.iter().collect();
        assert_eq!(distinct.len(), keys.len(), "{keys:?}");
    }

    fn make_base_law() -> &'static str {
        r#"
$id: base_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Provides a base value
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 100
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#
    }

    fn make_dependent_law() -> &'static str {
        r#"
$id: dependent_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses value from another law
    machine_readable:
      execution:
        input:
          - name: external_base
            type: number
            source:
              regulation: base_law
              output: base_value
        output:
          - name: doubled_value
            type: number
        actions:
          - output: doubled_value
            operation: MULTIPLY
            values:
              - $external_base
              - 2
"#
    }

    /// A `base_law` version stamped with `valid_from` and a distinguishing
    /// `base_value`, so a test can assert *which* version the resolver picked.
    fn base_law_version(valid_from: &str, value: i32) -> String {
        format!(
            r#"
$id: base_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '{valid_from}'
articles:
  - number: '1'
    text: Provides a base value
    machine_readable:
      definitions:
        BASE_VALUE:
          value: {value}
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#
        )
    }

    // -------------------------------------------------------------------------
    // Basic Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_basic() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        assert!(service.has_law("base_law"));
        assert_eq!(service.law_count(), 1);

        let laws = service.list_laws();
        assert_eq!(laws, vec!["base_law"]);
    }

    #[test]
    fn test_service_execute_simple() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let result = service
            .evaluate_law_output("base_law", "base_value", BTreeMap::new(), "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("base_value"), Some(&Value::Int(100)));
    }

    // -------------------------------------------------------------------------
    // Cross-Law Resolution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_cross_law_resolution() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();

        // Execute dependent law - should automatically resolve base_law reference
        let result = service
            .evaluate_law_output(
                "dependent_law",
                "doubled_value",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        // doubled_value = base_value (100) * 2 = 200
        assert_eq!(result.outputs.get("doubled_value"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_service_cross_law_resolution_picks_in_force_version_over_future() {
        // Regression for the temporal-federation bug: a referenced law that has
        // BOTH an in-force and a future-dated version must resolve the in-force
        // one when executed as-of a date before the future version. This is the
        // version *set* the scenario loader now hands the engine; before the
        // fix the loader fed only the single "best for today" version (the
        // future one in a federated corpus) and execution failed NotYetInForce.
        let mut service = LawExecutionService::new();
        // Load future first to prove order doesn't matter (resolver sorts).
        service
            .load_law(&base_law_version("2099-01-01", 999))
            .unwrap();
        service
            .load_law(&base_law_version("2025-01-01", 100))
            .unwrap();
        service.load_law(make_dependent_law()).unwrap();

        let result = service
            .evaluate_law_output(
                "dependent_law",
                "doubled_value",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();

        // Uses the in-force base_value (100 * 2), NOT the future one (999 * 2).
        assert_eq!(result.outputs.get("doubled_value"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_service_cross_law_only_future_version_is_not_yet_in_force() {
        // Control: with ONLY the future version loaded — the pre-fix behaviour
        // where the loader handed the engine a single future-dated version —
        // the same as-of-2025 execution fails NotYetInForce. This proves the
        // fix is the version *set* reaching the engine, not `select_in` itself.
        let mut service = LawExecutionService::new();
        service
            .load_law(&base_law_version("2099-01-01", 999))
            .unwrap();
        service.load_law(make_dependent_law()).unwrap();

        let result = service.evaluate_law_output(
            "dependent_law",
            "doubled_value",
            BTreeMap::new(),
            "2025-06-01",
        );

        assert!(
            matches!(result, Err(EngineError::LawNotYetInForce { .. })),
            "expected LawNotYetInForce, got: {result:?}",
        );
    }

    #[test]
    fn test_service_missing_dependency() {
        let mut service = LawExecutionService::new();
        // Only load dependent law, not base law
        service.load_law(make_dependent_law()).unwrap();

        let result = service.evaluate_law_output(
            "dependent_law",
            "doubled_value",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::LawNotFound(_))),
            "Expected LawNotFound error, got: {:?}",
            result
        );
    }

    // -------------------------------------------------------------------------
    // URI Resolution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_uri_resolution() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let result = service
            .evaluate_uri(
                "regelrecht://base_law/base_value",
                &BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(result.outputs.get("base_value"), Some(&Value::Int(100)));
    }

    // -------------------------------------------------------------------------
    // Circular Reference Detection Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_cross_law_circular_reference() {
        // Law A references Law B, which references Law A
        let law_a = r#"
$id: law_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References law_b
    machine_readable:
      execution:
        input:
          - name: from_b
            type: number
            source:
              regulation: law_b
              output: output_b
        output:
          - name: output_a
            type: number
        actions:
          - output: output_a
            value: $from_b
"#;

        let law_b = r#"
$id: law_b
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References law_a
    machine_readable:
      execution:
        input:
          - name: from_a
            type: number
            source:
              regulation: law_a
              output: output_a
        output:
          - name: output_b
            type: number
        actions:
          - output: output_b
            value: $from_a
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law_a).unwrap();
        service.load_law(law_b).unwrap();

        let result =
            service.evaluate_law_output("law_a", "output_a", BTreeMap::new(), "2025-01-01");

        match result {
            Err(EngineError::CircularReference(msg)) => {
                // The cycle guard has to catch this on re-entry. Reporting an
                // exhausted depth budget instead would send the reader looking
                // for a long chain that is not there.
                assert!(
                    msg.contains("already being resolved"),
                    "Expected the cycle itself to be reported, got: {}",
                    msg
                );
            }
            other => panic!("Expected CircularReference error, got: {:?}", other),
        }
    }

    /// A policy doubling what the statute says: the overriding article reads
    /// the output it overrides. That read is the statute's own value, whether
    /// the override fires on a read of the statute or is asked directly, and
    /// the override is applied once, not again to its own input. A later
    /// action of the overridden article reads the replaced value, like every
    /// other reader does, and a base value never reaches the cache.
    #[test]
    fn an_override_reading_what_it_overrides_gets_the_base_value() {
        let wet = r#"
$id: boterhammenwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Men eet twee boterhammen.
    machine_readable:
      execution:
        output:
          - name: aantal_boterhammen
            type: number
          - name: plakjes_kaas
            type: number
        actions:
          - output: aantal_boterhammen
            value: 2
          - output: plakjes_kaas
            value: $aantal_boterhammen
"#;
        let beleid = r#"
$id: beleidsregel_honger
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking van artikel 1 eet wie honger heeft twee keer zoveel.
    machine_readable:
      overrides:
        - law: boterhammenwet
          article: '1'
          output: aantal_boterhammen
      execution:
        parameters:
          - name: heeft_honger
            type: boolean
        input:
          - name: wettelijk_aantal
            type: number
            source:
              regulation: boterhammenwet
              output: aantal_boterhammen
              parameters:
                heeft_honger: $heeft_honger
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $heeft_honger
                    value: true
                  then:
                    operation: MULTIPLY
                    values:
                      - 2
                      - $wettelijk_aantal
              default: $wettelijk_aantal
  - number: '2'
    text: Het aantal van vandaag is dat van artikel 1 van de Boterhammenwet.
    machine_readable:
      execution:
        parameters:
          - name: heeft_honger
            type: boolean
        input:
          - name: wettelijk_aantal
            type: number
            source:
              regulation: boterhammenwet
              output: aantal_boterhammen
              parameters:
                heeft_honger: $heeft_honger
          - name: kaas
            type: number
            source:
              regulation: boterhammenwet
              output: plakjes_kaas
              parameters:
                heeft_honger: $heeft_honger
        output:
          - name: boterhammen_vandaag
            type: number
          - name: kaas_vandaag
            type: number
          - name: samen
            type: number
        actions:
          - output: boterhammen_vandaag
            value: $wettelijk_aantal
          - output: kaas_vandaag
            value: $kaas
          - output: samen
            value:
              operation: ADD
              values: [$kaas, $wettelijk_aantal]
"#;
        let mut service = LawExecutionService::new();
        service.load_law(wet).unwrap();
        service.load_law(beleid).unwrap();

        for (output, honger, expected) in [
            ("boterhammen_vandaag", true, 4),
            ("boterhammen_vandaag", false, 2),
            ("kaas_vandaag", true, 4),
            ("kaas_vandaag", false, 2),
            // The kaas read makes a base read of aantal_boterhammen (2); the
            // read after it must not be served that value from the cache.
            ("samen", true, 8),
            ("samen", false, 4),
            ("aantal_boterhammen", true, 4),
            ("aantal_boterhammen", false, 2),
        ] {
            let params = BTreeMap::from([("heeft_honger".to_string(), Value::Bool(honger))]);
            let result = service
                .evaluate_law_output("beleidsregel_honger", output, params, "2025-01-01")
                .unwrap_or_else(|e| panic!("{output} (honger={honger}) failed: {e}"));
            assert_eq!(
                result.outputs.get(output),
                Some(&Value::Int(expected)),
                "{output} (honger={honger})"
            );
        }
    }

    /// The same policy as a filling of an open term: the law says two unless
    /// policy says otherwise, and the policy doubles what the law says. The
    /// implementing article's read of the law is the law without it (its
    /// default), and the filling applies whether the law or the policy is
    /// asked (RFC-003).
    #[test]
    fn an_implementation_reading_what_it_fills_in_gets_the_default() {
        let wet = r#"
$id: boterhammenwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Men eet twee boterhammen, tenzij bij beleid anders is bepaald.
    machine_readable:
      open_terms:
        - id: aantal_boterhammen
          type: number
          required: true
          default:
            actions:
              - output: aantal_boterhammen
                value: 2
      execution:
        output:
          - name: aantal_boterhammen
            type: number
          - name: plakjes_kaas
            type: number
        actions:
          - output: aantal_boterhammen
            value: $aantal_boterhammen
          - output: plakjes_kaas
            value: $aantal_boterhammen
"#;
        let beleid = r#"
$id: beleidsregel_honger
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Gelet op artikel 1 eet wie honger heeft twee keer zoveel.
    machine_readable:
      implements:
        - law: boterhammenwet
          article: '1'
          open_term: aantal_boterhammen
      execution:
        parameters:
          - name: heeft_honger
            type: boolean
        input:
          - name: wettelijk_aantal
            type: number
            source:
              regulation: boterhammenwet
              output: aantal_boterhammen
              parameters:
                heeft_honger: $heeft_honger
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $heeft_honger
                    value: true
                  then:
                    operation: MULTIPLY
                    values: [2, $wettelijk_aantal]
              default: $wettelijk_aantal
"#;
        let mut service = LawExecutionService::new();
        service.load_law(wet).unwrap();
        service.load_law(beleid).unwrap();

        for (law, output, honger, expected) in [
            ("boterhammenwet", "plakjes_kaas", true, 4),
            ("boterhammenwet", "plakjes_kaas", false, 2),
            ("beleidsregel_honger", "aantal_boterhammen", true, 4),
            ("beleidsregel_honger", "aantal_boterhammen", false, 2),
        ] {
            let params = BTreeMap::from([("heeft_honger".to_string(), Value::Bool(honger))]);
            let result = service
                .evaluate_law_output(law, output, params, "2025-01-01")
                .unwrap_or_else(|e| panic!("{law}.{output} (honger={honger}) failed: {e}"));
            assert_eq!(
                result.outputs.get(output),
                Some(&Value::Int(expected)),
                "{law}.{output} (honger={honger})"
            );
        }
    }

    /// A law with an open term (default 2), for the stacked base-read tests.
    const BOTERHAMMENWET_OPEN: &str = r#"
$id: boterhammenwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Men eet twee boterhammen, tenzij anders is bepaald.
    machine_readable:
      open_terms:
        - id: aantal_boterhammen
          type: number
          required: true
          default:
            actions:
              - output: aantal_boterhammen
                value: 2
      execution:
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value: $aantal_boterhammen
"#;

    /// An article of `id` on `layer` that declares `relation` (an `overrides`
    /// or `implements` block) and yields `operation` applied to what it reads
    /// of `boterhammenwet.aantal_boterhammen` (`$basis`), under `output`.
    fn reading_rule(
        id: &str,
        layer: &str,
        relation: &str,
        output: &str,
        operation: &str,
    ) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: {layer}
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Regel die leest waar hij van afwijkt.
    machine_readable:
{relation}
      execution:
        input:
          - name: basis
            type: number
            source:
              regulation: boterhammenwet
              output: aantal_boterhammen
        output:
          - name: {output}
            type: number
        actions:
          - output: {output}
            value:
{operation}
"#
        )
    }

    const IMPLEMENTS_AANTAL: &str = "      implements:
        - law: boterhammenwet
          article: '1'
          open_term: aantal_boterhammen";
    const DOUBLE_BASIS: &str = "              operation: MULTIPLY
              values: [2, $basis]";
    const BASIS_PLUS_ONE: &str = "              operation: ADD
              values: [$basis, 1]";

    /// Two implementations of one open term, both reading the law they fill
    /// in: the winner's base read is the law with the runner-up's filling,
    /// whose own base read is the default. Stacked base reads of the same
    /// output are not a cycle.
    #[test]
    fn stacked_base_reads_of_one_output_are_not_a_cycle() {
        let mut service = LawExecutionService::new();
        service.load_law(BOTERHAMMENWET_OPEN).unwrap();
        service
            .load_law(&reading_rule(
                "regeling_boterhammen",
                "MINISTERIELE_REGELING",
                IMPLEMENTS_AANTAL,
                "aantal_boterhammen",
                DOUBLE_BASIS,
            ))
            .unwrap();
        service
            .load_law(&reading_rule(
                "beleid_boterhammen",
                "BELEIDSREGEL",
                IMPLEMENTS_AANTAL,
                "aantal_boterhammen",
                BASIS_PLUS_ONE,
            ))
            .unwrap();

        // Default 2, the policy adds one (3), the regulation ranks higher
        // and doubles what the law says without it (6).
        let result = service
            .evaluate_law_output(
                "boterhammenwet",
                "aantal_boterhammen",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("aantal_boterhammen"),
            Some(&Value::Int(6))
        );
    }

    /// An override and an implementation on the same article, both reading
    /// base, asked through the overriding law: the override departs from the
    /// filled-in law (3), the filling from the default (2).
    #[test]
    fn an_override_and_an_implementation_both_reading_base() {
        let overriding = format!(
            "{}  - number: '2'
    text: Het aantal van vandaag.
    machine_readable:
      execution:
        input:
          - name: aantal
            type: number
            source:
              regulation: boterhammenwet
              output: aantal_boterhammen
        output:
          - name: vandaag
            type: number
        actions:
          - output: vandaag
            value: $aantal
",
            reading_rule(
                "beleid_boterhammen",
                "BELEIDSREGEL",
                "      overrides:
        - law: boterhammenwet
          article: '1'
          output: aantal_boterhammen",
                "aantal_boterhammen",
                DOUBLE_BASIS,
            )
        );
        let mut service = LawExecutionService::new();
        service.load_law(BOTERHAMMENWET_OPEN).unwrap();
        service
            .load_law(&reading_rule(
                "regeling_boterhammen",
                "MINISTERIELE_REGELING",
                IMPLEMENTS_AANTAL,
                "aantal_boterhammen",
                BASIS_PLUS_ONE,
            ))
            .unwrap();
        service.load_law(&overriding).unwrap();

        let result = service
            .evaluate_law_output(
                "beleid_boterhammen",
                "vandaag",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(result.outputs.get("vandaag"), Some(&Value::Int(6)));
    }

    /// An overridden output assigned twice is replaced once, after its last
    /// assignment, and every reader reads that value: also an action declared
    /// between the two assignments, because readers run after all of them.
    #[test]
    fn an_output_assigned_twice_is_replaced_after_its_last_assignment() {
        let wet = r#"
$id: boterhammenwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Men eet een boterham, en daarna twee.
    machine_readable:
      execution:
        output:
          - name: aantal_boterhammen
            type: number
          - name: tussenstand
            type: number
          - name: plakjes_kaas
            type: number
        actions:
          - output: aantal_boterhammen
            value: 1
          - output: tussenstand
            value: $aantal_boterhammen
          - output: aantal_boterhammen
            value: 2
          - output: plakjes_kaas
            value: $aantal_boterhammen
"#;
        let beleid = format!(
            "{}  - number: '2'
    text: Wat de wet zegt, gelezen binnen dit beleid.
    machine_readable:
      execution:
        input:
          - name: tussen
            type: number
            source:
              regulation: boterhammenwet
              output: tussenstand
          - name: kaas
            type: number
            source:
              regulation: boterhammenwet
              output: plakjes_kaas
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value:
              operation: ADD
              values:
                - operation: MULTIPLY
                  values: [100, $tussen]
                - $kaas
",
            reading_rule(
                "beleid_boterhammen",
                "WET",
                "      overrides:
        - law: boterhammenwet
          article: '1'
          output: aantal_boterhammen",
                "aantal_boterhammen",
                DOUBLE_BASIS,
            )
        );
        let mut service = LawExecutionService::new();
        service.load_law(wet).unwrap();
        service.load_law(&beleid).unwrap();
        // tussenstand and plakjes_kaas both read the replaced value (4).
        let result = service
            .evaluate_law_output(
                "beleid_boterhammen",
                "gelezen",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(result.outputs.get("gelezen"), Some(&Value::Int(404)));
    }

    /// The order of the actions in the file never changes a value: the same
    /// article with its actions in any order gives the same outputs, also
    /// when an action reads an output declared after it.
    #[test]
    fn the_order_of_actions_in_the_file_does_not_change_a_value() {
        let wet = |actions: &str| {
            format!(
                r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A, B en C.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: b
            type: number
          - name: c
            type: number
        actions:
{actions}"#
            )
        };
        let a = "          - output: a\n            value: 2\n";
        let b = "          - output: b\n            value:\n              operation: ADD\n              values: [$a, $c]\n";
        let c = "          - output: c\n            value: 10\n";
        for order in [
            [a, b, c],
            [a, c, b],
            [b, a, c],
            [b, c, a],
            [c, a, b],
            [c, b, a],
        ] {
            let actions = order.concat();
            let mut service = LawExecutionService::new();
            service.load_law(&wet(&actions)).unwrap();
            let result = service
                .evaluate_law_output("wet_a", "b", BTreeMap::new(), "2025-01-01")
                .unwrap_or_else(|e| panic!("{actions}: {e}"));
            assert_eq!(result.outputs.get("b"), Some(&Value::Int(12)), "{actions}");
        }
    }

    /// Outputs of one article that read each other are a cycle, reported as
    /// one, not a variable that happens to be missing in file order.
    #[test]
    fn outputs_reading_each_other_are_a_cycle() {
        let law = r#"
$id: wet_cyclus
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is B, B is A.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: b
            type: number
        actions:
          - output: a
            value: $b
          - output: b
            value: $a
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        match service.evaluate_law_output("wet_cyclus", "a", BTreeMap::new(), "2025-01-01") {
            Err(EngineError::CircularReference(msg)) => {
                assert!(msg.contains("depends on itself"), "{msg}");
            }
            other => panic!("expected CircularReference, got {other:?}"),
        }
    }

    /// A cycle among outputs nobody asked for does not fail the request: only
    /// the actions the requested output depends on are ordered and run
    /// (RFC-043).
    #[test]
    fn a_cycle_outside_the_requested_outputs_does_not_fail_the_request() {
        let law = r#"
$id: wet_cyclus_ernaast
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is B, B is A, en C is drie.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: b
            type: number
          - name: c
            type: number
        actions:
          - output: a
            value: $b
          - output: b
            value: $a
          - output: c
            value: 3
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        let result = service
            .evaluate_law_output("wet_cyclus_ernaast", "c", BTreeMap::new(), "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("c"), Some(&Value::Int(3)));
        assert!(matches!(
            service.evaluate_law_output("wet_cyclus_ernaast", "a", BTreeMap::new(), "2025-01-01"),
            Err(EngineError::CircularReference(_))
        ));
    }

    /// An article that both fills in an open term of a law and overrides an
    /// output of it reads that law without either: both are held back.
    #[test]
    fn an_article_that_implements_and_overrides_holds_back_both() {
        let wet = format!(
            "{}  - number: '2'
    text: Op elke boterham een plak.
    machine_readable:
      execution:
        input:
          - name: aantal
            type: number
            source:
              output: aantal_boterhammen
        output:
          - name: plakjes
            type: number
        actions:
          - output: plakjes
            value: $aantal
",
            BOTERHAMMENWET_OPEN
        );
        let beleid = r#"
$id: beleid_boterhammen
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een boterham meer dan er plakjes zijn, en twee plakjes per boterham.
    machine_readable:
      implements:
        - law: boterhammenwet
          article: '1'
          open_term: aantal_boterhammen
      overrides:
        - law: boterhammenwet
          article: '2'
          output: plakjes
      execution:
        input:
          - name: basis
            type: number
            source:
              regulation: boterhammenwet
              output: plakjes
        output:
          - name: aantal_boterhammen
            type: number
          - name: plakjes
            type: number
        actions:
          - output: aantal_boterhammen
            value:
              operation: ADD
              values: [$basis, 1]
          - output: plakjes
            value:
              operation: MULTIPLY
              values: [$basis, 2]
"#;
        let mut service = LawExecutionService::new();
        service.load_law(&wet).unwrap();
        service.load_law(beleid).unwrap();
        // plakjes without the filling and without the override: the
        // default (2), so one sandwich more is 3.
        let result = service
            .evaluate_law_output(
                "beleid_boterhammen",
                "aantal_boterhammen",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("aantal_boterhammen"),
            Some(&Value::Int(3))
        );
    }

    /// A name that is both an input and an output of the overridden article
    /// is the input to the override, in either file order, and is not
    /// refused.
    #[test]
    fn an_override_reads_an_input_named_like_an_output_as_the_input() {
        let wet = |actions: &str| {
            format!(
                r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is twee, N is tien keer N.
    machine_readable:
      execution:
        parameters:
          - name: n
            type: number
            required: true
        output:
          - name: a
            type: number
          - name: n
            type: number
        actions:
{actions}"#
            )
        };
        let a = "          - output: a\n            value: 2\n";
        let n = "          - output: n\n            value:\n              operation: MULTIPLY\n              values: [$n, 10]\n";
        let beleid = r#"
$id: beleid_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is A gelijk aan N.
    machine_readable:
      overrides:
        - law: wet_a
          article: '1'
          output: a
      execution:
        parameters:
          - name: n
            type: number
            required: true
        output:
          - name: a
            type: number
        actions:
          - output: a
            value: $n
  - number: '2'
    text: A volgens de wet.
    machine_readable:
      execution:
        parameters:
          - name: n
            type: number
            required: true
        input:
          - name: a
            type: number
            source:
              regulation: wet_a
              output: a
              parameters:
                n: $n
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value: $a
"#;
        for actions in [[a, n].concat(), [n, a].concat()] {
            let mut service = LawExecutionService::new();
            service.load_law(&wet(&actions)).unwrap();
            service.load_law(beleid).unwrap();
            let params = BTreeMap::from([("n".to_string(), Value::Int(3))]);
            let result = service
                .evaluate_law_output("beleid_a", "gelezen", params, "2025-01-01")
                .unwrap_or_else(|e| panic!("{actions}: {e}"));
            assert_eq!(
                result.outputs.get("gelezen"),
                Some(&Value::Int(3)),
                "{actions}"
            );
        }
    }

    /// An override article replacing two outputs is judged per output: the
    /// rule for `b` does not read `a`, so declaring `a` for the rule for `a`
    /// does not refuse `b`.
    #[test]
    fn an_override_is_judged_on_what_its_rule_for_the_output_reads() {
        let wet = r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is twee, B is drie.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: b
            type: number
        actions:
          - output: a
            value: 2
          - output: b
            value: 3
"#;
        let beleid = r#"
$id: beleid_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is A het dubbele en B zeven.
    machine_readable:
      overrides:
        - law: wet_a
          article: '1'
          output: a
        - law: wet_a
          article: '1'
          output: b
      execution:
        parameters:
          - name: a
            type: number
            required: true
        output:
          - name: a
            type: number
          - name: b
            type: number
        actions:
          - output: a
            value:
              operation: MULTIPLY
              values: [$a, 2]
          - output: b
            value: 7
  - number: '2'
    text: A en B volgens de wet.
    machine_readable:
      execution:
        input:
          - name: a
            type: number
            source:
              regulation: wet_a
              output: a
          - name: b
            type: number
            source:
              regulation: wet_a
              output: b
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value:
              operation: ADD
              values: [$a, $b]
"#;
        let mut service = LawExecutionService::new();
        service.load_law(wet).unwrap();
        service.load_law(beleid).unwrap();
        let result = service
            .evaluate_law_output("beleid_a", "gelezen", BTreeMap::new(), "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("gelezen"), Some(&Value::Int(11)));
    }

    /// An override that reaches another output of its target through an open
    /// term of its own (a default reading the parameter) is refused as well,
    /// in either file order.
    #[test]
    fn an_override_reading_another_output_through_an_open_term_is_refused() {
        let wet = |actions: &str| {
            format!(
                r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is twee, B is tien.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: b
            type: number
        actions:
{actions}"#
            )
        };
        let a = "          - output: a\n            value: 2\n";
        let b = "          - output: b\n            value: 10\n";
        let beleid = r#"
$id: beleid_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is A een meer dan T.
    machine_readable:
      overrides:
        - law: wet_a
          article: '1'
          output: a
      open_terms:
        - id: t
          type: number
          required: true
          default:
            actions:
              - output: t
                value: $b
      execution:
        parameters:
          - name: b
            type: number
            required: true
        output:
          - name: a
            type: number
        actions:
          - output: a
            value:
              operation: ADD
              values: [$t, 1]
  - number: '2'
    text: A volgens de wet.
    machine_readable:
      execution:
        input:
          - name: a
            type: number
            source:
              regulation: wet_a
              output: a
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value: $a
"#;
        for actions in [[a, b].concat(), [b, a].concat()] {
            let mut service = LawExecutionService::new();
            service.load_law(&wet(&actions)).unwrap();
            service.load_law(beleid).unwrap();
            match service.evaluate_law_output("beleid_a", "gelezen", BTreeMap::new(), "2025-01-01")
            {
                Err(EngineError::InvalidOperation(msg)) => {
                    assert!(msg.contains("another output"), "{actions}: {msg}");
                }
                other => panic!("{actions}: expected the override to be refused, got {other:?}"),
            }
        }
    }

    /// An override that reaches another output of its target through a
    /// same-law input (which receives all its parameters) is refused as
    /// well, in either file order.
    #[test]
    fn an_override_reading_another_output_through_a_same_law_input_is_refused() {
        let wet = |actions: &str| {
            format!(
                r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is een, X is vijf.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: x
            type: number
        actions:
{actions}"#
            )
        };
        let a = "          - output: a\n            value: 1\n";
        let x = "          - output: x\n            value: 5\n";
        let beleid = r#"
$id: beleid_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is A gelijk aan Y.
    machine_readable:
      overrides:
        - law: wet_a
          article: '1'
          output: a
      execution:
        parameters:
          - name: x
            type: number
            required: true
        input:
          - name: y
            type: number
            source:
              output: y
        output:
          - name: a
            type: number
        actions:
          - output: a
            value: $y
  - number: '2'
    text: A volgens de wet.
    machine_readable:
      execution:
        input:
          - name: a
            type: number
            source:
              regulation: wet_a
              output: a
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value: $a
  - number: '3'
    text: Y is tien keer X.
    machine_readable:
      execution:
        parameters:
          - name: x
            type: number
            required: true
        output:
          - name: y
            type: number
        actions:
          - output: y
            value:
              operation: MULTIPLY
              values: [$x, 10]
"#;
        for actions in [[a, x].concat(), [x, a].concat()] {
            let mut service = LawExecutionService::new();
            service.load_law(&wet(&actions)).unwrap();
            service.load_law(beleid).unwrap();
            match service.evaluate_law_output("beleid_a", "gelezen", BTreeMap::new(), "2025-01-01")
            {
                Err(EngineError::InvalidOperation(msg)) => {
                    assert!(msg.contains("another output"), "{actions}: {msg}");
                }
                other => panic!("{actions}: expected the override to be refused, got {other:?}"),
            }
        }
    }

    /// An override whose own output is overridden in turn hands its
    /// parameters on to that override, so a declared parameter that is
    /// another output of its target refuses it, in either file order.
    #[test]
    fn an_override_whose_output_is_overridden_in_turn_is_judged_on_all_it_declares() {
        let wet = |actions: &str| {
            format!(
                r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is een, X is vijf.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: x
            type: number
        actions:
{actions}"#
            )
        };
        let a = "          - output: a\n            value: 1\n";
        let x = "          - output: x\n            value: 5\n";
        let beleid = r#"
$id: beleid_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is A twee.
    machine_readable:
      overrides:
        - law: wet_a
          article: '1'
          output: a
      execution:
        parameters:
          - name: x
            type: number
            required: true
        output:
          - name: a
            type: number
        actions:
          - output: a
            value: 2
  - number: '2'
    text: A volgens de wet.
    machine_readable:
      execution:
        input:
          - name: a
            type: number
            source:
              regulation: wet_a
              output: a
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value: $a
  - number: '3'
    text: In afwijking van artikel 1 is A tien keer X.
    machine_readable:
      overrides:
        - law: beleid_a
          article: '1'
          output: a
      execution:
        parameters:
          - name: x
            type: number
            required: true
        output:
          - name: a
            type: number
        actions:
          - output: a
            value:
              operation: MULTIPLY
              values: [$x, 10]
"#;
        for actions in [[a, x].concat(), [x, a].concat()] {
            let mut service = LawExecutionService::new();
            service.load_law(&wet(&actions)).unwrap();
            service.load_law(beleid).unwrap();
            match service.evaluate_law_output("beleid_a", "gelezen", BTreeMap::new(), "2025-01-01")
            {
                Err(EngineError::InvalidOperation(msg)) => {
                    assert!(msg.contains("another output"), "{actions}: {msg}");
                }
                other => panic!("{actions}: expected the override to be refused, got {other:?}"),
            }
        }
    }

    /// A same-law override reading the output it replaces through a same-law
    /// reference (`source: {output}`) reads the base value, as across laws.
    #[test]
    fn a_same_law_override_reads_its_base_through_a_same_law_reference() {
        let law = r#"
$id: boterhammenwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Men eet twee boterhammen.
    machine_readable:
      execution:
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value: 2
  - number: '2'
    text: In afwijking van artikel 1 eet men er twee keer zoveel.
    machine_readable:
      overrides:
        - law: boterhammenwet
          article: '1'
          output: aantal_boterhammen
      execution:
        input:
          - name: basis
            type: number
            source:
              output: aantal_boterhammen
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value:
              operation: MULTIPLY
              values: [2, $basis]
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        // Asked through the general article (the override applies within its
        // own law) and through the overriding one: doubled once, not twice.
        let result = service
            .evaluate_law_output(
                "boterhammenwet",
                "aantal_boterhammen",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("aantal_boterhammen"),
            Some(&Value::Int(4))
        );
    }

    /// A same-law implementation reading the output it fills in through a
    /// same-law reference reads the default, as across laws.
    #[test]
    fn a_same_law_implementation_reads_the_default_through_a_same_law_reference() {
        let law = r#"
$id: boterhammenwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Men eet twee boterhammen, tenzij anders bepaald.
    machine_readable:
      open_terms:
        - id: aantal_boterhammen
          type: number
          required: true
          default:
            actions:
              - output: aantal_boterhammen
                value: 2
      execution:
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value: $aantal_boterhammen
  - number: '2'
    text: Gelet op artikel 1 eet men er een meer.
    machine_readable:
      implements:
        - law: boterhammenwet
          article: '1'
          open_term: aantal_boterhammen
      execution:
        input:
          - name: basis
            type: number
            source:
              output: aantal_boterhammen
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value:
              operation: ADD
              values: [$basis, 1]
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        let result = service
            .evaluate_law_output(
                "boterhammenwet",
                "aantal_boterhammen",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("aantal_boterhammen"),
            Some(&Value::Int(3))
        );
    }

    /// An article filling open terms in two laws, reading one of them, is held
    /// back only for that one: the other law keeps its filling.
    #[test]
    fn an_implementation_is_held_back_only_for_the_law_it_reads() {
        let l2 = r#"
$id: wet_twee
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Z is T, honderd tenzij anders bepaald.
    machine_readable:
      open_terms:
        - id: t
          type: number
          required: true
          default:
            actions:
              - output: t
                value: 100
      execution:
        output:
          - name: z
            type: number
        actions:
          - output: z
            value: $t
"#;
        let l1 = r#"
$id: wet_een
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: X is S plus Z, S nul tenzij anders bepaald.
    machine_readable:
      open_terms:
        - id: s
          type: number
          required: true
          default:
            actions:
              - output: s
                value: 0
      execution:
        input:
          - name: z
            type: number
            source:
              regulation: wet_twee
              output: z
        output:
          - name: x
            type: number
        actions:
          - output: x
            value:
              operation: ADD
              values: [$s, $z]
"#;
        let regeling = r#"
$id: regeling_beide
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: S is een meer dan X; T is zeven.
    machine_readable:
      implements:
        - law: wet_een
          article: '1'
          open_term: s
        - law: wet_twee
          article: '1'
          open_term: t
      execution:
        input:
          - name: basis
            type: number
            source:
              regulation: wet_een
              output: x
        output:
          - name: s
            type: number
          - name: t
            type: number
        actions:
          - output: s
            value:
              operation: ADD
              values: [$basis, 1]
          - output: t
            value: 7
"#;
        let mut service = LawExecutionService::new();
        service.load_law(l2).unwrap();
        service.load_law(l1).unwrap();
        service.load_law(regeling).unwrap();
        // X without the filling of S is 0 + 7; S is 8; X is 8 + 7.
        let result = service
            .evaluate_law_output("wet_een", "x", BTreeMap::new(), "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("x"), Some(&Value::Int(15)));
    }

    /// An override that asks for another output of the article it overrides
    /// is refused, whether the file sets that output before or after the one
    /// it replaces: otherwise the file order would decide a value.
    #[test]
    fn an_override_declaring_another_output_of_its_target_is_refused() {
        let wet = |actions: &str| {
            format!(
                r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A is twee, B is tien.
    machine_readable:
      execution:
        output:
          - name: a
            type: number
          - name: b
            type: number
        actions:
{actions}"#
            )
        };
        let a = "          - output: a\n            value: 2\n";
        let b = "          - output: b\n            value: 10\n";
        let beleid = r#"
$id: beleid_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan is A gelijk aan B.
    machine_readable:
      overrides:
        - law: wet_a
          article: '1'
          output: a
      execution:
        parameters:
          - name: b
            type: number
            required: true
        output:
          - name: a
            type: number
        actions:
          - output: a
            value: $b
  - number: '2'
    text: A volgens de wet.
    machine_readable:
      execution:
        input:
          - name: a
            type: number
            source:
              regulation: wet_a
              output: a
        output:
          - name: gelezen
            type: number
        actions:
          - output: gelezen
            value: $a
"#;
        for actions in [[a, b].concat(), [b, a].concat()] {
            let mut service = LawExecutionService::new();
            service.load_law(&wet(&actions)).unwrap();
            service.load_law(beleid).unwrap();
            match service.evaluate_law_output("beleid_a", "gelezen", BTreeMap::new(), "2025-01-01")
            {
                Err(EngineError::InvalidOperation(msg)) => {
                    assert!(msg.contains("another output"), "{actions}: {msg}");
                }
                other => panic!("{actions}: expected the override to be refused, got {other:?}"),
            }
        }
    }

    /// An implementation reads the whole law it fills in without itself, not
    /// only the article declaring the term: an output of another article that
    /// depends on the term is a base read too.
    #[test]
    fn an_implementation_reads_the_whole_law_without_itself() {
        let wet = format!(
            "{}  - number: '2'
    text: Op elke boterham gaat een plak ham.
    machine_readable:
      execution:
        input:
          - name: aantal
            type: number
            source:
              output: aantal_boterhammen
        output:
          - name: plakken_ham
            type: number
        actions:
          - output: plakken_ham
            value: $aantal
",
            BOTERHAMMENWET_OPEN
        );
        let beleid = r#"
$id: beleid_boterhammen
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twee keer zoveel als er plakken ham zijn.
    machine_readable:
      implements:
        - law: boterhammenwet
          article: '1'
          open_term: aantal_boterhammen
      execution:
        input:
          - name: ham
            type: number
            source:
              regulation: boterhammenwet
              output: plakken_ham
        output:
          - name: aantal_boterhammen
            type: number
        actions:
          - output: aantal_boterhammen
            value:
              operation: MULTIPLY
              values: [2, $ham]
"#;
        let mut service = LawExecutionService::new();
        service.load_law(&wet).unwrap();
        service.load_law(beleid).unwrap();
        let result = service
            .evaluate_law_output(
                "boterhammenwet",
                "aantal_boterhammen",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("aantal_boterhammen"),
            Some(&Value::Int(4))
        );

        // The same through the article that reads the term's output: its
        // internal reference is entered again inside the base read, which is
        // another resolution and not a cycle.
        let result = service
            .evaluate_law_output(
                "boterhammenwet",
                "plakken_ham",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(result.outputs.get("plakken_ham"), Some(&Value::Int(4)));
    }

    #[test]
    fn test_service_internal_circular_reference() {
        // Same-law cycle: article 1's input sources article 2's output,
        // and article 2's input sources article 1's output.
        // Previously this recursed unbounded and crashed with a stack overflow.
        let law = r#"
$id: internal_cycle_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References output of article 2
    machine_readable:
      execution:
        input:
          - name: from_b
            type: number
            source:
              output: output_b
        output:
          - name: output_a
            type: number
        actions:
          - output: output_a
            value: $from_b
  - number: '2'
    text: References output of article 1
    machine_readable:
      execution:
        input:
          - name: from_a
            type: number
            source:
              output: output_a
        output:
          - name: output_b
            type: number
        actions:
          - output: output_b
            value: $from_a
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service.evaluate_law_output(
            "internal_cycle_law",
            "output_a",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_service_internal_circular_reference_indirect() {
        // Indirect same-law cycle across three articles:
        // article 1 -> article 2 -> article 3 -> article 1.
        let law = r#"
$id: internal_cycle_law_3
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References output of article 2
    machine_readable:
      execution:
        input:
          - name: from_b
            type: number
            source:
              output: output_b
        output:
          - name: output_a
            type: number
        actions:
          - output: output_a
            value: $from_b
  - number: '2'
    text: References output of article 3
    machine_readable:
      execution:
        input:
          - name: from_c
            type: number
            source:
              output: output_c
        output:
          - name: output_b
            type: number
        actions:
          - output: output_b
            value: $from_c
  - number: '3'
    text: References output of article 1
    machine_readable:
      execution:
        input:
          - name: from_a
            type: number
            source:
              output: output_a
        output:
          - name: output_c
            type: number
        actions:
          - output: output_c
            value: $from_a
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service.evaluate_law_output(
            "internal_cycle_law_3",
            "output_a",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_service_internal_diamond_reference_succeeds() {
        // Legitimate diamond, no cycle: two inputs of article 1 both source
        // the SAME output of article 2. The cycle guard must not produce a
        // false positive here (the visited key is released between siblings).
        let law = r#"
$id: internal_diamond_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Sums the same output of article 2 twice
    machine_readable:
      execution:
        input:
          - name: left
            type: number
            source:
              output: shared_value
          - name: right
            type: number
            source:
              output: shared_value
        output:
          - name: combined
            type: number
        actions:
          - output: combined
            operation: ADD
            values:
              - $left
              - $right
  - number: '2'
    text: Provides a shared value
    machine_readable:
      execution:
        output:
          - name: shared_value
            type: number
        actions:
          - output: shared_value
            value: 21
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service
            .evaluate_law_output(
                "internal_diamond_law",
                "combined",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("diamond-shaped internal references must not be flagged as circular");

        assert_eq!(result.outputs.get("combined"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_service_internal_deep_chain_depth_limit() {
        // A linear (non-cyclic) chain of internal references longer than
        // MAX_CROSS_LAW_DEPTH must hit the depth limit instead of recursing
        // until the stack overflows.
        let chain_len = config::MAX_CROSS_LAW_DEPTH + 5;
        let mut yaml = String::from(
            "$id: internal_deep_chain_law\n\
             regulatory_layer: WET\n\
             publication_date: '2025-01-01'\n\
             articles:\n",
        );
        for i in 0..chain_len {
            yaml.push_str(&format!(
                r#"  - number: '{num}'
    text: Chain link {num}
    machine_readable:
      execution:
        input:
          - name: from_next
            type: number
            source:
              output: value_{next}
        output:
          - name: value_{num}
            type: number
        actions:
          - output: value_{num}
            value: $from_next
"#,
                num = i,
                next = i + 1
            ));
        }
        // Terminal article: a literal value, no further references.
        yaml.push_str(&format!(
            r#"  - number: '{num}'
    text: Chain terminal
    machine_readable:
      execution:
        output:
          - name: value_{num}
            type: number
        actions:
          - output: value_{num}
            value: 1
"#,
            num = chain_len
        ));

        let mut service = LawExecutionService::new();
        service.load_law(&yaml).unwrap();

        let result = service.evaluate_law_output(
            "internal_deep_chain_law",
            "value_0",
            BTreeMap::new(),
            "2025-01-01",
        );

        match result {
            Err(EngineError::CircularReference(msg)) => {
                assert!(
                    msg.contains("depth exceeded"),
                    "Expected depth-exceeded error, got: {}",
                    msg
                );
            }
            other => panic!("Expected CircularReference depth error, got: {:?}", other),
        }
    }

    #[test]
    fn test_service_internal_self_reference_is_circular() {
        // Degenerate same-law cycle: an article's input sources its own output.
        let law = r#"
$id: internal_self_ref_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: References its own output
    machine_readable:
      execution:
        input:
          - name: my_output
            type: number
            source:
              output: my_output
        output:
          - name: my_output
            type: number
        actions:
          - output: my_output
            value: $my_output
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service.evaluate_law_output(
            "internal_self_ref_law",
            "my_output",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::CircularReference(_))),
            "Expected CircularReference error for a self-referencing article, got: {:?}",
            result
        );
    }

    #[test]
    fn test_service_internal_reference_basic() {
        // Article 2's input sources article 1's output (same law). Resolution is
        // owned by the service; the value flows through to article 2's action.
        let law = r#"
$id: internal_ref_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Base calculation article
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 100
      execution:
        output:
          - name: base_amount
            type: number
        actions:
          - output: base_amount
            value: $BASE_VALUE
  - number: '2'
    text: Derived calculation using internal reference
    machine_readable:
      execution:
        input:
          - name: base_amount
            type: number
            source:
              output: base_amount
        output:
          - name: doubled_amount
            type: number
        actions:
          - output: doubled_amount
            operation: MULTIPLY
            values:
              - $base_amount
              - 2
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service
            .evaluate_law_output(
                "internal_ref_law",
                "doubled_amount",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("internal reference must resolve");

        // doubled_amount = base_amount (100) * 2 = 200
        assert_eq!(result.outputs.get("doubled_amount"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_service_internal_reference_chain() {
        // A chain of internal references within one law: 3 -> 2 -> 1.
        let law = r#"
$id: chain_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: First article
    machine_readable:
      execution:
        output:
          - name: first_value
            type: number
        actions:
          - output: first_value
            value: 10
  - number: '2'
    text: Second article references first
    machine_readable:
      execution:
        input:
          - name: first_value
            type: number
            source:
              output: first_value
        output:
          - name: second_value
            type: number
        actions:
          - output: second_value
            operation: ADD
            values:
              - $first_value
              - 5
  - number: '3'
    text: Third article references second
    machine_readable:
      execution:
        input:
          - name: second_value
            type: number
            source:
              output: second_value
        output:
          - name: third_value
            type: number
        actions:
          - output: third_value
            operation: MULTIPLY
            values:
              - $second_value
              - 2
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service
            .evaluate_law_output("chain_law", "third_value", BTreeMap::new(), "2025-01-01")
            .expect("internal reference chain must resolve");

        // first_value = 10, second_value = 10 + 5 = 15, third_value = 15 * 2 = 30
        assert_eq!(result.outputs.get("third_value"), Some(&Value::Int(30)));
    }

    #[test]
    fn test_service_internal_reference_with_parameters() {
        // The referenced article consumes a parameter that the caller supplies;
        // the internal reference must still resolve correctly.
        let law = r#"
$id: param_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article that uses parameter
    machine_readable:
      execution:
        parameters:
          - name: multiplier
            type: number
            required: true
        output:
          - name: base_result
            type: number
        actions:
          - output: base_result
            operation: MULTIPLY
            values:
              - 100
              - $multiplier
  - number: '2'
    text: Article that references article 1
    machine_readable:
      execution:
        parameters:
          - name: multiplier
            type: number
            required: true
        input:
          - name: base_result
            type: number
            source:
              output: base_result
        output:
          - name: final_result
            type: number
        actions:
          - output: final_result
            operation: ADD
            values:
              - $base_result
              - 50
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let mut params = BTreeMap::new();
        params.insert("multiplier".to_string(), Value::Int(3));

        let result = service
            .evaluate_law_output("param_law", "final_result", params, "2025-01-01")
            .expect("internal reference with parameters must resolve");

        // base_result = 100 * 3 = 300, final_result = 300 + 50 = 350
        assert_eq!(result.outputs.get("final_result"), Some(&Value::Int(350)));
    }

    #[test]
    fn test_service_internal_reference_missing_output_errors() {
        // Article 2 internally references `missing_output`, which article 1
        // declares but never produces (no action emits it). The referenced
        // article runs successfully but its result lacks the output. This must
        // surface as OutputNotFound at the resolution site — mirroring the
        // external-reference path — rather than being silently left unresolved
        // and failing later with a generic VariableNotFound on the input.
        let law = r#"
$id: missing_internal_output_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Declares an output it never produces
    machine_readable:
      execution:
        output:
          - name: produced
            type: number
          - name: missing_output
            type: number
        actions:
          - output: produced
            value: 1
  - number: '2'
    text: References the never-produced output
    machine_readable:
      execution:
        input:
          - name: missing_output
            type: number
            source:
              output: missing_output
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $missing_output
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let result = service.evaluate_law_output(
            "missing_internal_output_law",
            "result",
            BTreeMap::new(),
            "2025-01-01",
        );

        match result {
            Err(EngineError::OutputNotFound { output, .. }) => {
                assert_eq!(output, "missing_output");
            }
            other => panic!(
                "Expected OutputNotFound for an unproduced internal output, got: {:?}",
                other
            ),
        }
    }

    // -------------------------------------------------------------------------
    // Parameter Override Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_parameter_override() {
        // When a parameter is provided that matches an input name,
        // the parameter value should be used instead of resolving the source
        let law = r#"
$id: override_test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Has external reference but can be overridden
    machine_readable:
      execution:
        parameters:
          - name: external_value
            type: number
        input:
          - name: external_value
            type: number
            source:
              regulation: nonexistent_law
              output: some_output
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $external_value
              - 2
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        // Provide the value directly - should skip external resolution
        let mut params = BTreeMap::new();
        params.insert("external_value".to_string(), Value::Int(50));

        let result = service
            .evaluate_law_output("override_test_law", "result", params, "2025-01-01")
            .unwrap();

        // result = 50 * 2 = 100
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(100)));
    }

    // -------------------------------------------------------------------------
    // API Method Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_law_info() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let info = service.get_law_info("base_law").unwrap();
        assert_eq!(info.id, "base_law");
        assert_eq!(info.regulatory_layer, RegulatoryLayer::Wet);
        assert_eq!(info.publication_date, "2025-01-01");
        assert!(info.bwb_id.is_none());
        assert!(info.url.is_none());
        assert_eq!(info.outputs, vec!["base_value"]);
        assert_eq!(info.article_count, 1);

        // Non-existent law
        assert!(service.get_law_info("nonexistent").is_none());
    }

    #[test]
    fn test_list_all_outputs() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();

        let outputs = service.list_all_outputs();
        assert!(outputs.contains(&("base_law", "base_value")));
        assert!(outputs.contains(&("dependent_law", "doubled_value")));
        assert_eq!(outputs.len(), 2);
    }

    #[test]
    fn test_output_count() {
        let mut service = LawExecutionService::new();
        assert_eq!(service.get_output_count(), 0);

        service.load_law(make_base_law()).unwrap();
        assert_eq!(service.get_output_count(), 1);

        service.load_law(make_dependent_law()).unwrap();
        assert_eq!(service.get_output_count(), 2);
    }

    // -------------------------------------------------------------------------
    // Integration Tests with Real Regulation Files
    // -------------------------------------------------------------------------

    mod integration {
        use super::*;
        use std::path::PathBuf;

        fn get_regulation_path() -> PathBuf {
            std::env::var("REGULATION_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("..")
                        .join("..")
                        .join("corpus")
                        .join("regulation")
                })
        }

        #[test]
        fn test_service_resolve_with_real_files() {
            // Integration test: load real wet_op_de_zorgtoeslag + regeling_standaardpremie
            // and verify resolve action works end-to-end
            let regulation_path = get_regulation_path();

            let wet_op_de_zorgtoeslag_path =
                regulation_path.join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let regeling_path = regulation_path
                .join("nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml");

            let wet_op_de_zorgtoeslag_law =
                ArticleBasedLaw::from_yaml_file(&wet_op_de_zorgtoeslag_path).unwrap();
            let rsp_law = ArticleBasedLaw::from_yaml_file(&regeling_path).unwrap();

            let mut service = LawExecutionService::new();
            service.load_law_struct(wet_op_de_zorgtoeslag_law).unwrap();
            service.load_law_struct(rsp_law).unwrap();

            // Execute wet_op_de_zorgtoeslag standaardpremie output
            // This should resolve the regeling_standaardpremie via legal_basis
            let result = service
                .evaluate_law_output(
                    "wet_op_de_zorgtoeslag",
                    "standaardpremie",
                    BTreeMap::new(),
                    "2025-01-01",
                )
                .unwrap();

            // standaardpremie for 2025 = 211200 eurocent (€2112)
            assert_eq!(
                result.outputs.get("standaardpremie"),
                Some(&Value::Int(211200)),
                "Expected standaardpremie=211200 for 2025"
            );
        }

        #[test]
        fn test_get_law_info_real() {
            let regulation_path = get_regulation_path();
            let path = regulation_path.join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            let mut service = LawExecutionService::new();
            service.load_law_struct(law).unwrap();

            let info = service.get_law_info("wet_op_de_zorgtoeslag").unwrap();
            assert_eq!(info.id, "wet_op_de_zorgtoeslag");
            assert_eq!(info.regulatory_layer, RegulatoryLayer::Wet);
            assert!(info.article_count > 0);
            assert!(!info.outputs.is_empty());
            // Should include standaardpremie output
            assert!(
                info.outputs.contains(&"standaardpremie".to_string()),
                "Expected standaardpremie in outputs: {:?}",
                info.outputs
            );
        }
    }

    #[test]
    fn test_cross_law_uses_version_aware_lookup() {
        // Two versions of a referenced law with different definitions.
        // Cross-law resolution should pick the correct version based on
        // the reference date, not just the latest version.
        // Version selection uses `valid_from` to determine applicability.
        let base_v1 = r#"
$id: versioned_base
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Base value v1
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 100
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#;
        let base_v2 = r#"
$id: versioned_base
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Base value v2
    machine_readable:
      definitions:
        BASE_VALUE:
          value: 200
      execution:
        output:
          - name: base_value
            type: number
        actions:
          - output: base_value
            value: $BASE_VALUE
"#;
        let dependent = r#"
$id: cross_law_consumer
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Uses versioned base
    machine_readable:
      execution:
        input:
          - name: external_base
            type: number
            source:
              regulation: versioned_base
              output: base_value
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_base
"#;
        let mut service = LawExecutionService::new();
        service.load_law(base_v1).unwrap();
        service.load_law(base_v2).unwrap();
        service.load_law(dependent).unwrap();

        // Reference date 2024-06-15 should use v1 (BASE_VALUE=100)
        let result = service
            .evaluate_law_output(
                "cross_law_consumer",
                "result",
                BTreeMap::new(),
                "2024-06-15",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(100)),
            "2024 reference date should resolve to v1 (100)"
        );

        // Reference date 2025-06-15 should use v2 (BASE_VALUE=200)
        let result = service
            .evaluate_law_output(
                "cross_law_consumer",
                "result",
                BTreeMap::new(),
                "2025-06-15",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(200)),
            "2025 reference date should resolve to v2 (200)"
        );
    }

    // -------------------------------------------------------------------------
    // DataSourceRegistry Integration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_data_registry_provides_input() {
        // A law references a regulation for an input, but the data registry
        // provides the value directly. The referenced regulation is NOT loaded,
        // proving the registry short-circuits cross-law resolution.
        let law = r#"
$id: registry_test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Has external reference resolved by registry
    machine_readable:
      execution:
        parameters:
          - name: BSN
            type: string
            required: true
        input:
          - name: external_value
            type: number
            source:
              regulation: nonexistent_law
              output: some_output
              parameters:
                BSN: $BSN
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MULTIPLY
            values:
              - $external_value
              - 3
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        // Register data source with the input field
        let mut record = BTreeMap::new();
        record.insert("BSN".to_string(), Value::String("123".to_string()));
        record.insert("external_value".to_string(), Value::Int(42));

        service
            .register_dict_source("test_data", "BSN", vec![record])
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert("BSN".to_string(), Value::String("123".to_string()));

        let result = service
            .evaluate_law_output("registry_test_law", "result", params, "2025-01-01")
            .unwrap();

        // result = 42 * 3 = 126
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(126)));
    }

    #[test]
    fn test_cross_law_call_leaves_an_omitted_optional_parameter_unknown() {
        // The tax law asks the permit law for "heeft_vergunning" and passes only
        // kvk_nummer; the permit law also declares an optional application-form
        // parameter it uses in another action. That action resolves to unknown
        // for lack of the form field (RFC-036), and the asked-for output still
        // comes back.
        let (permit, tax) = fill_laws("required: false");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service.load_law(&tax).unwrap();
        register_fill_permit(&mut service);

        let mut params = BTreeMap::new();
        params.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        let result = service
            .evaluate_law_output(
                "fill_belasting",
                "belastingplichtig",
                params.clone(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("belastingplichtig"),
            Some(&Value::Bool(true))
        );

        // The same call for the output that needs the form field: unknown, and
        // it says which fact of which law is missing.
        let tax = tax.replace("heeft_vergunning", "past_oppervlakte");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service.load_law(&tax).unwrap();
        register_fill_permit(&mut service);
        let result = service
            .evaluate_law_output("fill_belasting", "belastingplichtig", params, "2025-01-01")
            .unwrap();
        let outcome = result.outputs.get("belastingplichtig").unwrap();
        assert_eq!(
            outcome.missing_facts(),
            &[MissingFact {
                law: "fill_vergunning".to_string(),
                name: "terras_oppervlakte".to_string(),
                kind: MissingKind::NotPassed,
            }]
        );
    }

    #[test]
    fn test_top_level_call_leaves_an_omitted_optional_parameter_unknown() {
        // Not only across laws: a caller that evaluates the permit law itself
        // without the optional form field gets unknown for lack of it, with the
        // same provenance.
        let (permit, _) = fill_laws("required: false");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        register_fill_permit(&mut service);

        let mut params = BTreeMap::new();
        params.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        let result = service
            .evaluate_law_output("fill_vergunning", "past_oppervlakte", params, "2025-01-01")
            .unwrap();
        let outcome = result.outputs.get("past_oppervlakte").unwrap();
        assert_eq!(
            outcome.missing_facts(),
            &[MissingFact {
                law: "fill_vergunning".to_string(),
                name: "terras_oppervlakte".to_string(),
                kind: MissingKind::NotPassed,
            }]
        );
    }

    #[test]
    fn test_cross_law_call_missing_required_parameter_still_fails() {
        // The same call, but the permit law insists on the form field. The
        // engine must not invent a null for it: a caller that forgets (or
        // misspells) a required parameter gets the error, not an unknown. The
        // tax law asks for the output that reads the form field: an output that
        // does not read it runs without it (RFC-043).
        let (permit, tax) = fill_laws("required: true");
        let tax = tax.replace("heeft_vergunning", "past_oppervlakte");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service.load_law(&tax).unwrap();
        register_fill_permit(&mut service);

        let mut params = BTreeMap::new();
        params.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        let result = service.evaluate_law_output(
            "fill_belasting",
            "belastingplichtig",
            params,
            "2025-01-01",
        );
        assert!(
            matches!(result, Err(EngineError::VariableNotFound(_))),
            "expected VariableNotFound, got {result:?}"
        );
    }

    #[test]
    fn test_cross_law_call_does_not_fill_an_omitted_optional_parameter_from_law_data() {
        // The permit law's form field was answered earlier and kept as data
        // bound to that law. That data is *not* consulted for a parameter the
        // caller omits (RFC-036): a parameter is what the caller passes, and
        // an omitted optional one is unknown for lack of exactly that
        // parameter, so the decision process can ask for it. Filling it from
        // the target's own data would answer a question the caller never
        // asked, with a value the caller never saw.
        let (permit, tax) = fill_laws("required: false");
        let tax = tax.replace("heeft_vergunning", "past_oppervlakte");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service.load_law(&tax).unwrap();
        register_fill_permit(&mut service);
        let mut answers = BTreeMap::new();
        answers.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        answers.insert("terras_oppervlakte".to_string(), Value::Int(30));
        service
            .register_dict_source_for_law(
                "fill_vergunning",
                "aanvraag",
                "kvk_nummer",
                vec![answers],
                100,
            )
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        let result = service
            .evaluate_law_output("fill_belasting", "belastingplichtig", params, "2025-01-01")
            .unwrap();
        let outcome = result.outputs.get("belastingplichtig").unwrap();
        assert_eq!(
            outcome.missing_facts(),
            &[MissingFact {
                law: "fill_vergunning".to_string(),
                name: "terras_oppervlakte".to_string(),
                kind: MissingKind::NotPassed,
            }]
        );
    }

    #[test]
    fn test_cross_law_call_passes_null_optional_parameter_through() {
        // The tax law passes the form field along, but its own register holds
        // no value for it (null). Null for an *optional* parameter is passed
        // through, so the permit law still runs and answers; only a null
        // *required* parameter stops the call. The permit law tests for the
        // absence before it compares (RFC-036), so `past_oppervlakte` is a
        // definite false, not an error.
        let (permit, tax) = fill_laws("required: false");
        let tax = tax
            .replace(
                "        input:\n          - name: heeft_vergunning",
                "        input:\n          - name: onbekende_oppervlakte\n            type: number\n            nullable: true\n            source: {}\n          - name: heeft_vergunning",
            )
            .replace(
                "                kvk_nummer: $kvk_nummer",
                "                kvk_nummer: $kvk_nummer\n                terras_oppervlakte: $onbekende_oppervlakte",
            );
        assert!(
            tax.contains("terras_oppervlakte: $onbekende_oppervlakte"),
            "test law not rewritten"
        );
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service.load_law(&tax).unwrap();
        register_fill_permit(&mut service);
        let mut record = BTreeMap::new();
        record.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        record.insert("onbekende_oppervlakte".to_string(), Value::Null);
        service
            .register_dict_source_for_law(
                "fill_belasting",
                "aanvraag",
                "kvk_nummer",
                vec![record.clone()],
                10,
            )
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        let result = service
            .evaluate_law_output(
                "fill_belasting",
                "belastingplichtig",
                params.clone(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("belastingplichtig"),
            Some(&Value::Bool(true))
        );

        // The absent form field reached the permit law: its absence test
        // answers "past niet", where an omitted parameter would be unknown.
        let tax = tax.replace("heeft_vergunning", "past_oppervlakte");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service.load_law(&tax).unwrap();
        register_fill_permit(&mut service);
        service
            .register_dict_source_for_law(
                "fill_belasting",
                "aanvraag",
                "kvk_nummer",
                vec![record],
                10,
            )
            .unwrap();
        let result = service
            .evaluate_law_output("fill_belasting", "belastingplichtig", params, "2025-01-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("belastingplichtig"),
            Some(&Value::Bool(false))
        );
    }

    fn register_fill_permit(service: &mut LawExecutionService) {
        let mut record = BTreeMap::new();
        record.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        record.insert(
            "vergunning_status".to_string(),
            Value::String("ACTIEF".to_string()),
        );
        service
            .register_dict_source_for_law(
                "fill_vergunning",
                "gemeente",
                "kvk_nummer",
                vec![record],
                10,
            )
            .unwrap();
    }

    /// A permit law with a form parameter (`required` as given) and a tax law
    /// that asks it for "heeft_vergunning" with only the KVK number.
    /// The permit article of `fill_laws`, for the parameter helpers.
    fn fill_permit_article(form_param_required: &str) -> Article {
        let (permit, _) = fill_laws(form_param_required);
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        service
            .get_law("fill_vergunning")
            .unwrap()
            .find_article_by_output("past_oppervlakte")
            .unwrap()
            .clone()
    }

    fn params(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn required_parameter_not_passed_names_only_a_required_parameter_that_is_absent() {
        // RFC-036: a forgotten required parameter keeps the register
        // unasked. A parameter that is passed, whatever its value, and an
        // optional one that is left out, do not trigger it.
        let kvk = ("kvk_nummer", Value::String("85234567".to_string()));
        let article = fill_permit_article("required: true");
        assert_eq!(
            required_parameter_not_passed(&article, &params(std::slice::from_ref(&kvk))),
            Some("terras_oppervlakte".to_string())
        );
        assert_eq!(
            required_parameter_not_passed(
                &article,
                &params(&[kvk.clone(), ("terras_oppervlakte", Value::Null)])
            ),
            None
        );
        assert_eq!(
            required_parameter_not_passed(&article, &BTreeMap::new()),
            Some("kvk_nummer".to_string())
        );
        // `required` absent means required (schema v0.5.8); `required: false`
        // means the caller may leave it out.
        let implicit = fill_permit_article("");
        assert_eq!(
            required_parameter_not_passed(&implicit, &params(std::slice::from_ref(&kvk))),
            Some("terras_oppervlakte".to_string())
        );
        let optional = fill_permit_article("required: false");
        assert_eq!(
            required_parameter_not_passed(&optional, &params(&[kvk])),
            None
        );
    }

    /// An article with one parameter `oppervlakte` carrying the given
    /// attribute lines, for the parameter helpers. The parameter is only
    /// compared with EQUALS, so the law type-checks whatever its nullability.
    fn parameter_article(attributes: &str) -> Article {
        let yaml = format!(
            r#"
$id: param_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Oppervlakte
    machine_readable:
      execution:
        parameters:
          - name: oppervlakte
            type: number
            {attributes}
        output:
          - name: is_twaalf
            type: boolean
        actions:
          - output: is_twaalf
            operation: EQUALS
            subject: $oppervlakte
            value: 12
"#
        );
        let mut service = LawExecutionService::new();
        service.load_law(&yaml).unwrap();
        service
            .get_law("param_wet")
            .unwrap()
            .find_article_by_output("is_twaalf")
            .unwrap()
            .clone()
    }

    #[test]
    fn required_parameter_for_nobody_lets_the_type_decide_for_null_and_required_for_unknown() {
        // RFC-036: `null` is allowed exactly where the declaration says so
        // (`nullable: true`), whether the parameter is required or not.
        // Unknown is not governed by `nullable`: a required parameter that is
        // unknown names nobody, an optional one is a fact the law can do
        // without.
        let unknown = Value::unknown("param_wet", "oppervlakte", MissingKind::NotPassed);
        let null = || params(&[("oppervlakte", Value::Null)]);
        let unk = || params(&[("oppervlakte", unknown.clone())]);
        let some = || params(&[("oppervlakte", Value::Int(12))]);
        let hit = |word: &'static str| Some(("oppervlakte".to_string(), word));

        // Required and not nullable: neither empty form is allowed.
        let article = parameter_article("required: true");
        assert_eq!(
            required_parameter_for_nobody(&article, &null()),
            hit("null")
        );
        assert_eq!(
            required_parameter_for_nobody(&article, &unk()),
            hit("unknown")
        );
        assert_eq!(required_parameter_for_nobody(&article, &some()), None);

        // Required and nullable: null is an absence the law decides on;
        // unknown still names nobody.
        let article = parameter_article(
            "required: true
            nullable: true",
        );
        assert_eq!(required_parameter_for_nobody(&article, &null()), None);
        assert_eq!(
            required_parameter_for_nobody(&article, &unk()),
            hit("unknown")
        );

        // Optional and not nullable: the caller may leave it out (unknown),
        // but may not pass an absence the type rules out.
        let article = parameter_article("required: false");
        assert_eq!(
            required_parameter_for_nobody(&article, &null()),
            hit("null")
        );
        assert_eq!(required_parameter_for_nobody(&article, &unk()), None);

        // Optional and nullable: both empty forms are allowed.
        let article = parameter_article(
            "required: false
            nullable: true",
        );
        assert_eq!(required_parameter_for_nobody(&article, &null()), None);
        assert_eq!(required_parameter_for_nobody(&article, &unk()), None);
    }

    fn fill_laws(form_param_required: &str) -> (String, String) {
        let permit = format!(
            r#"
$id: fill_vergunning
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Vergunning
    machine_readable:
      execution:
        parameters:
          - name: kvk_nummer
            type: string
            required: true
          - name: terras_oppervlakte
            type: number
            nullable: true
            {form_param_required}
        input:
          - name: vergunning_status
            type: string
            source: {{}}
        output:
          - name: heeft_vergunning
            type: boolean
          - name: past_oppervlakte
            type: boolean
        actions:
          - output: heeft_vergunning
            operation: EQUALS
            subject: $vergunning_status
            value: ACTIEF
          # The form field may be absent (null): the law tests for that before
          # it compares, as RFC-036 requires of an ordered comparison.
          - output: past_oppervlakte
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $terras_oppervlakte
                    value: null
                  then: false
              default:
                operation: LESS_THAN
                subject: $terras_oppervlakte
                value: 50
"#
        );
        let tax = r#"
$id: fill_belasting
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Belasting
    machine_readable:
      execution:
        parameters:
          - name: kvk_nummer
            type: string
            required: true
        input:
          - name: heeft_vergunning
            type: boolean
            source:
              regulation: fill_vergunning
              output: heeft_vergunning
              parameters:
                kvk_nummer: $kvk_nummer
        output:
          - name: belastingplichtig
            type: boolean
        actions:
          - output: belastingplichtig
            value: $heeft_vergunning
"#;
        (permit, tax.to_string())
    }

    #[test]
    fn test_cross_law_call_with_null_parameter_resolves_to_null() {
        // A partner's age when there is no partner: the caller passes
        // `bsn: $partner_bsn`, which is null. The referenced law is not run and
        // the input is null, so the caller's own null check can decide.
        let brp = r#"
$id: null_brp
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: BRP
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: geboortedatum
            type: date
            source: {}
        output:
          - name: geboortedatum
            type: date
        actions:
          - output: geboortedatum
            value: $geboortedatum
"#;
        let caller = r#"
$id: null_caller
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Caller
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: partner_bsn
            type: string
            required: false
            nullable: true
        input:
          - name: partner_geboortedatum
            type: date
            nullable: true
            source:
              regulation: null_brp
              output: geboortedatum
              parameters:
                bsn: $partner_bsn
        output:
          - name: heeft_partner_geboortedatum
            type: boolean
        actions:
          - output: heeft_partner_geboortedatum
            operation: NOT
            value:
              operation: EQUALS
              subject: $partner_geboortedatum
              value: null
"#;
        let mut service = LawExecutionService::new();
        service.load_law(brp).unwrap();
        service.load_law(caller).unwrap();

        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("123".to_string()));
        params.insert("partner_bsn".to_string(), Value::Null);
        let result = service
            .evaluate_law_output(
                "null_caller",
                "heeft_partner_geboortedatum",
                params,
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("heeft_partner_geboortedatum"),
            Some(&Value::Bool(false))
        );
    }

    #[test]
    fn test_scoped_data_source_does_not_shadow_other_law() {
        // wet_bron computes `inkomen` from a raw register column that happens to
        // carry the same name. wet_afnemer takes `inkomen` cross-law from
        // wet_bron. The register source is bound to wet_bron, so wet_afnemer's
        // input must still come from the computed output (raw * 2), not from
        // the raw column.
        let bron = r#"
$id: wet_bron
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bron
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: inkomen
            type: number
            source: {}
        output:
          - name: inkomen
            type: number
        actions:
          - output: inkomen
            operation: MULTIPLY
            values:
              - $inkomen
              - 2
"#;
        let afnemer = r#"
$id: wet_afnemer
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Afnemer
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: inkomen
            type: number
            source:
              regulation: wet_bron
              output: inkomen
              parameters:
                bsn: $bsn
        output:
          - name: resultaat
            type: number
        actions:
          - output: resultaat
            value: $inkomen
"#;
        let mut service = LawExecutionService::new();
        service.load_law(bron).unwrap();
        service.load_law(afnemer).unwrap();

        let mut record = BTreeMap::new();
        record.insert("bsn".to_string(), Value::String("123".to_string()));
        record.insert("inkomen".to_string(), Value::Int(500));
        service
            .register_dict_source_for_law("wet_bron", "register", "bsn", vec![record], 10)
            .unwrap();

        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("123".to_string()));
        let result = service
            .evaluate_law_output("wet_afnemer", "resultaat", params.clone(), "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("resultaat"), Some(&Value::Int(1000)));

        // A higher-priority scoped source (a citizen correction) overrides the
        // register for the same law.
        let mut claim = BTreeMap::new();
        claim.insert("bsn".to_string(), Value::String("123".to_string()));
        claim.insert("inkomen".to_string(), Value::Int(700));
        service
            .register_dict_source_for_law("wet_bron", "claims", "bsn", vec![claim], 100)
            .unwrap();
        let result = service
            .evaluate_law_output("wet_afnemer", "resultaat", params, "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("resultaat"), Some(&Value::Int(1400)));
    }

    #[test]
    fn test_data_registry_fallback_to_cross_law() {
        // Registry has no matching field → cross-law resolution should still work
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();

        // Register a data source with an unrelated field
        let mut record = BTreeMap::new();
        record.insert("key".to_string(), Value::String("x".to_string()));
        record.insert("unrelated_field".to_string(), Value::Int(999));

        service
            .register_dict_source("unrelated_data", "key", vec![record])
            .unwrap();

        // Execute dependent law - should fall back to cross-law resolution
        let result = service
            .evaluate_law_output(
                "dependent_law",
                "doubled_value",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        // doubled_value = base_value (100) * 2 = 200
        assert_eq!(result.outputs.get("doubled_value"), Some(&Value::Int(200)));
    }

    #[test]
    fn test_parameters_take_priority_over_registry() {
        // Both a parameter and a registry entry exist for the same field.
        // The parameter should win because it's checked first.
        let law = r#"
$id: priority_test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Tests parameter vs registry priority
    machine_readable:
      execution:
        parameters:
          - name: BSN
            type: string
          - name: external_value
            type: number
        input:
          - name: external_value
            type: number
            source:
              regulation: some_law
              output: some_output
              parameters:
                BSN: $BSN
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_value
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        // Register data source with external_value = 100
        let mut record = BTreeMap::new();
        record.insert("BSN".to_string(), Value::String("123".to_string()));
        record.insert("external_value".to_string(), Value::Int(100));

        service
            .register_dict_source("test_data", "BSN", vec![record])
            .unwrap();

        // Pass external_value = 50 as parameter (should win)
        let mut params = BTreeMap::new();
        params.insert("BSN".to_string(), Value::String("123".to_string()));
        params.insert("external_value".to_string(), Value::Int(50));

        let result = service
            .evaluate_law_output("priority_test_law", "result", params, "2025-01-01")
            .unwrap();

        // Parameter value (50) should win over registry value (100)
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(50)));
    }

    // -------------------------------------------------------------------------
    // IoC (open_terms + implements) Tests
    // -------------------------------------------------------------------------

    fn make_law_with_open_term() -> &'static str {
        r#"
$id: zorgtoeslag_ioc
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: "$standaardpremie"
"#
    }

    fn make_implementing_regulation() -> &'static str {
        r#"
$id: regeling_sp_ioc
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1928
    machine_readable:
      implements:
        - law: zorgtoeslag_ioc
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1928
"#
    }

    #[test]
    fn test_ioc_resolve_open_term() {
        let mut service = LawExecutionService::new();
        service.load_law(make_law_with_open_term()).unwrap();
        service.load_law(make_implementing_regulation()).unwrap();

        let result = service
            .evaluate_law_output(
                "zorgtoeslag_ioc",
                "standaardpremie",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(
            result.outputs.get("standaardpremie"),
            Some(&Value::Int(1928))
        );
    }

    /// End to end: a beleidsregel that declares it fills a term the law reserves
    /// for a ministeriële regeling leaves that term unresolved. The execution
    /// fails on the required open term, not on the beleidsregel's value —
    /// nobody the law authorised has spoken.
    #[test]
    fn test_ioc_unauthorised_layer_leaves_required_open_term_unresolved() {
        let mut service = LawExecutionService::new();
        service.load_law(make_law_with_open_term()).unwrap();
        service
            .load_law(
                r#"
$id: beleidsregel_sp_ioc
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 2500
    machine_readable:
      implements:
        - law: zorgtoeslag_ioc
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 2500
"#,
            )
            .unwrap();

        let result = service.evaluate_law_output(
            "zorgtoeslag_ioc",
            "standaardpremie",
            BTreeMap::new(),
            "2025-01-01",
        );

        match result {
            Err(EngineError::ResolutionError(msg)) => {
                assert!(
                    msg.contains("standaardpremie") && msg.contains("no implementation"),
                    "expected an unresolved-open-term error, got: {msg}"
                );
            }
            other => panic!("expected ResolutionError, got: {other:?}"),
        }
    }

    #[test]
    fn test_ioc_unknown_implementation_output_does_not_take_the_default() {
        // The default of an open term stands in for an absent deviation (no
        // verordening, or one that says nothing for this case). It never
        // stands in for a deviation nobody has yet: an implementation whose
        // own register value is unknown leaves the term unknown, so the
        // caller asks for the missing fact instead of silently applying the
        // statutory value (RFC-036).
        let bw = r#"
$id: bw_afstand_u
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '42'
    text: Afstand
    machine_readable:
      open_terms:
        - id: gemeentelijke_afstand_cm
          type: number
          required: false
          delegation_type: GEMEENTELIJKE_VERORDENING
          default:
            actions:
              - output: gemeentelijke_afstand_cm
                value: 200
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
        output:
          - name: minimale_afstand_cm
            type: number
        actions:
          - output: minimale_afstand_cm
            value: $gemeentelijke_afstand_cm
"#;
        let apv = r#"
$id: apv_afstand_u
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
gemeente_code: GM0363
articles:
  - number: '2.75'
    text: Afstand uit het beleid
    machine_readable:
      implements:
        - law: bw_afstand_u
          article: '42'
          open_term: gemeentelijke_afstand_cm
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
        input:
          - name: afstand_uit_beleid
            type: number
            source: {}
        output:
          - name: gemeentelijke_afstand_cm
            type: number
        actions:
          - output: gemeentelijke_afstand_cm
            value: $afstand_uit_beleid
"#;
        let mut service = LawExecutionService::new();
        service.load_law(bw).unwrap();
        service.load_law(apv).unwrap();
        // No data source holds the policy distance: the implementation's
        // output is unknown for lack of it.
        let result = service
            .evaluate_law_output_with_trace(
                "bw_afstand_u",
                "minimale_afstand_cm",
                params(&[("gemeente_code", Value::String("GM0363".to_string()))]),
                "2025-01-01",
            )
            .unwrap();
        let outcome = result.outputs.get("minimale_afstand_cm").unwrap();
        assert!(outcome.is_unknown(), "expected unknown, got {outcome:?}");
        assert_eq!(outcome.missing_facts()[0].name, "afstand_uit_beleid");
        assert!(!trace_resolves_by(
            result.trace.as_ref().unwrap(),
            &ResolveType::OpenTermSilent
        ));
    }

    #[test]
    fn test_ioc_silent_implementation_falls_back_to_the_default() {
        // BW 5:42: the statutory distance applies "tenzij ingevolge een
        // verordening een kleinere afstand is toegelaten". The Amsterdam APV
        // allows one for hedges and says nothing about trees: for a tree its
        // output is absent, no deviation was allowed, and the open term takes
        // the delegating law's default. Without a verordening at all the
        // default applies as well.
        let bw = r#"
$id: bw_afstand
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '42'
    text: Afstand
    machine_readable:
      open_terms:
        - id: gemeentelijke_afstand_cm
          type: number
          required: false
          delegation_type: GEMEENTELIJKE_VERORDENING
          default:
            actions:
              - output: gemeentelijke_afstand_cm
                value:
                  operation: IF
                  cases:
                    - when:
                        operation: EQUALS
                        subject: $type_beplanting
                        value: boom
                      then: 200
                  default: 50
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
          - name: type_beplanting
            type: string
            required: true
        output:
          - name: minimale_afstand_cm
            type: number
        actions:
          - output: minimale_afstand_cm
            value: $gemeentelijke_afstand_cm
"#;
        let apv = r#"
$id: apv_afstand
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
gemeente_code: GM0363
articles:
  - number: '2.75'
    text: Heggen
    machine_readable:
      implements:
        - law: bw_afstand
          article: '42'
          open_term: gemeentelijke_afstand_cm
      execution:
        parameters:
          - name: type_beplanting
            type: string
            required: true
        output:
          - name: gemeentelijke_afstand_cm
            type: number
            nullable: true
        actions:
          - output: gemeentelijke_afstand_cm
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $type_beplanting
                    value: heg
                  then: 30
"#;
        let mut service = LawExecutionService::new();
        service.load_law(bw).unwrap();
        service.load_law(apv).unwrap();
        let distance = |gemeente: &str, beplanting: &str| {
            let params = params(&[
                ("gemeente_code", Value::String(gemeente.to_string())),
                ("type_beplanting", Value::String(beplanting.to_string())),
            ]);
            service
                .evaluate_law_output_with_trace(
                    "bw_afstand",
                    "minimale_afstand_cm",
                    params,
                    "2025-01-01",
                )
                .unwrap()
        };
        // The verordening deviates for hedges.
        assert_eq!(
            distance("GM0363", "heg").outputs.get("minimale_afstand_cm"),
            Some(&Value::Int(30))
        );
        // It is silent for trees: the default applies, and the trace says why.
        let tree = distance("GM0363", "boom");
        assert_eq!(
            tree.outputs.get("minimale_afstand_cm"),
            Some(&Value::Int(200))
        );
        assert!(trace_mentions(
            tree.trace.as_ref().unwrap(),
            "using default value: implementation apv_afstand article 2.75 is silent for this case"
        ));
        // No verordening in this municipality: the default applies too.
        assert_eq!(
            distance("GM9999", "heg").outputs.get("minimale_afstand_cm"),
            Some(&Value::Int(50))
        );
        // The trace marks the silent case distinctly from an implemented one
        // and from a plain default.
        assert!(trace_resolves_by(
            tree.trace.as_ref().unwrap(),
            &ResolveType::OpenTermSilent
        ));
        let hedge = distance("GM0363", "heg");
        assert!(!trace_resolves_by(
            hedge.trace.as_ref().unwrap(),
            &ResolveType::OpenTermSilent
        ));
        let elsewhere = distance("GM9999", "heg");
        assert!(!trace_resolves_by(
            elsewhere.trace.as_ref().unwrap(),
            &ResolveType::OpenTermSilent
        ));
    }

    /// A norm decided per case that a regulation fills: the regulation's
    /// value is the norm's, null included, and no finding takes its place
    /// (RFC-048 §2). Optional, null is the value; required, the regulation
    /// that may give null does not load (N5), and a finding given for the
    /// norm is asked by none.
    #[test]
    fn test_a_norm_decided_per_case_that_a_regulation_fills_takes_its_null() {
        let wet = |required: bool| {
            format!(
                r#"
$id: wet_verlof
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent verlof als de grond aanwezig is
    machine_readable:
      open_terms:
        - id: grond_aanwezig
          type: boolean
          required: {required}
          decided_per_case_by: het hoofd
      execution:
        output:
          - name: verlof_verleend
            type: boolean
            nullable: true
        actions:
          - output: verlof_verleend
            value: $grond_aanwezig
"#
            )
        };
        let beleid = r#"
$id: beleid_verlof_zwijgt
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid-zwijgt
articles:
  - number: '1'
    text: Over de grond zegt dit beleid niets
    machine_readable:
      implements:
        - {law: wet_verlof, article: '1', open_term: grond_aanwezig}
      execution:
        output:
          - {name: grond_aanwezig, type: boolean, nullable: true}
        actions:
          - {output: grond_aanwezig, value: null}
"#;
        let mut service = LawExecutionService::new();
        service.load_law(&wet(false)).unwrap();
        service.load_law(beleid).unwrap();
        let result = service
            .evaluate_law_with_trace(
                "wet_verlof",
                &["verlof_verleend"],
                BTreeMap::new(),
                "2026-10-13",
            )
            .unwrap();
        assert_eq!(result.outputs["verlof_verleend"], Value::Null);
        let root = result.trace.unwrap();
        assert!(
            trace_mentions(
                &root,
                "Open term 'grond_aanwezig' implemented by beleid_verlof_zwijgt article 1"
            ),
            "{}",
            root.render_box_drawing()
        );
        assert!(!trace_resolves_by(&root, &ResolveType::OpenTermSilent));
        let e = service
            .evaluate_law_with_findings(
                "wet_verlof",
                &["verlof_verleend"],
                BTreeMap::new(),
                BTreeMap::from([("grond_aanwezig".to_string(), Value::Bool(true))]),
                "2026-10-13",
            )
            .unwrap_err();
        assert!(e.to_string().contains("is asked by none"), "{e}");

        let mut service = LawExecutionService::new();
        service.load_law(&wet(true)).unwrap();
        let e = service.load_law(beleid).unwrap_err();
        assert!(e.to_string().contains("[N5]"), "{e}");
    }

    #[test]
    fn test_ioc_a_filling_implementation_wins_over_a_silent_one_whatever_the_order() {
        // Two verordeningen of one municipality implement the same term. The
        // newer one (lex posterior, first candidate) is silent for trees, the
        // older one has a rule for them: the older one's value applies, not
        // the delegating law's default. With the ages swapped the outcome is
        // the same.
        let bw = r#"
$id: bw_twee
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '42'
    text: Afstand
    machine_readable:
      open_terms:
        - id: afstand_cm
          type: number
          required: false
          delegation_type: GEMEENTELIJKE_VERORDENING
          default:
            actions:
              - output: afstand_cm
                value: 200
      execution:
        parameters:
          - name: gemeente_code
            type: string
            required: true
          - name: type_beplanting
            type: string
            required: true
        output:
          - name: minimale_afstand_cm
            type: number
        actions:
          - output: minimale_afstand_cm
            value: $afstand_cm
"#;
        let apv = |id: &str, valid_from: &str, case: &str, value: i32| {
            format!(
                r#"
$id: {id}
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2024-01-01'
valid_from: '{valid_from}'
gemeente_code: GM0363
articles:
  - number: '1'
    text: Afstand
    machine_readable:
      implements:
        - law: bw_twee
          article: '42'
          open_term: afstand_cm
      execution:
        parameters:
          - name: type_beplanting
            type: string
            required: true
        output:
          - name: afstand_cm
            type: number
            nullable: true
        actions:
          - output: afstand_cm
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $type_beplanting
                    value: {case}
                  then: {value}
"#
            )
        };
        for (hedges_from, trees_from) in
            [("2025-01-01", "2024-06-01"), ("2024-06-01", "2025-01-01")]
        {
            let mut service = LawExecutionService::new();
            service.load_law(bw).unwrap();
            service
                .load_law(&apv("apv_heggen", hedges_from, "heg", 30))
                .unwrap();
            service
                .load_law(&apv("apv_bomen", trees_from, "boom", 100))
                .unwrap();
            let distance = |beplanting: &str| {
                service
                    .evaluate_law_output(
                        "bw_twee",
                        "minimale_afstand_cm",
                        params(&[
                            ("gemeente_code", Value::String("GM0363".to_string())),
                            ("type_beplanting", Value::String(beplanting.to_string())),
                        ]),
                        "2025-06-01",
                    )
                    .unwrap()
            };
            assert_eq!(
                distance("boom").outputs.get("minimale_afstand_cm"),
                Some(&Value::Int(100)),
                "hedges from {hedges_from}, trees from {trees_from}"
            );
            assert_eq!(
                distance("heg").outputs.get("minimale_afstand_cm"),
                Some(&Value::Int(30))
            );
            // Both silent: the default, and the trace names both.
            let result = service
                .evaluate_law_output_with_trace(
                    "bw_twee",
                    "minimale_afstand_cm",
                    params(&[
                        ("gemeente_code", Value::String("GM0363".to_string())),
                        ("type_beplanting", Value::String("struik".to_string())),
                    ]),
                    "2025-06-01",
                )
                .unwrap();
            assert_eq!(
                result.outputs.get("minimale_afstand_cm"),
                Some(&Value::Int(200))
            );
            let trace = result.trace.as_ref().unwrap();
            assert!(trace_mentions(trace, "apv_heggen article 1"));
            assert!(trace_mentions(trace, "apv_bomen article 1"));
        }
    }

    #[test]
    fn test_ioc_required_no_implementation() {
        let mut service = LawExecutionService::new();
        service.load_law(make_law_with_open_term()).unwrap();
        // No implementing regulation loaded

        let result = service.evaluate_law_output(
            "zorgtoeslag_ioc",
            "standaardpremie",
            BTreeMap::new(),
            "2025-01-01",
        );

        assert!(
            matches!(result, Err(EngineError::ResolutionError(_))),
            "Expected ResolutionError for missing required implementation, got: {:?}",
            result
        );
    }

    #[test]
    fn test_ioc_optional_no_implementation() {
        let yaml = r#"
$id: optional_term_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Optional open term
    machine_readable:
      open_terms:
        - id: bijzondere_premie
          type: amount
          required: false
      execution:
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: 42
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();

        // Should succeed — optional term not implemented is fine
        let result = service
            .evaluate_law_output("optional_term_law", "result", BTreeMap::new(), "2025-01-01")
            .unwrap();

        assert_eq!(result.outputs.get("result"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_ioc_with_default() {
        let yaml = r#"
$id: default_term_law
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Default open term
    machine_readable:
      open_terms:
        - id: redelijk_percentage
          type: number
          required: true
          default:
            actions:
              - output: redelijk_percentage
                value: 6
      execution:
        output:
          - name: redelijk_percentage
            type: number
        actions:
          - output: redelijk_percentage
            value: "$redelijk_percentage"
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();

        // No implementation loaded — should fall back to default
        let result = service
            .evaluate_law_output(
                "default_term_law",
                "redelijk_percentage",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(
            result.outputs.get("redelijk_percentage"),
            Some(&Value::Int(6))
        );
    }

    /// The law reserves the term for a ministeriële regeling and carries last
    /// year's amount as a default. A beleidsregel offers this year's amount and
    /// is refused. The default runs — but the refusal is on the receipt and in
    /// the trace, so someone contesting the decision can read that a filling
    /// was offered and why it did not count. Before, the beleidsregel vanished
    /// without a word and the trace said only "using default".
    #[test]
    fn test_ioc_refused_layer_is_recorded_while_the_default_runs() {
        let law_yaml = r#"
$id: wet_met_default
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
          default:
            actions:
              - output: standaardpremie
                value: 1889
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: "$standaardpremie"
"#;
        let beleidsregel_yaml = r#"
$id: beleidsregel_met_default
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1928
    machine_readable:
      implements:
        - law: wet_met_default
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1928
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law_yaml).unwrap();
        service.load_law(beleidsregel_yaml).unwrap();

        // Without a trace: the refusal still travels on the result, and from
        // there onto the receipt.
        let result = service
            .evaluate_law_output(
                "wet_met_default",
                "standaardpremie",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("standaardpremie"),
            Some(&Value::Int(1889)),
            "the default runs: no authorised regulation filled the term"
        );
        assert!(result.trace.is_none(), "this run asked for no trace");
        assert_eq!(result.delegation_refusals.len(), 1);
        let refusal = &result.delegation_refusals[0];
        assert_eq!(refusal.refused_law, "beleidsregel_met_default");
        assert_eq!(refusal.refused_layer, "BELEIDSREGEL");
        assert_eq!(refusal.required_layer, "MINISTERIELE_REGELING");
        assert_eq!(refusal.open_term, "standaardpremie");

        let receipt =
            service.build_receipt_with_outputs(&result, &BTreeMap::new(), "2025-01-01", &[]);
        assert_eq!(
            receipt.results.delegation_refusals, result.delegation_refusals,
            "the receipt carries the refusal even without a trace"
        );

        // With a trace: the refusal is a node under the open term, next to the
        // default that ran in its place.
        let traced = service
            .evaluate_law_output_with_trace(
                "wet_met_default",
                "standaardpremie",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(traced.delegation_refusals.len(), 1);
        let rendered = traced
            .trace
            .as_ref()
            .expect("a traced run has a trace")
            .render_box_drawing();
        assert!(
            rendered.contains("beleidsregel_met_default")
                && rendered.contains("may not fill open term 'standaardpremie'"),
            "the refusal must be readable in the trace:\n{rendered}"
        );
    }

    #[test]
    fn test_ioc_implementation_overrides_default() {
        let law_yaml = r#"
$id: default_override_law
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Default open term
    machine_readable:
      open_terms:
        - id: percentage
          type: number
          required: true
          default:
            actions:
              - output: percentage
                value: 6
      execution:
        output:
          - name: percentage
            type: number
        actions:
          - output: percentage
            value: "$percentage"
"#;

        let impl_yaml = r#"
$id: override_implementation
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-06-01'
valid_from: '2025-06-01'
articles:
  - number: '1'
    text: Override percentage
    machine_readable:
      implements:
        - law: default_override_law
          article: '1'
          open_term: percentage
      execution:
        output:
          - name: percentage
            type: number
        actions:
          - output: percentage
            value: 4
"#;

        let mut service = LawExecutionService::new();
        service.load_law(law_yaml).unwrap();
        service.load_law(impl_yaml).unwrap();

        // Implementation should override default
        let result = service
            .evaluate_law_output(
                "default_override_law",
                "percentage",
                BTreeMap::new(),
                "2025-07-01",
            )
            .unwrap();

        assert_eq!(result.outputs.get("percentage"), Some(&Value::Int(4)));
    }

    #[test]
    fn test_ioc_temporal_filtering() {
        // Two versions of the same implementing regulation (same $id, different valid_from).
        // The engine should select the version valid for the calculation date.
        let higher_law = r#"
$id: test_higher_law
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Test article with open term
    machine_readable:
      open_terms:
        - id: yearly_amount
          type: number
          required: true
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $yearly_amount
"#;

        let impl_v2025 = r#"
$id: test_impl_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-11-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: 2025 amount
    machine_readable:
      implements:
        - law: test_higher_law
          article: '1'
          open_term: yearly_amount
      execution:
        output:
          - name: yearly_amount
            type: number
        actions:
          - output: yearly_amount
            value: 211200
"#;

        let impl_v2026 = r#"
$id: test_impl_regulation
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-11-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: 2026 amount
    machine_readable:
      implements:
        - law: test_higher_law
          article: '1'
          open_term: yearly_amount
      execution:
        output:
          - name: yearly_amount
            type: number
        actions:
          - output: yearly_amount
            value: 220000
"#;

        let mut service = LawExecutionService::new();
        service.load_law(higher_law).unwrap();
        service.load_law(impl_v2025).unwrap();
        service.load_law(impl_v2026).unwrap();

        // Calculate for 2025: should use the 2025 version
        let result = service
            .evaluate_law_output("test_higher_law", "result", BTreeMap::new(), "2025-06-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(211200)),
            "2025 calculation should use 2025 version"
        );

        // Calculate for 2026: should use the 2026 version
        let result = service
            .evaluate_law_output("test_higher_law", "result", BTreeMap::new(), "2026-06-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("result"),
            Some(&Value::Int(220000)),
            "2026 calculation should use 2026 version"
        );
    }

    // -------------------------------------------------------------------------
    // Memoization cache identity
    // -------------------------------------------------------------------------

    #[test]
    fn test_cache_key_depends_on_parameter_values() {
        // The per-execution memo cache is keyed on a hash of law, output and
        // parameters. The parameter *values* must reach that hash: without
        // them every call with the same parameter names lands on one key and
        // the identity check has to reject nearly every hit.
        let key_of = |value: Value| {
            let mut params = BTreeMap::new();
            params.insert("bsn".to_string(), value);
            cache_key("zorgtoeslag", "recht_op_zorgtoeslag", &params)
        };

        assert_ne!(key_of(Value::Int(1)), key_of(Value::Int(2)));
        assert_ne!(key_of(Value::Bool(true)), key_of(Value::Bool(false)));
        assert_ne!(
            key_of(Value::String("111".to_string())),
            key_of(Value::String("222".to_string()))
        );
        assert_ne!(
            key_of(Value::Array(vec![Value::Int(1)])),
            key_of(Value::Array(vec![Value::Int(2)]))
        );
        let obj = |v: i64| {
            let mut map = BTreeMap::new();
            map.insert("field".to_string(), Value::Int(v));
            Value::Object(map)
        };
        assert_ne!(key_of(obj(1)), key_of(obj(2)));

        // Equal decimals hash equally, matching PartialEq (1.0 == 1.00).
        assert_eq!(
            key_of(Value::Decimal("1.0".parse().unwrap())),
            key_of(Value::Decimal("1.00".parse().unwrap()))
        );

        // Law and output are part of the identity too.
        let empty = BTreeMap::new();
        assert_ne!(
            cache_key("law_a", "output", &empty),
            cache_key("law_b", "output", &empty)
        );
        assert_ne!(
            cache_key("law_a", "output_x", &empty),
            cache_key("law_a", "output_y", &empty)
        );
    }

    #[test]
    fn test_cache_entry_matches_requires_every_identity_field() {
        // A cache hit is only honoured when law, output, parameters and the
        // way the output is read match.
        // The key is a u64 hash, so a collision must never hand one decision
        // the outputs of another.
        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("111".to_string()));

        let entry = CacheEntry {
            law_id: "law_a".to_string(),
            output_name: "output_x".to_string(),
            outputs: BTreeMap::new(),
            output_provenance: BTreeMap::new(),
            parameters: params.clone(),
        };

        assert!(entry.matches("law_a", "output_x", &params));
        assert!(
            !entry.matches("law_b", "output_x", &params),
            "another law must never reuse this entry"
        );
        assert!(
            !entry.matches("law_a", "output_y", &params),
            "another output must never reuse this entry"
        );

        let mut other_params = params.clone();
        other_params.insert("bsn".to_string(), Value::String("222".to_string()));
        assert!(
            !entry.matches("law_a", "output_x", &other_params),
            "other parameters must never reuse this entry"
        );
    }

    // -------------------------------------------------------------------------
    // Resolution depth accounting
    // -------------------------------------------------------------------------

    /// Build `links` laws where `chain_law_{i}` sources `chain_law_{i+1}`, plus
    /// a terminal law with a literal value. Each link costs one depth level.
    fn cross_law_chain(links: usize) -> Vec<String> {
        let mut laws = Vec::new();
        for i in 0..links {
            let next = i + 1;
            laws.push(format!(
                r#"
$id: chain_law_{i}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Chain link {i}
    machine_readable:
      execution:
        input:
          - name: from_next
            type: number
            source:
              regulation: chain_law_{next}
              output: value_{next}
        output:
          - name: value_{i}
            type: number
        actions:
          - output: value_{i}
            value: $from_next
"#
            ));
        }
        laws.push(format!(
            r#"
$id: chain_law_{links}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Chain terminal
    machine_readable:
      execution:
        output:
          - name: value_{links}
            type: number
        actions:
          - output: value_{links}
            value: 7
"#
        ));
        laws
    }

    #[test]
    fn test_service_cross_law_depth_limit_boundary() {
        // The depth limit exists to stop runaway recursion, not to reject
        // chains the configuration declares admissible: exactly
        // MAX_CROSS_LAW_DEPTH hops must resolve, one hop more must not.
        let mut service = LawExecutionService::new();
        for law in cross_law_chain(config::MAX_CROSS_LAW_DEPTH) {
            service.load_law(&law).unwrap();
        }
        let result = service
            .evaluate_law_output("chain_law_0", "value_0", BTreeMap::new(), "2025-01-01")
            .expect("a chain of exactly MAX_CROSS_LAW_DEPTH hops must resolve");
        assert_eq!(result.outputs.get("value_0"), Some(&Value::Int(7)));

        let mut service = LawExecutionService::new();
        for law in cross_law_chain(config::MAX_CROSS_LAW_DEPTH + 1) {
            service.load_law(&law).unwrap();
        }
        let result =
            service.evaluate_law_output("chain_law_0", "value_0", BTreeMap::new(), "2025-01-01");
        match result {
            Err(EngineError::CircularReference(msg)) => {
                assert!(
                    msg.contains("depth exceeded"),
                    "Expected depth-exceeded error, got: {}",
                    msg
                );
            }
            other => panic!("Expected CircularReference depth error, got: {:?}", other),
        }
    }

    #[test]
    fn test_service_internal_depth_limit_boundary() {
        // Same boundary for same-law (internal) references: a chain of exactly
        // MAX_CROSS_LAW_DEPTH links stays within budget.
        let links = config::MAX_CROSS_LAW_DEPTH;
        let mut yaml = String::from(
            "$id: internal_max_chain_law\n\
             regulatory_layer: WET\n\
             publication_date: '2025-01-01'\n\
             articles:\n",
        );
        for i in 0..links {
            yaml.push_str(&format!(
                r#"  - number: '{num}'
    text: Chain link {num}
    machine_readable:
      execution:
        input:
          - name: from_next
            type: number
            source:
              output: value_{next}
        output:
          - name: value_{num}
            type: number
        actions:
          - output: value_{num}
            value: $from_next
"#,
                num = i,
                next = i + 1
            ));
        }
        yaml.push_str(&format!(
            r#"  - number: '{links}'
    text: Chain terminal
    machine_readable:
      execution:
        output:
          - name: value_{links}
            type: number
        actions:
          - output: value_{links}
            value: 3
"#
        ));

        let mut service = LawExecutionService::new();
        service.load_law(&yaml).unwrap();

        let result = service
            .evaluate_law_output(
                "internal_max_chain_law",
                "value_0",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("a chain of exactly MAX_CROSS_LAW_DEPTH internal links must resolve");
        assert_eq!(result.outputs.get("value_0"), Some(&Value::Int(3)));
    }

    #[test]
    fn test_service_sibling_cross_law_references_do_not_consume_depth() {
        // Breadth is not depth. Every sibling reference of one article resolves
        // at depth one, so leaving a resolution scope has to hand the budget
        // back; otherwise a wide article trips the recursion guard.
        let siblings = config::MAX_CROSS_LAW_DEPTH + 5;
        let mut service = LawExecutionService::new();
        for i in 0..siblings {
            service
                .load_law(&format!(
                    r#"
$id: leaf_law_{i}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Leaf {i}
    machine_readable:
      execution:
        output:
          - name: leaf_value
            type: number
        actions:
          - output: leaf_value
            value: 1
"#
                ))
                .unwrap();
        }

        let mut yaml = String::from(
            "$id: wide_law\n\
             regulatory_layer: WET\n\
             publication_date: '2025-01-01'\n\
             articles:\n  \
             - number: '1'\n    \
             text: References many laws side by side\n    \
             machine_readable:\n      \
             execution:\n        \
             input:\n",
        );
        for i in 0..siblings {
            yaml.push_str(&format!(
                "          - name: leaf_{i}\n            type: number\n            source:\n              regulation: leaf_law_{i}\n              output: leaf_value\n"
            ));
        }
        yaml.push_str(
            "        output:\n          - name: total\n            type: number\n        actions:\n          - output: total\n            operation: ADD\n            values:\n",
        );
        for i in 0..siblings {
            yaml.push_str(&format!("              - $leaf_{i}\n"));
        }
        service.load_law(&yaml).unwrap();

        let result = service
            .evaluate_law_output("wide_law", "total", BTreeMap::new(), "2025-01-01")
            .expect("sibling references must not consume the depth budget");
        assert_eq!(
            result.outputs.get("total"),
            Some(&Value::Int(siblings as i64))
        );
    }

    // -------------------------------------------------------------------------
    // Multi-output evaluation
    // -------------------------------------------------------------------------

    fn make_two_article_law() -> &'static str {
        r#"
$id: two_article_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Produces the first output
    machine_readable:
      execution:
        output:
          - name: out_a
            type: number
        actions:
          - output: out_a
            value: 1
  - number: '2'
    text: Produces the second output
    machine_readable:
      execution:
        output:
          - name: out_b
            type: number
        actions:
          - output: out_b
            value: 2
"#
    }

    #[test]
    fn test_evaluate_law_reports_every_producing_article() {
        // Outputs from two articles are merged into one result. The result then
        // names both articles: a single number would tell the reader the
        // decision rests on an article that produced only half of it.
        let mut service = LawExecutionService::new();
        service.load_law(make_two_article_law()).unwrap();

        let result = service
            .evaluate_law(
                "two_article_law",
                &["out_a", "out_b"],
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();

        assert_eq!(result.outputs.get("out_a"), Some(&Value::Int(1)));
        assert_eq!(result.outputs.get("out_b"), Some(&Value::Int(2)));
        assert_eq!(result.article_number, "1, 2");

        // A single article keeps its own number.
        let result = service
            .evaluate_law("two_article_law", &["out_b"], BTreeMap::new(), "2025-01-01")
            .unwrap();
        assert_eq!(result.article_number, "2");
    }

    // -------------------------------------------------------------------------
    // Lex specialis overrides
    // -------------------------------------------------------------------------

    /// A regulation that voids the very term it implements gave the caller
    /// "did not produce output named 'x'", which reads as a defect in a
    /// regulation that is correct: it did answer, and the answer is that the
    /// value does not arise.
    #[test]
    fn an_implementation_that_voids_its_own_term_says_so() {
        let delegating = r#"
$id: void_term_delegating
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief wordt bij ministeriele regeling vastgesteld.
    machine_readable:
      open_terms:
        - id: tarief
          type: number
          required: true
          delegated_to: Onze Minister
      execution:
        output:
          - name: uitkomst
            type: number
        actions:
          - output: uitkomst
            value: $tarief
"#;
        let implementation = r#"
$id: void_term_regeling
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief bedraagt honderd euro.
    machine_readable:
      implements:
        - law: void_term_delegating
          article: '1'
          open_term: tarief
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: 10000
  - number: '2'
    text: In afwijking van artikel 1 bestaat geen tarief voor deze categorie.
    machine_readable:
      overrides:
        - law: void_term_regeling
          article: '1'
          output: tarief
          voids: true
          legal_text_excerpt: bestaat geen tarief
"#;
        let mut service = LawExecutionService::new();
        service.load_law(delegating).unwrap();
        service.load_law(implementation).unwrap();

        match service.evaluate_law_output(
            "void_term_delegating",
            "uitkomst",
            BTreeMap::new(),
            "2025-01-01",
        ) {
            Err(EngineError::OutputVoided { grounds, .. }) => {
                assert_eq!(grounds, "bestaat geen tarief");
            }
            Err(EngineError::InvalidOperation(message)) => {
                panic!("a voided term must not read as a missing output: {message}")
            }
            Ok(r) => panic!("expected the void to surface, got {:?}", r.outputs),
            Err(other) => panic!("expected OutputVoided, got {other:?}"),
        }
    }

    /// An override from another law stays scoped to the execution that law
    /// started. Widening the rule for an intra-law void must not widen this:
    /// a Vreemdelingenwet exclusion on an Awb term is still not a
    /// Participatiewet case (RFC-007, amended by RFC-041).
    #[test]
    fn a_void_from_another_law_still_needs_its_context() {
        let granting = r#"
$id: void_scope_granting
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Bestaat aanspraak ter grootte van dat verschil.
    machine_readable:
      execution:
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: 155045
"#;
        let excluder = r#"
$id: void_scope_excluder
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan bestaat geen aanspraak.
    machine_readable:
      overrides:
        - law: void_scope_granting
          article: '2'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
        let mut service = LawExecutionService::new();
        service.load_law(granting).unwrap();
        service.load_law(excluder).unwrap();

        // Asked outside the excluding law's execution, the general rule holds.
        let value = service
            .evaluate_law_output(
                "void_scope_granting",
                "aanspraak",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("a cross-law void outside its context does not apply")
            .outputs
            .get("aanspraak")
            .cloned();
        assert_eq!(value, Some(Value::from(155045)));
    }

    /// Two overrides on one output are a contradiction, and since an intra-law
    /// override is no longer filtered out by context, the pair can now meet.
    /// One law saying the entitlement does not arise while another says it is
    /// a different amount cannot be settled by declaration order: that would
    /// pay out on a ground nobody wrote. The error names both laws.
    #[test]
    fn an_intra_law_void_and_a_contextual_replacement_are_a_contradiction() {
        let granting = r#"
$id: void_clash_granting
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Bestaat aanspraak ter grootte van dat verschil.
    machine_readable:
      execution:
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: 155045
  - number: '3'
    text: In afwijking van artikel 2 bestaat geen aanspraak.
    machine_readable:
      overrides:
        - law: void_clash_granting
          article: '2'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
        let other = r#"
$id: void_clash_other
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: In afwijking daarvan bedraagt de aanspraak de helft.
    machine_readable:
      overrides:
        - law: void_clash_granting
          article: '2'
          output: aanspraak
      execution:
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: 77522
  - number: '2'
    text: Leest de aanspraak uit de andere wet.
    machine_readable:
      execution:
        input:
          - name: aanspraak
            type: number
            source:
              regulation: void_clash_granting
              output: aanspraak
        output:
          - name: doorgegeven
            type: number
        actions:
          - output: doorgegeven
            value: $aanspraak
"#;
        let mut service = LawExecutionService::new();
        service.load_law(granting).unwrap();
        service.load_law(other).unwrap();

        // Asked through the other law, so its replacement is in context, while
        // the granting law's own void applies regardless of route. Both reach
        // the same output, which is the pair that could not meet before.
        let result = service.evaluate_law_output(
            "void_clash_other",
            "doorgegeven",
            BTreeMap::new(),
            "2025-01-01",
        );
        match result {
            Err(EngineError::InvalidOperation(message)) => {
                assert!(
                    message.contains("void_clash_granting") && message.contains("void_clash_other"),
                    "the error must name both laws, got: {message}"
                );
            }
            Ok(r) => panic!(
                "two overrides on one output must not silently pick one, got {:?}",
                r.outputs
            ),
            Err(other) => panic!("expected InvalidOperation, got {other:?}"),
        }
    }

    /// The traced entry point must answer the same as the untraced one.
    ///
    /// It is the path the editor's scenario runner, the demo, the TUI and the
    /// BDD harness take, so a void that only surfaces without a trace leaves
    /// the silence this error exists to remove exactly where a human looks: a
    /// green run with an empty result panel and no ground.
    #[test]
    fn a_traced_run_reports_a_void_like_an_untraced_one() {
        let law = r#"
$id: void_traced_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Bestaat aanspraak ter grootte van dat verschil.
    machine_readable:
      execution:
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: 155045
  - number: '3'
    text: In afwijking van artikel 2 bestaat geen aanspraak indien het vermogen de grens overschrijdt.
    machine_readable:
      overrides:
        - law: void_traced_law
          article: '2'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();

        let untraced = service.evaluate_law_output(
            "void_traced_law",
            "aanspraak",
            BTreeMap::new(),
            "2025-01-01",
        );
        assert!(
            matches!(untraced, Err(EngineError::OutputVoided { .. })),
            "untraced: expected OutputVoided, got {untraced:?}"
        );

        let traced = service.evaluate_law_output_with_trace(
            "void_traced_law",
            "aanspraak",
            BTreeMap::new(),
            "2025-01-01",
        );
        match traced {
            // The traced path wraps its errors to carry the path that produced
            // them, so the void sits inside.
            Err(EngineError::TracedError { source, .. }) => match *source {
                EngineError::OutputVoided { grounds, .. } => {
                    assert_eq!(grounds, "bestaat geen aanspraak");
                }
                other => panic!("traced: expected OutputVoided inside, got {other:?}"),
            },
            Ok(r) => panic!("traced: a voided entitlement yielded {:?}", r.outputs),
            Err(other) => panic!("traced: expected a traced OutputVoided, got {other:?}"),
        }
    }

    /// A hook's voided co-product keeps its ground on the way out.
    ///
    /// A hook fires inside another law's evaluation and its outputs are merged
    /// into that law's result. The merge walks the produced values, and a
    /// voided output is by construction not among them, so its ground was
    /// carried over inside `fire_hooks` and then dropped again at the merge.
    /// The caller saw a co-product that was simply absent: no value, no
    /// provenance, nothing a receipt could point at.
    ///
    /// This is reachable precisely because an intra-law override now applies
    /// whatever the contextual law is. Inside a hook the contextual law is the
    /// triggering law, so under the old scoping the hook law's own override
    /// never fired here at all.
    ///
    /// The requested output is unaffected: a hook co-product is never itself a
    /// requestable output of the triggering law, so the top-level check does
    /// not cover this. AWB 1:3 treats the beschikking as indivisible, which is
    /// why the co-product has to be accounted for rather than quietly missing.
    #[test]
    fn a_hook_that_voids_its_own_output_still_reports_the_ground() {
        // The triggering law produces a BESCHIKKING, which is what the hook
        // attaches to.
        let triggering = r#"
$id: hook_void_trigger_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        // The hook law produces a co-product and voids it in the same law.
        let hook_law = r#"
$id: hook_void_hook_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bij de beschikking wordt een toeslag vastgesteld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: toeslag
            type: number
        actions:
          - output: toeslag
            value: 42
  - number: '2'
    text: In afwijking van artikel 1 bestaat geen aanspraak op de toeslag.
    machine_readable:
      overrides:
        - law: hook_void_hook_law
          article: '1'
          output: toeslag
          voids: true
          legal_text_excerpt: bestaat geen aanspraak op de toeslag
"#;
        let mut service = LawExecutionService::new();
        service.load_law(triggering).unwrap();
        service.load_law(hook_law).unwrap();

        let result = service
            .evaluate_law_output(
                "hook_void_trigger_law",
                "bedrag",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("the requested output is not the voided one");

        // The co-product carries no value, which was already true.
        assert_eq!(
            result.outputs.get("toeslag"),
            None,
            "a voided co-product must not carry a value"
        );

        // What this test is about: the ground survives the merge. Before this
        // fix the name was absent from the provenance too, so nothing recorded
        // that the toeslag was excluded rather than never computed.
        match result.output_provenance.get("toeslag") {
            Some(OutputProvenance::Voided { grounds, .. }) => {
                assert_eq!(
                    grounds.as_deref(),
                    Some("bestaat geen aanspraak op de toeslag")
                );
            }
            Some(other) => panic!("expected Voided provenance, got {other:?}"),
            None => panic!(
                "the voided co-product lost its ground: provenance holds {:?}",
                result.output_provenance.keys().collect::<Vec<_>>()
            ),
        }

        // The carry-over only covers names that have no value. The requested
        // output does have one, and it keeps the provenance it earned rather
        // than being relabelled as voided. Both halves of the condition are
        // load-bearing: with the absence check dropped, an output that was
        // computed normally would be reported as excluded, which is the one
        // error worse than the silence this carry-over removes.
        assert_eq!(result.outputs.get("bedrag"), Some(&Value::Int(100)));
        assert!(
            !matches!(
                result.output_provenance.get("bedrag"),
                Some(OutputProvenance::Voided { .. })
            ),
            "an output that was computed must not be reported as voided, got {:?}",
            result.output_provenance.get("bedrag")
        );
    }

    /// A hook whose own output is produced keeps that value and its provenance.
    ///
    /// The mirror of the test above, and the reason the carry-over asks two
    /// questions rather than one. A hook co-product that the hook law does not
    /// void arrives with a value, and nothing about the void machinery may
    /// touch it: not its value, and not the `Reactive` provenance that says a
    /// hook produced it.
    #[test]
    fn a_hook_co_product_that_is_not_voided_keeps_its_value() {
        let triggering = r#"
$id: hook_live_trigger_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        // Same shape as the voided case, minus the override.
        let hook_law = r#"
$id: hook_live_hook_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bij de beschikking wordt een toeslag vastgesteld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: toeslag
            type: number
        actions:
          - output: toeslag
            value: 42
"#;
        let mut service = LawExecutionService::new();
        service.load_law(triggering).unwrap();
        service.load_law(hook_law).unwrap();

        let result = service
            .evaluate_law_output(
                "hook_live_trigger_law",
                "bedrag",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("nothing is voided here");

        assert_eq!(
            result.outputs.get("toeslag"),
            Some(&Value::Int(42)),
            "the co-product arrives with its value"
        );
        assert!(
            matches!(
                result.output_provenance.get("toeslag"),
                Some(OutputProvenance::Reactive { .. })
            ),
            "and says a hook produced it, got {:?}",
            result.output_provenance.get("toeslag")
        );
    }

    /// The exclusion holds however the question is asked.
    ///
    /// Scoping an override to the contextual law is right for a replacement:
    /// how much you get may differ per the law invoking it (RFC-007). A void
    /// is a different claim. "Bestaat geen aanspraak" says the entitlement
    /// does not arise, and that does not stop being true because a different
    /// law asked. Measured before this changed: asked through the law itself
    /// the millionaire got nothing, asked through the granting law directly
    /// or through a third law reading that output, the same person received
    /// € 1.550,45 with provenance reporting `Direct`.
    ///
    /// This applies to an article overriding an output of its own law, which
    /// is the shape of zorgtoeslag article 3 voiding article 2. Overrides
    /// from another law keep their RFC-007 scoping.
    #[test]
    fn an_intra_law_void_holds_however_the_question_is_asked() {
        let law = r#"
$id: void_intra_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Bestaat aanspraak ter grootte van dat verschil.
    machine_readable:
      execution:
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: 155045
  - number: '3'
    text: In afwijking van artikel 2 bestaat geen aanspraak indien het vermogen de grens overschrijdt.
    machine_readable:
      overrides:
        - law: void_intra_law
          article: '2'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
        let consumer = r#"
$id: void_third_reader
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Leest de aanspraak uit een andere wet.
    machine_readable:
      execution:
        input:
          - name: aanspraak
            type: number
            source:
              regulation: void_intra_law
              output: aanspraak
        output:
          - name: doorgegeven
            type: number
        actions:
          - output: doorgegeven
            value: $aanspraak
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        service.load_law(consumer).unwrap();

        // The granting law asked directly: the route a citizen-facing service
        // takes, and the one that used to hand out the amount.
        match service.evaluate_law_output(
            "void_intra_law",
            "aanspraak",
            BTreeMap::new(),
            "2025-01-01",
        ) {
            Err(EngineError::OutputVoided { grounds, .. }) => {
                assert_eq!(grounds, "bestaat geen aanspraak");
            }
            Ok(r) => panic!(
                "asked directly, a voided entitlement yielded {:?}",
                r.outputs
            ),
            Err(other) => panic!("asked directly, expected OutputVoided, got {other:?}"),
        }

        // A third law reading that output across the law boundary.
        match service.evaluate_law_output(
            "void_third_reader",
            "doorgegeven",
            BTreeMap::new(),
            "2025-01-01",
        ) {
            Err(EngineError::OutputVoided { grounds, .. }) => {
                assert_eq!(grounds, "bestaat geen aanspraak");
            }
            Ok(r) => panic!(
                "read across a law boundary, a voided entitlement yielded {:?}",
                r.outputs
            ),
            Err(other) => panic!("read cross-law, expected OutputVoided, got {other:?}"),
        }
    }

    #[test]
    fn a_voiding_override_removes_the_output_and_records_the_ground() {
        // The shape of Wet op de zorgtoeslag articles 2 and 3. Article 2
        // establishes an entitlement "ter grootte van dat verschil"; article 3
        // says "bestaat geen aanspraak" above a wealth threshold. That is not
        // an entitlement of zero: with zero there is a decision carrying legal
        // remedies and a ground for recovery, with none there is neither.
        //
        // Measured before this existed: a millionaire received both
        // `bestaat_aanspraak = false` and an amount of € 1.550,45, and nothing
        // obliged a consumer to read the first.
        let entitlement = r#"
$id: voiding_target_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Indien de normpremie lager is dan de standaardpremie, bestaat aanspraak ter grootte van dat verschil.
    machine_readable:
      execution:
        output:
          - name: aanspraak
            type: number
        actions:
          - output: aanspraak
            value: 155045
"#;
        let exclusion = r#"
$id: voiding_specialis_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Vraagt de aanspraak op.
    machine_readable:
      execution:
        input:
          - name: aanspraak
            type: number
            source:
              regulation: voiding_target_law
              output: aanspraak
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $aanspraak
  - number: '3'
    text: In afwijking daarvan bestaat geen aanspraak indien het vermogen de grens overschrijdt.
    machine_readable:
      overrides:
        - law: voiding_target_law
          article: '3'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
        // Article 3 addresses article 2, where the entitlement is produced.
        let addressed = exclusion.replace("article: '3'", "article: '2'");
        let mut service = LawExecutionService::new();
        service.load_law(entitlement).unwrap();
        service.load_law(&addressed).unwrap();

        // Asked through the law carrying the exclusion, because an override
        // applies from the contextual law.
        let result = service.evaluate_law_output(
            "voiding_specialis_law",
            "result",
            BTreeMap::new(),
            "2025-01-01",
        );

        // The entitlement does not exist, so a consumer asking for it gets no
        // number. Failing here is the point: an amount that the law does not
        // recognise must not travel on quietly, and before this change the
        // millionaire received € 1.550,45 without a word.
        match result {
            Err(EngineError::OutputVoided {
                output, grounds, ..
            }) => {
                assert_eq!(output, "aanspraak");
                assert_eq!(grounds, "bestaat geen aanspraak");
            }
            Ok(r) => panic!(
                "a voided entitlement must not yield a value, got {:?}",
                r.outputs
            ),
            Err(other) => panic!("expected OutputVoided, got {other:?}"),
        }

        // Without the exclusion the same chain does produce the amount, which
        // shows the void is doing the work rather than a broken binding.
        let mut service = LawExecutionService::new();
        service.load_law(entitlement).unwrap();
        service
            .load_law(&addressed.replace("voids: true", "voids: false"))
            .unwrap();
        let value = service
            .evaluate_law_output(
                "voiding_specialis_law",
                "result",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("without the void the chain completes");
        assert_eq!(value.outputs.get("result"), Some(&Value::Int(155045)));
    }

    #[test]
    fn test_conflicting_overrides_from_one_law_are_rejected() {
        // Two articles of the same law both claim to replace the same output.
        // There is no rule that picks between them, so the engine must refuse
        // rather than silently apply whichever came first.
        let target = r#"
$id: override_target_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Provides the amount
    machine_readable:
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let specialis = r#"
$id: override_specialis_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Uses the amount from the target law
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: number
            source:
              regulation: override_target_law
              output: bedrag
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $bedrag
  - number: '2'
    text: In afwijking van het bedrag
    machine_readable:
      overrides:
        - law: override_target_law
          article: '1'
          output: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 200
  - number: '3'
    text: Eveneens in afwijking van hetzelfde bedrag
    machine_readable:
      overrides:
        - law: override_target_law
          article: '1'
          output: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 300
"#;
        let mut service = LawExecutionService::new();
        service.load_law(target).unwrap();
        service.load_law(specialis).unwrap();

        let result = service.evaluate_law_output(
            "override_specialis_law",
            "result",
            BTreeMap::new(),
            "2025-01-01",
        );

        match result {
            Err(EngineError::InvalidOperation(msg)) => {
                assert!(
                    msg.contains("Multiple overrides"),
                    "Expected a conflicting-overrides error, got: {}",
                    msg
                );
            }
            other => panic!("Expected conflicting overrides to be rejected, got: {other:?}"),
        }
    }

    /// Build the pair of laws for the mis-addressing tests.
    ///
    /// `stray` is an extra override declaration that the specialis article
    /// carries *before* the one that really addresses the output under test. It
    /// voids, and it names a target that differs from that output on exactly one
    /// of the three addressing fields. An article may carry several override
    /// declarations, so the engine has to read all three fields; matching on
    /// fewer would let a void meant for one output take another one away.
    fn misaddressed_void_laws(stray: &str) -> (String, String) {
        let target = r#"
$id: misaddress_target_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag bedraagt 100.
    machine_readable:
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#
        .to_string();
        let specialis = format!(
            r#"
$id: misaddress_specialis_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Vraagt het bedrag op.
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: number
            source:
              regulation: misaddress_target_law
              output: bedrag
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $bedrag
  - number: '2'
    text: In afwijking van het bedrag.
    machine_readable:
      overrides:
{stray}        - law: misaddress_target_law
          article: '1'
          output: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 200
"#
        );
        (target, specialis)
    }

    /// The stray declaration must not void: `result` is the replacement value,
    /// not an absence.
    fn assert_misaddressed_void_does_not_bite(stray: &str) {
        let (target, specialis) = misaddressed_void_laws(stray);
        let mut service = LawExecutionService::new();
        service.load_law(&target).unwrap();
        service.load_law(&specialis).unwrap();

        let result = service
            .evaluate_law_output(
                "misaddress_specialis_law",
                "result",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("a void addressed elsewhere must leave this output standing");
        assert_eq!(result.outputs.get("result"), Some(&Value::Int(200)));
    }

    #[test]
    fn a_void_for_another_law_does_not_remove_this_output() {
        assert_misaddressed_void_does_not_bite(
            "        - law: misaddress_other_law\n          article: '1'\n          output: bedrag\n          voids: true\n          legal_text_excerpt: bestaat geen aanspraak\n",
        );
    }

    #[test]
    fn a_void_for_another_article_does_not_remove_this_output() {
        assert_misaddressed_void_does_not_bite(
            "        - law: misaddress_target_law\n          article: '9'\n          output: bedrag\n          voids: true\n          legal_text_excerpt: bestaat geen aanspraak\n",
        );
    }

    #[test]
    fn a_void_for_another_output_does_not_remove_this_output() {
        assert_misaddressed_void_does_not_bite(
            "        - law: misaddress_target_law\n          article: '1'\n          output: ander_bedrag\n          voids: true\n          legal_text_excerpt: bestaat geen aanspraak\n",
        );
    }

    /// The other side of the same coin: when all three fields address this
    /// output, the value goes and the ground stays.
    #[test]
    fn a_void_that_addresses_this_output_removes_it_with_its_ground() {
        let target = r#"
$id: void_provenance_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Er bestaat aanspraak, en de toelichting hoort erbij.
    machine_readable:
      execution:
        output:
          - name: aanspraak
            type: number
          - name: toelichting
            type: number
        actions:
          - output: aanspraak
            value: 155045
          - output: toelichting
            value: 1
  - number: '2'
    text: In afwijking daarvan bestaat geen aanspraak.
    machine_readable:
      overrides:
        - law: void_provenance_law
          article: '1'
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak
"#;
        let mut service = LawExecutionService::new();
        service.load_law(target).unwrap();

        // Asking for the neighbouring output does not compute the entitlement
        // at all (RFC-043): it depends on nothing, so the result carries no
        // value and no provenance for it.
        let result = service
            .evaluate_law(
                "void_provenance_law",
                &["toelichting"],
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("the article still produces its other output");
        assert_eq!(result.outputs.get("toelichting"), Some(&Value::Int(1)));
        assert!(!result.outputs.contains_key("aanspraak"));
        assert!(!result.output_provenance.contains_key("aanspraak"));

        // Asking for the entitlement itself gets the ground (RFC-041).
        match service.evaluate_law(
            "void_provenance_law",
            &["aanspraak"],
            BTreeMap::new(),
            "2025-01-01",
        ) {
            Err(EngineError::OutputVoided {
                law_id,
                voided_by,
                article,
                grounds,
                ..
            }) => {
                assert_eq!(law_id, "void_provenance_law");
                assert_eq!(voided_by, "void_provenance_law");
                assert_eq!(article, "2");
                assert_eq!(grounds, "bestaat geen aanspraak");
            }
            other => panic!("expected OutputVoided with its ground, got {other:?}"),
        }
    }

    /// A `post_actions` hook receives the parameters it declares from the
    /// article, so an output it declares is computed even when the caller asks
    /// for another one (RFC-043): here the hook reads `grondslag`, which the
    /// requested `bedrag` does not depend on.
    #[test]
    fn an_output_a_post_hook_declares_is_computed() {
        let triggering = r#"
$id: demand_hook_trigger_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast op grond van de grondslag.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - name: bedrag
            type: number
          - name: grondslag
            type: number
        actions:
          - output: bedrag
            value: 100
          - output: grondslag
            value: 7
"#;
        let hook_law = r#"
$id: demand_hook_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bij de beschikking wordt de grondslag vermeld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        parameters:
          - name: grondslag
            type: number
            required: true
        output:
          - name: vermelde_grondslag
            type: number
        actions:
          - output: vermelde_grondslag
            value: $grondslag
"#;
        let mut service = LawExecutionService::new();
        service.load_law(triggering).unwrap();
        service.load_law(hook_law).unwrap();

        let result = service
            .evaluate_law_output(
                "demand_hook_trigger_law",
                "bedrag",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("the hook finds the output it reads");
        assert_eq!(result.outputs.get("bedrag"), Some(&Value::Int(100)));
        assert_eq!(
            result.outputs.get("vermelde_grondslag"),
            Some(&Value::Int(7))
        );
    }

    // -------------------------------------------------------------------------
    // Procedure stages (RFC-008)
    // -------------------------------------------------------------------------

    /// Build a procedure-bearing law: one article producing a TEST_BESCHIKKING
    /// with `outputs`, governed by a procedure whose stages are given as
    /// `(stage name, required external inputs)` pairs.
    fn make_stage_law(law_id: &str, stages: &[(&str, &[&str])], outputs: &[&str]) -> String {
        let mut yaml = String::new();
        yaml.push_str(&format!("$id: {law_id}\n"));
        yaml.push_str("regulatory_layer: WET\n");
        yaml.push_str("publication_date: '2025-01-01'\n");
        yaml.push_str("procedure:\n");
        yaml.push_str("  - id: test_procedure\n");
        yaml.push_str("    default: true\n");
        yaml.push_str("    applies_to:\n");
        yaml.push_str("      legal_character: TEST_BESCHIKKING\n");
        yaml.push_str("    stages:\n");
        for (name, requires) in stages {
            yaml.push_str(&format!("      - name: {name}\n"));
            if !requires.is_empty() {
                yaml.push_str("        requires:\n");
                for req in *requires {
                    yaml.push_str(&format!("          - name: {req}\n"));
                    yaml.push_str("            type: string\n");
                }
            }
        }
        yaml.push_str("articles:\n");
        yaml.push_str("  - number: '1'\n");
        yaml.push_str("    text: Neemt een beschikking\n");
        yaml.push_str("    machine_readable:\n");
        yaml.push_str("      execution:\n");
        yaml.push_str("        produces:\n");
        yaml.push_str("          legal_character: TEST_BESCHIKKING\n");
        yaml.push_str("        parameters:\n");
        yaml.push_str("          - name: bedrag\n");
        yaml.push_str("            type: number\n");
        yaml.push_str("            required: true\n");
        yaml.push_str("        output:\n");
        for out in outputs {
            yaml.push_str(&format!("          - name: {out}\n"));
            yaml.push_str("            type: number\n");
        }
        yaml.push_str("        actions:\n");
        for out in outputs {
            yaml.push_str(&format!("          - output: {out}\n"));
            yaml.push_str("            value: $bedrag\n");
        }
        yaml
    }

    fn stage_params(pairs: &[(&str, &str)]) -> BTreeMap<String, Value> {
        let mut params = BTreeMap::new();
        params.insert("bedrag".to_string(), Value::Int(100));
        for (name, value) in pairs {
            params.insert(name.to_string(), Value::String(value.to_string()));
        }
        params
    }

    #[test]
    fn test_execute_stage_yields_on_a_missing_input_of_the_current_stage() {
        // The first stage needs an external input the caller did not supply, so
        // the procedure must stop there and name exactly that input.
        let law = make_stage_law(
            "stage_law_yield",
            &[("AANVRAAG", &["aanvraag_datum"]), ("BESLUIT", &[])],
            &["toekenning"],
        );
        let mut service = LawExecutionService::new();
        service.load_law(&law).unwrap();

        let outcome = service
            .execute_stage(
                "stage_law_yield",
                "toekenning",
                None,
                stage_params(&[]),
                "2025-01-01",
            )
            .unwrap();

        match outcome {
            ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
            } => {
                assert_eq!(
                    state.current_stage, "AANVRAAG",
                    "execution must halt in the stage whose input is missing"
                );
                assert_eq!(state.procedure_id, "test_procedure");
                assert_eq!(state.contextual_law, "stage_law_yield");
                assert_eq!(pending_inputs, vec!["aanvraag_datum".to_string()]);
                assert!(
                    outputs.is_empty(),
                    "nothing is decided before the stage has run"
                );
            }
            other => panic!("Expected a yield on the missing stage input, got: {other:?}"),
        }
    }

    #[test]
    fn test_execute_stage_runs_the_stage_and_yields_on_the_next_one() {
        // With the first stage's input supplied, that stage runs and its
        // outputs travel with the yield for the next stage's missing input.
        let law = make_stage_law(
            "stage_law_advance",
            &[
                ("AANVRAAG", &["aanvraag_datum"]),
                ("BESLUIT", &["besluit_datum"]),
            ],
            &["toekenning"],
        );
        let mut service = LawExecutionService::new();
        service.load_law(&law).unwrap();

        let outcome = service
            .execute_stage(
                "stage_law_advance",
                "toekenning",
                None,
                stage_params(&[("aanvraag_datum", "2025-01-05")]),
                "2025-01-01",
            )
            .unwrap();

        match outcome {
            ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
            } => {
                assert_eq!(
                    state.current_stage, "BESLUIT",
                    "the first stage ran, so the state moved on"
                );
                assert_eq!(pending_inputs, vec!["besluit_datum".to_string()]);
                assert_eq!(
                    outputs.get("toekenning"),
                    Some(&Value::Int(100)),
                    "the completed stage's outputs travel with the yield"
                );
            }
            other => panic!("Expected a yield awaiting the next stage's input, got: {other:?}"),
        }
    }

    #[test]
    fn test_execute_stage_next_stage_input_from_an_earlier_stage_output() {
        // A stage may require what an earlier stage produced. That value is in
        // the accumulated outputs, so the procedure runs on to the end instead
        // of asking the caller for it.
        let law = make_stage_law(
            "stage_law_accumulated",
            &[
                ("AANVRAAG", &["aanvraag_datum"]),
                ("BESLUIT", &["beschikking_nummer"]),
            ],
            &["toekenning", "beschikking_nummer"],
        );
        let mut service = LawExecutionService::new();
        service.load_law(&law).unwrap();

        let outcome = service
            .execute_stage(
                "stage_law_accumulated",
                "toekenning",
                None,
                stage_params(&[("aanvraag_datum", "2025-01-05")]),
                "2025-01-01",
            )
            .unwrap();

        match outcome {
            ExecutionOutcome::Complete(result) => {
                assert_eq!(result.outputs.get("toekenning"), Some(&Value::Int(100)));
                assert_eq!(
                    result.outputs.get("beschikking_nummer"),
                    Some(&Value::Int(100)),
                    "the final result carries the outputs of every stage"
                );
            }
            other => panic!("Expected the procedure to run to completion, got: {other:?}"),
        }
    }

    #[test]
    fn test_execute_stage_resume_asks_only_for_what_is_still_missing() {
        // An input supplied with the first call stays available across the
        // yield: on resuming, the engine asks only for what it does not have.
        let law = make_stage_law(
            "stage_law_resume",
            &[
                ("AANVRAAG", &["aanvraag_datum"]),
                ("BESLUIT", &["besluit_datum"]),
                ("BEKENDMAKING", &["zienswijze", "bekendmaking_datum"]),
            ],
            &["toekenning"],
        );
        let mut service = LawExecutionService::new();
        service.load_law(&law).unwrap();

        let first = service
            .execute_stage(
                "stage_law_resume",
                "toekenning",
                None,
                stage_params(&[("aanvraag_datum", "2025-01-05"), ("zienswijze", "geen")]),
                "2025-01-01",
            )
            .unwrap();

        let state = match first {
            ExecutionOutcome::Yielded {
                state,
                pending_inputs,
                ..
            } => {
                assert_eq!(state.current_stage, "BESLUIT");
                assert_eq!(pending_inputs, vec!["besluit_datum".to_string()]);
                state
            }
            other => panic!("Expected a yield awaiting besluit_datum, got: {other:?}"),
        };

        let mut resume = BTreeMap::new();
        resume.insert(
            "besluit_datum".to_string(),
            Value::String("2025-02-01".to_string()),
        );

        let second = service
            .execute_stage(
                "stage_law_resume",
                "toekenning",
                Some(state),
                resume,
                "2025-01-01",
            )
            .unwrap();

        match second {
            ExecutionOutcome::Yielded {
                state,
                pending_inputs,
                ..
            } => {
                assert_eq!(state.current_stage, "BEKENDMAKING");
                assert_eq!(
                    pending_inputs,
                    vec!["bekendmaking_datum".to_string()],
                    "zienswijze was supplied on the first call and must not be asked again"
                );
            }
            other => panic!("Expected a yield awaiting bekendmaking_datum, got: {other:?}"),
        }
    }

    /// An article that names a procedure no loaded law defines must not be
    /// executed as if it had none. Falling through would run the article once
    /// and return a decision that looks complete, while every stage the
    /// procedure imposes — the hearing, the notification, the objection period
    /// — silently did not happen. Those are the rights of the person the
    /// decision is about, so this is a refusal, not a warning.
    #[test]
    fn test_a_named_procedure_that_no_law_defines_stops_execution() {
        // The law defines `test_procedure`; the article asks for another one.
        let law = make_stage_law(
            "stage_law_named",
            &[("AANVRAAG", &[]), ("BESLUIT", &[])],
            &["toekenning"],
        )
        .replace(
            "        produces:\n          legal_character: TEST_BESCHIKKING\n",
            "        produces:\n          legal_character: TEST_BESCHIKKING\n          \
             procedure_id: uitgebreide_procedure\n",
        );
        let mut service = LawExecutionService::new();
        service.load_law(&law).unwrap();

        let outcome = service.execute_stage(
            "stage_law_named",
            "toekenning",
            None,
            stage_params(&[]),
            "2025-01-01",
        );

        match outcome {
            Err(EngineError::ResolutionError(msg)) => {
                assert!(
                    msg.contains("uitgebreide_procedure") && msg.contains("TEST_BESCHIKKING"),
                    "the error must name the procedure that was asked for: {msg}"
                );
            }
            other => panic!("expected a refusal, got: {other:?}"),
        }

        // The contrast: load a law that does define that procedure, and the
        // same article runs it. The refusal was about the missing definition,
        // not about naming a procedure at all.
        service
            .load_law(
                r#"
$id: uitgebreide_procedurewet
regulatory_layer: WET
publication_date: '2025-01-01'
procedure:
  - id: uitgebreide_procedure
    applies_to:
      legal_character: TEST_BESCHIKKING
    stages:
      - name: ZIENSWIJZE
        requires:
          - name: zienswijze_datum
            type: string
      - name: BESLUIT
articles: []
"#,
            )
            .unwrap();

        let outcome = service
            .execute_stage(
                "stage_law_named",
                "toekenning",
                None,
                stage_params(&[]),
                "2025-01-01",
            )
            .expect("with the procedure defined, the article runs it");
        match outcome {
            ExecutionOutcome::Yielded {
                state,
                pending_inputs,
                ..
            } => {
                assert_eq!(state.procedure_id, "uitgebreide_procedure");
                assert_eq!(state.current_stage, "ZIENSWIJZE");
                assert_eq!(pending_inputs, vec!["zienswijze_datum".to_string()]);
            }
            other => panic!("expected the procedure's first stage to yield, got: {other:?}"),
        }
    }

    /// A legal character with no procedure anywhere is an ordinary fact, not a
    /// defect: the article runs in one go. This is the answer the refusal above
    /// must stay distinguishable from.
    #[test]
    fn test_a_legal_character_without_any_procedure_still_executes() {
        let law = make_stage_law("stage_law_none", &[("BESLUIT", &[])], &["toekenning"]);
        // Strip the procedure block: the article still produces a legal
        // character, but nothing in the corpus gives it a lifecycle.
        let without_procedure = law
            .lines()
            .skip_while(|line| !line.starts_with("articles:"))
            .collect::<Vec<_>>()
            .join("\n");
        let yaml = format!(
            "$id: stage_law_none\nregulatory_layer: WET\npublication_date: '2025-01-01'\n{without_procedure}\n"
        );

        let mut service = LawExecutionService::new();
        service.load_law(&yaml).unwrap();

        let outcome = service
            .execute_stage(
                "stage_law_none",
                "toekenning",
                None,
                stage_params(&[]),
                "2025-01-01",
            )
            .expect("an article without a procedure executes in one go");

        match outcome {
            ExecutionOutcome::Complete(result) => {
                assert_eq!(result.outputs.get("toekenning"), Some(&Value::Int(100)));
            }
            other => panic!("expected a completed execution, got: {other:?}"),
        }
    }

    /// When the version in force on the calculation date is not the version the
    /// hooks/overrides/procedure indexes were built from, the execution says so
    /// — in the trace and on the receipt. Without that, "no motiveringsplicht"
    /// reads as a finding while it is an answer about another version.
    #[test]
    fn test_declarations_from_another_version_reach_the_receipt() {
        let mut service = LawExecutionService::new();
        service
            .load_law(
                r#"
$id: wet_versies
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Oude versie met een hook
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 1
"#,
            )
            .unwrap();
        service
            .load_law(
                r#"
$id: wet_versies
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Nieuwe versie zonder hook
    machine_readable:
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 2
"#,
            )
            .unwrap();

        // On a date the indexed version covers, nothing is noted.
        let current = service
            .evaluate_law_output("wet_versies", "bedrag", BTreeMap::new(), "2025-06-01")
            .unwrap();
        assert!(current.declaration_version_notes.is_empty());

        // On a date the older version covers, the answer is flagged.
        let historic = service
            .evaluate_law_output("wet_versies", "bedrag", BTreeMap::new(), "2024-06-01")
            .unwrap();
        assert_eq!(historic.outputs.get("bedrag"), Some(&Value::Int(1)));
        assert_eq!(historic.declaration_version_notes.len(), 1);
        let note = &historic.declaration_version_notes[0];
        assert_eq!(note.law_id, "wet_versies");
        assert_eq!(note.indexed_version.as_deref(), Some("2025-01-01"));
        assert_eq!(note.in_force_version.as_deref(), Some("2024-01-01"));

        let receipt =
            service.build_receipt_with_outputs(&historic, &BTreeMap::new(), "2024-06-01", &[]);
        assert_eq!(
            receipt.results.declaration_version_notes, historic.declaration_version_notes,
            "the note belongs on the receipt, not only in the log"
        );

        let traced = service
            .evaluate_law_output_with_trace("wet_versies", "bedrag", BTreeMap::new(), "2024-06-01")
            .unwrap();
        let rendered = traced
            .trace
            .as_ref()
            .expect("a traced run has a trace")
            .render_box_drawing();
        assert!(
            rendered.contains("wet_versies") && rendered.contains("not established"),
            "the trace must carry the note:\n{rendered}"
        );
    }

    // -------------------------------------------------------------------------
    // Declarations that exist but are not in force on the reference date
    // -------------------------------------------------------------------------

    /// A lex specialis that only commences next year is offered by the
    /// overrides index today. It is rightly not applied — but before, it was
    /// not applied *and* not mentioned: the citizen got the general rule's
    /// amount on a receipt in which no special rule ever existed.
    #[test]
    fn test_an_override_not_in_force_is_recorded_while_the_general_rule_runs() {
        let kaderwet = r#"
$id: kaderwet_bedrag
regulatory_layer: WET
publication_date: '2020-01-01'
valid_from: '2020-01-01'
procedure:
  - id: beschikking_procedure
    default: true
    applies_to:
      legal_character: TEST_BESCHIKKING
    stages:
      - name: BESLUIT
articles:
  - number: '1'
    text: Het bedrag bedraagt 100
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let bijzondere = r#"
$id: bijzondere_regeling
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2027-01-01'
articles:
  - number: '1'
    text: In afwijking van artikel 1 van de kaderwet bedraagt het bedrag 250
    machine_readable:
      overrides:
        - law: kaderwet_bedrag
          article: '1'
          output: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 250
"#;
        let mut service = LawExecutionService::new();
        service.load_law(kaderwet).unwrap();
        service.load_law(bijzondere).unwrap();

        // A procedure resumed on a date the special rule does not yet cover:
        // the caller carries the contextual law in the state it persisted.
        let state = |stage: &str| StageState {
            procedure_id: "beschikking_procedure".to_string(),
            contextual_law: "bijzondere_regeling".to_string(),
            current_stage: stage.to_string(),
            accumulated_outputs: BTreeMap::new(),
            parameters: BTreeMap::new(),
            report: Box::default(),
        };

        let outcome = service
            .execute_stage(
                "kaderwet_bedrag",
                "bedrag",
                Some(state("BESLUIT")),
                BTreeMap::new(),
                "2026-06-01",
            )
            .expect("the general rule is in force and executes");
        let result = match outcome {
            ExecutionOutcome::Complete(result) => result,
            other => panic!("expected a completed execution, got: {other:?}"),
        };
        assert_eq!(
            result.outputs.get("bedrag"),
            Some(&Value::Int(100)),
            "the special rule is not in force, so the general rule stands"
        );
        assert_eq!(
            result.declarations_not_in_force.len(),
            1,
            "the skipped lex specialis must leave a mark"
        );
        let note = &result.declarations_not_in_force[0];
        assert_eq!(note.kind, DeclarationKind::Override);
        assert_eq!(note.law_id, "bijzondere_regeling");
        assert_eq!(note.article, "1");
        assert!(
            note.subject.contains("output 'bedrag'") && note.subject.contains("kaderwet_bedrag"),
            "the note must name what would have been overridden: {}",
            note.subject
        );
        assert!(
            note.reason.contains("in force yet"),
            "the note must state the data fact, not a verdict: {}",
            note.reason
        );

        let receipt =
            service.build_receipt_with_outputs(&result, &BTreeMap::new(), "2026-06-01", &[]);
        assert_eq!(
            receipt.results.declarations_not_in_force, result.declarations_not_in_force,
            "the receipt carries the skip even without a trace"
        );

        // With a trace the same skip is a node next to the output it addresses.
        // `execute_stage_internal` takes the builder as it is, so the test
        // holds a root node of its own to read the tree back.
        let trace = Rc::new(RefCell::new(TraceBuilder::new()));
        trace
            .borrow_mut()
            .push("test harness".to_string(), PathNodeType::Article);
        service
            .execute_stage_internal(
                "kaderwet_bedrag",
                "bedrag",
                Some(state("BESLUIT")),
                BTreeMap::new(),
                "2026-06-01",
                Some(Rc::clone(&trace)),
            )
            .unwrap();
        let rendered = trace
            .borrow_mut()
            .pop()
            .expect("a traced run has a trace")
            .render_box_drawing();
        assert!(
            rendered.contains("bijzondere_regeling") && rendered.contains("Not applied: override"),
            "the trace must carry the skipped override, and say it was an \
             override rather than a hook or an implementation:\n{rendered}"
        );

        // Once it commences, the same call applies it — which shows the note
        // above marks a date and not a broken binding.
        let later = service
            .execute_stage(
                "kaderwet_bedrag",
                "bedrag",
                Some(state("BESLUIT")),
                BTreeMap::new(),
                "2027-06-01",
            )
            .unwrap();
        match later {
            ExecutionOutcome::Complete(result) => {
                assert_eq!(result.outputs.get("bedrag"), Some(&Value::Int(250)));
                assert!(result.declarations_not_in_force.is_empty());
            }
            other => panic!("expected a completed execution, got: {other:?}"),
        }
    }

    /// The special rule is in force, but the version in force today does not
    /// yet carry the article the overrides index points at: it arrives with a
    /// later version (or the article was renumbered). The general rule stands,
    /// and it used to stand with a trace that showed nothing else: the skip was
    /// a bare `continue`, before the trace node was even pushed.
    #[test]
    fn test_an_override_whose_article_is_missing_on_the_date_is_recorded() {
        let kaderwet = r#"
$id: kaderwet_bedrag
regulatory_layer: WET
publication_date: '2020-01-01'
valid_from: '2020-01-01'
procedure:
  - id: beschikking_procedure
    default: true
    applies_to:
      legal_character: TEST_BESCHIKKING
    stages:
      - name: BESLUIT
articles:
  - number: '1'
    text: Het bedrag bedraagt 100
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let bijzondere_oud = r#"
$id: bijzondere_regeling
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '2'
    text: Deze regeling treedt in werking op 1 januari 2024
"#;
        let bijzondere_nieuw = r#"
$id: bijzondere_regeling
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2027-01-01'
articles:
  - number: '1'
    text: In afwijking van artikel 1 van de kaderwet bedraagt het bedrag 250
    machine_readable:
      overrides:
        - law: kaderwet_bedrag
          article: '1'
          output: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 250
  - number: '2'
    text: Deze regeling treedt in werking op 1 januari 2024
"#;
        let mut service = LawExecutionService::new();
        service.load_law(kaderwet).unwrap();
        service.load_law(bijzondere_oud).unwrap();
        service.load_law(bijzondere_nieuw).unwrap();

        let state = || StageState {
            procedure_id: "beschikking_procedure".to_string(),
            contextual_law: "bijzondere_regeling".to_string(),
            current_stage: "BESLUIT".to_string(),
            accumulated_outputs: BTreeMap::new(),
            parameters: BTreeMap::new(),
            report: Box::default(),
        };

        let outcome = service
            .execute_stage(
                "kaderwet_bedrag",
                "bedrag",
                Some(state()),
                BTreeMap::new(),
                "2025-06-01",
            )
            .expect("the general rule is in force and executes");
        let result = match outcome {
            ExecutionOutcome::Complete(result) => result,
            other => panic!("expected a completed execution, got: {other:?}"),
        };
        assert_eq!(
            result.outputs.get("bedrag"),
            Some(&Value::Int(100)),
            "the article is not there on this date, so the general rule stands"
        );
        assert_eq!(
            result.declarations_not_in_force,
            vec![DeclarationNotInForce {
                kind: DeclarationKind::Override,
                law_id: "bijzondere_regeling".to_string(),
                article: "1".to_string(),
                subject: "output 'bedrag' of kaderwet_bedrag article 1".to_string(),
                reason: "the version of bijzondere_regeling in force on this date (valid_from 2024-01-01) \
                         has no article 1"
                    .to_string(),
            }],
            "the skipped lex specialis must leave a mark naming the version that lacks it"
        );
        assert!(
            result
                .declaration_version_notes
                .iter()
                .any(|n| n.law_id == "bijzondere_regeling"),
            "the receipt also says the indexes read another version of the special rule: {:?}",
            result.declaration_version_notes
        );

        let trace = Rc::new(RefCell::new(TraceBuilder::new()));
        trace
            .borrow_mut()
            .push("test harness".to_string(), PathNodeType::Article);
        service
            .execute_stage_internal(
                "kaderwet_bedrag",
                "bedrag",
                Some(state()),
                BTreeMap::new(),
                "2025-06-01",
                Some(Rc::clone(&trace)),
            )
            .unwrap();
        let root = trace.borrow_mut().pop().expect("a traced run has a trace");
        fn find_override(node: &crate::trace::PathNode) -> Option<&crate::trace::PathNode> {
            if node.node_type == PathNodeType::OverrideResolution {
                return Some(node);
            }
            node.children.iter().find_map(find_override)
        }
        let node = find_override(&root).unwrap_or_else(|| {
            panic!(
                "the trace must carry the skipped override:\n{}",
                root.render_box_drawing()
            )
        });
        assert_eq!(node.name, "bijzondere_regeling:1");
        assert_eq!(
            node.message.as_deref(),
            Some(
                "Not applied: override bijzondere_regeling article 1 would have applied to \
                 output 'bedrag' of kaderwet_bedrag article 1, but the version of bijzondere_regeling in \
                 force on this date (valid_from 2024-01-01) has no article 1"
            )
        );

        // Once the version carrying the article is in force, it applies.
        match service
            .execute_stage(
                "kaderwet_bedrag",
                "bedrag",
                Some(state()),
                BTreeMap::new(),
                "2027-06-01",
            )
            .unwrap()
        {
            ExecutionOutcome::Complete(result) => {
                assert_eq!(result.outputs.get("bedrag"), Some(&Value::Int(250)));
                assert!(result.declarations_not_in_force.is_empty());
            }
            other => panic!("expected a completed execution, got: {other:?}"),
        }
    }

    /// After a renumbering the special rule is in force under another number.
    /// The engine still does not apply it, but the reason must not let it pass
    /// for a rule that does not exist yet: it names the article that carries
    /// the declaration now. Only a declaration of exactly the same output of
    /// exactly the same article counts.
    #[test]
    fn test_article_missing_from_version_names_a_renumbered_declaration() {
        let law = ArticleBasedLaw::from_yaml_str(
            r#"
$id: bijzondere_regeling
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '3'
    text: Hernummerd
    machine_readable:
      overrides:
        - law: kaderwet_bedrag
          article: '1'
          output: bedrag
  - number: '4'
    text: Ander output
    machine_readable:
      overrides:
        - law: kaderwet_bedrag
          article: '1'
          output: iets_anders
  - number: '5'
    text: Ander artikel
    machine_readable:
      overrides:
        - law: kaderwet_bedrag
          article: '2'
          output: bedrag
  - number: '6'
    text: Andere wet
    machine_readable:
      overrides:
        - law: andere_wet
          article: '1'
          output: bedrag
  - number: '7'
    text: Geen machine_readable
"#,
        )
        .unwrap();
        assert_eq!(
            article_missing_from_version(&law, "1", "kaderwet_bedrag", "1", "bedrag"),
            "the version of bijzondere_regeling in force on this date (valid_from 2024-01-01) has no \
             article 1; article 3 of that version declares an override of the same output"
        );
    }

    /// A law without a `valid_from` is in force on any date; the note then
    /// names no version date rather than an empty one.
    #[test]
    fn test_article_missing_from_version_without_valid_from() {
        let law = ArticleBasedLaw::from_yaml_str(
            r#"
$id: zonder_datum
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '2'
    text: Tekst
"#,
        )
        .unwrap();
        assert_eq!(
            article_missing_from_version(&law, "1", "kaderwet", "1", "bedrag"),
            "the version of zonder_datum in force on this date has no article 1"
        );
    }

    /// A hook produces two values and a lex specialis replaces one of them, as
    /// Vw art. 69 does with the objection period of Awb 6:7. Only the replaced
    /// value takes the override as its ground; the other stays reactive.
    #[test]
    fn test_hook_output_keeps_override_provenance_only_where_overridden() {
        let besluit = r#"
$id: wet_beschikking
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
  - number: '2'
    text: In afwijking van artikel 7 van de termijnenwet bedraagt de bezwaartermijn vier weken
    machine_readable:
      overrides:
        - law: wet_termijnen
          article: '7'
          output: bezwaartermijn_weken
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 4
"#;
        let termijnen = r#"
$id: wet_termijnen
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '7'
    text: De bezwaartermijn en de beroepstermijn bedragen zes weken
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: TEST_BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
          - name: beroepstermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 6
          - output: beroepstermijn_weken
            value: 6
"#;
        let mut service = LawExecutionService::new();
        service.load_law(besluit).unwrap();
        service.load_law(termijnen).unwrap();

        let result = service
            .evaluate_law_output("wet_beschikking", "bedrag", BTreeMap::new(), "2025-06-01")
            .unwrap();

        assert_eq!(
            result.outputs.get("bezwaartermijn_weken"),
            Some(&Value::Int(4))
        );
        assert_eq!(
            result.output_provenance.get("bezwaartermijn_weken"),
            Some(&OutputProvenance::Override {
                law_id: "wet_beschikking".to_string(),
                article: "2".to_string(),
            })
        );
        assert_eq!(
            result.outputs.get("beroepstermijn_weken"),
            Some(&Value::Int(6))
        );
        assert_eq!(
            result.output_provenance.get("beroepstermijn_weken"),
            Some(&OutputProvenance::Reactive {
                law_id: "wet_termijnen".to_string(),
                article: "7".to_string(),
                hook_point: "post_actions".to_string(),
            })
        );
    }

    /// The motiveringsplicht commences next year. Today the beschikking comes
    /// out without a motivering, and the engine used to log "Hook law not
    /// found" — untrue, the law is loaded — and say nothing anywhere else.
    #[test]
    fn test_a_hook_not_in_force_is_recorded_instead_of_silently_skipped() {
        let besluit = r#"
$id: wet_beschikking
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let motivering = r#"
$id: wet_motiveringsplicht
regulatory_layer: WET
publication_date: '2025-06-01'
valid_from: '2026-01-01'
articles:
  - number: '3'
    text: Een beschikking wordt gemotiveerd
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: TEST_BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: motivering
            type: string
        actions:
          - output: motivering
            value: "gemotiveerd"
"#;
        let mut service = LawExecutionService::new();
        service.load_law(besluit).unwrap();
        service.load_law(motivering).unwrap();

        let before = service
            .evaluate_law_output("wet_beschikking", "bedrag", BTreeMap::new(), "2025-06-01")
            .unwrap();
        assert!(
            !before.outputs.contains_key("motivering"),
            "the duty to state reasons does not yet exist on this date"
        );
        assert_eq!(before.declarations_not_in_force.len(), 1);
        let note = &before.declarations_not_in_force[0];
        assert_eq!(note.kind, DeclarationKind::Hook);
        assert_eq!(note.law_id, "wet_motiveringsplicht");
        assert_eq!(note.article, "3");
        assert!(
            note.subject.contains("post_actions") && note.subject.contains("TEST_BESCHIKKING"),
            "the note must name the hook point it would have fired at: {}",
            note.subject
        );
        assert!(
            note.reason.contains("in force yet"),
            "'not found' was untrue: the law is loaded, this date is not covered: {}",
            note.reason
        );

        let receipt =
            service.build_receipt_with_outputs(&before, &BTreeMap::new(), "2025-06-01", &[]);
        assert_eq!(
            receipt.results.declarations_not_in_force, before.declarations_not_in_force,
            "a beschikking without its motivering must say so on the receipt"
        );

        // Once the duty commences the hook fires, so the note marks a date.
        let after = service
            .evaluate_law_output("wet_beschikking", "bedrag", BTreeMap::new(), "2026-06-01")
            .unwrap();
        assert_eq!(
            after.outputs.get("motivering"),
            Some(&Value::String("gemotiveerd".to_string()))
        );
        assert!(after.declarations_not_in_force.is_empty());
    }

    /// A procedure reports what each of its stages reported, not only the
    /// last: a hook not in force at the first stage stays on the result of
    /// the last, whether the stages run in one call or across a yield.
    #[test]
    fn test_a_procedure_reports_what_an_earlier_stage_reported() {
        let besluit = r#"
$id: wet_beschikking
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
procedure:
  - id: beschikking_procedure
    default: true
    applies_to:
      legal_character: TEST_BESCHIKKING
    stages:
      - name: AANVRAAG
      - name: BESLUIT
        requires:
          - name: ontvangen
            type: boolean
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let ontvangst = r#"
$id: wet_ontvangst
regulatory_layer: WET
publication_date: '2025-06-01'
valid_from: '2026-01-01'
articles:
  - number: '2'
    text: Van een aanvraag wordt de ontvangst bevestigd
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: TEST_BESCHIKKING
            stage: AANVRAAG
      execution:
        output:
          - name: ontvangst_bevestigd
            type: boolean
        actions:
          - output: ontvangst_bevestigd
            value: true
"#;
        let mut service = LawExecutionService::new();
        service.load_law(besluit).unwrap();
        service.load_law(ontvangst).unwrap();
        let date = "2025-06-01";
        let ontvangen = || BTreeMap::from([("ontvangen".to_string(), Value::Bool(true))]);
        let hook_note = |result: &ArticleResult| {
            result
                .declarations_not_in_force
                .iter()
                .filter(|n| n.kind == DeclarationKind::Hook && n.law_id == "wet_ontvangst")
                .count()
        };

        // The stage BESLUIT alone does not meet the hook.
        let besluit_only = service
            .execute_stage_at("wet_beschikking", "1", "BESLUIT", None, ontvangen(), date)
            .unwrap();
        assert_eq!(hook_note(&besluit_only), 0);

        // Both stages in one call.
        match service
            .execute_stage("wet_beschikking", "bedrag", None, ontvangen(), date)
            .unwrap()
        {
            ExecutionOutcome::Complete(result) => assert_eq!(hook_note(&result), 1),
            other => panic!("expected a completed execution, got: {other:?}"),
        }

        // Across a yield, with the state persisted as JSON in between.
        let state = match service
            .execute_stage("wet_beschikking", "bedrag", None, BTreeMap::new(), date)
            .unwrap()
        {
            ExecutionOutcome::Yielded { state, .. } => state,
            other => panic!("expected a yield for 'ontvangen', got: {other:?}"),
        };
        let state: StageState =
            serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        match service
            .execute_stage("wet_beschikking", "bedrag", Some(state), ontvangen(), date)
            .unwrap()
        {
            ExecutionOutcome::Complete(result) => assert_eq!(hook_note(&result), 1),
            other => panic!("expected a completed execution, got: {other:?}"),
        }

        // A state persisted before the report existed still resumes.
        let state: StageState = serde_json::from_value(serde_json::json!({
            "procedure_id": "beschikking_procedure",
            "contextual_law": "wet_beschikking",
            "current_stage": "BESLUIT",
            "accumulated_outputs": {},
            "parameters": {},
        }))
        .unwrap();
        assert_eq!(*state.report, StageReport::default());
    }

    /// A ministeriële regeling fills the open term from 2026. A calculation over
    /// 2025 finds no candidate and falls through to null — the same answer it
    /// gives when nobody ever wrote one. The regeling now stands in the record.
    #[test]
    fn test_an_implementation_not_in_force_is_recorded_before_falling_through() {
        let wet = r#"
$id: wet_met_open_term
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '2'
    text: Bij ministeriele regeling kan een afwijkende afstand worden vastgesteld
    machine_readable:
      open_terms:
        - id: afwijkende_afstand
          type: number
          required: false
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: afstand
            type: number
        actions:
          - output: afstand
            value:
              operation: IF
              cases:
                - when:
                    operation: NOT_NULL
                    subject: $afwijkende_afstand
                  then: $afwijkende_afstand
              default: 200
"#;
        let regeling = r#"
$id: regeling_afstand
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-12-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: De afwijkende afstand bedraagt 50
    machine_readable:
      implements:
        - law: wet_met_open_term
          article: '2'
          open_term: afwijkende_afstand
      execution:
        output:
          - name: afwijkende_afstand
            type: number
        actions:
          - output: afwijkende_afstand
            value: 50
"#;
        let mut service = LawExecutionService::new();
        service.load_law(wet).unwrap();
        service.load_law(regeling).unwrap();

        let before = service
            .evaluate_law_output(
                "wet_met_open_term",
                "afstand",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        assert_eq!(
            before.outputs.get("afstand"),
            Some(&Value::Int(200)),
            "the statutory distance stands while the regeling is not in force"
        );
        assert_eq!(before.declarations_not_in_force.len(), 1);
        let note = &before.declarations_not_in_force[0];
        assert_eq!(note.kind, DeclarationKind::Implementation);
        assert_eq!(note.law_id, "regeling_afstand");
        assert!(
            note.subject.contains("open term 'afwijkende_afstand'"),
            "the note must name the term it would have filled: {}",
            note.subject
        );
        assert!(note.reason.contains("in force yet"), "{}", note.reason);

        let receipt =
            service.build_receipt_with_outputs(&before, &BTreeMap::new(), "2025-06-01", &[]);
        assert_eq!(
            receipt.results.declarations_not_in_force,
            before.declarations_not_in_force
        );

        let traced = service
            .evaluate_law_output_with_trace(
                "wet_met_open_term",
                "afstand",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        let rendered = traced
            .trace
            .as_ref()
            .expect("a traced run has a trace")
            .render_box_drawing();
        assert!(
            rendered.contains("regeling_afstand") && rendered.contains("Not applied"),
            "the trace must show that a filling exists but not on this date:\n{rendered}"
        );

        let after = service
            .evaluate_law_output(
                "wet_met_open_term",
                "afstand",
                BTreeMap::new(),
                "2026-06-01",
            )
            .unwrap();
        assert_eq!(after.outputs.get("afstand"), Some(&Value::Int(50)));
        assert!(after.declarations_not_in_force.is_empty());
    }

    /// The hook law is in force, but the motiveringsplicht is an article that a
    /// later version inserts. Today the beschikking comes out without a
    /// motivering, and the skip used to leave a log line and nothing else: no
    /// trace node, no note, nothing on the receipt.
    #[test]
    fn test_a_hook_whose_article_is_missing_on_the_date_is_recorded() {
        let besluit = r#"
$id: wet_beschikking
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let motivering_oud = r#"
$id: wet_motiveringsplicht
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '2'
    text: Deze wet treedt in werking op 1 januari 2024
"#;
        let motivering_nieuw = r#"
$id: wet_motiveringsplicht
regulatory_layer: WET
publication_date: '2026-06-01'
valid_from: '2027-01-01'
articles:
  - number: '2'
    text: Deze wet treedt in werking op 1 januari 2024
  - number: '3'
    text: Een beschikking wordt gemotiveerd
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: TEST_BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: motivering
            type: string
        actions:
          - output: motivering
            value: "gemotiveerd"
"#;
        let mut service = LawExecutionService::new();
        service.load_law(besluit).unwrap();
        service.load_law(motivering_oud).unwrap();
        service.load_law(motivering_nieuw).unwrap();

        let before = service
            .evaluate_law_output("wet_beschikking", "bedrag", BTreeMap::new(), "2025-06-01")
            .unwrap();
        assert_eq!(before.outputs.get("bedrag"), Some(&Value::Int(100)));
        assert!(
            !before.outputs.contains_key("motivering"),
            "the article is not in the version in force, so the hook does not fire"
        );
        assert_eq!(
            before.declarations_not_in_force,
            vec![DeclarationNotInForce {
                kind: DeclarationKind::Hook,
                law_id: "wet_motiveringsplicht".to_string(),
                article: "3".to_string(),
                subject: "hook point post_actions on TEST_BESCHIKKING at stage BESLUIT".to_string(),
                reason: "the version of wet_motiveringsplicht in force on this date (valid_from 2024-01-01) \
                         has no article 3"
                    .to_string(),
            }],
            "the hook that did not fire must leave a note naming the version that lacks it"
        );

        let receipt =
            service.build_receipt_with_outputs(&before, &BTreeMap::new(), "2025-06-01", &[]);
        assert_eq!(
            receipt.results.declarations_not_in_force, before.declarations_not_in_force,
            "a beschikking without its motivering must say so on the receipt"
        );

        let traced = service
            .evaluate_law_output_with_trace(
                "wet_beschikking",
                "bedrag",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        let root = traced.trace.as_ref().expect("a traced run has a trace");
        fn find_hook(node: &crate::trace::PathNode) -> Option<&crate::trace::PathNode> {
            if node.node_type == PathNodeType::HookResolution {
                return Some(node);
            }
            node.children.iter().find_map(find_hook)
        }
        let node = find_hook(root).unwrap_or_else(|| {
            panic!(
                "the trace must carry the hook that did not fire:\n{}",
                root.render_box_drawing()
            )
        });
        assert_eq!(node.name, "wet_motiveringsplicht:3");
        assert_eq!(
            node.message.as_deref(),
            Some(
                "Not applied: hook wet_motiveringsplicht article 3 would have applied to \
                 hook point post_actions on TEST_BESCHIKKING at stage BESLUIT, but the \
                 version of wet_motiveringsplicht in force on this date (valid_from 2024-01-01) has no article 3"
            )
        );

        // Once the version carrying the article is in force, the hook fires.
        let after = service
            .evaluate_law_output("wet_beschikking", "bedrag", BTreeMap::new(), "2027-06-01")
            .unwrap();
        assert_eq!(
            after.outputs.get("motivering"),
            Some(&Value::String("gemotiveerd".to_string()))
        );
        assert!(after.declarations_not_in_force.is_empty());
    }

    /// After a renumbering the hook exists on this date under another number.
    /// It still does not fire (the undated index misses it, a separate
    /// question), but the reason names the article that carries it, so the skip
    /// does not pass for a safeguard that does not exist yet. What counts is a
    /// hook that would fire on this decision by the rules `find_hooks` applies
    /// (an absent stage is BESLUIT), and not an article the index offered
    /// itself, which was tried on its own and fired.
    #[test]
    fn test_a_renumbered_hook_is_named_in_the_reason() {
        let besluit = r#"
$id: wet_beschikking
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan stelt het bedrag vast
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#;
        let hook = |number: &str, hook_point: &str, filter: &str| {
            format!(
                r#"
  - number: '{number}'
    text: Een beschikking wordt gemotiveerd
    machine_readable:
      hooks:
        - hook_point: {hook_point}
          applies_to:
            {filter}
      execution:
        output:
          - name: motivering_{number}
            type: string
        actions:
          - output: motivering_{number}
            value: "gemotiveerd"
"#
            )
        };
        let header = |valid_from: &str| {
            format!(
                "$id: wet_motiveringsplicht\nregulatory_layer: WET\n\
                 publication_date: '{valid_from}'\nvalid_from: '{valid_from}'\narticles:"
            )
        };
        let besluit_filter = "legal_character: TEST_BESCHIKKING\n            stage: BESLUIT";
        let oud = [
            header("2024-01-01"),
            // Offered by the index itself, and fires: no renumbering.
            hook("4", "post_actions", besluit_filter),
            // Stage left out, which means BESLUIT: this one would fire here.
            hook("5", "post_actions", "legal_character: TEST_BESCHIKKING"),
            hook("6", "pre_actions", besluit_filter),
            hook(
                "7",
                "post_actions",
                "legal_character: TEST_BESCHIKKING\n            stage: BEKENDMAKING",
            ),
            hook(
                "8",
                "post_actions",
                "legal_character: ANDERE_BESCHIKKING\n            stage: BESLUIT",
            ),
            hook(
                "9",
                "post_actions",
                "legal_character: TEST_BESCHIKKING\n            decision_type: AFWIJZING",
            ),
        ]
        .concat();
        let nieuw = [
            header("2027-01-01"),
            hook("3", "post_actions", besluit_filter),
            hook("4", "post_actions", besluit_filter),
        ]
        .concat();
        let mut service = LawExecutionService::new();
        service.load_law(besluit).unwrap();
        service.load_law(&oud).unwrap();
        service.load_law(&nieuw).unwrap();

        let result = service
            .evaluate_law_output("wet_beschikking", "bedrag", BTreeMap::new(), "2025-06-01")
            .unwrap();
        assert!(!result.outputs.contains_key("motivering_3"));
        assert_eq!(
            result.outputs.get("motivering_4"),
            Some(&Value::String("gemotiveerd".to_string())),
            "the article both versions carry fires as before"
        );
        assert_eq!(
            result
                .declarations_not_in_force
                .iter()
                .map(|n| (n.article.as_str(), n.reason.as_str()))
                .collect::<Vec<_>>(),
            vec![(
                "3",
                "the version of wet_motiveringsplicht in force on this date (valid_from \
                 2024-01-01) has no article 3; article 5 of that version declares a hook \
                 that fires at the same point on this decision"
            )]
        );
    }

    /// The same renumbering, of a hook on a submission (RFC-045), with the
    /// application taken in a procedure (RFC-047). Staged at AANVRAAG, the
    /// hook on the application would fire, so the reason names the article
    /// that carries it. Staged at BESLUIT, the application's own hooks wait,
    /// but another article that establishes an AANVRAAG is not the staged
    /// one, even with the same number: its hooks fire on every run, and the
    /// renumbered one is recorded there too.
    #[test]
    fn test_a_renumbered_hook_on_a_staged_submission_is_named_in_the_reason() {
        let aanvraag = r#"
$id: wet_aanvraag
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
procedure:
  - id: beschikking
    default: true
    applies_to: {legal_character: TEST_BESCHIKKING}
    stages:
      - name: AANVRAAG
      - name: BESLUIT
articles:
  - number: '1'
    text: Op aanvraag stelt het bestuursorgaan het bedrag vast
    machine_readable:
      execution:
        produces:
          legal_character: TEST_BESCHIKKING
          submission: {kind: AANVRAAG}
        input:
          - name: geregistreerd
            type: boolean
            source:
              regulation: wet_register
              output: geregistreerd
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value:
              operation: IF
              cases:
                - when: $geregistreerd
                  then: 100
              default: 0
"#;
        // Another law's article with the same number that establishes an
        // AANVRAAG of its own.
        let register = r#"
$id: wet_register
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Wie zich aanmeldt, wordt ingeschreven
    machine_readable:
      execution:
        produces:
          submission: {kind: AANVRAAG}
        output:
          - name: geregistreerd
            type: boolean
        actions:
          - output: geregistreerd
            value: true
"#;
        let vereisten = |valid_from: &str, number: &str| {
            format!(
                r#"
$id: wet_aanvraagvereisten
regulatory_layer: WET
publication_date: '{valid_from}'
valid_from: '{valid_from}'
articles:
  - number: '{number}'
    text: Een aanvraag wordt ondertekend
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {{submission: AANVRAAG}}
      execution:
        output:
          - name: ondertekend_{number}
            type: boolean
        actions:
          - output: ondertekend_{number}
            value: true
"#
            )
        };
        let mut service = LawExecutionService::new();
        service.load_law(aanvraag).unwrap();
        service.load_law(register).unwrap();
        service.load_law(&vereisten("2024-01-01", "5")).unwrap();
        service.load_law(&vereisten("2027-01-01", "3")).unwrap();
        let reason = "the version of wet_aanvraagvereisten in force on this date \
                      (valid_from 2024-01-01) has no article 3; article 5 of that version \
                      declares a hook that fires at the same point on this submission";
        let recorded = |result: &ArticleResult| {
            result
                .declarations_not_in_force
                .iter()
                .map(|n| (n.article.clone(), n.subject.clone(), n.reason.clone()))
                .collect::<Vec<_>>()
        };

        let at_aanvraag = service
            .execute_stage_at(
                "wet_aanvraag",
                "1",
                "AANVRAAG",
                Some(&[]),
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        assert_eq!(
            recorded(&at_aanvraag),
            vec![(
                "3".to_string(),
                "hook point pre_actions on TEST_BESCHIKKING at stage AANVRAAG, \
                 submission AANVRAAG"
                    .to_string(),
                reason.to_string()
            )]
        );

        let at_besluit = service
            .execute_stage_at(
                "wet_aanvraag",
                "1",
                "BESLUIT",
                Some(&["bedrag"]),
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        assert_eq!(at_besluit.outputs.get("bedrag"), Some(&Value::Int(100)));
        assert_eq!(
            recorded(&at_besluit),
            vec![(
                "3".to_string(),
                "hook point pre_actions on submission AANVRAAG".to_string(),
                reason.to_string()
            )]
        );
    }

    /// The regeling fills the open term in both versions, but the newest one
    /// renumbered the article. The implements index is built from the newest
    /// version, so on an earlier date it points at an article that version does
    /// not have and the term falls through to its default. That outcome is
    /// unchanged (applying the renumbered article is a separate question); the
    /// skip used to be a log line, and now stands in the record naming the
    /// article that carries the filling on this date.
    #[test]
    fn test_an_implementation_whose_article_is_missing_on_the_date_is_recorded() {
        let wet = r#"
$id: wet_met_open_term
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '2'
    text: Bij ministeriele regeling kan een afwijkende afstand worden vastgesteld
    machine_readable:
      open_terms:
        - id: afwijkende_afstand
          type: number
          required: false
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: afstand
            type: number
        actions:
          - output: afstand
            value:
              operation: IF
              cases:
                - when:
                    operation: NOT_NULL
                    subject: $afwijkende_afstand
                  then: $afwijkende_afstand
              default: 200
"#;
        // Only an article that fills exactly this open term of exactly this
        // article counts as the renumbered filling; the others are decoys.
        let decoys = r#"
  - number: '5'
    text: Andere wet
    machine_readable:
      implements:
        - law: andere_wet
          article: '2'
          open_term: afwijkende_afstand
  - number: '6'
    text: Ander artikel
    machine_readable:
      implements:
        - law: wet_met_open_term
          article: '3'
          open_term: afwijkende_afstand
  - number: '7'
    text: Andere open term
    machine_readable:
      implements:
        - law: wet_met_open_term
          article: '2'
          open_term: andere_term
  - number: '8'
    text: Geen machine_readable
"#;
        let regeling = |valid_from: &str, number: &str, extra: &str| {
            format!(
                r#"
$id: regeling_afstand
regulatory_layer: MINISTERIELE_REGELING
publication_date: '{valid_from}'
valid_from: '{valid_from}'
articles:
  - number: '{number}'
    text: De afwijkende afstand bedraagt 50
    machine_readable:
      implements:
        - law: wet_met_open_term
          article: '2'
          open_term: afwijkende_afstand
      execution:
        output:
          - name: afwijkende_afstand
            type: number
        actions:
          - output: afwijkende_afstand
            value: 50
{extra}"#
            )
        };
        let mut service = LawExecutionService::new();
        service.load_law(wet).unwrap();
        service
            .load_law(&regeling("2025-01-01", "4", decoys))
            .unwrap();
        service.load_law(&regeling("2027-01-01", "1", "")).unwrap();
        // Another regeling, scoped to a gemeente this execution is not about,
        // fills the same term under the renumbered article's number. It is
        // offered by the index but is another law, so it does not hide
        // article 4 of regeling_afstand from the reason.
        service
            .load_law(
                r#"
$id: regeling_elders
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
gemeente_code: GM0363
articles:
  - number: '4'
    text: De afwijkende afstand bedraagt 75
    machine_readable:
      implements:
        - law: wet_met_open_term
          article: '2'
          open_term: afwijkende_afstand
      execution:
        output:
          - name: afwijkende_afstand
            type: number
        actions:
          - output: afwijkende_afstand
            value: 75
"#,
            )
            .unwrap();

        let before = service
            .evaluate_law_output(
                "wet_met_open_term",
                "afstand",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        assert_eq!(
            before.outputs.get("afstand"),
            Some(&Value::Int(200)),
            "the outcome is unchanged: the indexed article is not in this version"
        );
        assert_eq!(
            before.declarations_not_in_force,
            vec![DeclarationNotInForce {
                kind: DeclarationKind::Implementation,
                law_id: "regeling_afstand".to_string(),
                article: "1".to_string(),
                subject: "open term 'afwijkende_afstand' of wet_met_open_term article 2"
                    .to_string(),
                reason:
                    "the version of regeling_afstand in force on this date (valid_from 2025-01-01) \
                         has no article 1; article 4 of that version declares an \
                         implementation of the same open term"
                        .to_string(),
            }],
            "the skipped filling must be recorded, naming the renumbered article"
        );

        let receipt =
            service.build_receipt_with_outputs(&before, &BTreeMap::new(), "2025-06-01", &[]);
        assert_eq!(
            receipt.results.declarations_not_in_force,
            before.declarations_not_in_force
        );

        let traced = service
            .evaluate_law_output_with_trace(
                "wet_met_open_term",
                "afstand",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap();
        let root = traced.trace.as_ref().expect("a traced run has a trace");
        fn find_named<'a>(
            node: &'a crate::trace::PathNode,
            name: &str,
        ) -> Option<&'a crate::trace::PathNode> {
            if node.node_type == PathNodeType::OpenTermResolution && node.name == name {
                return Some(node);
            }
            node.children.iter().find_map(|c| find_named(c, name))
        }
        let node = find_named(root, "regeling_afstand:1").unwrap_or_else(|| {
            panic!(
                "the trace must carry the skipped filling:\n{}",
                root.render_box_drawing()
            )
        });
        assert_eq!(
            node.message.as_deref(),
            Some(before.declarations_not_in_force[0].message().as_str())
        );

        let after = service
            .evaluate_law_output(
                "wet_met_open_term",
                "afstand",
                BTreeMap::new(),
                "2027-06-01",
            )
            .unwrap();
        assert_eq!(after.outputs.get("afstand"), Some(&Value::Int(50)));
        assert!(after.declarations_not_in_force.is_empty());
    }

    /// A default whose actions never produce the term itself yielded null under
    /// the message "using default value". Null is a signal downstream ("no
    /// verordening, fall back to the statutory rule"), so the citizen got the
    /// general rule while the default was supposed to apply.
    #[test]
    fn test_a_default_that_produces_nothing_is_not_reported_as_a_default_value() {
        let yaml = r#"
$id: wet_met_lege_default
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie wordt bij ministeriele regeling vastgesteld
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: false
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
          default:
            actions:
              - output: standaardpremie_basis
                value: 1889
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: $standaardpremie
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();

        let error = service
            .evaluate_law_output(
                "wet_met_lege_default",
                "standaardpremie",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect_err("a default that produces nothing must not pass for a default value");
        let message = error.to_string();
        assert!(
            message.contains("standaardpremie") && message.contains("did not produce output"),
            "the error must name the term the default failed to produce: {message}"
        );

        // A default that does produce the term still applies, including when
        // the value it produces is null: that is a default, not a miss.
        let with_value = yaml.replace("standaardpremie_basis", "standaardpremie");
        let mut service = LawExecutionService::new();
        service.load_law(&with_value).unwrap();
        let result = service
            .evaluate_law_output(
                "wet_met_lege_default",
                "standaardpremie",
                BTreeMap::new(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("standaardpremie"),
            Some(&Value::Int(1889))
        );
    }

    // -------------------------------------------------------------------------
    // Law loading, provenance and inventory
    // -------------------------------------------------------------------------

    #[test]
    fn test_load_law_with_source_records_provenance() {
        let mut service = LawExecutionService::new();

        let law_id = service
            .load_law_with_source(make_base_law(), "bwb-corpus", "BWB corpus")
            .unwrap();
        assert_eq!(law_id, "base_law");

        assert_eq!(
            service.get_law_source("base_law"),
            Some(("bwb-corpus", "BWB corpus")),
            "a receipt must be able to name where a law came from"
        );
        assert_eq!(
            service.get_law_source("unknown_law"),
            None,
            "an unknown law has no provenance"
        );

        // A law loaded without provenance keeps none.
        service.load_law(make_dependent_law()).unwrap();
        assert_eq!(service.get_law_source("dependent_law"), None);
    }

    #[test]
    fn test_law_inventory_and_unloading() {
        let mut service = LawExecutionService::new();
        assert_eq!(service.law_count(), 0);
        assert!(!service.has_law("base_law"));

        service.load_law(make_base_law()).unwrap();
        service.load_law(make_dependent_law()).unwrap();
        assert_eq!(service.law_count(), 2);
        assert!(service.has_law("base_law"));
        assert!(service.has_law("dependent_law"));
        assert!(!service.has_law("unknown_law"));

        assert!(
            service.unload_law("base_law"),
            "unloading a loaded law reports success"
        );
        assert_eq!(service.law_count(), 1);
        assert!(!service.has_law("base_law"));
        assert!(
            !service.unload_law("base_law"),
            "unloading a law that is not there reports failure"
        );
    }

    #[test]
    fn test_resolver_exposes_the_loaded_laws() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let resolver = service.resolver();
        assert!(
            resolver.has_law("base_law"),
            "the exposed resolver must be the service's own, not an empty one"
        );
        assert_eq!(resolver.law_count(), 1);
    }

    // -------------------------------------------------------------------------
    // Data source management
    // -------------------------------------------------------------------------

    fn dict_record(field: &str, value: i64) -> BTreeMap<String, BTreeMap<String, Value>> {
        let mut fields = BTreeMap::new();
        fields.insert(field.to_string(), Value::Int(value));
        let mut data = BTreeMap::new();
        data.insert("123".to_string(), fields);
        data
    }

    #[test]
    fn test_data_source_registration_and_removal() {
        let mut service = LawExecutionService::new();
        assert_eq!(service.data_source_count(), 0);
        assert!(service.list_data_sources().is_empty());

        service.add_dict_source("hoge_prioriteit", 10, dict_record("inkomen", 1));
        assert_eq!(service.data_source_count(), 1);
        assert_eq!(service.list_data_sources(), vec!["hoge_prioriteit"]);

        service.add_data_source(Box::new(DictDataSource::new(
            "lage_prioriteit",
            5,
            dict_record("vermogen", 2),
        )));
        assert_eq!(service.data_source_count(), 2);
        assert_eq!(
            service.list_data_sources(),
            vec!["hoge_prioriteit", "lage_prioriteit"],
            "sources are ordered by priority, highest first"
        );
        assert_eq!(
            service.data_registry().source_count(),
            2,
            "the exposed registry must be the service's own, not an empty one"
        );

        assert!(service.remove_data_source("lage_prioriteit"));
        assert_eq!(service.list_data_sources(), vec!["hoge_prioriteit"]);
        assert!(
            !service.remove_data_source("lage_prioriteit"),
            "removing a source that is not there reports failure"
        );

        service.clear_data_sources();
        assert_eq!(service.data_source_count(), 0);
        assert!(service.list_data_sources().is_empty());
    }

    // -------------------------------------------------------------------------
    // ServiceProvider trait surface
    // -------------------------------------------------------------------------

    #[test]
    fn test_service_provider_get_law() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let law = ServiceProvider::get_law(&service, "base_law")
            .expect("a loaded law must be reachable through the trait");
        assert_eq!(law.id, "base_law");
        assert!(ServiceProvider::get_law(&service, "unknown_law").is_none());
    }

    // -------------------------------------------------------------------------
    // Flagged constructs: untranslatables (v0.5.x) and markings (v0.7.0)
    // -------------------------------------------------------------------------

    /// The RFC-012 modes read `markings` too. A marked article behaves exactly
    /// as an article with an `untranslatables` entry does: unaccepted stops
    /// execution, accepted runs partial logic.
    #[test]
    fn test_markings_drive_the_modes_like_untranslatables_do() {
        let yaml = r#"
$id: wet_met_markeringen
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De som van alle deeltoeslagen wordt berekend.
    machine_readable:
      markings:
        - about: som van alle deeltoeslagen
          reason: het model kent alleen regels over een waarde, niet over een verzameling
          resolution: model
          resolved_by: Een vorm voor een regel over een verzameling
          target: []
          legal_text_excerpt: De som van alle deeltoeslagen
          accepted: true
      execution:
        output:
          - name: som
            type: number
        actions:
          - output: som
            value: 0
  - number: '2'
    text: Het bedrag wordt naar boven afgerond op hele euro's.
    machine_readable:
      markings:
        - about: afronden op hele euro's
          reason: de motor kent geen wettelijke afronding op eurocenten
          resolution: operation
          resolved_by: Een CEIL-bewerking op eurocenten
          target: []
          legal_text_excerpt: naar boven afgerond
      execution:
        output:
          - name: afgerond
            type: number
        actions:
          - output: afgerond
            value: 1234
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();

        let accepted = service
            .evaluate_law_output("wet_met_markeringen", "som", BTreeMap::new(), "2025-01-01")
            .expect("an accepted marking executes with partial logic");
        assert_eq!(accepted.outputs.get("som"), Some(&Value::Int(0)));

        let unaccepted = service.evaluate_law_output(
            "wet_met_markeringen",
            "afgerond",
            BTreeMap::new(),
            "2025-01-01",
        );
        assert!(
            matches!(unaccepted, Err(EngineError::Untranslatable { ref construct, .. })
                if construct == "afronden op hele euro's"),
            "an unaccepted marking must stop execution: {unaccepted:?}"
        );

        // Propagate mode taints the outputs of a marked article, exactly as it
        // does for an untranslatable.
        service.set_untranslatable_mode(UntranslatableMode::Propagate);
        let tainted = service
            .evaluate_law_output(
                "wet_met_markeringen",
                "afgerond",
                BTreeMap::new(),
                "2025-01-01",
            )
            .expect("propagate mode executes and taints");
        assert!(
            matches!(tainted.outputs.get("afgerond"), Some(Value::Untranslatable { construct, .. })
                if construct == "afronden op hele euro's"),
            "the marked output must carry the taint: {:?}",
            tainted.outputs.get("afgerond")
        );
    }

    /// A marked article that errors must say which of the two has to move
    /// before it can be translated: the engine's operation set, or the format.
    /// A reader who only gets "cannot express this" learns nothing actionable.
    #[test]
    fn test_marking_error_names_the_layer_and_the_change() {
        fn law_with(resolution: &str, resolved_by: Option<&str>) -> String {
            let resolved_by_line = match resolved_by {
                Some(text) => format!("\n          resolved_by: {text}"),
                None => String::new(),
            };
            format!(
                r#"
$id: wet_met_markering
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag wordt naar boven afgerond op hele euro's.
    machine_readable:
      markings:
        - about: afronden op hele euro's
          reason: de motor kent geen wettelijke afronding op eurocenten
          resolution: {resolution}{resolved_by_line}
          target: []
          legal_text_excerpt: naar boven afgerond op hele euro's
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 1234
"#
            )
        }

        let run = |yaml: String| {
            let mut service = LawExecutionService::new();
            service.load_law(&yaml).unwrap();
            service
                .evaluate_law_output("wet_met_markering", "bedrag", BTreeMap::new(), "2025-01-01")
                .unwrap_err()
                .to_string()
        };

        let operation_side = run(law_with(
            "operation",
            Some("Een CEIL-bewerking op eurocenten"),
        ));
        assert!(
            operation_side.contains("needs a new operation")
                && operation_side.contains("Een CEIL-bewerking op eurocenten"),
            "an operation marking must name the operation to build: {operation_side}"
        );

        let model_side = run(law_with(
            "model",
            Some("Een vorm voor een regel over een verzameling"),
        ));
        assert!(
            model_side.contains("needs a new shape in the format")
                && model_side.contains("Een vorm voor een regel over een verzameling"),
            "a model marking must name the shape the format lacks: {model_side}"
        );

        // `resolved_by` is required by schema v0.7.0 but optional on the model,
        // which also reads files written before it was. Then the layer alone is
        // the answer, and the message must not trail off into an empty phrase.
        let bare = run(law_with("model", None));
        assert!(
            bare.contains("needs a new shape in the format") && !bare.contains(": —"),
            "without resolved_by the layer alone must read as a sentence: {bare}"
        );
    }

    /// Ignore mode is the quiet one, and quiet is not the same as permissive:
    /// it drops the logging, not the refusal. An unaccepted flagged construct
    /// still stops execution, and only an accepted one runs. Without this the
    /// mode name would read as "execute anyway", which is what it must not do.
    #[test]
    fn test_ignore_mode_still_refuses_an_unaccepted_construct() {
        fn law(accepted: bool) -> String {
            format!(
                r#"
$id: wet_stil
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag wordt naar boven afgerond op hele euro's.
    machine_readable:
      markings:
        - about: afronden op hele euro's
          reason: de motor kent geen wettelijke afronding op eurocenten
          resolution: operation
          resolved_by: Een CEIL-bewerking op eurocenten
          target: []
          legal_text_excerpt: naar boven afgerond
          accepted: {accepted}
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 1234
"#
            )
        }

        let run = |accepted: bool| {
            let mut service = LawExecutionService::new();
            service.load_law(&law(accepted)).unwrap();
            service.set_untranslatable_mode(UntranslatableMode::Ignore);
            service.evaluate_law_output("wet_stil", "bedrag", BTreeMap::new(), "2025-01-01")
        };

        let unaccepted = run(false);
        assert!(
            matches!(unaccepted, Err(EngineError::Untranslatable { .. })),
            "ignore mode must still refuse an unaccepted construct: {unaccepted:?}"
        );

        let accepted = run(true).expect("an accepted construct executes silently");
        assert_eq!(accepted.outputs.get("bedrag"), Some(&Value::Int(1234)));
    }

    /// Both channels on one article are handled, and neither hides the other:
    /// the v0.5.x entry is not silently dropped now that markings exist, and a
    /// marking is not ignored because an untranslatable came first.
    #[test]
    fn test_both_channels_on_one_article_are_handled() {
        let yaml = r#"
$id: wet_met_beide
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twee dingen die dit artikel niet kan uitdrukken.
    machine_readable:
      untranslatables:
        - construct: afronden op hele euro's
          reason: geen CEIL-bewerking
          accepted: true
      markings:
        - about: som van alle deeltoeslagen
          reason: het model kent alleen regels over een waarde, niet over een verzameling
          resolution: model
          resolved_by: Een vorm voor een regel over een verzameling
          target: []
          legal_text_excerpt: De som van alle deeltoeslagen
          accepted: true
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 1234
"#;
        let mut service = LawExecutionService::new();
        service.load_law(yaml).unwrap();
        service.set_untranslatable_mode(UntranslatableMode::Propagate);

        let result = service
            .evaluate_law_output("wet_met_beide", "bedrag", BTreeMap::new(), "2025-01-01")
            .expect("accepted entries execute with partial logic");

        match result.outputs.get("bedrag") {
            Some(Value::Untranslatable { construct, .. }) => {
                assert!(
                    construct.contains("afronden op hele euro's")
                        && construct.contains("som van alle deeltoeslagen"),
                    "both channels must show up in the taint: {construct}"
                );
            }
            other => panic!("expected a tainted output, got: {other:?}"),
        }
    }

    #[test]
    fn test_service_provider_resolve_external_input() {
        let mut service = LawExecutionService::new();
        service.load_law(make_base_law()).unwrap();

        let context = RuleContext::new(BTreeMap::new(), "2025-01-01").unwrap();
        let value = service
            .resolve_external_input("base_law", "base_value", None, &context, "2025-01-01")
            .expect("the referenced law must be executed");

        assert_eq!(value, Value::Int(100));
    }

    // -------------------------------------------------------------------------
    // Version-aware law lookup for annotation resolution (get_law_version)
    // -------------------------------------------------------------------------

    /// Two versions of the same law whose *article text* genuinely differs, so
    /// a quote from the old redaction does not occur in the new one. This is
    /// the premise of the annotation-versioning bug: text-only laws are enough,
    /// notes anchor on text, not on execution logic.
    fn text_law_version(valid_from: &str, text: &str) -> String {
        format!(
            r#"
$id: test_versioned_text_law
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '{valid_from}'
articles:
  - number: '2'
    text: '{text}'
"#
        )
    }

    const OLD_TEXT: &str =
        "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil";
    const NEW_TEXT: &str = "wordt de tegemoetkoming ambtshalve toegekend door de dienst";

    fn service_with_two_text_versions() -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service
            .load_law(&text_law_version("2024-01-01", OLD_TEXT))
            .unwrap();
        service
            .load_law(&text_law_version("2025-01-01", NEW_TEXT))
            .unwrap();
        service
    }

    #[test]
    fn get_law_version_selects_the_version_being_viewed() {
        let service = service_with_two_text_versions();

        let old = service
            .get_law_version("test_versioned_text_law", Some("2024-01-01"))
            .expect("the 2024 version is in force on its own valid_from");
        assert_eq!(old.articles[0].text, OLD_TEXT);

        let latest = service
            .get_law_version("test_versioned_text_law", None)
            .expect("without a date the latest version is the answer");
        assert_eq!(latest.articles[0].text, NEW_TEXT);
    }

    #[test]
    fn a_note_on_an_old_version_resolves_against_that_version() {
        // The full annotation premise: the quoted text exists only in the old
        // redaction. Resolving against the viewed (old) version anchors the
        // note; the newest version — what `ServiceProvider::get_law` returns,
        // and what the resolver used to be handed — orphans it.
        let service = service_with_two_text_versions();
        let selector = crate::annotation::TextQuoteSelector {
            exact: "zorgtoeslag".to_string(),
            prefix: "op een ".to_string(),
            suffix: " ter grootte".to_string(),
            hint: None,
        };

        let viewed = service
            .get_law_version("test_versioned_text_law", Some("2024-01-01"))
            .unwrap();
        let r = crate::annotation::resolve(&selector, &viewed.articles);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(r.single().unwrap().article_number, "2");

        let newest = ServiceProvider::get_law(&service, "test_versioned_text_law").unwrap();
        let r = crate::annotation::resolve(&selector, &newest.articles);
        assert!(
            r.is_orphaned(),
            "premise check: the quote must be absent from the newest version, got {:?}",
            r.status
        );
    }

    #[test]
    fn get_law_version_refuses_to_slide_to_an_older_loaded_version() {
        // The requested version is not loaded (the editor's dependency loader
        // skips versions the engine cannot parse). Date-based selection alone
        // would silently pick the next-older loaded version and the resolver
        // would mis-highlight the text on screen; this must be an error.
        let service = service_with_two_text_versions(); // 2024 and 2025 loaded
        let err = service
            .get_law_version("test_versioned_text_law", Some("2024-07-01"))
            .unwrap_err();
        match err {
            EngineError::LoadError(msg) => {
                assert!(
                    msg.contains("2024-07-01"),
                    "message must name the version: {msg}"
                );
                assert!(msg.contains("test_versioned_text_law"));
            }
            other => panic!("expected LoadError, got {other:?}"),
        }
    }

    #[test]
    fn get_law_version_rejects_a_malformed_date() {
        let service = service_with_two_text_versions();
        let err = service
            .get_law_version("test_versioned_text_law", Some("gisteren"))
            .unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidDate(_)),
            "a malformed date must not silently fall back to the newest version: {err:?}"
        );
    }

    #[test]
    fn get_law_version_reports_an_unknown_law() {
        let service = LawExecutionService::new();
        let err = service
            .get_law_version("nonexistent_law", None)
            .unwrap_err();
        assert!(matches!(err, EngineError::LawNotFound(id) if id == "nonexistent_law"));
    }

    #[test]
    fn get_law_version_reports_a_not_yet_in_force_date() {
        let mut service = LawExecutionService::new();
        service
            .load_law(&text_law_version("2025-01-01", NEW_TEXT))
            .unwrap();
        let err = service
            .get_law_version("test_versioned_text_law", Some("2024-06-01"))
            .unwrap_err();
        match err {
            EngineError::LawNotYetInForce {
                law_id,
                reference_date,
            } => {
                assert_eq!(law_id, "test_versioned_text_law");
                assert_eq!(
                    reference_date, "2024-06-01",
                    "the error must name the date the caller asked about"
                );
            }
            other => panic!("expected LawNotYetInForce, got {other:?}"),
        }
    }

    // -------------------------------------------------------------------------
    // Required parameters passed as null or unknown (RFC-036, review findings 3 and 4)
    // -------------------------------------------------------------------------

    /// Whether any node of the trace carries a message containing `needle`.
    /// Whether any node of the trace was resolved the given way.
    fn trace_resolves_by(node: &crate::trace::PathNode, resolve_type: &ResolveType) -> bool {
        node.resolve_type.as_ref() == Some(resolve_type)
            || node
                .children
                .iter()
                .any(|child| trace_resolves_by(child, resolve_type))
    }

    fn trace_mentions(node: &crate::trace::PathNode, needle: &str) -> bool {
        node.message.as_deref().is_some_and(|m| m.contains(needle))
            || node.children.iter().any(|c| trace_mentions(c, needle))
    }

    #[test]
    fn test_top_level_null_or_unknown_required_parameter_is_the_callers_error() {
        // Review probe P2: `bsn: null` at the top level used to make every
        // register input unknown "for lack of huur", as if the register had
        // been asked and had nothing. It was asked about nobody. That is the
        // caller's error, and it names the law and the parameter.
        let (permit, _) = fill_laws("required: false");
        let mut service = LawExecutionService::new();
        service.load_law(&permit).unwrap();
        register_fill_permit(&mut service);

        for (value, word) in [
            (Value::Null, "null"),
            (
                Value::unknown("elders", "kvk_nummer", MissingKind::NoData),
                "unknown",
            ),
        ] {
            let mut params = BTreeMap::new();
            params.insert("kvk_nummer".to_string(), value);
            let err = service
                .evaluate_law_output("fill_vergunning", "heeft_vergunning", params, "2025-01-01")
                .unwrap_err();
            match &err {
                EngineError::MissingParameter {
                    law_id,
                    name,
                    value,
                } => {
                    assert_eq!(law_id, "fill_vergunning");
                    assert_eq!(name, "kvk_nummer");
                    assert_eq!(value, word);
                }
                other => panic!("expected MissingParameter, got {other:?}"),
            }
            assert!(
                err.to_string().contains("cannot be evaluated for nobody"),
                "got {err}"
            );
        }

        // An optional parameter passed as null is a value, not this error: the
        // law tests for the absence and decides.
        let mut params = BTreeMap::new();
        params.insert(
            "kvk_nummer".to_string(),
            Value::String("85234567".to_string()),
        );
        params.insert("terras_oppervlakte".to_string(), Value::Null);
        let result = service
            .evaluate_law_output("fill_vergunning", "past_oppervlakte", params, "2025-01-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("past_oppervlakte"),
            Some(&Value::Bool(false))
        );
    }

    /// A target law with two required parameters and a caller that feeds both
    /// from its own register, so either can be null or unknown independently.
    fn two_key_laws() -> (&'static str, &'static str) {
        let target = r#"
$id: two_keys
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Doel
    machine_readable:
      execution:
        parameters:
          - name: eerste
            type: string
            required: true
          - name: tweede
            type: string
            required: true
        input:
          - name: waarde
            type: number
            source: {}
        output:
          - name: waarde
            type: number
        actions:
          - output: waarde
            value: $waarde
"#;
        let caller = r#"
$id: two_caller
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Aanroeper
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: eerste
            type: string
            nullable: true
            source: {}
          - name: tweede
            type: string
            nullable: true
            source: {}
          - name: waarde
            type: number
            nullable: true
            source:
              regulation: two_keys
              output: waarde
              parameters:
                eerste: $eerste
                tweede: $tweede
        output:
          - name: waarde
            type: number
            nullable: true
        actions:
          - output: waarde
            value: $waarde
"#;
        (target, caller)
    }

    /// Evaluate `two_caller` with the given register row for bsn 1, with trace.
    fn two_caller_result(row: Vec<(&str, Value)>) -> ArticleResult {
        let (target, caller) = two_key_laws();
        let mut service = LawExecutionService::new();
        service.load_law(target).unwrap();
        service.load_law(caller).unwrap();
        let mut record: BTreeMap<String, Value> =
            row.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        record.insert("bsn".to_string(), Value::String("1".to_string()));
        service
            .register_dict_source_for_law("two_caller", "register", "bsn", vec![record], 10)
            .unwrap();
        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("1".to_string()));
        service
            .evaluate_law_output_with_trace("two_caller", "waarde", params, "2025-01-01")
            .unwrap()
    }

    #[test]
    fn test_cross_law_skip_with_several_empty_required_parameters_is_deterministic() {
        // Two required parameters, one null and one unknown: the skip used to
        // return whichever sorted first. Now an unknown among them wins (the
        // outcome still has something to ask for), whichever it is, and the
        // trace names them all.
        let fact = |name: &str| MissingFact {
            law: "two_caller".to_string(),
            name: name.to_string(),
            kind: MissingKind::NoData,
        };

        // eerste null, tweede unknown (no cell).
        let result = two_caller_result(vec![("eerste", Value::Null)]);
        assert_eq!(
            result.outputs.get("waarde").unwrap().missing_facts(),
            &[fact("tweede")]
        );
        assert!(trace_mentions(
            result.trace.as_ref().unwrap(),
            "Parameters 'eerste', 'tweede' are unknown, so two_keys is not executed"
        ));

        // eerste unknown, tweede null: the same rule, the other way round.
        let result = two_caller_result(vec![("tweede", Value::Null)]);
        assert_eq!(
            result.outputs.get("waarde").unwrap().missing_facts(),
            &[fact("eerste")]
        );

        // Both unknown: the union.
        let result = two_caller_result(vec![]);
        assert_eq!(
            result.outputs.get("waarde").unwrap().missing_facts(),
            &[fact("eerste"), fact("tweede")]
        );

        // Both null: nobody to look up, the input is null.
        let result = two_caller_result(vec![("eerste", Value::Null), ("tweede", Value::Null)]);
        assert_eq!(result.outputs.get("waarde"), Some(&Value::Null));
        assert!(trace_mentions(
            result.trace.as_ref().unwrap(),
            "Parameters 'eerste', 'tweede' are null, so two_keys is not executed"
        ));

        // One empty parameter keeps the singular wording.
        let result = two_caller_result(vec![
            ("eerste", Value::String("a".to_string())),
            ("tweede", Value::Null),
        ]);
        assert_eq!(result.outputs.get("waarde"), Some(&Value::Null));
        assert!(trace_mentions(
            result.trace.as_ref().unwrap(),
            "Parameter 'tweede' is null, so two_keys is not executed"
        ));
    }

    /// The review's probe laws P4/P5: the caller passes `bsn: $partner_bsn` to
    /// a source law whose `bsn` is optional, so the unknown or null key is
    /// passed through and reaches the register lookup.
    fn optional_key_laws() -> (&'static str, &'static str) {
        let bron = r#"
$id: probe_bron
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bron met optionele sleutel
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: false
            nullable: true
        input:
          - name: geboortejaar
            type: number
            nullable: true
            source: {}
        output:
          - name: geboortejaar
            type: number
            nullable: true
        actions:
          - output: geboortejaar
            value: $geboortejaar
"#;
        let caller = r#"
$id: probe
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Partner
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: partner_bsn
            type: string
            nullable: true
            source: {}
          - name: partner_geboortejaar
            type: number
            nullable: true
            source:
              regulation: probe_bron
              output: geboortejaar
              parameters:
                bsn: $partner_bsn
        output:
          - name: partner_geboortejaar
            type: number
            nullable: true
        actions:
          - output: partner_geboortejaar
            value: $partner_geboortejaar
"#;
        (bron, caller)
    }

    fn optional_key_result(partner_bsn: Option<Value>) -> ArticleResult {
        let (bron, caller) = optional_key_laws();
        let mut service = LawExecutionService::new();
        service.load_law(bron).unwrap();
        service.load_law(caller).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        if let Some(partner) = partner_bsn {
            row.insert("partner_bsn".to_string(), partner);
        }
        service
            .register_dict_source_for_law("probe", "register", "bsn", vec![row], 10)
            .unwrap();
        let mut record = BTreeMap::new();
        record.insert("bsn".to_string(), Value::String("2".to_string()));
        record.insert("geboortejaar".to_string(), Value::Int(1980));
        service
            .register_dict_source_for_law("probe_bron", "bron", "bsn", vec![record], 10)
            .unwrap();
        let mut params = BTreeMap::new();
        params.insert("bsn".to_string(), Value::String("1".to_string()));
        service
            .evaluate_law_output_with_trace("probe", "partner_geboortejaar", params, "2025-01-01")
            .unwrap()
    }

    #[test]
    fn test_register_lookup_with_an_unknown_key_keeps_the_keys_provenance() {
        // Probe P4/P4b: the register cannot be read for a partner nobody has
        // named. The outcome used to say "unknown for lack of geboortejaar",
        // sending the portal to the register for a birth year; it names the
        // partner instead.
        let result = optional_key_result(None);
        assert_eq!(
            result
                .outputs
                .get("partner_geboortejaar")
                .unwrap()
                .missing_facts(),
            &[MissingFact {
                law: "probe".to_string(),
                name: "partner_bsn".to_string(),
                kind: MissingKind::NoData,
            }]
        );
        assert!(trace_mentions(
            result.trace.as_ref().unwrap(),
            "Input 'geboortejaar' cannot be looked up, the key is unknown"
        ));

        // A known partner still reads the record.
        let result = optional_key_result(Some(Value::String("2".to_string())));
        assert_eq!(
            result.outputs.get("partner_geboortejaar"),
            Some(&Value::Int(1980))
        );
    }

    #[test]
    fn test_register_lookup_with_a_null_key_is_absent() {
        // Probe P5: there is no partner, so there is no partner's birth year.
        // Asked about nobody, the register answers with an absence, the same
        // as the cross-law skip rule for a null required parameter.
        let result = optional_key_result(Some(Value::Null));
        assert_eq!(
            result.outputs.get("partner_geboortejaar"),
            Some(&Value::Null)
        );
        assert!(trace_mentions(
            result.trace.as_ref().unwrap(),
            "Input 'geboortejaar' cannot be looked up, the key is null"
        ));
    }

    // -------------------------------------------------------------------------
    // Nullability as a property of the type (RFC-036, `nullable`)
    // -------------------------------------------------------------------------

    /// A register law whose `huur` input carries the given attribute lines, a
    /// `bron` law it calls for the partner's birth year, and a caller of the
    /// bron's optional parameter. The outputs are chosen so that every action
    /// type-checks for either nullability of `huur`.
    fn nullable_laws(huur_attributes: &str) -> (String, &'static str) {
        let register = format!(
            r#"
$id: nul_register
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Huur
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: huur
            type: number
            {huur_attributes}
            source: {{}}
        output:
          - name: huur_bekend
            type: boolean
        actions:
          - output: huur_bekend
            operation: EQUALS
            subject: $huur
            value: 650
  - number: '2'
    text: Partner
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: partner_bsn
            type: string
            nullable: true
            source: {{}}
          - name: partner_geboortejaar
            type: number
            nullable: true
            source:
              regulation: nul_bron
              output: geboortejaar
              parameters:
                bsn: $partner_bsn
        output:
          - name: partner_geboortejaar
            type: number
            nullable: true
        actions:
          - output: partner_geboortejaar
            value: $partner_geboortejaar
  - number: '3'
    text: Toeslag
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: toeslag
            type: number
            nullable: true
            source: {{}}
          - name: toeslagen
            type: array
            source: {{}}
        output:
          - name: toeslagklasse
            type: string
            nullable: true
          - name: toeslag_strikt
            type: number
        actions:
          - output: toeslagklasse
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $toeslag
                    value: 650
                  then: gewoon
          - output: toeslag_strikt
            value:
              operation: FOREACH
              collection: $toeslagen
              as: t
              body: $t
              combine: MAX
"#
        );
        let bron = r#"
$id: nul_bron
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bron
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: aanvraag_bedrag
            type: number
            required: false
        input:
          - name: geboortejaar
            type: number
            source: {}
        output:
          - name: geboortejaar
            type: number
          - name: past_aanvraag
            type: boolean
        actions:
          - output: geboortejaar
            value: $geboortejaar
          - output: past_aanvraag
            operation: EQUALS
            subject: $aanvraag_bedrag
            value: 100
"#;
        (register, bron)
    }

    /// A service with both laws loaded and a register row for bsn 1 holding
    /// the given cells.
    fn nullable_service(huur_attributes: &str, cells: &[(&str, Value)]) -> LawExecutionService {
        let (register, bron) = nullable_laws(huur_attributes);
        let mut service = LawExecutionService::new();
        service.load_law(&register).unwrap();
        service.load_law(bron).unwrap();
        let mut row = params(cells);
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        service
            .register_dict_source_for_law("nul_register", "huurregister", "bsn", vec![row], 10)
            .unwrap();
        service
    }

    fn bsn_one() -> BTreeMap<String, Value> {
        params(&[("bsn", Value::String("1".to_string()))])
    }

    #[test]
    fn test_null_cell_for_a_non_nullable_input_is_a_data_error_at_the_boundary() {
        // The register says "there is no rent"; the law declared that a rent
        // is always there. The contradiction names the source, the field and
        // the law, and is raised before any action runs.
        let service = nullable_service("", &[("huur", Value::Null)]);
        let err = service
            .evaluate_law_output("nul_register", "huur_bekend", bsn_one(), "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "nul_register");
                assert_eq!(field, "huur");
                assert_eq!(origin, "source huurregister");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
        assert!(
            err.to_string().contains("declares as never absent"),
            "{err}"
        );
    }

    #[test]
    fn test_null_cell_for_a_nullable_input_is_a_value_the_law_tests() {
        let service = nullable_service("nullable: true", &[("huur", Value::Null)]);
        let result = service
            .evaluate_law_output("nul_register", "huur_bekend", bsn_one(), "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("huur_bekend"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_unknown_is_not_governed_by_nullable() {
        // No cell at all: nobody has the fact. That is an unknown whatever the
        // declaration says, and it propagates as before.
        for attributes in ["", "nullable: true"] {
            let service = nullable_service(attributes, &[]);
            let result = service
                .evaluate_law_output("nul_register", "huur_bekend", bsn_one(), "2025-01-01")
                .unwrap();
            assert!(
                result.outputs.get("huur_bekend").unwrap().is_unknown(),
                "attributes {attributes:?}"
            );
        }
    }

    #[test]
    fn test_skipped_cross_law_call_into_a_non_nullable_input_names_the_skip() {
        // There is no partner, so the bron is not run and the input would be
        // null; the caller declared it never absent. No well-typed set reaches
        // this: the static skip rule (N5) refuses such a caller, in either
        // load order (RFC-037). The run-time check stays as defence in depth
        // and is tested through the unchecked loader.
        let (register, bron) = nullable_laws("");
        let strict = register.replacen(
            "          - name: partner_geboortejaar\n            type: number\n            nullable: true\n            source:",
            "          - name: partner_geboortejaar\n            type: number\n            source:",
            1,
        );
        assert_ne!(strict, register);
        let mut service = LawExecutionService::new();
        service.load_law(bron).unwrap();
        assert!(service.load_law(&strict).is_err());
        service.load_law_unchecked(&strict).unwrap();
        let mut row = params(&[("huur", Value::Int(650)), ("partner_bsn", Value::Null)]);
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        service
            .register_dict_source_for_law("nul_register", "huurregister", "bsn", vec![row], 10)
            .unwrap();
        let err = service
            .evaluate_law_output(
                "nul_register",
                "partner_geboortejaar",
                bsn_one(),
                "2025-01-01",
            )
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable { field, origin, .. } => {
                assert_eq!(field, "partner_geboortejaar");
                assert_eq!(origin, "skipped call to nul_bron");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
    }

    #[test]
    fn test_null_output_of_another_law_into_a_non_nullable_input_names_the_output() {
        // The bron runs and its output is null (the bron's `geboortejaar` is a
        // nullable pass-through of a null register cell). The caller's input
        // is not nullable, so the null is refused at the caller's boundary.
        let bron = r#"
$id: leeg_bron
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bron
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: geboortejaar
            type: number
            nullable: true
            source: {}
        output:
          - name: geboortejaar
            type: number
            nullable: true
        actions:
          - output: geboortejaar
            value: $geboortejaar
"#;
        let caller = r#"
$id: leeg_afnemer
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Afnemer
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: geboortejaar
            type: number
            source:
              regulation: leeg_bron
              output: geboortejaar
              parameters:
                bsn: $bsn
        output:
          - name: geboortejaar
            type: number
        actions:
          - output: geboortejaar
            value: $geboortejaar
"#;
        // No well-typed set reaches this boundary: with the bron in view the
        // static check (N5) refuses the caller, and a bron arriving later is
        // refused for making the caller ill-typed (RFC-037). The run-time
        // check stays as defence in depth, so the caller is loaded unchecked
        // here to test it directly.
        let mut service = LawExecutionService::new();
        service.load_law(bron).unwrap();
        service.load_law_unchecked(caller).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        row.insert("geboortejaar".to_string(), Value::Null);
        service
            .register_dict_source_for_law("leeg_bron", "brp", "bsn", vec![row], 10)
            .unwrap();
        let err = service
            .evaluate_law_output("leeg_afnemer", "geboortejaar", bsn_one(), "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "leeg_afnemer");
                assert_eq!(field, "geboortejaar");
                assert_eq!(origin, "leeg_bron.geboortejaar");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
    }

    #[test]
    fn test_null_output_of_the_same_law_into_a_non_nullable_input_names_the_article() {
        // The same rule one article further in: an input that reads another
        // article of its OWN law (`source.output` without a `regulation`) is
        // held to its declaration too. Article 1 says "no partner" through a
        // nullable output; article 2 reads it into an input that promised a
        // value, and the null is refused at that boundary with the law's own
        // id as the origin — not carried in to fail three operations later.
        let law = r#"
$id: intern_nul
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Partner
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: partner_bsn
            type: string
            nullable: true
            source: {}
        output:
          - name: partner
            type: string
            nullable: true
        actions:
          - output: partner
            value: $partner_bsn
  - number: '2'
    text: Toeslag
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: partner
            type: string
            source:
              output: partner
        output:
          - name: heeft_partner
            type: boolean
        actions:
          - output: heeft_partner
            operation: EQUALS
            subject: $partner
            value: '2'
"#;
        // No well-typed law reaches this boundary: N5 refuses an input that
        // takes a nullable output, within a law just as across laws (RFC-037).
        // The run-time check stays as defence in depth, so the law is loaded
        // unchecked here to test it directly.
        let mut service = LawExecutionService::new();
        assert!(service.load_law(law).is_err());
        service.load_law_unchecked(law).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        row.insert("partner_bsn".to_string(), Value::Null);
        service
            .register_dict_source_for_law("intern_nul", "brp", "bsn", vec![row], 10)
            .unwrap();
        let err = service
            .evaluate_law_output("intern_nul", "heeft_partner", bsn_one(), "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "intern_nul");
                assert_eq!(field, "partner");
                assert_eq!(origin, "intern_nul.partner");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
        // A partner present is a value, and the same reference runs.
        let mut service = LawExecutionService::new();
        service.load_law_unchecked(law).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        row.insert("partner_bsn".to_string(), Value::String("2".to_string()));
        service
            .register_dict_source_for_law("intern_nul", "brp", "bsn", vec![row], 10)
            .unwrap();
        let result = service
            .evaluate_law_output("intern_nul", "heeft_partner", bsn_one(), "2025-01-01")
            .unwrap();
        assert_eq!(
            result.outputs.get("heeft_partner"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn test_non_nullable_output_that_evaluates_to_null_is_an_error() {
        // Article 3 takes the highest of a list the register delivers empty.
        // The highest of nothing is absent, which the static check cannot see
        // (RFC-037: MIN/MAX over a collection make no claim); the output is
        // what breaks the law's promise, and the run-time check says so.
        let service = nullable_service("", &[("toeslagen", Value::Array(vec![]))]);
        let err = service
            .evaluate_law_output("nul_register", "toeslag_strikt", bsn_one(), "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullOutput { law_id, output } => {
                assert_eq!(law_id, "nul_register");
                assert_eq!(output, "toeslag_strikt");
            }
            other => panic!("expected NullOutput, got {other:?}"),
        }
        assert!(err.to_string().contains("not declared nullable"), "{err}");
        // The trace records the failure on the output's own node.
        let err = service
            .evaluate_law_output_with_trace(
                "nul_register",
                "toeslag_strikt",
                bsn_one(),
                "2025-01-01",
            )
            .unwrap_err();
        match err {
            EngineError::TracedError { trace, .. } => {
                assert!(trace_mentions(
                    trace.as_deref().unwrap(),
                    "evaluated to null (absent)"
                ));
            }
            other => panic!("expected TracedError, got {other:?}"),
        }
    }

    #[test]
    fn test_nullable_output_may_evaluate_to_null() {
        // The IF without default matches nothing for 700: the nullable output
        // is absent, and the highest of a non-empty list is a value.
        let service = nullable_service(
            "",
            &[
                ("toeslag", Value::Int(700)),
                ("toeslagen", Value::Array(vec![Value::Int(700)])),
            ],
        );
        let result = service
            .evaluate_law(
                "nul_register",
                &["toeslagklasse", "toeslag_strikt"],
                bsn_one(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(result.outputs.get("toeslagklasse"), Some(&Value::Null));
        assert_eq!(result.outputs.get("toeslag_strikt"), Some(&Value::Int(700)));
    }

    #[test]
    fn test_null_for_an_optional_non_nullable_parameter_is_refused_at_the_call() {
        // The caller hands the bron `aanvraag_bedrag: null`. The bron declared
        // the parameter optional (the caller may leave it out) but not
        // nullable, so an absence is not a value it takes.
        let (register, bron) = nullable_laws("nullable: true");
        let caller = r#"
$id: nul_aanvrager
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Aanvrager
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: bedrag
            type: number
            nullable: true
        input:
          - name: past
            type: boolean
            source:
              regulation: nul_bron
              output: past_aanvraag
              parameters:
                bsn: $bsn
                aanvraag_bedrag: $bedrag
        output:
          - name: past
            type: boolean
        actions:
          - output: past
            value: $past
"#;
        let mut service = LawExecutionService::new();
        service.load_law(&register).unwrap();
        service.load_law(bron).unwrap();
        service.load_law(caller).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        row.insert("geboortejaar".to_string(), Value::Int(1980));
        service
            .register_dict_source_for_law("nul_bron", "brp", "bsn", vec![row], 10)
            .unwrap();
        let mut params = bsn_one();
        params.insert("bedrag".to_string(), Value::Null);
        let err = service
            .evaluate_law_output("nul_aanvrager", "past", params, "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "nul_bron");
                assert_eq!(field, "aanvraag_bedrag");
                assert_eq!(origin, "parameter from nul_aanvrager");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
        // Passed as a value, the same call runs.
        let mut params = bsn_one();
        params.insert("bedrag".to_string(), Value::Int(100));
        let result = service
            .evaluate_law_output("nul_aanvrager", "past", params, "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("past"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_a_parameter_that_overrides_a_non_nullable_input_may_not_be_null() {
        // A caller hands `huur` in directly, under the input's name. The
        // source is bypassed; the declaration is not, and the null is refused
        // at the boundary with the caller as its origin, not three operations
        // later as an absent operand.
        let service = nullable_service("", &[("huur", Value::Int(650))]);
        let mut params = bsn_one();
        params.insert("huur".to_string(), Value::Null);
        let err = service
            .evaluate_law_output("nul_register", "huur_bekend", params, "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "nul_register");
                assert_eq!(field, "huur");
                assert_eq!(origin, "parameter from caller");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
        // A nullable input takes the null and the law decides on it.
        let service = nullable_service("nullable: true", &[("huur", Value::Int(650))]);
        let mut params = bsn_one();
        params.insert("huur".to_string(), Value::Null);
        let result = service
            .evaluate_law_output("nul_register", "huur_bekend", params, "2025-01-01")
            .unwrap();
        assert_eq!(result.outputs.get("huur_bekend"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_a_required_parameter_the_target_declares_nullable_is_run_with_null() {
        // The bron declares `bsn` required and nullable and decides on its
        // absence. Across laws the null is passed, not skipped on: the target
        // said it can decide on nobody, so the flag means the same thing on
        // both paths (RFC-036).
        let bron = r#"
$id: klasse_bron
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Klasse
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
            nullable: true
        output:
          - name: klasse
            type: string
        actions:
          - output: klasse
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $bsn
                    value: null
                  then: niemand
              default: iemand
"#;
        let caller = r#"
$id: klasse_afnemer
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Partner
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: partner_bsn
            type: string
            nullable: true
            source: {}
          - name: partner_klasse
            type: string
            source:
              regulation: klasse_bron
              output: klasse
              parameters:
                bsn: $partner_bsn
        output:
          - name: partner_klasse
            type: string
        actions:
          - output: partner_klasse
            value: $partner_klasse
"#;
        let mut service = LawExecutionService::new();
        service.load_law(bron).unwrap();
        // The caller's input is not nullable, and the static check agrees:
        // the call is not skipped, so the input is never null.
        service.load_law(caller).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        row.insert("partner_bsn".to_string(), Value::Null);
        service
            .register_dict_source_for_law("klasse_afnemer", "register", "bsn", vec![row], 10)
            .unwrap();
        let result = service
            .evaluate_law_output_with_trace(
                "klasse_afnemer",
                "partner_klasse",
                bsn_one(),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(
            result.outputs.get("partner_klasse"),
            Some(&Value::String("niemand".to_string()))
        );
        assert!(!trace_mentions(
            result.trace.as_ref().unwrap(),
            "is not executed"
        ));
    }

    #[test]
    fn test_nullable_parameter_passed_as_null_at_the_top_level_runs() {
        // `bedrag` is declared nullable and is only compared with EQUALS, so
        // the caller may pass an absence and the law decides on it.
        let law = r#"
$id: nul_param
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bedrag
    machine_readable:
      execution:
        parameters:
          - name: bedrag
            type: number
            required: true
            nullable: true
        output:
          - name: geen_bedrag
            type: boolean
        actions:
          - output: geen_bedrag
            value:
              operation: EQUALS
              subject: $bedrag
              value: null
"#;
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        let result = service
            .evaluate_law_output(
                "nul_param",
                "geen_bedrag",
                params(&[("bedrag", Value::Null)]),
                "2025-01-01",
            )
            .unwrap();
        assert_eq!(result.outputs.get("geen_bedrag"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_null_key_cell_for_a_non_nullable_input_is_refused_at_the_register() {
        // `probe` with its partner key declared never absent: the register's
        // null cell for it contradicts the law, and is refused where it is
        // read, before the bron is asked about nobody.
        let (bron, caller) = optional_key_laws();
        let caller = caller.replacen(
            "          - name: partner_bsn\n            type: string\n            nullable: true\n",
            "          - name: partner_bsn\n            type: string\n",
            1,
        );
        assert_eq!(caller.matches("nullable: true").count(), 2, "{caller}");
        let mut service = LawExecutionService::new();
        service.load_law(bron).unwrap();
        service.load_law(caller.as_str()).unwrap();
        let mut row = BTreeMap::new();
        row.insert("bsn".to_string(), Value::String("1".to_string()));
        row.insert("partner_bsn".to_string(), Value::Null);
        service
            .register_dict_source_for_law("probe", "register", "bsn", vec![row], 10)
            .unwrap();
        let err = service
            .evaluate_law_output("probe", "partner_geboortejaar", bsn_one(), "2025-01-01")
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "probe");
                assert_eq!(field, "partner_bsn");
                assert_eq!(origin, "source register");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
    }

    #[test]
    fn test_register_that_cannot_be_asked_about_nobody_refuses_a_non_nullable_input() {
        // The bron's key `bsn` is nullable and passed as null: the register is
        // asked about nobody and answers with an absence. Its `geboortejaar`
        // input, made non-nullable here, does not take one.
        let (bron, _) = optional_key_laws();
        let bron = bron.replacen(
            "            type: number\n            nullable: true\n            source: {}",
            "            type: number\n            source: {}",
            1,
        );
        let mut service = LawExecutionService::new();
        service.load_law(&bron).unwrap();
        let mut record = BTreeMap::new();
        record.insert("bsn".to_string(), Value::String("2".to_string()));
        record.insert("geboortejaar".to_string(), Value::Int(1980));
        service
            .register_dict_source_for_law("probe_bron", "bron", "bsn", vec![record], 10)
            .unwrap();
        let err = service
            .evaluate_law_output(
                "probe_bron",
                "geboortejaar",
                params(&[("bsn", Value::Null)]),
                "2025-01-01",
            )
            .unwrap_err();
        match &err {
            EngineError::NullForNonNullable {
                law_id,
                field,
                origin,
            } => {
                assert_eq!(law_id, "probe_bron");
                assert_eq!(field, "geboortejaar");
                assert_eq!(origin, "lookup keyed on null");
            }
            other => panic!("expected NullForNonNullable, got {other:?}"),
        }
    }
}
