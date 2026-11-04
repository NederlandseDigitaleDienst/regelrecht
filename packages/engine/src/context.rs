//! Execution context for article evaluation
//!
//! Manages state and value resolution during article execution.
//!
//! # Resolution Priority
//!
//! Variables are resolved in the following order (first match wins):
//! 1. **Context variables** - Built-in variables like `referencedate`
//! 2. **Local scope** - Loop variables from FOREACH operations
//! 3. **Outputs** - Previously calculated output values
//! 4. **Definitions** - Article-level constants
//! 5. **Inputs and open terms** - Resolved on first read by the service (RFC-043)
//! 6. **Parameters** - Direct input parameters (e.g., BSN)
//! 7. **Unpassed optional parameters** - Unknown for lack of them (RFC-036)
//!
//! # Dot Notation
//!
//! Supports nested property access using dot notation:
//! - `referencedate.year` - Get year from reference date
//! - `person.address.city` - Navigate nested objects
//!
//! # Child Context Behavior
//!
//! The `create_child()` method creates a child context for nested evaluation
//! (e.g., FOREACH loops). Important: **child contexts start with an empty local scope**.
//!
//! This is an intentional design difference from the Python implementation:
//! - **Rust**: Clears local scope in child contexts (safer, prevents variable pollution)
//! - **Python**: Copies local scope to child contexts (allows cross-iteration access)
//!
//! If you need to pass values between iterations, use parameters or store them
//! in outputs rather than relying on local scope inheritance.

use crate::article::{ActionValue, CombineOp, Definition};
use crate::config;
use crate::error::{EngineError, Result};
use crate::operations::ValueResolver;
use crate::trace::{LegalAnchor, TraceBuilder};
use crate::types::{MissingKind, PathNodeType, ResolveType, TypeSpec, Value};
use chrono::{Datelike, NaiveDate};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

/// Execution context for article evaluation.
///
/// Holds all state needed during article execution including parameters,
/// definitions, outputs, and cached values.
///
/// # Shadowing Warning
///
/// Variables in higher-priority scopes shadow those in lower scopes.
/// For example, a local variable named "x" will shadow a parameter "x".
/// The priority order is: local > outputs > definitions > inputs and open
/// terms > parameters.
#[derive(Debug, Clone)]
pub struct RuleContext<'l> {
    /// Article-level definitions (constants)
    definitions: Rc<BTreeMap<String, Value>>,

    /// Input parameters (e.g., BSN, income)
    parameters: Rc<BTreeMap<String, Value>>,

    /// Calculated output values
    outputs: Rc<BTreeMap<String, Value>>,

    /// Local scope variables (for FOREACH loops)
    local: BTreeMap<String, Value>,

    /// Names a FOLD binds (RFC-056): unlike `local`, a child context keeps
    /// them, so they stay visible in what a FOLD body nests, a FOREACH
    /// among them. A local of the same name shadows one.
    lexical: Rc<BTreeMap<String, Value>>,

    /// Reference date for calculations
    reference_date: NaiveDate,

    /// Cached Value representation of reference_date (avoids repeated allocation)
    reference_date_value: Value,

    /// Optional shared trace builder for execution tracing
    trace: Option<Rc<RefCell<TraceBuilder>>>,

    /// `$id` of the law being executed; the provenance of an Unknown produced
    /// here names it (RFC-036). Empty for a bare context.
    law_id: Rc<str>,

    /// Parameters the article declares with `required: false` that the caller
    /// did not pass. A reference to one resolves to an Unknown for lack of that
    /// parameter, not to a `VariableNotFound` (RFC-036).
    unpassed_optional: Rc<BTreeSet<String>>,

    /// The provision whose rules this context executes (RFC-039). Every trace
    /// step pushed from here inherits it, which is what gives an arithmetic
    /// step an article: the operation evaluator knows no law, but the context
    /// it resolves against does.
    anchor: Option<LegalAnchor>,

    /// Resolves a declared input or open term the first time an operation
    /// reads it (RFC-043). Absent for a bare context, whose inputs are its
    /// parameters.
    lazy: Option<LazyHook<'l>>,
}

/// Resolution on first read (RFC-043): the service implements this for the
/// article being executed, so an input is fetched when an operation reads it,
/// and not at all when none does.
pub(crate) trait LazyInputs {
    /// The value of `name` if it is an input or open term this article
    /// resolves itself; `None` if it is not, or if it stays unresolved (a
    /// reference to it then fails like any unknown variable).
    /// The value comes with how the trace names where it came from.
    fn resolve_input(&self, name: &str) -> Option<Result<(Value, ResolveType)>>;

    /// What output `name` becomes once this article has computed it as
    /// `value`, with `outputs` the ones computed before it. `None` keeps
    /// `value`; `Some` is what a replacing override made of it (RFC-007),
    /// which the later actions of this article then read.
    fn replace_output(
        &self,
        _name: &str,
        _value: &Value,
        _outputs: &BTreeMap<String, Value>,
    ) -> Option<Result<Value>> {
        None
    }
}

/// The hook as a context field: a reference, copied into every child scope,
/// so a FOREACH body resolves against the same memo as the article.
#[derive(Clone, Copy)]
struct LazyHook<'l>(&'l dyn LazyInputs);

impl std::fmt::Debug for LazyHook<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LazyHook")
    }
}

impl<'l> RuleContext<'l> {
    /// Create a new execution context.
    ///
    /// # Arguments
    /// * `parameters` - Input parameters for the execution
    /// * `calculation_date` - Reference date for calculations (YYYY-MM-DD format)
    pub fn new(parameters: BTreeMap<String, Value>, calculation_date: &str) -> Result<Self> {
        let reference_date = NaiveDate::parse_from_str(calculation_date, "%Y-%m-%d")
            .map_err(|e| EngineError::InvalidDate(format!("{}: {}", calculation_date, e)))?;

        let reference_date_value = date_to_value(reference_date);

        Ok(Self {
            definitions: Rc::new(BTreeMap::new()),
            parameters: Rc::new(parameters),
            outputs: Rc::new(BTreeMap::new()),
            local: BTreeMap::new(),
            lexical: Rc::new(BTreeMap::new()),
            reference_date,
            reference_date_value,
            trace: None,
            law_id: Rc::from(""),
            unpassed_optional: Rc::new(BTreeSet::new()),
            anchor: None,
            lazy: None,
        })
    }

    /// What output `name`, just computed as `value`, becomes: see
    /// [`LazyInputs::replace_output`]. `None` for a bare context.
    pub(crate) fn replaced_output(&self, name: &str, value: &Value) -> Option<Result<Value>> {
        let LazyHook(lazy) = self.lazy?;
        lazy.replace_output(name, value, &self.outputs)
    }

    /// Resolve declared inputs and open terms on first read (RFC-043).
    pub(crate) fn set_lazy(&mut self, lazy: &'l dyn LazyInputs) {
        self.lazy = Some(LazyHook(lazy));
    }

    /// Name the law this context executes and the optional parameters the
    /// caller left out (RFC-036).
    ///
    /// Both feed the provenance of an Unknown: a reference to an unpassed
    /// optional parameter resolves to `Unknown` for lack of `law_id.name`, so a
    /// decision process can ask for exactly that fact. A required parameter is
    /// never in this set; leaving it out stays the error it always was.
    pub fn set_law_scope(&mut self, law_id: &str, unpassed_optional: BTreeSet<String>) {
        self.law_id = Rc::from(law_id);
        self.unpassed_optional = Rc::new(unpassed_optional);
    }

    /// `$id` of the law this context executes (empty for a bare context).
    pub fn law_id(&self) -> &str {
        &self.law_id
    }

    /// Set definitions from an article's definitions section.
    ///
    /// Processes the Definition enum to extract actual values.
    pub fn set_definitions(&mut self, definitions: &HashMap<String, Definition>) {
        self.definitions = Rc::new(
            definitions
                .iter()
                .map(|(k, v)| (k.clone(), v.value().clone()))
                .collect(),
        );
    }

    /// Set definitions directly from a Value HashMap.
    pub fn set_definitions_raw(&mut self, definitions: BTreeMap<String, Value>) {
        self.definitions = Rc::new(definitions);
    }

    /// Set an output value.
    pub fn set_output(&mut self, name: impl Into<String>, value: Value) {
        Rc::make_mut(&mut self.outputs).insert(name.into(), value);
    }

    /// Get an output value.
    pub fn get_output(&self, name: &str) -> Option<&Value> {
        self.outputs.get(name)
    }

    /// Get all outputs.
    pub fn outputs(&self) -> &BTreeMap<String, Value> {
        &self.outputs
    }

    /// Set a local variable (for FOREACH loops).
    pub fn set_local(&mut self, name: impl Into<String>, value: Value) {
        self.local.insert(name.into(), value);
    }

    /// Bind a name of a FOLD (RFC-056): visible here and in every child
    /// context made from this one, unless a local shadows it.
    pub fn set_lexical(&mut self, name: impl Into<String>, value: Value) {
        Rc::make_mut(&mut self.lexical).insert(name.into(), value);
    }

    /// Clear all local variables.
    pub fn clear_local(&mut self) {
        self.local.clear();
    }

    /// Get the reference date.
    pub fn reference_date(&self) -> NaiveDate {
        self.reference_date
    }

    /// Get the calculation date as a string (YYYY-MM-DD format).
    pub fn get_calculation_date(&self) -> &str {
        // Extract ISO date string from the cached reference_date_value
        // This is safe because we always set it in new()
        if let Value::Object(obj) = &self.reference_date_value {
            if let Some(Value::String(iso)) = obj.get("iso") {
                return iso.as_str();
            }
        }
        // Fallback - should never happen since we always set iso in new()
        "1970-01-01"
    }

    /// Create a child context for nested evaluation (e.g., FOREACH).
    ///
    /// The child inherits definitions, parameters, and outputs,
    /// but starts with an **empty local scope**. This ensures that FOREACH loop
    /// variables from a parent context don't leak into child iterations.
    ///
    /// # Design Note
    ///
    /// This behavior differs from the Python implementation, which copies the
    /// local scope to child contexts. The Rust implementation intentionally
    /// clears local scope to:
    ///
    /// 1. **Prevent variable pollution**: Loop variables shouldn't accidentally
    ///    affect nested operations
    /// 2. **Explicit is better**: If you need values in a child context, pass
    ///    them explicitly via parameters or outputs
    /// 3. **Safety**: Reduces the risk of subtle bugs from shared mutable state
    ///
    /// If Python compatibility is required for specific use cases, pass the
    /// needed values explicitly via parameters before evaluation.
    pub fn create_child(&self) -> Self {
        Self {
            definitions: Rc::clone(&self.definitions),
            parameters: Rc::clone(&self.parameters),
            outputs: Rc::clone(&self.outputs),
            local: BTreeMap::new(), // Child starts with empty local scope
            lexical: Rc::clone(&self.lexical), // A FOLD's names reach nested bodies
            reference_date: self.reference_date,
            reference_date_value: self.reference_date_value.clone(),
            trace: self.trace.clone(), // Share the same trace builder
            law_id: Rc::clone(&self.law_id),
            unpassed_optional: Rc::clone(&self.unpassed_optional),
            // A child context evaluates inside the same article as its parent:
            // `create_child` is what a FOREACH body runs in, and that body is
            // no less anchored than the operation containing it. Resetting this
            // to `None` dropped the anchor from every trace step inside a loop
            // (participatiewet uses FOREACH in the live corpus), because
            // `trace_push` only stamps one when the context carries it.
            anchor: self.anchor.clone(),
            lazy: self.lazy,
        }
    }

    // -------------------------------------------------------------------------
    // Trace Methods
    // -------------------------------------------------------------------------

    /// Set a shared trace builder on this context.
    pub fn set_trace(&mut self, trace: Rc<RefCell<TraceBuilder>>) {
        self.trace = Some(trace);
    }

    /// Get a reference to the shared trace builder, if any.
    pub fn trace(&self) -> Option<&Rc<RefCell<TraceBuilder>>> {
        self.trace.as_ref()
    }

    /// The provision whose rules this context executes (RFC-039).
    pub fn set_anchor(&mut self, anchor: LegalAnchor) {
        self.anchor = Some(anchor);
    }

    /// Push a new node onto the trace stack. No-op if trace is None.
    ///
    /// Stamps the provision this context is executing (RFC-039), so a bare
    /// `ADD` inside an article names that article without the operation
    /// evaluator having to know one.
    pub fn trace_push(&self, name: &str, node_type: PathNodeType) {
        if let Some(ref trace) = self.trace {
            let mut tb = trace.borrow_mut();
            tb.push(name, node_type);
            if let Some(ref anchor) = self.anchor {
                tb.set_anchor(anchor.clone());
            }
        }
    }

    /// Pop the current node from the trace stack. No-op if trace is None.
    pub fn trace_pop(&self) {
        if let Some(ref trace) = self.trace {
            trace.borrow_mut().pop();
        }
    }

    /// Set the result on the current trace node. No-op if trace is None.
    pub fn trace_set_result(&self, result: Value) {
        if let Some(ref trace) = self.trace {
            trace.borrow_mut().set_result(result);
        }
    }

    /// Set the resolve type on the current trace node. No-op if trace is None.
    pub fn trace_set_resolve_type(&self, rt: ResolveType) {
        if let Some(ref trace) = self.trace {
            trace.borrow_mut().set_resolve_type(rt);
        }
    }

    /// Record the unit and precision the law declares for the current step's
    /// value (RFC-023). No-op if trace is None.
    pub fn trace_set_type_spec(&self, type_spec: TypeSpec) {
        if let Some(ref trace) = self.trace {
            trace.borrow_mut().set_type_spec(type_spec);
        }
    }

    /// Record what the corpus cites as the basis of the current step (RFC-039).
    /// No-op if trace is None.
    pub fn trace_set_legal_basis(&self, legal_basis: LegalAnchor) {
        if let Some(ref trace) = self.trace {
            trace.borrow_mut().set_legal_basis(legal_basis);
        }
    }

    /// Set a message on the current trace node. No-op if trace is None.
    pub fn trace_set_message(&self, msg: impl Into<String>) {
        if let Some(ref trace) = self.trace {
            trace.borrow_mut().set_message(msg);
        }
    }

    /// Get message from the current trace node. Returns None if trace is None.
    pub fn trace_get_message(&self) -> Option<String> {
        self.trace
            .as_ref()
            .and_then(|trace| trace.borrow().get_message().map(String::from))
    }

    /// Check if tracing is active.
    pub fn has_trace(&self) -> bool {
        self.trace.is_some()
    }

    /// Resolve a variable name using the priority chain.
    ///
    /// # Resolution Priority
    /// 1. Context variables (referencedate)
    /// 2. Local scope (loop variables)
    /// 3. Outputs (calculated values)
    /// 4. Definitions (constants)
    /// 5. Inputs and open terms (resolved on first read, RFC-043)
    /// 6. Parameters (direct inputs)
    /// 7. Unpassed optional parameters (unknown, RFC-036)
    ///
    /// # Dot Notation
    /// Supports nested property access: `referencedate.year`, `person.name`
    fn resolve_variable(&self, path: &str) -> Result<Value> {
        if self.trace.is_none() {
            return self.resolve_variable_internal(path);
        }

        // Tracing path: push a Resolve node
        self.trace_push(path, PathNodeType::Resolve);

        let result = self.resolve_variable_internal(path);

        match &result {
            Ok(value) => {
                self.trace_set_result(value.clone());
            }
            Err(_) => {
                self.trace_set_message(format!(
                    "Could not resolve value for {}",
                    path.to_uppercase()
                ));
            }
        }
        self.trace_pop();
        result
    }

    /// Internal variable resolution without tracing.
    fn resolve_variable_internal(&self, path: &str) -> Result<Value> {
        // Handle dot notation for property access
        if let Some((base, property)) = path.split_once('.') {
            // Fast-path for referencedate sub-properties — avoids cloning the Object
            if base == "referencedate" {
                return match property {
                    "year" => {
                        self.trace_set_resolve_type(ResolveType::Context);
                        Ok(Value::Int(i64::from(self.reference_date.year())))
                    }
                    "month" => {
                        self.trace_set_resolve_type(ResolveType::Context);
                        Ok(Value::Int(i64::from(self.reference_date.month())))
                    }
                    "day" => {
                        self.trace_set_resolve_type(ResolveType::Context);
                        Ok(Value::Int(i64::from(self.reference_date.day())))
                    }
                    "iso" => {
                        self.trace_set_resolve_type(ResolveType::Context);
                        Ok(Value::String(
                            self.reference_date.format("%Y-%m-%d").to_string(),
                        ))
                    }
                    _ => {
                        let base_value = self.resolve_variable(base)?;
                        get_property(&base_value, property, 0)
                    }
                };
            }
            let base_value = self.resolve_variable(base)?;
            return get_property(&base_value, property, 0);
        }

        // 1. Context variables (cached)
        if path == "referencedate" {
            self.trace_set_resolve_type(ResolveType::Context);
            return Ok(self.reference_date_value.clone());
        }

        // 2. Local scope (FOREACH loop variables)
        if let Some(value) = self.local.get(path).or_else(|| self.lexical.get(path)) {
            self.trace_set_resolve_type(ResolveType::Local);
            return Ok(value.clone());
        }

        // 3. Outputs (calculated values)
        if let Some(value) = self.outputs.get(path) {
            self.trace_set_resolve_type(ResolveType::Output);
            return Ok(value.clone());
        }

        // 4. Definitions (constants)
        if let Some(value) = self.definitions.get(path) {
            self.trace_set_resolve_type(ResolveType::Definition);
            return Ok(value.clone());
        }

        // 5. An input or open term of this article, resolved on its first
        // read (RFC-043). Where a value was passed under the input's name the
        // hook answers `None`, and the parameter below wins.
        if let Some(LazyHook(lazy)) = self.lazy {
            if let Some(result) = lazy.resolve_input(path) {
                return result.map(|(value, resolve_type)| {
                    self.trace_set_resolve_type(resolve_type);
                    value
                });
            }
        }

        // 6. Parameters (direct inputs)
        if let Some(value) = self.parameters.get(path) {
            self.trace_set_resolve_type(ResolveType::Parameter);
            return Ok(value.clone());
        }

        // 7. An optional parameter the caller did not pass (RFC-036). The
        // article said it can do without, so the fact is unknown rather than
        // the reference being an error; the Unknown names it, so whoever
        // completes the case knows what to ask for. Top-level and cross-law
        // calls are treated alike here.
        if self.unpassed_optional.contains(path) {
            self.trace_set_resolve_type(ResolveType::Parameter);
            self.trace_set_message(format!(
                "Parameter '{path}' is optional and was not passed: unknown"
            ));
            return Ok(Value::unknown(
                self.law_id.as_ref(),
                path,
                MissingKind::NotPassed,
            ));
        }

        // Not found
        Err(EngineError::VariableNotFound(path.to_string()))
    }
}

impl ValueResolver for RuleContext<'_> {
    fn resolve(&self, name: &str) -> Result<Value> {
        self.resolve_variable(name)
    }

    fn trace_push(&self, name: &str, node_type: PathNodeType) {
        RuleContext::trace_push(self, name, node_type);
    }

    fn trace_pop(&self) {
        RuleContext::trace_pop(self);
    }

    fn trace_set_result(&self, result: Value) {
        RuleContext::trace_set_result(self, result);
    }

    fn trace_set_message(&self, msg: String) {
        RuleContext::trace_set_message(self, msg);
    }

    fn trace_get_message(&self) -> Option<String> {
        RuleContext::trace_get_message(self)
    }

    fn has_trace(&self) -> bool {
        RuleContext::has_trace(self)
    }

    /// `RuleContext` can create child scopes, so it is the resolver that can
    /// run a FOREACH (RFC-016).
    fn execute_foreach_op(
        &self,
        collection: &ActionValue,
        as_name: Option<&str>,
        body: &ActionValue,
        filter: Option<&ActionValue>,
        combine: Option<&CombineOp>,
        depth: usize,
    ) -> Option<Result<Value>> {
        Some(crate::operations::execute_foreach(
            collection, as_name, body, filter, combine, self, depth,
        ))
    }

    /// A FOLD binds its names in a child scope too (RFC-056).
    fn execute_fold_op(
        &self,
        collection: &ActionValue,
        as_name: &str,
        state: &str,
        initial: &ActionValue,
        body: &ActionValue,
        depth: usize,
    ) -> Option<Result<Value>> {
        Some(crate::operations::execute_fold(
            collection, as_name, state, initial, body, self, depth,
        ))
    }
}

/// Convert a NaiveDate to a Value object with year, month, day properties.
fn date_to_value(date: NaiveDate) -> Value {
    let mut obj = BTreeMap::new();
    obj.insert("year".to_string(), Value::Int(date.year() as i64));
    obj.insert("month".to_string(), Value::Int(date.month() as i64));
    obj.insert("day".to_string(), Value::Int(date.day() as i64));
    // Also include ISO format string for direct use
    obj.insert(
        "iso".to_string(),
        Value::String(date.format("%Y-%m-%d").to_string()),
    );
    Value::Object(obj)
}

/// Get a property from a Value, supporting nested access.
///
/// Handles:
/// - Object property access: `obj.property`
/// - Nested paths: `obj.nested.property`
/// - Array indexing: `arr.0`, `arr.1`
///
/// # Arguments
/// * `value` - The value to access property from
/// * `property_path` - Property path (may contain dots for nesting)
/// * `depth` - Current recursion depth (for stack overflow protection)
fn get_property(value: &Value, property_path: &str, depth: usize) -> Result<Value> {
    // Prevent stack overflow on deeply nested or malicious input
    if depth >= config::MAX_PROPERTY_DEPTH {
        return Err(EngineError::InvalidOperation(format!(
            "Property access depth exceeds maximum of {}",
            config::MAX_PROPERTY_DEPTH
        )));
    }

    // Handle nested paths recursively. Beide aanroepen krijgen dezelfde,
    // eenmaal opgehoogde diepte. Twee losse `depth + 1`-uitdrukkingen leverden
    // twee mutanten op met exact dezelfde omschrijving, waarvan alleen die op
    // de `rest`-tak te doden is: de `first`-tak stopt altijd meteen, want daar
    // zit per definitie geen punt meer in. Eén uitdrukking, één mutant, en die
    // wordt gedood door test_property_depth_limit_boundary.
    if let Some((first, rest)) = property_path.split_once('.') {
        let deeper = depth + 1;
        let intermediate = get_property(value, first, deeper)?;
        return get_property(&intermediate, rest, deeper);
    }

    match value {
        Value::Object(obj) => obj
            .get(property_path)
            .cloned()
            .ok_or_else(|| EngineError::VariableNotFound(format!(".{}", property_path))),
        // A property of an absent record is absent: the register says there is
        // no WIA decision, no partner, so every field of it is "geen" too, and
        // the law's own absence checks decide (RFC-036). Failing here would fail
        // the whole calculation for a person the record does not apply to.
        Value::Null => Ok(Value::Null),
        // A property of a record nobody has yet is unknown for the same facts.
        Value::Unknown(_) => Ok(value.clone()),
        Value::Array(arr) => {
            // Support numeric indexing for arrays
            if let Ok(index) = property_path.parse::<usize>() {
                arr.get(index)
                    .cloned()
                    .ok_or_else(|| EngineError::VariableNotFound(format!("[{}]", index)))
            } else {
                Err(EngineError::TypeMismatch {
                    expected: "object".to_string(),
                    actual: "array".to_string(),
                })
            }
        }
        _ => Err(EngineError::TypeMismatch {
            expected: "object".to_string(),
            actual: value.type_name().to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn make_context() -> RuleContext<'static> {
        let mut params = BTreeMap::new();
        params.insert("BSN".to_string(), Value::String("123456789".to_string()));
        params.insert("income".to_string(), Value::Int(30000));

        RuleContext::new(params, "2025-06-15").unwrap()
    }

    // -------------------------------------------------------------------------
    // Basic Resolution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolve_parameter() {
        let ctx = make_context();

        let bsn = ctx.resolve("BSN").unwrap();
        assert_eq!(bsn, Value::String("123456789".to_string()));

        let income = ctx.resolve("income").unwrap();
        assert_eq!(income, Value::Int(30000));
    }

    #[test]
    fn test_resolve_definition() {
        let mut ctx = make_context();
        let mut defs = BTreeMap::new();
        defs.insert("MAX_INCOME".to_string(), Value::Int(50000));
        defs.insert("TAX_RATE".to_string(), Value::from(0.21));
        ctx.set_definitions_raw(defs);

        let max = ctx.resolve("MAX_INCOME").unwrap();
        assert_eq!(max, Value::Int(50000));

        let rate = ctx.resolve("TAX_RATE").unwrap();
        assert_eq!(rate, Value::from(0.21));
    }

    #[test]
    fn test_resolve_output() {
        let mut ctx = make_context();
        ctx.set_output("result", Value::Bool(true));
        ctx.set_output("amount", Value::Int(1000));

        let result = ctx.resolve("result").unwrap();
        assert_eq!(result, Value::Bool(true));

        let amount = ctx.resolve("amount").unwrap();
        assert_eq!(amount, Value::Int(1000));
    }

    #[test]
    fn test_resolve_local() {
        let mut ctx = make_context();
        ctx.set_local("item", Value::String("test".to_string()));
        ctx.set_local("index", Value::Int(0));

        let item = ctx.resolve("item").unwrap();
        assert_eq!(item, Value::String("test".to_string()));

        let index = ctx.resolve("index").unwrap();
        assert_eq!(index, Value::Int(0));
    }

    #[test]
    fn test_resolve_not_found() {
        let ctx = make_context();
        let result = ctx.resolve("nonexistent");
        assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
    }

    // -------------------------------------------------------------------------
    // Priority Chain Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_priority_local_over_output() {
        let mut ctx = make_context();
        ctx.set_output("x", Value::Int(100));
        ctx.set_local("x", Value::Int(200));

        // Local should win
        let x = ctx.resolve("x").unwrap();
        assert_eq!(x, Value::Int(200));

        // After clearing local, output should be visible
        ctx.clear_local();
        let x = ctx.resolve("x").unwrap();
        assert_eq!(x, Value::Int(100));
    }

    #[test]
    fn test_priority_output_over_definition() {
        let mut ctx = make_context();
        let mut defs = BTreeMap::new();
        defs.insert("x".to_string(), Value::Int(100));
        ctx.set_definitions_raw(defs);
        ctx.set_output("x", Value::Int(200));

        // Output should win over definition
        let x = ctx.resolve("x").unwrap();
        assert_eq!(x, Value::Int(200));
    }

    #[test]
    fn test_priority_definition_over_parameter() {
        let mut ctx = make_context();
        // "income" exists as parameter (30000)
        let mut defs = BTreeMap::new();
        defs.insert("income".to_string(), Value::Int(50000));
        ctx.set_definitions_raw(defs);

        // Definition should win over parameter
        let income = ctx.resolve("income").unwrap();
        assert_eq!(income, Value::Int(50000));
    }

    // -------------------------------------------------------------------------
    // Reference Date Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolve_referencedate() {
        let ctx = make_context();
        let refdate = ctx.resolve("referencedate").unwrap();

        // Should be an object with year, month, day
        let obj = refdate.as_object().unwrap();
        assert_eq!(obj.get("year"), Some(&Value::Int(2025)));
        assert_eq!(obj.get("month"), Some(&Value::Int(6)));
        assert_eq!(obj.get("day"), Some(&Value::Int(15)));
        assert_eq!(
            obj.get("iso"),
            Some(&Value::String("2025-06-15".to_string()))
        );
    }

    #[test]
    fn test_resolve_referencedate_year() {
        let ctx = make_context();
        let year = ctx.resolve("referencedate.year").unwrap();
        assert_eq!(year, Value::Int(2025));
    }

    #[test]
    fn test_resolve_referencedate_month() {
        let ctx = make_context();
        let month = ctx.resolve("referencedate.month").unwrap();
        assert_eq!(month, Value::Int(6));
    }

    // -------------------------------------------------------------------------
    // Dot Notation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_dot_notation_object() {
        let mut ctx = make_context();

        let mut person = BTreeMap::new();
        person.insert("name".to_string(), Value::String("Jan".to_string()));
        person.insert("age".to_string(), Value::Int(35));
        ctx.set_output("person", Value::Object(person));

        let name = ctx.resolve("person.name").unwrap();
        assert_eq!(name, Value::String("Jan".to_string()));

        let age = ctx.resolve("person.age").unwrap();
        assert_eq!(age, Value::Int(35));
    }

    #[test]
    fn test_dot_notation_nested() {
        let mut ctx = make_context();

        let mut address = BTreeMap::new();
        address.insert("city".to_string(), Value::String("Amsterdam".to_string()));
        address.insert("zip".to_string(), Value::String("1012AB".to_string()));

        let mut person = BTreeMap::new();
        person.insert("name".to_string(), Value::String("Jan".to_string()));
        person.insert("address".to_string(), Value::Object(address));
        ctx.set_output("person", Value::Object(person));

        let city = ctx.resolve("person.address.city").unwrap();
        assert_eq!(city, Value::String("Amsterdam".to_string()));
    }

    #[test]
    fn test_dot_notation_array_index() {
        let mut ctx = make_context();

        let items = vec![
            Value::String("first".to_string()),
            Value::String("second".to_string()),
            Value::String("third".to_string()),
        ];
        ctx.set_output("items", Value::Array(items));

        let first = ctx.resolve("items.0").unwrap();
        assert_eq!(first, Value::String("first".to_string()));

        let second = ctx.resolve("items.1").unwrap();
        assert_eq!(second, Value::String("second".to_string()));
    }

    #[test]
    fn test_dot_notation_not_found() {
        let mut ctx = make_context();

        let mut person = BTreeMap::new();
        person.insert("name".to_string(), Value::String("Jan".to_string()));
        ctx.set_output("person", Value::Object(person));

        let result = ctx.resolve("person.nonexistent");
        assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
    }

    #[test]
    fn test_dot_notation_type_error() {
        let mut ctx = make_context();
        ctx.set_output("value", Value::Int(42));

        // Can't access property on integer
        let result = ctx.resolve("value.something");
        assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
    }

    // -------------------------------------------------------------------------
    // Child Context Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_child_context_inherits() {
        let mut ctx = make_context();
        ctx.set_output("parent_output", Value::Int(100));
        let mut defs = BTreeMap::new();
        defs.insert("CONSTANT".to_string(), Value::Int(42));
        ctx.set_definitions_raw(defs);

        let child = ctx.create_child();

        // Child should see parent's output and definition
        assert_eq!(child.resolve("parent_output").unwrap(), Value::Int(100));
        assert_eq!(child.resolve("CONSTANT").unwrap(), Value::Int(42));
        // And parameters
        assert_eq!(
            child.resolve("BSN").unwrap(),
            Value::String("123456789".to_string())
        );
    }

    #[test]
    fn test_child_context_isolation() {
        let mut ctx = make_context();
        ctx.set_output("shared", Value::Int(100));

        let mut child = ctx.create_child();
        child.set_output("shared", Value::Int(200));
        child.set_local("child_only", Value::Int(999));

        // Child's changes shouldn't affect parent
        assert_eq!(ctx.resolve("shared").unwrap(), Value::Int(100));
        assert!(ctx.resolve("child_only").is_err());

        // Child should see its own value
        assert_eq!(child.resolve("shared").unwrap(), Value::Int(200));
    }

    #[test]
    fn test_child_context_empty_local_scope() {
        let mut ctx = make_context();
        ctx.set_local("parent_loop_var", Value::Int(999));
        ctx.set_local("index", Value::Int(5));

        // Create child - should NOT inherit parent's local variables
        let child = ctx.create_child();

        // Child should NOT see parent's loop variables
        assert!(child.resolve("parent_loop_var").is_err());
        assert!(child.resolve("index").is_err());

        // But parent should still have them
        assert_eq!(ctx.resolve("parent_loop_var").unwrap(), Value::Int(999));
        assert_eq!(ctx.resolve("index").unwrap(), Value::Int(5));
    }

    // -------------------------------------------------------------------------
    // Edge Cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_invalid_date() {
        let params = BTreeMap::new();
        let result = RuleContext::new(params, "not-a-date");
        assert!(matches!(result, Err(EngineError::InvalidDate(_))));
    }

    #[test]
    fn test_empty_context() {
        let ctx = RuleContext::new(BTreeMap::new(), "2025-01-01").unwrap();
        let result = ctx.resolve("anything");
        assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
    }

    // -------------------------------------------------------------------------
    // Accessor Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_get_output_returns_stored_value() {
        let mut ctx = make_context();
        ctx.set_output("amount", Value::Int(1000));

        assert_eq!(ctx.get_output("amount"), Some(&Value::Int(1000)));
        assert_eq!(ctx.get_output("never_set"), None);
    }

    #[test]
    fn test_reference_date_matches_calculation_date() {
        let ctx = make_context();

        assert_eq!(
            ctx.reference_date(),
            NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()
        );
    }

    #[test]
    fn test_get_calculation_date_returns_the_iso_date() {
        let ctx = make_context();

        assert_eq!(ctx.get_calculation_date(), "2025-06-15");
    }

    // -------------------------------------------------------------------------
    // Trace Tests
    // -------------------------------------------------------------------------

    fn traced_context() -> (RuleContext<'static>, Rc<RefCell<TraceBuilder>>) {
        let mut ctx = make_context();
        let trace = Rc::new(RefCell::new(TraceBuilder::new_untimed()));
        ctx.set_trace(Rc::clone(&trace));
        (ctx, trace)
    }

    /// Resolve `path` with tracing on and return the value plus the resolve node
    /// that was recorded for it.
    fn traced_resolve(path: &str) -> (Result<Value>, crate::trace::PathNode) {
        let (ctx, trace) = traced_context();
        // A parent node, so the resolve node is kept as its child on pop.
        trace.borrow_mut().push("root", PathNodeType::Action);

        let value = ctx.resolve(path);

        let root = trace.borrow_mut().pop().expect("root node should pop");
        let node = root
            .children
            .into_iter()
            .next()
            .expect("resolving should record a node");
        (value, node)
    }

    #[test]
    fn test_trace_is_absent_until_set() {
        let ctx = make_context();
        assert!(ctx.trace().is_none());
        assert!(!ctx.has_trace());
        assert!(!ValueResolver::has_trace(&ctx));

        let (traced, trace) = traced_context();
        assert!(traced.has_trace());
        assert!(ValueResolver::has_trace(&traced));
        let held = traced.trace().expect("trace should be set");
        assert!(
            Rc::ptr_eq(held, &trace),
            "context must hand back the builder it was given"
        );
    }

    #[test]
    fn test_child_context_shares_the_trace_builder() {
        let (ctx, trace) = traced_context();

        let child = ctx.create_child();

        let held = child.trace().expect("child should inherit the trace");
        assert!(
            Rc::ptr_eq(held, &trace),
            "child must write into the same trace as its parent"
        );
    }

    #[test]
    fn test_value_resolver_records_result_on_current_node() {
        let (ctx, trace) = traced_context();

        ValueResolver::trace_push(&ctx, "operation", PathNodeType::Operation);
        ValueResolver::trace_set_result(&ctx, Value::Int(42));

        let node = trace.borrow_mut().pop().expect("pushed node should pop");
        assert_eq!(node.name, "operation");
        assert_eq!(node.result, Some(Value::Int(42)));
    }

    #[test]
    fn test_trace_reports_referencedate_parts_as_context() {
        let cases = [
            ("referencedate.year", Value::Int(2025)),
            ("referencedate.month", Value::Int(6)),
            ("referencedate.day", Value::Int(15)),
            ("referencedate.iso", Value::String("2025-06-15".to_string())),
        ];

        for (path, expected) in cases {
            let (value, node) = traced_resolve(path);
            assert_eq!(value.unwrap(), expected, "value for {}", path);
            assert_eq!(node.name, path);
            assert_eq!(
                node.resolve_type,
                Some(ResolveType::Context),
                "{} should be reported as resolved from the context",
                path
            );
            assert_eq!(node.result, Some(expected), "traced result for {}", path);
        }
    }

    #[test]
    fn test_date_like_properties_come_from_their_own_object() {
        let mut ctx = make_context();

        let mut period = BTreeMap::new();
        period.insert("year".to_string(), Value::Int(1999));
        period.insert("month".to_string(), Value::Int(1));
        period.insert("day".to_string(), Value::Int(2));
        period.insert("iso".to_string(), Value::String("1999-01-02".to_string()));
        ctx.set_output("period", Value::Object(period));

        // Only `referencedate` gets the built-in date parts; any other object
        // keeps its own values.
        assert_eq!(ctx.resolve("period.year").unwrap(), Value::Int(1999));
        assert_eq!(ctx.resolve("period.month").unwrap(), Value::Int(1));
        assert_eq!(ctx.resolve("period.day").unwrap(), Value::Int(2));
        assert_eq!(
            ctx.resolve("period.iso").unwrap(),
            Value::String("1999-01-02".to_string())
        );
    }

    #[test]
    fn test_property_of_null_is_null() {
        // An unresolved register record reads as null; so does every field of
        // it, however deep the path. The law's null checks decide what that
        // means instead of the calculation failing.
        assert_eq!(
            get_property(&Value::Null, "status", 0).unwrap(),
            Value::Null
        );
        assert_eq!(
            get_property(&Value::Null, "adres.postcode", 0).unwrap(),
            Value::Null
        );
        let mut obj = BTreeMap::new();
        obj.insert("partner".to_string(), Value::Null);
        assert_eq!(
            get_property(&Value::Object(obj), "partner.geboortedatum", 0).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn test_property_of_unknown_is_unknown() {
        // A field of a record nobody has yet is unknown for the same facts;
        // nested paths too (RFC-036).
        let unknown = Value::unknown("testwet", "beschikking", MissingKind::NoData);
        assert_eq!(get_property(&unknown, "status", 0).unwrap(), unknown);
        assert_eq!(
            get_property(&unknown, "adres.postcode", 0).unwrap(),
            unknown
        );
        let mut obj = BTreeMap::new();
        obj.insert("partner".to_string(), unknown.clone());
        assert_eq!(
            get_property(&Value::Object(obj), "partner.naam", 0).unwrap(),
            unknown
        );
    }

    #[test]
    fn test_unpassed_optional_parameter_resolves_to_unknown() {
        // The article declared `aanvraag_bedrag` with `required: false` and the
        // caller left it out: a reference to it is an Unknown that names the
        // law and the parameter, so a decision process can ask for it (RFC-036).
        let (mut ctx, trace) = traced_context();
        ctx.set_law_scope("testwet", BTreeSet::from(["aanvraag_bedrag".to_string()]));
        trace.borrow_mut().push("root", PathNodeType::Action);

        let value = ctx.resolve("aanvraag_bedrag").unwrap();
        assert_eq!(
            value.missing_facts(),
            &[crate::types::MissingFact {
                law: "testwet".to_string(),
                name: "aanvraag_bedrag".to_string(),
                kind: MissingKind::NotPassed,
            }]
        );
        // A property of it is that same unknown.
        assert_eq!(ctx.resolve("aanvraag_bedrag.bedrag").unwrap(), value);

        let root = trace.borrow_mut().pop().expect("root node should pop");
        let node = root
            .children
            .first()
            .expect("resolving should record a node");
        assert_eq!(node.resolve_type, Some(ResolveType::Parameter));
        assert_eq!(
            node.message.as_deref(),
            Some("Parameter 'aanvraag_bedrag' is optional and was not passed: unknown")
        );
        assert_eq!(node.result.as_ref(), Some(&value));
    }

    #[test]
    fn test_missing_required_parameter_stays_not_found() {
        // Only the declared optional parameters resolve to unknown; anything
        // else the article forgot to pass is the error it always was, so a
        // misspelled key cannot quietly become an unknown outcome.
        let mut ctx = make_context();
        ctx.set_law_scope("testwet", BTreeSet::from(["aanvraag_bedrag".to_string()]));
        assert!(matches!(
            ctx.resolve("kvk_nummer"),
            Err(EngineError::VariableNotFound(_))
        ));
        // A passed value always wins over the unpassed set.
        assert_eq!(ctx.resolve("income").unwrap(), Value::Int(30000));
    }

    #[test]
    fn test_child_context_inherits_law_scope() {
        let mut ctx = make_context();
        ctx.set_law_scope("testwet", BTreeSet::from(["aanvraag_bedrag".to_string()]));
        let child = ctx.create_child();
        let value = child.resolve("aanvraag_bedrag").unwrap();
        assert_eq!(value.missing_facts()[0].law, "testwet");
    }

    #[test]
    fn test_property_depth_limit_boundary() {
        // `levels` wrappers of "n" around {"end": 999}, reached by a path with
        // `levels + 1` property segments after the base variable.
        fn nested(levels: usize) -> Value {
            let mut inner = BTreeMap::new();
            inner.insert("end".to_string(), Value::Int(999));
            let mut value = Value::Object(inner);
            for _ in 0..levels {
                let mut wrapper = BTreeMap::new();
                wrapper.insert("n".to_string(), value);
                value = Value::Object(wrapper);
            }
            value
        }
        fn path(levels: usize) -> String {
            let mut p = String::from("root");
            for _ in 0..levels {
                p.push_str(".n");
            }
            p.push_str(".end");
            p
        }

        let max = config::MAX_PROPERTY_DEPTH;

        // Exactly MAX_PROPERTY_DEPTH property segments is still allowed.
        let mut ctx = make_context();
        ctx.set_output("root", nested(max - 1));
        assert_eq!(ctx.resolve(&path(max - 1)).unwrap(), Value::Int(999));

        // One segment more is refused.
        let mut ctx = make_context();
        ctx.set_output("root", nested(max));
        let result = ctx.resolve(&path(max));
        assert!(
            matches!(result, Err(EngineError::InvalidOperation(ref msg)) if msg.contains("depth exceeds")),
            "Expected InvalidOperation error one level past the limit, got: {:?}",
            result
        );
    }

    #[test]
    fn test_recursion_depth_limit() {
        let mut ctx = make_context();

        // Create a deeply nested object (but not exceeding limit)
        // 5 levels of nesting: {n: {n: {n: {n: {n: {value: 42}}}}}}
        let mut deep = BTreeMap::new();
        deep.insert("value".to_string(), Value::Int(42));

        for _ in 0..5 {
            let mut wrapper = BTreeMap::new();
            wrapper.insert("n".to_string(), Value::Object(deep));
            deep = wrapper;
        }
        ctx.set_output("deep", Value::Object(deep));

        // This should work: deep.n.n.n.n.n.value (6 property accesses, well within limit of 32)
        let path = "deep.n.n.n.n.n.value";
        let result = ctx.resolve(path);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
        assert_eq!(result.unwrap(), Value::Int(42));

        // Create a structure that's deeper than MAX_PROPERTY_DEPTH (32)
        // We need (MAX_PROPERTY_DEPTH + 3) levels to trigger the depth limit
        let depth = config::MAX_PROPERTY_DEPTH + 3;
        let mut very_deep = BTreeMap::new();
        very_deep.insert("end".to_string(), Value::Int(999));

        for _ in 0..depth {
            let mut wrapper = BTreeMap::new();
            wrapper.insert("n".to_string(), Value::Object(very_deep));
            very_deep = wrapper;
        }
        ctx.set_output("very_deep", Value::Object(very_deep));

        // Build a path with (depth) "n"s + "end", exceeding limit
        let excessive_path = format!(
            "very_deep.{}.end",
            (0..depth).map(|_| "n").collect::<Vec<_>>().join(".")
        );

        // This should fail due to depth limit
        let result = ctx.resolve(&excessive_path);
        assert!(
            matches!(result, Err(EngineError::InvalidOperation(ref msg)) if msg.contains("depth exceeds")),
            "Expected InvalidOperation error for excessive depth, got: {:?}",
            result
        );
    }
}
