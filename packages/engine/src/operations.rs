//! Operation execution for the RegelRecht engine
//!
//! Implements the execution logic for all operation types in the regulation
//! schema, plus engine-only operations retained for backward compatibility.
//!
//! **Schema operations:**
//! - **Comparison:** EQUALS, GREATER_THAN, LESS_THAN, GREATER_THAN_OR_EQUAL, LESS_THAN_OR_EQUAL
//! - **Arithmetic:** ADD, SUBTRACT, MULTIPLY, DIVIDE
//! - **Aggregate:** MAX, MIN
//! - **Logical:** AND, OR, NOT
//! - **Conditional:** IF (multi-case with cases/default)
//! - **Collection:** IN, LIST
//! - **Date:** AGE, DATE_ADD, DATE, DAY_OF_WEEK, DATE_DIFF, DATE_PART, START_OF
//!
//! **Engine-only (not in schema, accepted for backward compatibility):**
//! NOT_EQUALS, IS_NULL, NOT_NULL, NOT_IN
//!
//! # Two kinds of nothing (RFC-036)
//!
//! Every operation checks its operands in one fixed order before it computes:
//!
//! 1. **Untranslatable** (RFC-012) taints the result;
//! 2. **Unknown** (a fact nobody has) propagates, as the union of every
//!    unknown operand's missing facts, so the outcome still names what is
//!    missing;
//! 3. **Null** (an absence the data is authoritative about) is a value: it
//!    can be tested (`EQUALS … null`, `IS_NULL`, `IN`) but never calculated
//!    with or decided on. Where a number, a date or a truth value is needed,
//!    a null operand is an [`EngineError::AbsentOperand`]: the law has to say
//!    what "geen" means before it counts it.
//!
//! The logical operations and `IF` are the one place a definite operand beats
//! an unknown (Kleene): `AND` with a `false` is `false` whatever the unknown
//! turns out to be, `OR` with a `true` is `true`.

use crate::article::{foreach_binding, ActionOperation, ActionValue, Case, CombineOp};
use crate::context::RuleContext;
use crate::error::{EngineError, Result};
use crate::types::{PathNodeType, Value};
use chrono::{Datelike, NaiveDate};
use regelrecht_law_model::stored;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps, RoundingStrategy};
use std::cmp::Ordering;

/// Maximum nesting depth for operations to prevent stack overflow
const MAX_OPERATION_DEPTH: usize = 100;

/// If any value in the slice is Untranslatable, return it (NaN-like propagation).
fn find_untranslatable(values: &[Value]) -> Option<Value> {
    values.iter().find(|v| v.is_untranslatable()).cloned()
}

/// If either of two values is Untranslatable, return it.
fn propagate_binary(a: &Value, b: &Value) -> Option<Value> {
    if a.is_untranslatable() {
        Some(a.clone())
    } else if b.is_untranslatable() {
        Some(b.clone())
    } else {
        None
    }
}

/// Render the missing facts of an Unknown for a trace message: `law.name, …`.
fn describe_missing(unknown: &Value) -> String {
    unknown
        .missing_facts()
        .iter()
        .map(|m| format!("{}.{}", m.law, m.name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// If any operand is Unknown, the operation is Unknown for the union of their
/// missing facts (RFC-036), and the trace says so.
///
/// Called after the Untranslatable check and before the Null check, so the
/// precedence "Untranslatable beats Unknown beats everything else" holds in
/// every operation that uses it.
fn propagate_unknown<'a, R: ValueResolver>(
    resolver: &R,
    op: &str,
    evaluated: impl IntoIterator<Item = &'a Value>,
) -> Option<Value> {
    let unknown = Value::merge_unknown(evaluated)?;
    resolver.trace_set_message(format!(
        "{op} with an unknown operand: unknown (missing: {})",
        describe_missing(&unknown)
    ));
    Some(unknown)
}

/// The structural counterpart of [`propagate_unknown`]: an Unknown counts
/// wherever it sits inside an operand (RFC-036). `EQUALS`, `NOT_EQUALS`, `IN`
/// and `NOT_IN` compare arrays and objects element by element, so
/// `[unknown] == [1]` is as undecidable as `unknown == 1`, and `PartialEq`
/// (which equates two Unknowns for the test harnesses) must never be reached
/// with an Unknown at any depth.
fn propagate_unknown_deep<'a, R: ValueResolver>(
    resolver: &R,
    op: &str,
    evaluated: impl IntoIterator<Item = &'a Value>,
) -> Option<Value> {
    let unknown = Value::merge_unknown_deep(evaluated)?;
    resolver.trace_set_message(format!(
        "{op} with an unknown in an operand: unknown (missing: {})",
        describe_missing(&unknown)
    ));
    Some(unknown)
}

/// The error for a null operand where a value was needed (RFC-036).
fn absent_operand(op: &str) -> EngineError {
    EngineError::AbsentOperand {
        operation: op.to_string(),
    }
}

/// Refuse a null operand where the operation needs a number, date or truth
/// value (RFC-036). Absence is a fact the law can test for; it is not an
/// amount or a verdict, and treating it as one would decide something the
/// legal text never said.
fn reject_null<'a>(op: &str, evaluated: impl IntoIterator<Item = &'a Value>) -> Result<()> {
    if evaluated.into_iter().any(Value::is_null) {
        return Err(absent_operand(op));
    }
    Ok(())
}

/// Trait for resolving variable references ($var) during operation execution.
///
/// Implementations should provide variable resolution from context (parameters,
/// inputs, outputs, definitions, etc.)
///
/// # Error Handling
///
/// - Return `Err(EngineError::VariableNotFound)` when a variable doesn't exist
/// - The error will propagate up through the operation execution chain
/// - Callers should handle missing variables appropriately (e.g., default values)
///
/// # Example Implementation
///
/// ```ignore
/// impl ValueResolver for MyContext {
///     fn resolve(&self, name: &str) -> Result<Value> {
///         self.variables.get(name)
///             .cloned()
///             .ok_or_else(|| EngineError::VariableNotFound(name.to_string()))
///     }
/// }
/// ```
pub trait ValueResolver {
    /// Resolve a variable name to its value.
    ///
    /// # Arguments
    /// * `name` - Variable name without the `$` prefix (e.g., "age" not "$age")
    ///
    /// # Returns
    /// * `Ok(Value)` - The resolved value
    /// * `Err(EngineError::VariableNotFound)` - Variable doesn't exist in context
    fn resolve(&self, name: &str) -> Result<Value>;

    /// Push a trace node. No-op by default.
    fn trace_push(&self, _name: &str, _node_type: PathNodeType) {}

    /// Pop a trace node. No-op by default.
    fn trace_pop(&self) {}

    /// Set result on current trace node. No-op by default.
    fn trace_set_result(&self, _result: Value) {}

    /// Set message on current trace node. No-op by default.
    fn trace_set_message(&self, _msg: String) {}

    /// Get message from current trace node. Returns None by default.
    fn trace_get_message(&self) -> Option<String> {
        None
    }

    /// Check if tracing is active. Returns false by default.
    fn has_trace(&self) -> bool {
        false
    }

    /// Execute a FOREACH (RFC-016), if this resolver can create child scopes.
    ///
    /// FOREACH is the one operation that binds a variable, so it needs a
    /// resolver that can hand out a child scope to bind it in. That is more
    /// than [`ValueResolver`] promises: the trait is generic over `&self` and
    /// says nothing about scoping, and simple resolvers (a map of variables in
    /// a unit test) have no scope to create.
    ///
    /// Returning `Option` rather than `Result` keeps those two cases apart.
    /// `None` means "this resolver cannot do FOREACH at all", which the caller
    /// turns into an error naming that reason; `Some(Err(..))` is a FOREACH
    /// that ran and failed. [`RuleContext`](crate::context::RuleContext)
    /// overrides this and delegates to [`execute_foreach`].
    fn execute_foreach_op(
        &self,
        _collection: &ActionValue,
        _as_name: Option<&str>,
        _body: &ActionValue,
        _filter: Option<&ActionValue>,
        _combine: Option<&CombineOp>,
        _depth: usize,
    ) -> Option<Result<Value>> {
        None
    }
}

/// Evaluate an ActionValue to a concrete Value.
///
/// Handles:
/// - Literal values (returned directly)
/// - Variable references ($name) - resolved via the resolver
/// - Nested operations - executed recursively
///
/// The depth parameter tracks recursion to prevent stack overflow.
pub fn evaluate_value<R: ValueResolver>(
    value: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if depth > MAX_OPERATION_DEPTH {
        return Err(EngineError::MaxDepthExceeded(depth));
    }

    match value {
        ActionValue::Literal(v) => {
            // Check if it's a variable reference (starts with $)
            if let Value::String(s) = v {
                if let Some(var_name) = s.strip_prefix('$') {
                    return resolver.resolve(var_name);
                }
            }
            Ok(v.clone())
        }
        ActionValue::Operation(op) => execute_operation(op, resolver, depth + 1),
    }
}

/// Execute an operation and return the result.
///
/// Dispatches to the appropriate operation handler based on the operation type.
/// The depth parameter tracks recursion to prevent stack overflow.
pub fn execute_operation<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if depth > MAX_OPERATION_DEPTH {
        return Err(EngineError::MaxDepthExceeded(depth));
    }

    let op_name = op.operation_name();
    let tracing = resolver.has_trace();
    if tracing {
        resolver.trace_push(op_name, PathNodeType::Operation);
    }

    let result = execute_operation_internal(op, resolver, depth);

    if tracing {
        match &result {
            Ok(value) => {
                resolver.trace_set_result(value.clone());
                // IF and FOREACH write their own message from inside the
                // handler: which case matched, how many elements were seen.
                // That says more than the value alone, so keep it instead of
                // overwriting. FOREACH reports its own result through the
                // combine, so its message stands on its own. An unknown result
                // keeps the handler's message too: it says which operand was
                // unknown and which facts are missing (RFC-036), which the
                // value alone does not.
                let existing_msg = resolver.trace_get_message();
                let msg = match (op, existing_msg) {
                    (ActionOperation::If { .. }, Some(case_info)) => {
                        format!("IF({}) = {}", case_info, format_value_for_trace(value))
                    }
                    (ActionOperation::Foreach { .. }, Some(summary)) => {
                        format!("{} = {}", summary, format_value_for_trace(value))
                    }
                    (_, Some(why_unknown)) if value.is_unknown() => why_unknown,
                    _ => format!(
                        "Compute {}(...) = {}",
                        op_name,
                        format_value_for_trace(value)
                    ),
                };
                resolver.trace_set_message(msg);
            }
            Err(e) => {
                resolver.trace_set_message(format!("Error in {}: {}", op_name, e));
            }
        }
        resolver.trace_pop();
    }

    result
}

/// Format a value compactly for trace messages.
fn format_value_for_trace(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Decimal(d) => format!("{}", d),
        Value::String(s) => format!("'{}'", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_for_trace).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(_) => "{...}".to_string(),
        Value::Untranslatable { article, .. } => format!("UNTRANSLATABLE(art. {})", article),
        Value::Unknown(missing) => format!(
            "UNKNOWN({})",
            missing
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Internal operation dispatch (no tracing).
fn execute_operation_internal<R: ValueResolver>(
    op: &ActionOperation,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    match op {
        // Comparison operations
        ActionOperation::Equals { subject, value } => {
            execute_equality(subject, value, resolver, depth, false)
        }
        ActionOperation::NotEquals { subject, value } => {
            execute_equality(subject, value, resolver, depth, true)
        }
        ActionOperation::GreaterThan { subject, value } => {
            execute_ordered_comparison(subject, value, resolver, depth, "GREATER_THAN", |ord| {
                ord == Ordering::Greater
            })
        }
        ActionOperation::LessThan { subject, value } => {
            execute_ordered_comparison(subject, value, resolver, depth, "LESS_THAN", |ord| {
                ord == Ordering::Less
            })
        }
        ActionOperation::GreaterThanOrEqual { subject, value } => execute_ordered_comparison(
            subject,
            value,
            resolver,
            depth,
            "GREATER_THAN_OR_EQUAL",
            |ord| ord != Ordering::Less,
        ),
        ActionOperation::LessThanOrEqual { subject, value } => execute_ordered_comparison(
            subject,
            value,
            resolver,
            depth,
            "LESS_THAN_OR_EQUAL",
            |ord| ord != Ordering::Greater,
        ),

        // Arithmetic
        ActionOperation::Add { values } => execute_add(values, resolver, depth),
        ActionOperation::Subtract { values } => execute_subtract(values, resolver, depth),
        ActionOperation::Multiply { values } => execute_multiply(values, resolver, depth),
        ActionOperation::Divide { values } => execute_divide(values, resolver, depth),

        // Aggregate
        ActionOperation::Max { values } => execute_aggregate(values, resolver, depth, Decimal::max),
        ActionOperation::Min { values } => execute_aggregate(values, resolver, depth, Decimal::min),

        // Rounding (RFC-024)
        ActionOperation::Round { value, precision } => {
            execute_rounding(value, *precision, RoundMode::Round, resolver, depth)
        }
        ActionOperation::Ceil { value, precision } => {
            execute_rounding(value, *precision, RoundMode::Ceil, resolver, depth)
        }
        ActionOperation::Floor { value, precision } => {
            execute_rounding(value, *precision, RoundMode::Floor, resolver, depth)
        }

        // Logical
        ActionOperation::And { conditions } => execute_and(conditions, resolver, depth),
        ActionOperation::Or { conditions } => execute_or(conditions, resolver, depth),
        ActionOperation::Not { value } => execute_not(value, resolver, depth),

        // Conditional (multi-case with cases/default)
        ActionOperation::If { cases, default } => {
            execute_if(cases, default.as_ref(), resolver, depth)
        }

        // Null checking operations
        ActionOperation::IsNull { subject } => execute_null_check(subject, resolver, depth, false),
        ActionOperation::NotNull { subject } => execute_null_check(subject, resolver, depth, true),

        // Collection operations
        ActionOperation::In {
            subject,
            value,
            values,
        } => execute_membership(
            subject,
            value.as_ref(),
            values.as_deref(),
            resolver,
            depth,
            false,
        ),
        ActionOperation::NotIn {
            subject,
            value,
            values,
        } => execute_membership(
            subject,
            value.as_ref(),
            values.as_deref(),
            resolver,
            depth,
            true,
        ),
        ActionOperation::List { items } => execute_list(items, resolver, depth),

        // FOREACH needs a child scope to bind its element in, which the
        // `ValueResolver` contract does not provide. Resolvers that can create
        // one answer `Some`; the rest fall through to an error that says so,
        // rather than a confusing "variable not found" on the loop variable.
        ActionOperation::Foreach {
            collection,
            as_name,
            body,
            filter,
            combine,
        } => resolver
            .execute_foreach_op(
                collection,
                as_name.as_deref(),
                body,
                filter.as_ref(),
                combine.as_ref(),
                depth,
            )
            .unwrap_or_else(|| {
                Err(EngineError::InvalidOperation(
                    "FOREACH requires a resolver that supports child scopes".to_string(),
                ))
            }),

        // Date
        ActionOperation::Age {
            date_of_birth,
            reference_date,
        } => execute_age(date_of_birth, reference_date, resolver, depth),
        ActionOperation::DateAdd {
            date,
            years,
            months,
            weeks,
            days,
        } => execute_date_add(
            date,
            years.as_ref(),
            months.as_ref(),
            weeks.as_ref(),
            days.as_ref(),
            resolver,
            depth,
        ),
        ActionOperation::Date { year, month, day } => {
            execute_date_construct(year, month, day, resolver, depth)
        }
        ActionOperation::DayOfWeek { date } => execute_day_of_week(date, resolver, depth),
        ActionOperation::DateDiff { from, to, unit } => {
            execute_date_diff(from, to, unit, resolver, depth)
        }
        ActionOperation::DatePart { date, part } => execute_date_part(date, part, resolver, depth),
        ActionOperation::StartOf { date, unit } => execute_start_of(date, unit, resolver, depth),
    }
}

// =============================================================================
// Comparison Operations
// =============================================================================

/// Check if two Values are equal, with Python-style numeric coercion.
///
/// This matches Python's behavior where `42 == 42.0` is `True`.
/// For non-numeric types, uses standard equality.
///
/// Numbers compare exactly: `Decimal` carries full precision and `Int`/`Decimal`
/// are compared as decimals (e.g. `42 == 42.0`), with no float rounding or
/// ±2^53 precision loss.
pub(crate) fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        // Untranslatable: two untranslatables are equal, mixed is never equal
        (Value::Untranslatable { .. }, Value::Untranslatable { .. }) => true,
        (Value::Untranslatable { .. }, _) | (_, Value::Untranslatable { .. }) => false,
        // Decimal-Decimal comparison
        (Value::Decimal(d1), Value::Decimal(d2)) => d1 == d2,
        // Int-Decimal comparison (numeric, exact)
        (Value::Int(i), Value::Decimal(d)) | (Value::Decimal(d), Value::Int(i)) => {
            Decimal::from(*i) == *d
        }
        // Default: use structural equality
        _ => a == b,
    }
}

/// Execute EQUALS / NOT_EQUALS with Python-style numeric coercion.
///
/// - `Int(42) == Float(42.0)` returns `true` (like Python)
/// - The mixed date forms compare chronologically: the string form `"2025-01-01"`
///   equals the reference-date object form `{iso: "2025-01-01", ...}` (RFC-021).
///   This keeps EQUALS consistent with the ordering operators, which also accept
///   both date forms via `parse_date`. Two strings or two objects keep structural
///   equality, so objects that merely share an `iso` field are not equal.
/// - Two lists or two records compare structurally without numeric coercion
///   inside: an integer against a decimal of the same value is an error
///   ([`compound_equal`]).
/// - Other non-numeric types use structural equality
/// - When `negate` is true, the result is inverted (NOT_EQUALS).
fn execute_equality<R: ValueResolver>(
    subject: &ActionValue,
    value: &ActionValue,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&subject_val, &value_val) {
        return Ok(tainted);
    }
    // Whether an unknown value equals something cannot be said yet, and that
    // holds for an unknown inside a list or a record as much as for a bare
    // one: the comparison below is structural and would otherwise equate two
    // Unknowns. Null, by contrast, is a value here: `EQUALS($huur, null)` is
    // the absence test the corpus uses, and `values_equal` answers it
    // structurally (RFC-036).
    let op_name = if negate { "NOT_EQUALS" } else { "EQUALS" };
    if let Some(unknown) = propagate_unknown_deep(resolver, op_name, [&subject_val, &value_val]) {
        return Ok(unknown);
    }

    let mut equal = match (&subject_val, &value_val) {
        (Value::Object(_), Value::Object(_)) | (Value::Array(_), Value::Array(_)) => {
            compound_equal(&subject_val, &value_val, op_name)?
        }
        _ => values_equal(&subject_val, &value_val),
    };

    // Date-aware fallback, scoped to the mixed form (a string against an `{iso}`
    // object): the one date pairing structural equality can never match. Two
    // canonical date strings are equal structurally exactly when they are equal
    // chronologically, and object↔object stays structural so arbitrary objects
    // sharing an `iso` field do not become equal (RFC-021). A side that fails to
    // parse as a date leaves the structural verdict untouched. Scoped to EQUALS so
    // membership and other structural comparisons keep their existing semantics.
    if !equal && is_mixed_date_pair(&subject_val, &value_val) {
        if let (Ok(subject_date), Ok(value_date)) =
            (parse_date(&subject_val), parse_date(&value_val))
        {
            equal = subject_date == value_date;
        }
    }

    Ok(Value::Bool(if negate { !equal } else { equal }))
}

/// `EQUALS` on two lists or two records: structural, with the numbers inside
/// compared as written ([`stored::equal_as_written`], the rule a stored
/// value is compared by as well). A number that is an integer on one side
/// and a decimal of the same value on the other (`12` against `12.0`) is an
/// error, not an answer: whether the two are alike is a question about where
/// the values came from, and answering it silently either way would hide
/// that. A bare number keeps its coercion ([`values_equal`]).
fn compound_equal(a: &Value, b: &Value, op_name: &str) -> Result<bool> {
    stored::equal_as_written(a, b).map_err(|unlike| EngineError::TypeMismatch {
        expected: format!("numbers written alike inside the lists or records {op_name} compares"),
        actual: format!(
            "{} against {} at {}",
            unlike.left, unlike.right, unlike.path
        ),
    })
}

/// Whether the pair is the mixed date form: one string against one
/// reference-date `{iso}` object, in either order.
///
/// Used to scope date-aware equality to the single pairing that structural
/// equality cannot already handle.
fn is_mixed_date_pair(a: &Value, b: &Value) -> bool {
    fn iso_object(v: &Value) -> bool {
        matches!(v, Value::Object(obj) if matches!(obj.get("iso"), Some(Value::String(_))))
    }
    (matches!(a, Value::String(_)) && iso_object(b))
        || (iso_object(a) && matches!(b, Value::String(_)))
}

/// Execute an ordered comparison (>, <, >=, <=) on numbers or dates.
///
/// Operands are type-safe and dispatch on their resolved type:
/// - both numeric (Int/Decimal) → exact decimal comparison
/// - both ISO dates (YYYY-MM-DD strings, or `{iso, ...}` objects) → date comparison
/// - mixed or unsupported types → `TypeMismatch` error
///
/// This lets `$peildatum > $grensdatum` work the same way `$bedrag > 1000` does,
/// without introducing date-specific operators (RFC-021, route A).
///
/// The operator is a single predicate over [`Ordering`], shared by the numeric
/// and the date path so the two can never drift apart. `Decimal` is totally
/// ordered, so every numeric pair compares.
fn execute_ordered_comparison<R: ValueResolver, F>(
    subject: &ActionValue,
    value: &ActionValue,
    resolver: &R,
    depth: usize,
    op_name: &str,
    satisfies: F,
) -> Result<Value>
where
    F: Fn(Ordering) -> bool,
{
    let subject_val = evaluate_value(subject, resolver, depth)?;
    let value_val = evaluate_value(value, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&subject_val, &value_val) {
        return Ok(tainted);
    }

    // An unknown operand (a fact nobody has yet) makes the comparison unknown;
    // AND, OR, NOT and IF carry that onward. A null operand is different: "more
    // than nothing" is not a question the law asked, so ordering an absence is
    // an error the law has to resolve by testing for absence first (RFC-036).
    if let Some(unknown) = propagate_unknown(resolver, op_name, [&subject_val, &value_val]) {
        return Ok(unknown);
    }
    if subject_val.is_null() || value_val.is_null() {
        return Err(absent_operand(op_name));
    }

    // Numbers first: the common case and the historical behavior.
    if is_numeric(&subject_val) && is_numeric(&value_val) {
        let subject_num = to_decimal(&subject_val)?;
        let value_num = to_decimal(&value_val)?;
        return Ok(Value::Bool(satisfies(subject_num.cmp(&value_num))));
    }

    // Dates: only when BOTH operands parse as ISO dates, so a mismatch
    // (e.g. number vs date) surfaces as a clear error rather than a silent coercion.
    let subject_date = parse_date(&subject_val);
    let value_date = parse_date(&value_val);
    if let (Ok(subject_date), Ok(value_date)) = (&subject_date, &value_date) {
        return Ok(Value::Bool(satisfies(subject_date.cmp(value_date))));
    }

    // Neither a numeric pair nor a date pair. Point the error at what actually
    // went wrong: a number against a date is a mixed pair (each operand is fine
    // on its own), otherwise blame the operand that is neither number nor date.
    let subject_kind = comparable_kind(&subject_val, subject_date.is_ok());
    let value_kind = comparable_kind(&value_val, value_date.is_ok());
    match (subject_kind, value_kind) {
        (Some(subject_kind), Some(value_kind)) => Err(EngineError::TypeMismatch {
            expected: "two numbers or two dates".to_string(),
            actual: format!("a {subject_kind} compared against a {value_kind}"),
        }),
        (Some(_), None) => Err(type_error("number or date", &value_val)),
        _ => Err(type_error("number or date", &subject_val)),
    }
}

/// The comparable kind of an operand, for error reporting: `number`, `date`,
/// or `None` when it is neither.
fn comparable_kind(val: &Value, parses_as_date: bool) -> Option<&'static str> {
    if is_numeric(val) {
        Some("number")
    } else if parses_as_date {
        Some("date")
    } else {
        None
    }
}

/// Whether a value is a number (Int or Decimal) for comparison dispatch.
fn is_numeric(val: &Value) -> bool {
    matches!(val, Value::Int(_) | Value::Decimal(_))
}

// =============================================================================
// Arithmetic Operations
// =============================================================================

/// Execute ADD operation: sum numbers, concatenate arrays, or concatenate strings.
///
/// The type of the first value determines the operation mode:
/// - Numbers: sum all values
/// - Arrays: concatenate all arrays
/// - Strings: concatenate all strings
fn execute_add<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let evaluated = evaluate_values(values, resolver, depth)?;
    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "ADD", &evaluated) {
        return Ok(unknown);
    }
    reject_null("ADD", &evaluated)?;
    add_values(&evaluated)
}

/// Sum already-evaluated values with RFC-007's polymorphic ADD.
///
/// Split out of [`execute_add`] so FOREACH's `combine: ADD` reduces with the
/// same rules the standalone operation uses, instead of a second implementation
/// that could drift from it. The first value picks the mode: numbers add,
/// strings concatenate, arrays flatten.
pub(crate) fn add_values(evaluated: &[Value]) -> Result<Value> {
    if evaluated.is_empty() {
        return Err(EngineError::InvalidOperation(
            "ADD requires at least one value".to_string(),
        ));
    }

    if let Some(tainted) = find_untranslatable(evaluated) {
        return Ok(tainted);
    }

    // Determine type from first value
    match &evaluated[0] {
        Value::Array(_) => {
            // Concatenate arrays
            let mut result = Vec::new();
            for val in evaluated {
                match val {
                    Value::Array(arr) => result.extend(arr.iter().cloned()),
                    _ => {
                        return Err(EngineError::TypeMismatch {
                            expected: "array".to_string(),
                            actual: format!("{:?}", val),
                        })
                    }
                }
            }
            Ok(Value::Array(result))
        }
        Value::String(_) => {
            // Concatenate strings
            let mut result = String::new();
            for val in evaluated {
                match val {
                    Value::String(s) => result.push_str(s),
                    _ => {
                        return Err(EngineError::TypeMismatch {
                            expected: "string".to_string(),
                            actual: format!("{:?}", val),
                        })
                    }
                }
            }
            Ok(Value::String(result))
        }
        Value::Int(_) | Value::Decimal(_) => {
            // Original numeric addition
            let mut sum = Decimal::ZERO;
            let mut has_decimal = false;
            for val in evaluated {
                match val {
                    Value::Int(i) => sum += Decimal::from(*i),
                    Value::Decimal(d) => {
                        sum += d;
                        has_decimal = true;
                    }
                    _ => return Err(type_error("number", val)),
                }
            }
            int_or_decimal_result(sum, has_decimal)
        }
        _ => Err(type_error("number, string, or array", &evaluated[0])),
    }
}

/// Wrap a numeric arithmetic result so all-int inputs return `Int` and any
/// `Decimal` input promotes the result to `Decimal`. Centralises the
/// `decimal_to_i64_safe` overflow guard shared by ADD/SUBTRACT/MULTIPLY.
fn int_or_decimal_result(result: Decimal, has_decimal: bool) -> Result<Value> {
    Ok(if has_decimal {
        Value::Decimal(result)
    } else {
        Value::Int(decimal_to_i64_safe(result)?)
    })
}

/// Execute SUBTRACT operation: first value minus all subsequent values.
///
/// Computed in exact decimal arithmetic; all-integer inputs return `Int`.
fn execute_subtract<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "SUBTRACT requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "SUBTRACT", &evaluated) {
        return Ok(unknown);
    }
    reject_null("SUBTRACT", &evaluated)?;

    // SAFETY: values guaranteed non-empty by check above
    let Some((first, rest)) = evaluated.split_first() else {
        unreachable!("values checked non-empty above")
    };
    let mut result = to_decimal(first)?;
    let mut has_decimal = matches!(first, Value::Decimal(_));

    for val in rest {
        result -= to_decimal(val)?;
        if matches!(val, Value::Decimal(_)) {
            has_decimal = true;
        }
    }

    int_or_decimal_result(result, has_decimal)
}

/// Execute MULTIPLY operation: product of all values.
///
/// Computed in exact decimal arithmetic; all-integer inputs return `Int`.
fn execute_multiply<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "MULTIPLY requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "MULTIPLY", &evaluated) {
        return Ok(unknown);
    }
    reject_null("MULTIPLY", &evaluated)?;

    let mut result = Decimal::ONE;
    let mut has_decimal = false;

    for val in &evaluated {
        match val {
            Value::Int(i) => result *= Decimal::from(*i),
            Value::Decimal(d) => {
                result *= d;
                has_decimal = true;
            }
            _ => return Err(type_error("number", val)),
        }
    }

    int_or_decimal_result(result, has_decimal)
}

/// Execute DIVIDE operation: first value divided by all subsequent values.
///
/// Returns `Err(DivisionByZero)` for division by zero. Division is computed in
/// exact decimal arithmetic and always returns a `Decimal` (like Python's `/`).
fn execute_divide<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "DIVIDE requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "DIVIDE", &evaluated) {
        return Ok(unknown);
    }
    reject_null("DIVIDE", &evaluated)?;

    // SAFETY: values guaranteed non-empty by check above
    let Some((first, rest)) = evaluated.split_first() else {
        unreachable!("values checked non-empty above")
    };
    let mut result = to_decimal(first)?;

    for val in rest {
        let divisor = to_decimal(val)?;
        if divisor.is_zero() {
            return Err(EngineError::DivisionByZero);
        }
        result = result
            .checked_div(divisor)
            .ok_or_else(|| EngineError::ArithmeticOverflow("Division overflowed".to_string()))?;
    }

    // Division always returns a decimal (like Python's `/`)
    Ok(Value::Decimal(result))
}

// =============================================================================
// Aggregate Operations
// =============================================================================

/// Execute aggregate operation (MAX, MIN).
fn execute_aggregate<R: ValueResolver, F>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
    combine: F,
) -> Result<Value>
where
    F: Fn(Decimal, Decimal) -> Decimal,
{
    if values.is_empty() {
        return Err(EngineError::InvalidOperation(
            "Aggregate operation requires at least one value".to_string(),
        ));
    }

    let evaluated = evaluate_values(values, resolver, depth)?;

    if let Some(tainted) = find_untranslatable(&evaluated) {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "MIN/MAX", &evaluated) {
        return Ok(unknown);
    }
    reject_null("MIN/MAX", &evaluated)?;

    let mut has_decimal = false;
    let nums: Vec<Decimal> = evaluated
        .iter()
        .map(|v| {
            if matches!(v, Value::Decimal(_)) {
                has_decimal = true;
            }
            to_decimal(v)
        })
        .collect::<Result<Vec<_>>>()?;

    // SAFETY: values guaranteed non-empty by check above
    let Some(result) = nums.into_iter().reduce(combine) else {
        unreachable!("values checked non-empty above")
    };

    int_or_decimal_result(result, has_decimal)
}

// =============================================================================
// Rounding Operations (RFC-024)
// =============================================================================

/// Rounding direction.
#[derive(Clone, Copy)]
enum RoundMode {
    /// Round to nearest, ties away from zero (rekenkundig; the Hoge Raad default).
    Round,
    /// Round toward +∞ ("naar boven").
    Ceil,
    /// Round toward -∞ ("naar beneden" / "afkapping").
    Floor,
}

/// Maximum absolute `precision` a rounding operation accepts (Decimal holds at
/// most 28 fractional digits, so larger magnitudes are out of range).
const MAX_ROUND_PRECISION: i64 = 28;

/// Execute a rounding operation (ROUND/CEIL/FLOOR): round the single
/// operand to `precision` decimal places.
///
/// `precision` is in the value's own unit (RFC-023): `0` rounds to whole units,
/// `2` to two decimals, and a negative precision rounds to tens/hundreds (e.g.
/// `-2` rounds a eurocent value to whole euros). Rounding never happens
/// implicitly — only here, where the law asks for it (RFC-024).
///
/// `ROUND` is half-up in the legal sense (rekenkundig — ties away from zero, the
/// Hoge Raad 2009 default). The money domain is non-negative, where "half away
/// from zero" and "half toward +∞" coincide.
fn execute_rounding<R: ValueResolver>(
    value: &ActionValue,
    precision: i64,
    mode: RoundMode,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let evaluated = evaluate_value(value, resolver, depth)?;
    if evaluated.is_untranslatable() {
        return Ok(evaluated);
    }
    let op_name = match mode {
        RoundMode::Round => "ROUND",
        RoundMode::Ceil => "CEIL",
        RoundMode::Floor => "FLOOR",
    };
    // Rounding an unknown amount is unknown; rounding an absence is an error
    // (RFC-036): "geen" is not an amount to round.
    if let Some(unknown) = propagate_unknown(resolver, op_name, [&evaluated]) {
        return Ok(unknown);
    }
    reject_null(op_name, [&evaluated])?;
    let operand = to_decimal(&evaluated)?;
    let rounded = round_decimal(operand, precision, mode)?;
    // Mirror the arithmetic ops: an integral result returns `Int`, otherwise `Decimal`.
    Ok(match decimal_to_i64_safe(rounded) {
        Ok(i) => Value::Int(i),
        Err(_) => Value::Decimal(rounded),
    })
}

/// Round `value` to `precision` decimal places using `mode`.
fn round_decimal(value: Decimal, precision: i64, mode: RoundMode) -> Result<Decimal> {
    let strategy = match mode {
        RoundMode::Round => RoundingStrategy::MidpointAwayFromZero,
        RoundMode::Ceil => RoundingStrategy::ToPositiveInfinity,
        RoundMode::Floor => RoundingStrategy::ToNegativeInfinity,
    };

    if !(-MAX_ROUND_PRECISION..=MAX_ROUND_PRECISION).contains(&precision) {
        return Err(EngineError::InvalidOperation(format!(
            "rounding precision {precision} out of range [-{MAX_ROUND_PRECISION}, {MAX_ROUND_PRECISION}]"
        )));
    }

    if precision >= 0 {
        Ok(value.round_dp_with_strategy(precision as u32, strategy))
    } else {
        // Negative precision: round to a power of ten (e.g. -2 → whole hundreds).
        // Use checked arithmetic so an extreme magnitude errors rather than panics.
        let overflow = || EngineError::ArithmeticOverflow("rounding overflowed".to_string());
        let scale = Decimal::TEN.checked_powi(-precision).ok_or_else(overflow)?;
        let scaled = value
            .checked_div(scale)
            .ok_or_else(overflow)?
            .round_dp_with_strategy(0, strategy);
        scaled.checked_mul(scale).ok_or_else(overflow)
    }
}

// =============================================================================
// Logical Operations
// =============================================================================

/// Execute AND operation: short-circuit evaluation, returns false if any condition is false.
///
/// Three-valued (RFC-036): a definitive `false` decides, whatever the other
/// operands are; otherwise an untranslatable operand taints the result, and
/// otherwise an unknown operand (a fact nobody has) makes it unknown, so a
/// missing value can never come out as a confident "voldoet niet". A `null`
/// operand is an error: an absence is not a verdict, and the law has to test
/// for it (`EQUALS … null`) before it may count it as one.
///
/// The conditions are evaluated in order and a `false` stops the evaluation,
/// which is what lets a law guard a calculation: `AND(NOT EQUALS $huur null,
/// GREATER_THAN $huur 500)` never orders the absent rent.
fn execute_and<R: ValueResolver>(
    conditions: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let tracing = resolver.has_trace();
    let mut results: Option<Vec<Value>> = if tracing { Some(Vec::new()) } else { None };
    let mut taint: Option<Value> = None;
    let mut unknowns: Vec<Value> = Vec::new();
    for condition in conditions {
        let val = evaluate_value(condition, resolver, depth)?;
        if val.is_null() {
            return Err(absent_operand("AND"));
        }
        if val.is_unknown() {
            unknowns.push(val);
            continue;
        }
        // Definitive false wins over taint and unknown (AND commutativity)
        if !val.to_bool() && !val.is_untranslatable() {
            return Ok(Value::Bool(false));
        }
        if val.is_untranslatable() && taint.is_none() {
            taint = Some(val);
            continue;
        }
        if let Some(ref mut r) = results {
            r.push(val);
        }
    }

    // If any operand was tainted but none was definitively false, propagate
    if let Some(t) = taint {
        return Ok(t);
    }
    if let Some(unknown) = Value::merge_unknown(&unknowns) {
        resolver.trace_set_message(format!(
            "AND with an unknown operand and no false: unknown (missing: {})",
            describe_missing(&unknown)
        ));
        return Ok(unknown);
    }

    if let Some(results) = results {
        let result_strs: Vec<String> = results.iter().map(format_value_for_trace).collect();
        resolver.trace_set_message(format!("Result [{}] AND: True", result_strs.join(", ")));
    }

    Ok(Value::Bool(true))
}

/// Execute OR operation: short-circuit evaluation, returns true if any condition is true.
///
/// Three-valued (RFC-036): a definitive `true` decides; otherwise an
/// untranslatable taints, otherwise an unknown operand makes the result
/// unknown, and a `null` operand is an error (see [`execute_and`]).
fn execute_or<R: ValueResolver>(
    conditions: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let mut taint: Option<Value> = None;
    let mut unknowns: Vec<Value> = Vec::new();
    for condition in conditions {
        let val = evaluate_value(condition, resolver, depth)?;
        if val.is_null() {
            return Err(absent_operand("OR"));
        }
        if val.is_unknown() {
            unknowns.push(val);
            continue;
        }
        // Definitive true wins over taint and unknown (OR commutativity)
        if val.to_bool() {
            return Ok(Value::Bool(true));
        }
        if val.is_untranslatable() && taint.is_none() {
            taint = Some(val);
        }
    }

    // If any operand was tainted but none was definitively true, propagate
    if let Some(t) = taint {
        return Ok(t);
    }
    if let Some(unknown) = Value::merge_unknown(&unknowns) {
        resolver.trace_set_message(format!(
            "OR with an unknown operand and no true: unknown (missing: {})",
            describe_missing(&unknown)
        ));
        return Ok(unknown);
    }

    Ok(Value::Bool(false))
}

/// Execute NOT operation: logical negation.
///
/// Takes a single `value` field (which should be a boolean-returning operation).
/// The negation of unknown is unknown; the negation of an absence is an error
/// (RFC-036).
fn execute_not<R: ValueResolver>(value: &ActionValue, resolver: &R, depth: usize) -> Result<Value> {
    let val = evaluate_value(value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    if let Some(unknown) = propagate_unknown(resolver, "NOT", [&val]) {
        return Ok(unknown);
    }
    if val.is_null() {
        return Err(absent_operand("NOT"));
    }
    Ok(Value::Bool(!val.to_bool()))
}

// =============================================================================
// Conditional Operations
// =============================================================================

/// Execute IF operation: evaluates cases in order, returns first matching case's value.
fn execute_if<R: ValueResolver>(
    cases: &[Case],
    default: Option<&ActionValue>,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let tracing = resolver.has_trace();

    for (i, case) in cases.iter().enumerate() {
        if tracing {
            resolver.trace_push(&format!("CASE_{}", i), PathNodeType::Operation);
        }
        let condition_result = evaluate_value(&case.when, resolver, depth)?;
        if tracing {
            resolver.trace_set_result(condition_result.clone());
            resolver.trace_set_message(format!(
                "CASE {}: {}",
                i,
                format_value_for_trace(&condition_result)
            ));
            resolver.trace_pop();
        }

        if condition_result.is_untranslatable() {
            return Ok(condition_result);
        }
        // A case whose condition is unknown cannot be skipped in good faith:
        // the branch might have applied. The whole IF is that unknown; neither
        // this `then`, nor a later case, nor the default is taken in its place
        // (RFC-036). An absent condition is an error: "geen" is not a verdict.
        if condition_result.is_unknown() {
            resolver.trace_set_message(format!(
                "case {i}: condition unknown, so the result is unknown (missing: {})",
                describe_missing(&condition_result)
            ));
            return Ok(condition_result);
        }
        if condition_result.is_null() {
            return Err(absent_operand("IF"));
        }

        if condition_result.to_bool() {
            if tracing {
                resolver.trace_push("THEN", PathNodeType::Operation);
            }
            let result = evaluate_value(&case.then, resolver, depth)?;
            if tracing {
                resolver.trace_set_result(result.clone());
                resolver.trace_set_message(format!("THEN: {}", format_value_for_trace(&result)));
                resolver.trace_pop();
                resolver.trace_set_message(format!("case {} matched", i));
            }
            return Ok(result);
        }
    }

    // Return default if no case matched
    if let Some(default) = default {
        if tracing {
            resolver.trace_push("DEFAULT", PathNodeType::Operation);
        }
        let result = evaluate_value(default, resolver, depth)?;
        if tracing {
            resolver.trace_set_result(result.clone());
            resolver.trace_set_message(format!("DEFAULT: {}", format_value_for_trace(&result)));
            resolver.trace_pop();
            resolver.trace_set_message("took default".to_string());
        }
        Ok(result)
    } else {
        if tracing {
            resolver.trace_set_message("no case matched, no default".to_string());
        }
        Ok(Value::Null)
    }
}

// =============================================================================
// Null Checking Operations
// =============================================================================

/// Execute IS_NULL / NOT_NULL operation.
///
/// When `negate` is true, returns true if the subject is *not* null (NOT_NULL).
///
/// Absence is what this tests for, so `null` answers (`true` / `false`). An
/// unknown subject does not: whether the register holds a value is exactly the
/// fact nobody has yet, so the check is unknown (RFC-036).
fn execute_null_check<R: ValueResolver>(
    subject: &ActionValue,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject_val = evaluate_value(subject, resolver, depth)?;
    if subject_val.is_untranslatable() {
        return Ok(subject_val);
    }
    if subject_val.is_unknown() {
        let op_name = if negate { "NOT_NULL" } else { "IS_NULL" };
        resolver.trace_set_message(format!(
            "{op_name} of unknown: unknown (missing: {})",
            describe_missing(&subject_val)
        ));
        return Ok(subject_val);
    }
    let is_null = subject_val.is_null();
    Ok(Value::Bool(if negate { !is_null } else { is_null }))
}

// =============================================================================
// Collection Operations
// =============================================================================

/// Execute IN / NOT_IN operation.
///
/// Uses Python-style numeric coercion for equality comparison.
/// When `negate` is true, returns true if subject is *not* in the list (NOT_IN).
///
/// Supports both `values: [...]` (inline list) and `value: $list_ref` (reference to a
/// definition list). When `value` resolves to a non-array, it is wrapped in a single-element vec.
fn execute_membership<R: ValueResolver>(
    subject: &ActionValue,
    value: Option<&ActionValue>,
    values: Option<&[ActionValue]>,
    resolver: &R,
    depth: usize,
    negate: bool,
) -> Result<Value> {
    let subject_val = evaluate_value(subject, resolver, depth)?;
    if subject_val.is_untranslatable() {
        return Ok(subject_val);
    }

    let check_values = if let Some(values) = values {
        evaluate_values(values, resolver, depth)?
    } else if let Some(value) = value {
        let resolved = evaluate_value(value, resolver, depth)?;
        match resolved {
            Value::Array(items) => items,
            other => vec![other],
        }
    } else {
        let op_name = if negate { "NOT_IN" } else { "IN" };
        return Err(EngineError::InvalidOperation(format!(
            "{op_name} requires 'values' or 'value'"
        )));
    };

    // A definite match settles it whatever else is in the list (Kleene, as
    // for OR). A match is definite only when neither the subject nor the
    // element has an Unknown anywhere inside: `[unknown]` might be `[650]`,
    // but nobody can say so yet. Without a definite match, the precedence of
    // RFC-012 over RFC-036 holds per element: a tainted element could have
    // been the match, so the result is that taint; otherwise an unknown
    // subject or an unknown (inside an) element leaves the question open.
    // Null is structural, like EQUALS: `IN(null, [null])` is true,
    // `IN(null, [1])` is false (RFC-036).
    let found = !subject_val.contains_unknown()
        && check_values.iter().any(|val| {
            !val.contains_unknown() && !val.is_untranslatable() && values_equal(&subject_val, val)
        });
    if !found {
        if let Some(tainted) = find_untranslatable(&check_values) {
            return Ok(tainted);
        }
        let op_name = if negate { "NOT_IN" } else { "IN" };
        if let Some(unknown) = propagate_unknown_deep(
            resolver,
            op_name,
            std::iter::once(&subject_val).chain(check_values.iter()),
        ) {
            return Ok(unknown);
        }
    }
    Ok(Value::Bool(if negate { !found } else { found }))
}

/// Execute LIST operation: construct an array from items.
fn execute_list<R: ValueResolver>(
    items: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let values: Vec<Value> = items
        .iter()
        .map(|item| evaluate_value(item, resolver, depth))
        .collect::<Result<Vec<_>>>()?;

    Ok(Value::Array(values))
}

// =============================================================================
// Collection Iteration (RFC-016)
// =============================================================================

/// Execute FOREACH: evaluate `body` once per element of `collection`.
///
/// Takes a concrete [`RuleContext`] rather than a generic `ValueResolver`,
/// because FOREACH is the only operation that binds a variable and so needs a
/// child scope to bind it in. Callers reach this through
/// [`ValueResolver::execute_foreach_op`].
///
/// `collection` is evaluated in the context it was given, before any child
/// exists. That is what lets a nested FOREACH read the outer binding: the inner
/// `collection` expression is the only place the outer scope is still visible.
///
/// See RFC-016 for the scope, combine and taint rules this implements.
pub fn execute_foreach(
    collection: &ActionValue,
    as_name: Option<&str>,
    body: &ActionValue,
    filter: Option<&ActionValue>,
    combine: Option<&CombineOp>,
    context: &RuleContext,
    depth: usize,
) -> Result<Value> {
    // The schema constrains `as` with a pattern, but a law can reach the engine
    // without passing schema validation, and a bad binding name would otherwise
    // surface much later as a puzzling resolution failure.
    let binding = foreach_binding(as_name);
    if !is_valid_binding_name(binding) {
        return Err(EngineError::InvalidOperation(format!(
            "FOREACH 'as' must be a non-empty lowercase identifier (got '{binding}')"
        )));
    }

    let collection_value = evaluate_value(collection, context, depth)?;
    if collection_value.is_untranslatable() {
        return Ok(collection_value);
    }
    // A collection nobody has yet cannot be iterated: its total is unknown for
    // the same reason. A null collection is different and iterates nothing
    // (RFC-016): the register says there are no members (RFC-036).
    if let Some(unknown) = propagate_unknown(context, "FOREACH", [&collection_value]) {
        return Ok(unknown);
    }

    // A non-array iterates once. Laws reach the engine with a data source that
    // returned a single record where the law expects a list, and treating that
    // as "one element" matches what the law means better than an error does.
    let items = match &collection_value {
        Value::Array(arr) => arr.clone(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    };

    if items.len() > crate::config::MAX_ARRAY_SIZE {
        return Err(EngineError::InvalidOperation(format!(
            "FOREACH collection size {} exceeds limit {}",
            items.len(),
            crate::config::MAX_ARRAY_SIZE,
        )));
    }

    let tracing = context.has_trace();
    let mut results: Vec<Value> = Vec::with_capacity(items.len());
    let mut skipped = 0usize;

    for (index, item) in items.iter().enumerate() {
        let mut child = context.create_child();

        // Without `as`, an object element also exposes its fields as bare
        // locals, so `$status` works where `$item.status` does. With `as`, the
        // law named the element and reads it as `$<as>.<field>` only: a field
        // bound under its own name would shadow an outer variable of that
        // name, and the body could no longer compare the row with it
        // (RFC-049).
        if as_name.is_none() {
            if let Value::Object(fields) = item {
                for (key, value) in fields {
                    child.set_local(key.clone(), value.clone());
                }
            }
        }

        // Bound last, so a field named `item` cannot displace the element in
        // the form without `as`.
        //
        // A field named `referencedate` stays unreachable by bare name by
        // design: that name resolves ahead of local scope (see
        // `RuleContext::resolve_variable_internal`), so the flattened form reads
        // the calculation date. Dot notation gets at the field.
        child.set_local(binding.to_string(), item.clone());

        if tracing {
            context.trace_push(&format!("ITEM_{index}"), PathNodeType::Operation);
        }

        let outcome = evaluate_iteration(body, filter, &child, depth);

        match outcome {
            Ok(IterationOutcome::Skipped) => {
                skipped += 1;
                if tracing {
                    context.trace_set_result(item.clone());
                    context.trace_set_message(format!("ITEM {index}: skipped by filter"));
                    context.trace_pop();
                }
            }
            Ok(IterationOutcome::Value(value)) => {
                if tracing {
                    context.trace_set_result(value.clone());
                    context.trace_set_message(format!(
                        "ITEM {index}: {}",
                        format_value_for_trace(&value)
                    ));
                    context.trace_pop();
                }
                // An untranslatable element taints the whole result. Combining
                // the rest would produce a total that looks complete and is not
                // (RFC-012). An unknown or null element is not fatal here: the
                // combine functions decide what it means per aggregation
                // (RFC-036), and without a combine it is simply an element.
                if value.is_untranslatable() {
                    return Ok(value);
                }
                results.push(value);
            }
            Ok(IterationOutcome::Tainted(value)) => {
                if tracing {
                    context.trace_set_result(value.clone());
                    context.trace_set_message(format!("ITEM {index}: untranslatable, aborting"));
                    context.trace_pop();
                }
                return Ok(value);
            }
            Err(e) => {
                if tracing {
                    context.trace_set_message(format!("ITEM {index}: error: {e}"));
                    context.trace_pop();
                }
                // Partial results are not returned: a sum over some of the
                // children is not a legal determination.
                return Err(e);
            }
        }
    }

    if tracing {
        let evaluated = results.len();
        context.trace_set_message(match skipped {
            0 => format!("FOREACH over {evaluated} element(s)"),
            n => format!("FOREACH over {evaluated} element(s), {n} skipped by filter"),
        });
    }

    match combine {
        Some(op) => combine_with(*op, results),
        None => Ok(Value::Array(results)),
    }
}

/// Combine `results` with `op`: the per-element results of a FOREACH
/// (RFC-016).
fn combine_with(op: CombineOp, results: Vec<Value>) -> Result<Value> {
    match op {
        CombineOp::Add => combine_add(&results),
        CombineOp::Or => combine_or(&results),
        CombineOp::And => combine_and(&results),
        CombineOp::Min => combine_min_max(&results, true),
        CombineOp::Max => combine_min_max(&results, false),
        CombineOp::Only => combine_only(results),
    }
}

/// What one FOREACH iteration produced.
enum IterationOutcome {
    /// The filter rejected this element.
    Skipped,
    /// The body produced a value (possibly an unknown one, see
    /// [`evaluate_iteration`]).
    Value(Value),
    /// The filter is untranslatable, so the whole collection is tainted.
    Tainted(Value),
}

/// Run `filter` and `body` for one element, in its own child scope.
fn evaluate_iteration(
    body: &ActionValue,
    filter: Option<&ActionValue>,
    child: &RuleContext,
    depth: usize,
) -> Result<IterationOutcome> {
    if let Some(filter_expr) = filter {
        let verdict = evaluate_value(filter_expr, child, depth + 1)?;
        if verdict.is_untranslatable() {
            return Ok(IterationOutcome::Tainted(verdict));
        }
        // An unknown filter means the engine cannot tell whether this element
        // belongs in the collection. Silently excluding it would report a
        // confident wrong answer, so the element counts as an unknown element
        // and the combine decides what that means: a sum is unknown, an OR
        // with a definite `true` elsewhere is still true (RFC-036).
        if verdict.is_unknown() {
            return Ok(IterationOutcome::Value(verdict));
        }
        // An absent verdict is not a verdict: the law has to test for the
        // absence before it may select on it.
        if verdict.is_null() {
            return Err(absent_operand("FOREACH filter"));
        }
        if !verdict.to_bool() {
            return Ok(IterationOutcome::Skipped);
        }
    }

    Ok(IterationOutcome::Value(evaluate_value(
        body,
        child,
        depth + 1,
    )?))
}

/// Whether a FOREACH `as` name is a usable binding: lowercase, digits and
/// underscores, not starting with a digit. Mirrors the schema's pattern.
fn is_valid_binding_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some('a'..='z' | '_') => chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_')),
        _ => false,
    }
}

/// Combine FOREACH results with ADD.
///
/// An unknown element makes the total unknown, for the union of what the
/// elements miss. RFC-016 already says this for a filter that cannot be
/// decided ("the engine cannot tell whether this element belongs in the
/// collection, so it cannot tell what the total is"), and a body that is
/// unknown is the same situation one step later: the element is definitely in
/// the collection and its contribution is not known. Summing the rest would
/// report a confident number that is short by an unknown amount.
///
/// A null element is an error (RFC-036): an absence is not an amount, and a
/// law that sums amounts some members do not have has to say what "geen"
/// counts for (an `IF … EQUALS null` in the body) before the sum is legal.
///
/// An empty collection is `0`, the additive identity, because nothing is
/// missing there.
fn combine_add(results: &[Value]) -> Result<Value> {
    if results.is_empty() {
        return Ok(Value::Int(0));
    }
    if let Some(tainted) = find_untranslatable(results) {
        return Ok(tainted);
    }
    if let Some(unknown) = Value::merge_unknown(results) {
        return Ok(unknown);
    }
    reject_null("FOREACH combine ADD", results)?;
    add_values(results)
}

/// Combine FOREACH results with OR.
///
/// Mirrors `execute_or`: a definitive `true` wins over an unknown, because the
/// answer is `true` whatever the unknown turns out to be. Otherwise any unknown
/// element makes the result unknown; a null element is an error (RFC-036).
fn combine_or(results: &[Value]) -> Result<Value> {
    if results.is_empty() {
        return Ok(Value::Bool(false));
    }
    let mut unknowns: Vec<Value> = Vec::new();
    for v in results {
        if v.is_untranslatable() {
            return Ok(v.clone());
        }
        if v.is_null() {
            return Err(absent_operand("FOREACH combine OR"));
        }
        if v.is_unknown() {
            unknowns.push(v.clone());
            continue;
        }
        if v.to_bool() {
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::merge_unknown(&unknowns).unwrap_or(Value::Bool(false)))
}

/// Combine FOREACH results with AND.
///
/// Mirrors `execute_and`: a definitive `false` wins over an unknown. An empty
/// collection is `true` by vacuous truth, so a law that must not read "no
/// items" as "all conditions met" has to check for emptiness itself. A null
/// element is an error (RFC-036).
fn combine_and(results: &[Value]) -> Result<Value> {
    if results.is_empty() {
        return Ok(Value::Bool(true));
    }
    let mut unknowns: Vec<Value> = Vec::new();
    for v in results {
        if v.is_untranslatable() {
            return Ok(v.clone());
        }
        if v.is_null() {
            return Err(absent_operand("FOREACH combine AND"));
        }
        if v.is_unknown() {
            unknowns.push(v.clone());
            continue;
        }
        if !v.to_bool() {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::merge_unknown(&unknowns).unwrap_or(Value::Bool(true)))
}

/// Combine FOREACH results with ONLY: the one result, whatever it is.
///
/// Empty is `Null`, as for MIN and MAX: there is no element, and the caller
/// has to handle that. More than one is an error: the law asked for the one
/// element the filter selects, and two are not one. The engine does not pick
/// the first, because the order of a collection is not something a law can
/// rely on.
fn combine_only(mut results: Vec<Value>) -> Result<Value> {
    match results.len() {
        0 => Ok(Value::Null),
        1 => Ok(results.remove(0)),
        n => Err(EngineError::InvalidOperation(format!(
            "FOREACH combine ONLY: the filter left {n} elements, and ONLY takes at most one"
        ))),
    }
}

/// Combine FOREACH results with MIN or MAX.
///
/// Empty is `Null`: there is no lowest value of nothing, and the caller has to
/// handle that. An unknown element makes the result unknown; a null element
/// is an error (RFC-036). Comparison stays in `Decimal` so an all-integer
/// collection returns `Int` without a detour through `f64`.
fn combine_min_max(results: &[Value], is_min: bool) -> Result<Value> {
    if results.is_empty() {
        return Ok(Value::Null);
    }
    if let Some(tainted) = find_untranslatable(results) {
        return Ok(tainted);
    }
    if let Some(unknown) = Value::merge_unknown(results) {
        return Ok(unknown);
    }
    let op_name = if is_min {
        "FOREACH combine MIN"
    } else {
        "FOREACH combine MAX"
    };
    reject_null(op_name, results)?;

    let mut best: Option<Decimal> = None;
    let mut all_int = true;

    for v in results {
        if !matches!(v, Value::Int(_)) {
            all_int = false;
        }
        let d = to_decimal(v)?;
        best = Some(match best {
            None => d,
            Some(prev) if is_min => prev.min(d),
            Some(prev) => prev.max(d),
        });
    }

    match best {
        None => Ok(Value::Null),
        Some(d) if all_int => Ok(Value::Int(decimal_to_i64_safe(d)?)),
        Some(d) => Ok(Value::Decimal(d)),
    }
}

// =============================================================================
// Date Operations
// =============================================================================

/// Execute AGE operation: calculate age in whole years from date_of_birth to reference_date.
///
/// # Arguments
/// - `date_of_birth`: Birth date (ISO 8601 YYYY-MM-DD)
/// - `reference_date`: Date to calculate age at (ISO 8601 YYYY-MM-DD)
fn execute_age<R: ValueResolver>(
    date_of_birth: &ActionValue,
    reference_date: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let dob_val = evaluate_value(date_of_birth, resolver, depth)?;
    let ref_val = evaluate_value(reference_date, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&dob_val, &ref_val) {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "AGE", [&dob_val, &ref_val]) {
        return Ok(unknown);
    }
    reject_null("AGE", [&dob_val, &ref_val])?;

    let dob_date = parse_date(&dob_val)?;
    let ref_date_parsed = parse_date(&ref_val)?;

    let age = calculate_years_difference(ref_date_parsed, dob_date);
    Ok(Value::Int(age))
}

/// Execute DATE_ADD operation: add years, months, weeks, and/or days to a date.
///
/// Applied in order: years → months → weeks → days (coarsest to finest).
///
/// For months and years, uses standard calendar arithmetic: the result lands on
/// the same day number in the target month, clamped to the last day of that month
/// when the day doesn't exist. E.g., Jan 31 + 1 month = Feb 28 (or 29 in leap year).
///
/// This is not domain knowledge in the engine — it is pure calendar math. The Hoge
/// Raad confirmed that Dutch legal termijnberekening follows standard calendar
/// arithmetic (HR 1 September 2017, ECLI:NL:HR:2017:2225).
fn execute_date_add<R: ValueResolver>(
    date: &ActionValue,
    years: Option<&ActionValue>,
    months: Option<&ActionValue>,
    weeks: Option<&ActionValue>,
    days: Option<&ActionValue>,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let date_val = evaluate_value(date, resolver, depth)?;
    let years_val = years
        .map(|v| evaluate_value(v, resolver, depth))
        .transpose()?;
    let months_val = months
        .map(|v| evaluate_value(v, resolver, depth))
        .transpose()?;
    let weeks_val = weeks
        .map(|v| evaluate_value(v, resolver, depth))
        .transpose()?;
    let days_val = days
        .map(|v| evaluate_value(v, resolver, depth))
        .transpose()?;
    // The date and every component given take part in the RFC-012/RFC-036
    // checks alike: a shift by an unknown number of months is an unknown date.
    let operands: Vec<&Value> = std::iter::once(&date_val)
        .chain(years_val.iter())
        .chain(months_val.iter())
        .chain(weeks_val.iter())
        .chain(days_val.iter())
        .collect();
    if let Some(tainted) = operands.iter().find(|v| v.is_untranslatable()) {
        return Ok((*tainted).clone());
    }
    if let Some(unknown) = propagate_unknown(resolver, "DATE_ADD", operands.iter().copied()) {
        return Ok(unknown);
    }
    reject_null("DATE_ADD", operands.iter().copied())?;
    let mut result_date = parse_date(&date_val)?;

    // Years: add to year component, clamp day to last day of target month
    if let Some(years_val) = years_val {
        let years_i64 = years_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'years' must be a number".to_string())
        })?;
        let years_int = i32::try_from(years_i64).map_err(|_| {
            EngineError::InvalidOperation(format!(
                "DATE_ADD 'years' value {} exceeds supported range",
                years_i64
            ))
        })?;
        let target_year = result_date.year() + years_int;
        let clamped_day = result_date
            .day()
            .min(days_in_month(target_year, result_date.month()));
        result_date = NaiveDate::from_ymd_opt(target_year, result_date.month(), clamped_day)
            .ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "DATE_ADD: invalid date after adding {} years",
                    years_int
                ))
            })?;
    }

    // Months: add to month component, clamp day to last day of target month
    if let Some(months_val) = months_val {
        let months_int = months_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'months' must be a number".to_string())
        })?;
        result_date = add_months(result_date, months_int)?;
    }

    // Weeks
    if let Some(weeks_val) = weeks_val {
        let weeks_int = weeks_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'weeks' must be a number".to_string())
        })?;
        result_date = result_date
            .checked_add_signed(chrono::Duration::weeks(weeks_int))
            .ok_or_else(|| {
                EngineError::InvalidOperation(
                    "DATE_ADD: date out of range after adding weeks".to_string(),
                )
            })?;
    }

    // Days
    if let Some(days_val) = days_val {
        let days_int = days_val.as_int().ok_or_else(|| {
            EngineError::InvalidOperation("DATE_ADD 'days' must be a number".to_string())
        })?;
        result_date = result_date
            .checked_add_signed(chrono::Duration::days(days_int))
            .ok_or_else(|| {
                EngineError::InvalidOperation(
                    "DATE_ADD: date out of range after adding days".to_string(),
                )
            })?;
    }

    Ok(Value::String(result_date.format("%Y-%m-%d").to_string()))
}

/// Add months to a date using standard calendar arithmetic.
///
/// Clamps the day to the last day of the target month when the original day
/// doesn't exist in that month (e.g., Jan 31 + 1 month = Feb 28).
fn add_months(date: NaiveDate, months: i64) -> Result<NaiveDate> {
    let total_months = date.year() as i64 * 12 + (date.month() as i64 - 1) + months;
    let target_year = i32::try_from(total_months.div_euclid(12)).map_err(|_| {
        EngineError::InvalidOperation(format!(
            "DATE_ADD: year out of range after adding {} months",
            months
        ))
    })?;
    let target_month = (total_months.rem_euclid(12) + 1) as u32;
    let clamped_day = date.day().min(days_in_month(target_year, target_month));

    NaiveDate::from_ymd_opt(target_year, target_month, clamped_day).ok_or_else(|| {
        EngineError::InvalidOperation(format!(
            "DATE_ADD: invalid date after adding {} months",
            months
        ))
    })
}

/// Execute DATE operation: construct a date from year, month, day components.
///
/// # Arguments
/// - `year`: Year component (integer)
/// - `month`: Month component (integer, 1-12)
/// - `day`: Day component (integer, 1-31)
fn execute_date_construct<R: ValueResolver>(
    year: &ActionValue,
    month: &ActionValue,
    day: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let year_val = evaluate_value(year, resolver, depth)?;
    let month_val = evaluate_value(month, resolver, depth)?;
    let day_val = evaluate_value(day, resolver, depth)?;

    if let Some(tainted) =
        find_untranslatable(&[year_val.clone(), month_val.clone(), day_val.clone()])
    {
        return Ok(tainted);
    }
    if let Some(unknown) = propagate_unknown(resolver, "DATE", [&year_val, &month_val, &day_val]) {
        return Ok(unknown);
    }
    reject_null("DATE", [&year_val, &month_val, &day_val])?;

    let y_i64 = year_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'year' must be a number".to_string()))?;
    let y = i32::try_from(y_i64).map_err(|_| {
        EngineError::InvalidOperation(format!(
            "DATE 'year' value {} exceeds supported range",
            y_i64
        ))
    })?;
    let m = month_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'month' must be a number".to_string()))?
        as u32;
    let d = day_val
        .as_int()
        .ok_or_else(|| EngineError::InvalidOperation("DATE 'day' must be a number".to_string()))?
        as u32;

    let date = NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| {
        EngineError::InvalidOperation(format!("DATE: invalid date {}-{}-{}", y, m, d))
    })?;

    Ok(Value::String(date.format("%Y-%m-%d").to_string()))
}

/// Execute DAY_OF_WEEK operation: get the weekday number for a date.
///
/// Returns an integer where 0=Monday, 6=Sunday.
///
/// # Arguments
/// - `date`: Date to get weekday for (ISO 8601 YYYY-MM-DD)
fn execute_day_of_week<R: ValueResolver>(
    date_value: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let val = evaluate_value(date_value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    if let Some(unknown) = propagate_unknown(resolver, "DAY_OF_WEEK", [&val]) {
        return Ok(unknown);
    }
    reject_null("DAY_OF_WEEK", [&val])?;
    let parsed = parse_date(&val)?;
    Ok(Value::Int(parsed.weekday().num_days_from_monday() as i64))
}

/// Execute DATE_DIFF: the signed difference between two dates, expressed in a unit.
///
/// ```yaml
/// operation: DATE_DIFF
/// from: $informatie_datum
/// to: $referencedate.iso
/// in: days        # one of: days | months | years
/// ```
///
/// Returns an `Int`. The result is **positive when `to >= from`** and negative
/// otherwise, so `DATE_DIFF(from, to)` reads as "how far `to` is ahead of `from`".
///
/// - `days` — exact calendar days between the dates.
/// - `months` / `years` — *complete* calendar units, reusing the same arithmetic
///   as `AGE` (BW art. 1:2): e.g. Jan 31 → Feb 28 counts as one whole month.
///
/// The unit is an explicit argument (not inferred) because the difference between
/// two dates is genuinely ambiguous: 400 days is also "1 year" and "13 months".
fn execute_date_diff<R: ValueResolver>(
    from: &ActionValue,
    to: &ActionValue,
    unit: &ActionValue,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let from_val = evaluate_value(from, resolver, depth)?;
    let to_val = evaluate_value(to, resolver, depth)?;

    if let Some(tainted) = propagate_binary(&from_val, &to_val) {
        return Ok(tainted);
    }
    let unit_val = evaluate_value(unit, resolver, depth)?;
    // The unit participates in taint propagation like the date operands: an
    // Untranslatable unit flows through as a value (RFC-012), not as an error.
    if unit_val.is_untranslatable() {
        return Ok(unit_val);
    }
    // Likewise for unknown and absent operands (RFC-036).
    if let Some(unknown) = propagate_unknown(resolver, "DATE_DIFF", [&from_val, &to_val, &unit_val])
    {
        return Ok(unknown);
    }
    reject_null("DATE_DIFF", [&from_val, &to_val, &unit_val])?;

    let from_date = parse_date(&from_val)?;
    let to_date = parse_date(&to_val)?;
    let unit_str = match &unit_val {
        Value::String(s) => s.as_str(),
        _ => {
            return Err(type_error(
                "string ('days', 'months', or 'years')",
                &unit_val,
            ))
        }
    };

    // calculate_*_difference are positive when their first arg >= second arg.
    // Passing (to, from) makes the result positive when `to >= from`.
    let diff = match unit_str {
        "days" => (to_date - from_date).num_days(),
        "months" => calculate_months_difference(to_date, from_date),
        "years" => calculate_years_difference(to_date, from_date),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "DATE_DIFF 'in' must be one of 'days', 'months', 'years', got '{}'",
                other
            )));
        }
    };

    Ok(Value::Int(diff))
}

/// Execute DATE_PART: read one calendar component out of a date.
///
/// ```yaml
/// operation: DATE_PART
/// date: $referencedate.iso
/// in: year        # one of: year | month | day
/// ```
///
/// Returns an `Int`: the year number, the month number (1-12) or the day of the
/// month (1-31). It is the inverse of `DATE`, and `in` ranges over exactly the
/// components `DATE` takes — which is why `month` exists even though no legal
/// text has asked for it yet, and why the weekday does not (that is
/// `DAY_OF_WEEK`, a projection outside the year-month-day decomposition).
///
/// The unit is singular where `DATE_DIFF` is plural, because this selects a
/// component instead of counting units. An author who copies `in: months` from
/// `DATE_DIFF` gets a named error rather than a silent answer.
fn execute_date_part<R: ValueResolver>(
    date_value: &ActionValue,
    part: &str,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let val = evaluate_value(date_value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    let parsed = parse_date(&val)?;

    let component = match part {
        "year" => i64::from(parsed.year()),
        "month" => i64::from(parsed.month()),
        "day" => i64::from(parsed.day()),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "DATE_PART 'in' must be one of 'year', 'month', 'day', got '{}'",
                other
            )));
        }
    };

    Ok(Value::Int(component))
}

/// Execute START_OF: truncate a date down to the start of the calendar unit it
/// falls in.
///
/// ```yaml
/// operation: START_OF
/// date: $peildatum
/// in: month       # one of: year | month
/// ```
///
/// Returns a date string in canonical `%Y-%m-%d` form: 1 January of the year, or
/// the first day of the month. `day` is the identity and `week` would need a
/// convention (ISO Monday) that no legal text imposes, so neither is offered.
///
/// **Order matters for "the first day of the month following".** Truncate first,
/// add afterwards:
///
/// ```yaml
/// operation: DATE_ADD
/// date:
///   operation: START_OF
///   date: $peildatum
///   in: month
/// months: 1
/// ```
///
/// That is exact for every input date, including the first of the month and
/// including 31 January, because the day-clamping of `DATE_ADD` never comes into
/// play: the truncated date is always day 1, which exists in every month. The
/// reverse order happens to give the same answer, but reaches it through the
/// clamp — a step a reviewer has to re-check each time.
///
/// Truncation is an explicit operation and never an implicit property of a date
/// (RFC-024): the cut stands in the law-YAML where the legal text prescribes it,
/// and nowhere else.
fn execute_start_of<R: ValueResolver>(
    date_value: &ActionValue,
    unit: &str,
    resolver: &R,
    depth: usize,
) -> Result<Value> {
    let val = evaluate_value(date_value, resolver, depth)?;
    if val.is_untranslatable() {
        return Ok(val);
    }
    let parsed = parse_date(&val)?;

    let truncated = match unit {
        "year" => NaiveDate::from_ymd_opt(parsed.year(), 1, 1),
        "month" => NaiveDate::from_ymd_opt(parsed.year(), parsed.month(), 1),
        other => {
            return Err(EngineError::InvalidOperation(format!(
                "START_OF 'in' must be one of 'year', 'month', got '{}'",
                other
            )));
        }
    }
    .ok_or_else(|| {
        EngineError::InvalidOperation(format!(
            "START_OF: invalid date after truncating '{}' to {}",
            parsed, unit
        ))
    })?;

    Ok(Value::String(truncated.format("%Y-%m-%d").to_string()))
}

/// Parse a date from a Value.
///
/// Expects the value to be a string in ISO 8601 format (YYYY-MM-DD).
fn parse_date(value: &Value) -> Result<NaiveDate> {
    match value {
        Value::String(s) => parse_iso_date(s),
        // Handle referencedate objects with {iso, year, month, day}
        Value::Object(obj) => {
            if let Some(Value::String(iso)) = obj.get("iso") {
                parse_iso_date(iso)
            } else {
                Err(EngineError::TypeMismatch {
                    expected: "date string (YYYY-MM-DD) or object with 'iso' field".to_string(),
                    actual: format!("{:?}", value),
                })
            }
        }
        _ => Err(EngineError::TypeMismatch {
            expected: "date string (YYYY-MM-DD)".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

/// Parse a date string in canonical ISO 8601 `YYYY-MM-DD` form.
///
/// chrono's `%Y-%m-%d` parser is lenient and accepts non-zero-padded input like
/// `2025-1-1`. We reject those: a non-canonical literal would compare chronologically
/// under `>`/`<` but inequal under `EQUALS` (which uses string equality), so the two
/// must never disagree. Requiring canonical form keeps every operator consistent and
/// matches the format the engine itself produces for `$referencedate.iso`.
fn parse_iso_date(s: &str) -> Result<NaiveDate> {
    let parsed = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
        EngineError::InvalidOperation(format!(
            "Failed to parse date '{}': {}. Expected format: YYYY-MM-DD",
            s, e
        ))
    })?;
    // Reject lenient (non-zero-padded) input by round-tripping through the canonical form.
    if parsed.format("%Y-%m-%d").to_string() != s {
        return Err(EngineError::InvalidOperation(format!(
            "Date '{}' is not in canonical YYYY-MM-DD form (use zero-padded components, e.g. '{}')",
            s,
            parsed.format("%Y-%m-%d")
        )));
    }
    Ok(parsed)
}

/// Calculate the difference in complete months between two dates.
///
/// Uses proper calendar arithmetic. A month is counted as complete when
/// the same day-of-month (or end of month if day doesn't exist) is reached.
/// For end-of-month edge cases (e.g., Jan 31 -> Feb 28), if `earlier.day()`
/// exceeds the number of days in `later`'s month, it is capped to the last
/// day of that month so the month is correctly counted as complete.
///
/// Used by DATE_DIFF with `in: months`.
fn calculate_months_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    // Het teken als tak in plaats van als vermenigvuldiging: `total * sign` met
    // sign in {1, -1} levert een mutant op die per constructie hetzelfde
    // antwoord geeft (delen door -1 is vermenigvuldigen met -1), en die mutant
    // deelt zijn omschrijving met `years_diff * 12` een paar regels lager. Eén
    // uitsluiting zou daarmee ook echte rekenkunde stilzwijgend afdekken.
    let (earlier, later, negate) = if date1 >= date2 {
        (date2, date1, false)
    } else {
        (date1, date2, true)
    };

    let years_diff = later.year() - earlier.year();
    let months_diff = later.month() as i32 - earlier.month() as i32;
    let mut total_months = years_diff * 12 + months_diff;

    // Cap earlier.day() to the max days in later's month so that
    // Jan 31 → Feb 28 counts as 1 month (28 is the last day of Feb).
    let max_day_in_later_month = days_in_month(later.year(), later.month());
    let earlier_day_capped = earlier.day().min(max_day_in_later_month);

    // Adjust if we haven't reached the (capped) day in the month
    if later.day() < earlier_day_capped {
        total_months -= 1;
    }

    let total = total_months as i64;
    if negate {
        -total
    } else {
        total
    }
}

/// Return the number of days in a given month.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if NaiveDate::from_ymd_opt(year, 2, 29).is_some() {
                29
            } else {
                28
            }
        }
        _ => unreachable!("Invalid month: {month}"),
    }
}

/// Calculate the difference in complete years between two dates.
///
/// Uses proper calendar arithmetic. A year is counted as complete when
/// the anniversary date (or Feb 28 for leap year births on Feb 29) is reached.
fn calculate_years_difference(date1: NaiveDate, date2: NaiveDate) -> i64 {
    // Zie calculate_months_difference: het teken staat als tak, niet als
    // vermenigvuldiging, zodat er geen gedragsequivalente mutant ontstaat.
    let (earlier, later, negate) = if date1 >= date2 {
        (date2, date1, false)
    } else {
        (date1, date2, true)
    };

    let mut years = later.year() - earlier.year();

    // Check if we've reached the anniversary this year.
    // For Feb 29 birthdays in non-leap years, the anniversary falls on Feb 28
    // (per Dutch law: BW art. 1:2, Algemene Termijnenwet).
    let anniversary_month = earlier.month();
    let anniversary_day = {
        let day = earlier.day();
        if anniversary_month == 2 && day == 29 && days_in_month(later.year(), 2) < 29 {
            28
        } else {
            day
        }
    };

    if later.month() < anniversary_month
        || (later.month() == anniversary_month && later.day() < anniversary_day)
    {
        years -= 1;
    }

    let years = years as i64;
    if negate {
        -years
    } else {
        years
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Evaluate a slice of ActionValues to concrete Values.
fn evaluate_values<R: ValueResolver>(
    values: &[ActionValue],
    resolver: &R,
    depth: usize,
) -> Result<Vec<Value>> {
    values
        .iter()
        .map(|v| evaluate_value(v, resolver, depth))
        .collect()
}

/// Convert a Value to an exact [`Decimal`].
///
/// Integers widen to `Decimal` losslessly (no ±2^53 limit), so financial/legal
/// calculations carry full precision.
fn to_decimal(val: &Value) -> Result<Decimal> {
    match val {
        Value::Int(i) => Ok(Decimal::from(*i)),
        Value::Decimal(d) => Ok(*d),
        // Untranslatable should be caught by the caller before reaching to_decimal,
        // but handle it gracefully.
        _ => Err(type_error("number", val)),
    }
}

/// Convert an exact [`Decimal`] result to `i64`, erroring if it is non-integral
/// or out of `i64` range. Used to keep all-integer arithmetic results as `Int`.
pub(crate) fn decimal_to_i64_safe(d: Decimal) -> Result<i64> {
    let truncated = d.trunc();
    if truncated != d {
        // Not an overflow — the value simply has a fractional part.
        return Err(EngineError::InvalidOperation(format!(
            "Value {} is not an integer",
            d
        )));
    }
    truncated
        .to_i64()
        .ok_or_else(|| EngineError::ArithmeticOverflow(format!("Value {} exceeds i64 range", d)))
}

/// Create a TypeMismatch error.
fn type_error(expected: &str, actual: &Value) -> EngineError {
    EngineError::TypeMismatch {
        expected: expected.to_string(),
        actual: actual.type_name().to_string(),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::{BTreeMap, HashMap};

    /// Simple resolver for testing that uses a HashMap
    struct TestResolver {
        vars: HashMap<String, Value>,
    }

    impl TestResolver {
        fn new() -> Self {
            Self {
                vars: HashMap::new(),
            }
        }

        fn with_var(mut self, name: &str, value: impl Into<Value>) -> Self {
            self.vars.insert(name.to_string(), value.into());
            self
        }
    }

    impl ValueResolver for TestResolver {
        fn resolve(&self, name: &str) -> Result<Value> {
            self.vars
                .get(name)
                .cloned()
                .ok_or_else(|| EngineError::VariableNotFound(name.to_string()))
        }
    }

    /// Helper to create a literal ActionValue
    fn lit(v: impl Into<Value>) -> ActionValue {
        ActionValue::Literal(v.into())
    }

    /// Helper to create a variable reference
    fn var(name: &str) -> ActionValue {
        ActionValue::Literal(Value::String(format!("${}", name)))
    }

    /// An Unknown for one missing register fact of the test law (RFC-036).
    fn unknown_fact(name: &str) -> Value {
        Value::unknown("testwet", name, crate::types::MissingKind::NoData)
    }

    /// The names of the facts an Unknown misses, for assertions.
    fn missing_names(value: &Value) -> Vec<&str> {
        value
            .missing_facts()
            .iter()
            .map(|m| m.name.as_str())
            .collect()
    }

    /// Helper to build a reference-date `{iso, year, month, day}` object Value,
    /// matching how the engine injects `$referencedate`.
    fn date_obj(iso: &str) -> Value {
        let date = NaiveDate::parse_from_str(iso, "%Y-%m-%d").expect("valid test date");
        let mut obj = BTreeMap::new();
        obj.insert("iso".to_string(), Value::String(iso.to_string()));
        obj.insert("year".to_string(), Value::Int(i64::from(date.year())));
        obj.insert("month".to_string(), Value::Int(i64::from(date.month())));
        obj.insert("day".to_string(), Value::Int(i64::from(date.day())));
        Value::Object(obj)
    }

    // -------------------------------------------------------------------------
    // Comparison Operations Tests
    // -------------------------------------------------------------------------

    mod comparison {
        use super::*;

        #[test]
        fn test_equals_integers() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_equals_different_values() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(43i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_greater_than() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(50i64),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_less_than() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThan {
                subject: lit(30i64),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_greater_than_or_equal() {
            let resolver = TestResolver::new();

            let op = ActionOperation::GreaterThanOrEqual {
                subject: lit(42i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let op2 = ActionOperation::GreaterThanOrEqual {
                subject: lit(50i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_or_equal() {
            let resolver = TestResolver::new();

            let op = ActionOperation::LessThanOrEqual {
                subject: lit(42i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let op2 = ActionOperation::LessThanOrEqual {
                subject: lit(30i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_comparison_with_variables() {
            let resolver = TestResolver::new()
                .with_var("age", 25i64)
                .with_var("min_age", 18i64);

            let op = ActionOperation::GreaterThanOrEqual {
                subject: var("age"),
                value: var("min_age"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_comparison_mixed_int_float() {
            let resolver = TestResolver::new();

            let op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(42.0f64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );

            let op2 = ActionOperation::Equals {
                subject: lit(42.0f64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }
    }

    // -------------------------------------------------------------------------
    // Arithmetic Operations Tests
    // -------------------------------------------------------------------------

    mod arithmetic {
        use super::*;

        #[test]
        fn test_add_integers() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(10i64), lit(20i64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(60));
        }

        #[test]
        fn test_add_with_floats() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(10i64), lit(20.5f64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Decimal(dec!(60.5)));
        }

        #[test]
        fn test_subtract() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Subtract {
                values: vec![lit(100i64), lit(30i64), lit(20i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_multiply() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Multiply {
                values: vec![lit(2i64), lit(3i64), lit(4i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(24));
        }

        #[test]
        fn test_divide() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Divide {
                values: vec![lit(100i64), lit(2i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Decimal(dec!(50)));
        }

        #[test]
        fn test_divide_by_zero() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Divide {
                values: vec![lit(100i64), lit(0i64)],
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::DivisionByZero)));
        }

        #[test]
        fn test_arithmetic_with_variables() {
            let resolver = TestResolver::new()
                .with_var("base", 1000i64)
                .with_var("rate", 0.05f64);

            let op = ActionOperation::Multiply {
                values: vec![var("base"), var("rate")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Decimal(dec!(50)));
        }

        #[test]
        fn test_add_arrays() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![
                    lit(Value::Array(vec![Value::Int(1), Value::Int(2)])),
                    lit(Value::Array(vec![Value::Int(3), Value::Int(4)])),
                ],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(3),
                    Value::Int(4)
                ])
            );
        }

        #[test]
        fn test_add_strings() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit("hello"), lit(" "), lit("world")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("hello world".to_string()));
        }

        #[test]
        fn test_add_propagates_an_untranslatable_operand() {
            // RFC-012: a sum in which one term could not be translated has no
            // number as its answer. The taint travels on as a value; it is not a
            // type error and not a silent zero.
            let tainted = Value::Untranslatable {
                article: "12".to_string(),
                construct: "naar redelijkheid".to_string(),
            };
            let resolver = TestResolver::new().with_var("onvertaalbaar", tainted.clone());
            let op = ActionOperation::Add {
                values: vec![lit(100i64), var("onvertaalbaar"), lit(1i64)],
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // Aggregate Operations Tests
    // -------------------------------------------------------------------------

    mod aggregate {
        use super::*;

        #[test]
        fn test_max() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Max {
                values: vec![lit(10i64), lit(50i64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_min() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Min {
                values: vec![lit(10i64), lit(50i64), lit(30i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(10));
        }

        #[test]
        fn test_max_with_floats() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Max {
                values: vec![lit(10.5f64), lit(50.3f64), lit(30.7f64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Decimal(dec!(50.3)));
        }

        #[test]
        fn test_max_with_zero() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Max {
                values: vec![lit(0i64), lit(-10i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_max_propagates_an_untranslatable_operand() {
            // RFC-012: the highest of a set that contains an untranslatable is
            // itself untranslatable — picking the largest of the rest would
            // silently drop the value that could not be determined.
            let tainted = Value::Untranslatable {
                article: "12".to_string(),
                construct: "naar redelijkheid".to_string(),
            };
            let resolver = TestResolver::new().with_var("onvertaalbaar", tainted.clone());
            let op = ActionOperation::Max {
                values: vec![lit(10i64), var("onvertaalbaar")],
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // Rounding Operations Tests (RFC-024)
    // -------------------------------------------------------------------------

    mod rounding {
        use super::*;

        fn round_op(value: ActionValue, precision: i64) -> ActionOperation {
            ActionOperation::Round { value, precision }
        }

        #[test]
        fn round_to_whole_units_half_up() {
            let resolver = TestResolver::new();
            // 2.5 -> 3 (rekenkundig, ties away from zero)
            let op = round_op(lit(2.5f64), 0);
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
            // 2.4 -> 2
            let op = round_op(lit(2.4f64), 0);
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn round_to_two_decimals() {
            let resolver = TestResolver::new();
            // 1.005 -> 1.01 (exact in Decimal; the classic f64 failure case where
            // 1.005 is stored as 1.00499... and would wrongly round down to 1.00).
            let op = round_op(lit(Value::Decimal(dec!(1.005))), 2);
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Decimal(dec!(1.01))
            );
        }

        #[test]
        fn round_to_five_decimals_subcent_tariff() {
            let resolver = TestResolver::new();
            let op = round_op(lit(0.846789f64), 5);
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Decimal(dec!(0.84679))
            );
        }

        #[test]
        fn round_negative_precision_eurocent_to_whole_euros() {
            let resolver = TestResolver::new();
            // 209691.78888 eurocent rounded to whole euros (precision -2) -> 209700
            let op = round_op(lit(209691.78888f64), -2);
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(209700)
            );
        }

        #[test]
        fn ceil_rounds_up() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Ceil {
                value: lit(1.01f64),
                precision: 0,
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn floor_rounds_toward_negative_infinity() {
            let resolver = TestResolver::new();
            let floor = ActionOperation::Floor {
                value: lit(-1.5f64),
                precision: 0,
            };
            // FLOOR rounds toward -inf, so -1.5 → -2.
            assert_eq!(
                execute_operation(&floor, &resolver, 0).unwrap(),
                Value::Int(-2)
            );
        }

        #[test]
        fn rounding_propagates_untranslatable() {
            let resolver = TestResolver::new();
            let tainted = Value::Untranslatable {
                article: "1".into(),
                construct: "afronden".into(),
            };
            let op = round_op(lit(tainted), 0);
            assert!(execute_operation(&op, &resolver, 0)
                .unwrap()
                .is_untranslatable());
        }

        #[test]
        fn rounding_rejects_non_number() {
            let resolver = TestResolver::new();
            let op = round_op(lit("not a number"), 0);
            assert!(matches!(
                execute_operation(&op, &resolver, 0),
                Err(EngineError::TypeMismatch { .. })
            ));
        }

        #[test]
        fn rounding_rejects_out_of_range_precision() {
            let resolver = TestResolver::new();
            let op = round_op(lit(1.5f64), 100);
            assert!(matches!(
                execute_operation(&op, &resolver, 0),
                Err(EngineError::InvalidOperation(_))
            ));
        }

        #[test]
        fn rounding_extreme_magnitude_errors_not_panics() {
            // A near-max Decimal with large negative precision must error gracefully
            // (checked arithmetic), never panic mid-evaluation.
            let resolver = TestResolver::new();
            let huge = Value::Decimal(dec!(79000000000000000000000000000));
            let op = round_op(lit(huge), -28);
            assert!(matches!(
                execute_operation(&op, &resolver, 0),
                Err(EngineError::ArithmeticOverflow(_))
            ));
        }
    }

    // -------------------------------------------------------------------------
    // Logical Operations Tests
    // -------------------------------------------------------------------------

    mod logical {
        use super::*;

        #[test]
        fn test_and_all_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation::And {
                conditions: vec![lit(true), lit(true), lit(true)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_and_one_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation::And {
                conditions: vec![lit(true), lit(false), lit(true)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_or_one_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Or {
                conditions: vec![lit(false), lit(true), lit(false)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_or_all_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Or {
                conditions: vec![lit(false), lit(false), lit(false)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_and_with_nested_comparison() {
            let resolver = TestResolver::new()
                .with_var("age", 25i64)
                .with_var("has_insurance", true);

            let age_check_op = ActionOperation::GreaterThanOrEqual {
                subject: var("age"),
                value: lit(18i64),
            };
            let age_check = ActionValue::Operation(Box::new(age_check_op));

            let op = ActionOperation::And {
                conditions: vec![age_check, var("has_insurance")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_negates_true() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Not { value: lit(true) };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_not_negates_false() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Not { value: lit(false) };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_wrapping_equals() {
            let resolver = TestResolver::new();

            // NOT(EQUALS(42, 42)) should be false
            let eq_op = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(42i64),
            };

            let op = ActionOperation::Not {
                value: ActionValue::Operation(Box::new(eq_op)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));

            // NOT(EQUALS(42, 43)) should be true
            let eq_op2 = ActionOperation::Equals {
                subject: lit(42i64),
                value: lit(43i64),
            };

            let op2 = ActionOperation::Not {
                value: ActionValue::Operation(Box::new(eq_op2)),
            };

            let result2 = execute_operation(&op2, &resolver, 0).unwrap();
            assert_eq!(result2, Value::Bool(true));
        }

        /// Build an Untranslatable value for the taint tests.
        fn untranslatable(article: &str) -> Value {
            Value::Untranslatable {
                article: article.to_string(),
                construct: "open norm".to_string(),
            }
        }

        /// The article an untranslatable result points at. Asserted explicitly
        /// because `Value`'s equality treats *any* two untranslatables as equal,
        /// so `assert_eq!` alone would not see which one came back.
        fn untranslatable_article(value: &Value) -> &str {
            match value {
                Value::Untranslatable { article, .. } => article,
                other => panic!("expected an untranslatable value, got {:?}", other),
            }
        }

        #[test]
        fn test_and_reports_the_first_untranslatable_condition() {
            // RFC-012: when nothing is definitively false, AND hands back the
            // taint. Which one matters for the explanation shown to the citizen:
            // the first untranslatable condition is the one that blocked the
            // evaluation, so its article is the one that gets reported.
            let resolver = TestResolver::new()
                .with_var("eerste", untranslatable("5"))
                .with_var("tweede", untranslatable("9"));
            let op = ActionOperation::And {
                conditions: vec![lit(true), var("eerste"), var("tweede")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(untranslatable_article(&result), "5");
        }

        #[test]
        fn test_and_definitive_false_beats_a_taint() {
            // A condition that is certainly false decides the whole AND, whatever
            // order it appears in — an untranslatable elsewhere cannot revive it.
            let resolver = TestResolver::new().with_var("onvertaalbaar", untranslatable("5"));
            let op = ActionOperation::And {
                conditions: vec![var("onvertaalbaar"), lit(false)],
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_or_propagates_a_taint_over_a_plain_false() {
            // A false condition does not settle an OR: as long as another
            // condition could not be evaluated, the outcome is untranslatable
            // rather than a hard "no".
            let resolver = TestResolver::new().with_var("onvertaalbaar", untranslatable("7"));
            let op = ActionOperation::Or {
                conditions: vec![lit(false), var("onvertaalbaar")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(untranslatable_article(&result), "7");
        }

        #[test]
        fn test_or_reports_the_first_untranslatable_condition() {
            // Same provenance rule as AND: the explanation points at the first
            // condition that could not be evaluated, not at the last one seen.
            let resolver = TestResolver::new()
                .with_var("eerste", untranslatable("5"))
                .with_var("tweede", untranslatable("9"));
            let op = ActionOperation::Or {
                conditions: vec![lit(false), var("eerste"), var("tweede")],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(untranslatable_article(&result), "5");
        }

        #[test]
        fn test_or_definitive_true_beats_a_taint() {
            let resolver = TestResolver::new().with_var("onvertaalbaar", untranslatable("7"));
            let op = ActionOperation::Or {
                conditions: vec![var("onvertaalbaar"), lit(true)],
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }
    }

    // -------------------------------------------------------------------------
    // Conditional Operations Tests (IF with cases/default)
    // -------------------------------------------------------------------------

    mod conditional {
        use super::*;

        #[test]
        fn test_if_first_match() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(true),
                        then: lit(100i64),
                    },
                    Case {
                        when: lit(true),
                        then: lit(200i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(100));
        }

        #[test]
        fn test_if_second_match() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(false),
                        then: lit(100i64),
                    },
                    Case {
                        when: lit(true),
                        then: lit(200i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(200));
        }

        #[test]
        fn test_if_default() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(false),
                        then: lit(100i64),
                    },
                    Case {
                        when: lit(false),
                        then: lit(200i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_if_no_default_returns_null() {
            let resolver = TestResolver::new();
            let op = ActionOperation::If {
                cases: vec![Case {
                    when: lit(false),
                    then: lit(100i64),
                }],
                default: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn test_if_with_nested_conditions() {
            let resolver = TestResolver::new().with_var("status", "active");

            let pending_check = ActionOperation::Equals {
                subject: var("status"),
                value: lit("pending"),
            };

            let active_check = ActionOperation::Equals {
                subject: var("status"),
                value: lit("active"),
            };

            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: ActionValue::Operation(Box::new(pending_check)),
                        then: lit(10i64),
                    },
                    Case {
                        when: ActionValue::Operation(Box::new(active_check)),
                        then: lit(20i64),
                    },
                ],
                default: Some(lit(0i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(20));
        }
    }

    // -------------------------------------------------------------------------
    // Nested Operations Tests
    // -------------------------------------------------------------------------

    mod nested {
        use super::*;

        #[test]
        fn test_nested_arithmetic_in_max() {
            // MAX(0, 100 - 50) = MAX(0, 50) = 50
            let resolver = TestResolver::new();

            let sub_op = ActionOperation::Subtract {
                values: vec![lit(100i64), lit(50i64)],
            };
            let subtract_val = ActionValue::Operation(Box::new(sub_op));

            let op = ActionOperation::Max {
                values: vec![lit(0i64), subtract_val],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(50));
        }

        #[test]
        fn test_deeply_nested_operations() {
            // IF (10 > 5) THEN (2 * 3) ELSE (1 + 1)
            // Expressed as IF with cases: [case(10>5 -> 2*3)], default: 1+1
            let resolver = TestResolver::new();

            let gt_op = ActionOperation::GreaterThan {
                subject: lit(10i64),
                value: lit(5i64),
            };
            let condition = ActionValue::Operation(Box::new(gt_op));

            let mul_op = ActionOperation::Multiply {
                values: vec![lit(2i64), lit(3i64)],
            };
            let then_branch = ActionValue::Operation(Box::new(mul_op));

            let add_op = ActionOperation::Add {
                values: vec![lit(1i64), lit(1i64)],
            };
            let else_branch = ActionValue::Operation(Box::new(add_op));

            let op = ActionOperation::If {
                cases: vec![Case {
                    when: condition,
                    then: then_branch,
                }],
                default: Some(else_branch),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(6));
        }
    }

    // -------------------------------------------------------------------------
    // Error Handling Tests
    // -------------------------------------------------------------------------

    mod errors {
        use super::*;

        #[test]
        fn test_type_mismatch_in_arithmetic() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(10i64), lit("not a number")],
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
        }

        #[test]
        fn test_variable_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: var("nonexistent"),
                value: lit(42i64),
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
        }

        #[test]
        fn test_overflow_detection() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Multiply {
                values: vec![lit(i64::MAX), lit(2i64)],
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::ArithmeticOverflow(_))));
        }

        #[test]
        fn test_max_depth_exceeded() {
            let resolver = TestResolver::new();

            let mut nested: ActionValue = lit(42i64);
            for _ in 0..=MAX_OPERATION_DEPTH + 1 {
                let if_op = ActionOperation::If {
                    cases: vec![Case {
                        when: lit(true),
                        then: nested,
                    }],
                    default: None,
                };
                nested = ActionValue::Operation(Box::new(if_op));
            }

            if let ActionValue::Operation(op) = nested {
                let result = execute_operation(&op, &resolver, 0);
                assert!(
                    matches!(result, Err(EngineError::MaxDepthExceeded(_))),
                    "Expected MaxDepthExceeded error"
                );
            }
        }

        #[test]
        fn test_depth_guard_allows_the_maximum_depth() {
            // The guard rejects nesting *deeper* than MAX_OPERATION_DEPTH, so a
            // law that nests exactly that deep still evaluates. Pinning the
            // boundary keeps the limit from silently shifting by one.
            let resolver = TestResolver::new();

            assert_eq!(
                evaluate_value(&lit(42i64), &resolver, MAX_OPERATION_DEPTH).unwrap(),
                Value::Int(42)
            );

            let op = ActionOperation::List { items: vec![] };
            assert_eq!(
                execute_operation(&op, &resolver, MAX_OPERATION_DEPTH).unwrap(),
                Value::Array(vec![])
            );
        }

        #[test]
        fn test_depth_guard_rejects_beyond_the_maximum_depth() {
            // One step past the limit is refused, and so is anything deeper —
            // the guard is a threshold, not an equality check on one depth.
            let resolver = TestResolver::new();

            for depth in [MAX_OPERATION_DEPTH + 1, MAX_OPERATION_DEPTH + 2] {
                assert!(
                    matches!(
                        evaluate_value(&lit(42i64), &resolver, depth),
                        Err(EngineError::MaxDepthExceeded(_))
                    ),
                    "evaluate_value accepted depth {depth}"
                );

                let op = ActionOperation::List { items: vec![] };
                assert!(
                    matches!(
                        execute_operation(&op, &resolver, depth),
                        Err(EngineError::MaxDepthExceeded(_))
                    ),
                    "execute_operation accepted depth {depth}"
                );
            }
        }

        #[test]
        fn test_decimal_to_i64_safe_rejects_non_integer() {
            // A fractional value is "not an integer" (InvalidOperation), not an overflow.
            assert!(matches!(
                decimal_to_i64_safe(dec!(42.5)),
                Err(EngineError::InvalidOperation(_))
            ));
            assert_eq!(decimal_to_i64_safe(dec!(42)).unwrap(), 42);
            assert_eq!(decimal_to_i64_safe(dec!(-42)).unwrap(), -42);
        }

        #[test]
        fn test_values_equal_numeric() {
            // Int and Decimal compare numerically and exactly (no float rounding).
            assert!(values_equal(&Value::Int(42), &Value::Decimal(dec!(42))));
            assert!(values_equal(&Value::Decimal(dec!(42)), &Value::Int(42)));
            assert!(values_equal(
                &Value::Decimal(dec!(1.50)),
                &Value::Decimal(dec!(1.5))
            ));
            assert!(!values_equal(&Value::Decimal(dec!(42.5)), &Value::Int(42)));
        }

        #[test]
        fn test_values_equal_untranslatable() {
            // RFC-012: untranslatables compare like NaN in this domain. Two of
            // them count as equal (both are "could not be determined"), and one
            // never equals a concrete value — so a membership test against a
            // list can never accidentally match on a taint.
            let unknown = Value::Untranslatable {
                article: "5".to_string(),
                construct: "naar het oordeel van".to_string(),
            };
            let other_unknown = Value::Untranslatable {
                article: "9".to_string(),
                construct: "in redelijkheid".to_string(),
            };

            assert!(values_equal(&unknown, &other_unknown));
            assert!(!values_equal(&unknown, &Value::Int(42)));
            assert!(!values_equal(&Value::String("ja".to_string()), &unknown));
        }

        #[test]
        fn test_arithmetic_with_large_integer() {
            // Beyond 2^53 arithmetic is now exact (Decimal), no precision loss.
            let large_value: i64 = 9_007_199_254_740_993; // 2^53 + 1

            let resolver = TestResolver::new();
            let op = ActionOperation::Add {
                values: vec![lit(large_value), lit(1i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(9_007_199_254_740_994));
        }
    }

    // -------------------------------------------------------------------------
    // Collection Operations Tests
    // -------------------------------------------------------------------------

    mod collection {
        use super::*;

        #[test]
        fn test_in_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(42i64),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_not_found() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(99i64),
                value: None,
                values: Some(vec![lit(10i64), lit(20i64), lit(42i64), lit(50i64)]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_in_with_strings() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit("apple"),
                value: None,
                values: Some(vec![lit("banana"), lit("apple"), lit("orange")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_mixed_int_float() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(42i64),
                value: None,
                values: Some(vec![lit(10i64), lit(42.0f64), lit(50i64)]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_with_variables() {
            let resolver = TestResolver::new().with_var("status", "active");

            let op = ActionOperation::In {
                subject: var("status"),
                value: None,
                values: Some(vec![lit("active"), lit("pending"), lit("inactive")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_in_missing_values() {
            let resolver = TestResolver::new();
            let op = ActionOperation::In {
                subject: lit(42i64),
                value: None,
                values: None,
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_not_in_true_when_absent() {
            // NOT_IN is the complement of IN: absent subject → true.
            let resolver = TestResolver::new();
            let op = ActionOperation::NotIn {
                subject: lit("student"),
                value: None,
                values: Some(vec![lit("gehuwd"), lit("samenwonend")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(true));
        }

        #[test]
        fn test_not_in_false_when_present() {
            // The exclusion fires: a subject that IS in the list is not "not in" it.
            let resolver = TestResolver::new();
            let op = ActionOperation::NotIn {
                subject: lit("gehuwd"),
                value: None,
                values: Some(vec![lit("gehuwd"), lit("samenwonend")]),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Bool(false));
        }

        #[test]
        fn test_list_construct() {
            let resolver = TestResolver::new();
            let op = ActionOperation::List {
                items: vec![lit(1i64), lit(2i64), lit(3i64)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
            );
        }

        #[test]
        fn test_list_with_mixed_types() {
            let resolver = TestResolver::new();
            let op = ActionOperation::List {
                items: vec![lit(1i64), lit("two"), lit(true)],
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(
                result,
                Value::Array(vec![
                    Value::Int(1),
                    Value::String("two".to_string()),
                    Value::Bool(true),
                ])
            );
        }
    }

    // -------------------------------------------------------------------------
    // Collection Iteration Tests (FOREACH, RFC-016)
    // -------------------------------------------------------------------------
    //
    // These drive a real `RuleContext` rather than `TestResolver`: FOREACH binds
    // a variable, and only a resolver with child scopes can run it. A
    // `TestResolver` returns `None` from `execute_foreach_op`, which is itself
    // worth a test (see `test_foreach_needs_a_scoped_resolver`).

    mod foreach {
        use super::*;
        use crate::context::RuleContext;

        /// A context whose parameters are the given variables.
        fn ctx(vars: Vec<(&str, Value)>) -> RuleContext<'static> {
            let params: BTreeMap<String, Value> =
                vars.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            RuleContext::new(params, "2025-06-15").expect("valid date")
        }

        /// `FOREACH $items as x: x` with the given combine.
        fn sum_of(items: Vec<Value>, combine: Option<CombineOp>) -> Result<Value> {
            let context = ctx(vec![("items", Value::Array(items))]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine,
            };
            execute_operation(&op, &context, 0)
        }

        fn obj(pairs: Vec<(&str, Value)>) -> Value {
            Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
        }

        #[test]
        fn test_foreach_combine_only_gives_the_one_element() {
            let row = obj(vec![("gemeente", Value::String("Utrecht".to_string()))]);
            assert_eq!(
                sum_of(vec![row.clone()], Some(CombineOp::Only)).unwrap(),
                row
            );
            assert_eq!(
                sum_of(
                    vec![Value::String("Zuid".to_string())],
                    Some(CombineOp::Only)
                )
                .unwrap(),
                Value::String("Zuid".to_string())
            );
        }

        #[test]
        fn test_foreach_combine_only_of_nothing_is_absent() {
            assert_eq!(sum_of(vec![], Some(CombineOp::Only)).unwrap(), Value::Null);
        }

        #[test]
        fn test_foreach_combine_only_refuses_two_elements() {
            let two = vec![Value::Int(1), Value::Int(2)];
            let e = sum_of(two, Some(CombineOp::Only)).unwrap_err();
            assert!(
                matches!(&e, EngineError::InvalidOperation(m) if m.contains("left 2 elements")),
                "{e}"
            );
        }

        #[test]
        fn test_foreach_combine_add() {
            let result = sum_of(
                vec![Value::Int(10), Value::Int(20), Value::Int(30)],
                Some(CombineOp::Add),
            )
            .unwrap();
            assert_eq!(result, Value::Int(60));
        }

        #[test]
        fn test_foreach_without_combine_returns_array() {
            let result = sum_of(vec![Value::Int(1), Value::Int(2)], None).unwrap();
            assert_eq!(result, Value::Array(vec![Value::Int(1), Value::Int(2)]));
        }

        #[test]
        fn test_foreach_body_is_an_operation() {
            // FOREACH items as x: MULTIPLY(x, 2), combine ADD => 2+4+6
            let context = ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            )]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: ActionValue::Operation(Box::new(ActionOperation::Multiply {
                    values: vec![var("x"), lit(2i64)],
                })),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(12));
        }

        #[test]
        fn test_foreach_filter_skips_elements() {
            // Count the elements above 2, which is the kostendelersnorm shape:
            // a threshold that stays in the law rather than in the data source.
            let context = ctx(vec![(
                "items",
                Value::Array(vec![
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(3),
                    Value::Int(4),
                ]),
            )]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: lit(1i64),
                filter: Some(ActionValue::Operation(Box::new(
                    ActionOperation::GreaterThan {
                        subject: var("x"),
                        value: lit(2i64),
                    },
                ))),
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn test_foreach_filter_rejecting_everything_is_the_empty_identity() {
            let context = ctx(vec![("items", Value::Array(vec![Value::Int(1)]))]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: Some(lit(false)),
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(0));
        }

        #[test]
        fn test_foreach_dot_notation_on_objects() {
            let context = ctx(vec![(
                "kinderen",
                Value::Array(vec![
                    obj(vec![("bedrag", Value::Int(703))]),
                    obj(vec![("bedrag", Value::Int(936))]),
                ]),
            )]);
            let op = ActionOperation::Foreach {
                collection: var("kinderen"),
                as_name: Some("kind".to_string()),
                body: var("kind.bedrag"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(
                execute_operation(&op, &context, 0).unwrap(),
                Value::Int(1639)
            );
        }

        /// `FOREACH $kinderen` with the given `as` and body, summed.
        fn sum_over_kinderen(
            context: &RuleContext,
            as_name: Option<&str>,
            body: &str,
        ) -> Result<Value> {
            let op = ActionOperation::Foreach {
                collection: var("kinderen"),
                as_name: as_name.map(str::to_string),
                body: var(body),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            execute_operation(&op, context, 0)
        }

        #[test]
        fn test_foreach_without_as_flattens_object_fields_into_scope() {
            // Without `as`, `$bedrag` resolves without a prefix, and shadows
            // the outer `$bedrag` (RFC-016). `$item.bedrag` reads the same.
            let context = ctx(vec![
                (
                    "kinderen",
                    Value::Array(vec![obj(vec![("bedrag", Value::Int(42))])]),
                ),
                ("bedrag", Value::Int(1000)),
            ]);
            assert_eq!(
                sum_over_kinderen(&context, None, "bedrag").unwrap(),
                Value::Int(42)
            );
            assert_eq!(
                sum_over_kinderen(&context, None, "item.bedrag").unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn test_foreach_with_as_does_not_bind_fields_by_their_own_name() {
            // With `as`, the bare name is the outer variable, not the field
            // (RFC-049): that is what lets a body compare the row with a
            // parameter of the same name.
            let context = ctx(vec![
                (
                    "kinderen",
                    Value::Array(vec![
                        obj(vec![("bedrag", Value::Int(42))]),
                        obj(vec![("bedrag", Value::Int(8))]),
                    ]),
                ),
                ("bedrag", Value::Int(1000)),
            ]);
            assert_eq!(
                sum_over_kinderen(&context, Some("kind"), "bedrag").unwrap(),
                Value::Int(2000)
            );
            // The field is reachable through the binding.
            assert_eq!(
                sum_over_kinderen(&context, Some("kind"), "kind.bedrag").unwrap(),
                Value::Int(50)
            );
            // `item` is not bound either: the law named the element.
            assert!(sum_over_kinderen(&context, Some("kind"), "item.bedrag").is_err());
        }

        #[test]
        fn test_foreach_with_as_leaves_a_bare_field_name_undefined() {
            // Without an outer variable of that name, a bare field name does
            // not resolve at all, where it used to read the field.
            let context = ctx(vec![(
                "kinderen",
                Value::Array(vec![obj(vec![("bedrag", Value::Int(42))])]),
            )]);
            let err = sum_over_kinderen(&context, Some("kind"), "bedrag").unwrap_err();
            assert!(
                matches!(&err, EngineError::VariableNotFound(name) if name == "bedrag"),
                "got {err:?}"
            );
            assert_eq!(
                sum_over_kinderen(&context, None, "bedrag").unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn test_foreach_with_as_compares_the_row_with_a_parameter_of_its_field_name() {
            // The case that motivated RFC-049: a reduction with a parameter
            // `bsn_jongere` over rows that also carry `bsn_jongere`.
            let rij = |bsn: &str, uren: i64| {
                obj(vec![
                    ("bsn_jongere", Value::String(bsn.to_string())),
                    ("uren", Value::Int(uren)),
                ])
            };
            let context = ctx(vec![
                (
                    "kinderen",
                    Value::Array(vec![rij("111", 3), rij("222", 5), rij("111", 4)]),
                ),
                ("bsn_jongere", Value::String("111".to_string())),
            ]);
            let op = |as_name: Option<&str>, row: &str| ActionOperation::Foreach {
                collection: var("kinderen"),
                as_name: as_name.map(str::to_string),
                filter: Some(ActionValue::Operation(Box::new(ActionOperation::Equals {
                    subject: var(row),
                    value: var("bsn_jongere"),
                }))),
                body: var(if as_name.is_some() { "r.uren" } else { "uren" }),
                combine: Some(CombineOp::Add),
            };
            assert_eq!(
                execute_operation(&op(Some("r"), "r.bsn_jongere"), &context, 0).unwrap(),
                Value::Int(7)
            );
            // Without `as` the field shadows the parameter: every row equals
            // itself and the filter keeps all of them.
            assert_eq!(
                execute_operation(&op(None, "item.bsn_jongere"), &context, 0).unwrap(),
                Value::Int(12)
            );
        }

        #[test]
        fn test_foreach_nested_inner_collection_sees_outer_binding() {
            // The outer binding is reachable from the inner `collection`, and
            // only from there. That is the whole scope contract of RFC-016.
            let context = ctx(vec![(
                "huishoudens",
                Value::Array(vec![
                    obj(vec![(
                        "leden",
                        Value::Array(vec![Value::Int(100), Value::Int(200)]),
                    )]),
                    obj(vec![("leden", Value::Array(vec![Value::Int(50)]))]),
                ]),
            )]);
            let inner = ActionOperation::Foreach {
                collection: var("huishouden.leden"),
                as_name: Some("lid".to_string()),
                body: var("lid"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            let outer = ActionOperation::Foreach {
                collection: var("huishoudens"),
                as_name: Some("huishouden".to_string()),
                body: ActionValue::Operation(Box::new(inner)),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(
                execute_operation(&outer, &context, 0).unwrap(),
                Value::Int(350)
            );
        }

        #[test]
        fn test_foreach_inner_scope_does_not_see_outer_binding() {
            // `$huishouden` is bound in the parent, and a child context starts
            // with an empty local scope, so the inner body cannot resolve it.
            let context = ctx(vec![(
                "huishoudens",
                Value::Array(vec![obj(vec![(
                    "leden",
                    Value::Array(vec![Value::Int(1)]),
                )])]),
            )]);
            let inner = ActionOperation::Foreach {
                collection: var("huishouden.leden"),
                as_name: Some("lid".to_string()),
                body: var("huishouden"),
                filter: None,
                combine: None,
            };
            let outer = ActionOperation::Foreach {
                collection: var("huishoudens"),
                as_name: Some("huishouden".to_string()),
                body: ActionValue::Operation(Box::new(inner)),
                filter: None,
                combine: None,
            };
            assert!(matches!(
                execute_operation(&outer, &context, 0),
                Err(EngineError::VariableNotFound(_))
            ));
        }

        #[test]
        fn test_foreach_empty_collection_per_combine() {
            let empty = || Vec::new();
            assert_eq!(
                sum_of(empty(), Some(CombineOp::Add)).unwrap(),
                Value::Int(0)
            );
            assert_eq!(
                sum_of(empty(), Some(CombineOp::Or)).unwrap(),
                Value::Bool(false)
            );
            // Vacuous truth: AND over nothing is true.
            assert_eq!(
                sum_of(empty(), Some(CombineOp::And)).unwrap(),
                Value::Bool(true)
            );
            // There is no lowest value of nothing.
            assert_eq!(sum_of(empty(), Some(CombineOp::Min)).unwrap(), Value::Null);
            assert_eq!(sum_of(empty(), Some(CombineOp::Max)).unwrap(), Value::Null);
            assert_eq!(sum_of(empty(), None).unwrap(), Value::Array(vec![]));
        }

        #[test]
        fn test_foreach_null_collection_is_empty() {
            let context = ctx(vec![("items", Value::Null)]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(0));
        }

        #[test]
        fn test_foreach_non_array_iterates_once() {
            let context = ctx(vec![("item", Value::Int(7))]);
            let op = ActionOperation::Foreach {
                collection: var("item"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(7));
        }

        #[test]
        fn test_foreach_combine_min_and_max() {
            let items = vec![Value::Int(30), Value::Int(10), Value::Int(20)];
            assert_eq!(
                sum_of(items.clone(), Some(CombineOp::Min)).unwrap(),
                Value::Int(10)
            );
            assert_eq!(sum_of(items, Some(CombineOp::Max)).unwrap(), Value::Int(30));
        }

        #[test]
        fn test_foreach_min_keeps_decimals_decimal() {
            let items = vec![Value::Decimal(dec!(1.5)), Value::Int(3)];
            assert_eq!(
                sum_of(items, Some(CombineOp::Min)).unwrap(),
                Value::Decimal(dec!(1.5))
            );
        }

        #[test]
        fn test_foreach_combine_add_concatenates_strings() {
            // RFC-007 polymorphism reaches combine through the shared add_values.
            let items = vec![Value::String("a".into()), Value::String("b".into())];
            assert_eq!(
                sum_of(items, Some(CombineOp::Add)).unwrap(),
                Value::String("ab".to_string())
            );
        }

        #[test]
        fn test_foreach_combine_and_or_short_circuit_past_unknown() {
            // A definitive verdict beats an unknown, matching AND/OR themselves
            // (RFC-036).
            let unknown = unknown_fact("verzekerd");
            assert_eq!(
                sum_of(
                    vec![unknown.clone(), Value::Bool(true)],
                    Some(CombineOp::Or)
                )
                .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                sum_of(
                    vec![unknown.clone(), Value::Bool(false)],
                    Some(CombineOp::And)
                )
                .unwrap(),
                Value::Bool(false)
            );
            // With no definitive verdict, the result is that unknown.
            let or = sum_of(
                vec![unknown.clone(), Value::Bool(false)],
                Some(CombineOp::Or),
            )
            .unwrap();
            assert_eq!(or.missing_facts(), unknown.missing_facts());
            let and = sum_of(
                vec![unknown.clone(), Value::Bool(true)],
                Some(CombineOp::And),
            )
            .unwrap();
            assert_eq!(and.missing_facts(), unknown.missing_facts());
        }

        #[test]
        fn test_foreach_combine_and_or_reject_an_absent_verdict() {
            // An absence is not a verdict: the body has to test for it first.
            for combine in [CombineOp::Or, CombineOp::And] {
                let err = sum_of(vec![Value::Null, Value::Bool(true)], Some(combine)).unwrap_err();
                assert!(
                    matches!(&err, EngineError::AbsentOperand { operation } if operation.starts_with("FOREACH combine")),
                    "got {err:?}"
                );
            }
        }

        #[test]
        fn test_foreach_combine_add_is_unknown_as_soon_as_one_element_is() {
            // Not "skip the unknown and sum the rest": that reports a confident
            // total that is short by an unknown amount. The result names what
            // is missing, and names each fact once.
            let huur = unknown_fact("huur");
            let sum = sum_of(vec![Value::Int(5), huur.clone()], Some(CombineOp::Add)).unwrap();
            assert_eq!(sum.missing_facts(), huur.missing_facts());
            let both = sum_of(
                vec![huur.clone(), unknown_fact("servicekosten"), huur.clone()],
                Some(CombineOp::Add),
            )
            .unwrap();
            let names: Vec<&str> = both
                .missing_facts()
                .iter()
                .map(|m| m.name.as_str())
                .collect();
            assert_eq!(names, vec!["huur", "servicekosten"]);
            // Nothing is missing from an empty collection, so it stays the
            // additive identity.
            assert_eq!(sum_of(vec![], Some(CombineOp::Add)).unwrap(), Value::Int(0));
        }

        #[test]
        fn test_foreach_combine_add_min_max_reject_an_absent_element() {
            // A null element is an absence, not an amount (RFC-036): summing it
            // or ranking it is an error the law has to resolve (an IF on
            // `EQUALS … null` in the body), never a silent skip or a null total.
            for combine in [CombineOp::Add, CombineOp::Min, CombineOp::Max] {
                let err = sum_of(vec![Value::Int(5), Value::Null], Some(combine)).unwrap_err();
                assert!(
                    matches!(&err, EngineError::AbsentOperand { operation } if operation.starts_with("FOREACH combine")),
                    "got {err:?}"
                );
            }
            // Without a combine the array is a value and may hold the absence.
            assert_eq!(
                sum_of(vec![Value::Int(5), Value::Null], None).unwrap(),
                Value::Array(vec![Value::Int(5), Value::Null])
            );
        }

        #[test]
        fn test_foreach_combine_min_max_propagate_an_unknown_element() {
            let huur = unknown_fact("huur");
            for combine in [CombineOp::Min, CombineOp::Max] {
                let result = sum_of(vec![Value::Int(5), huur.clone()], Some(combine)).unwrap();
                assert_eq!(result.missing_facts(), huur.missing_facts());
            }
            // Unknown elements stay elements when there is no combine.
            let listed = sum_of(vec![Value::Int(5), huur.clone()], None).unwrap();
            assert_eq!(listed, Value::Array(vec![Value::Int(5), huur]));
        }

        #[test]
        fn test_foreach_unknown_collection_is_unknown() {
            // A collection nobody has yet cannot be iterated; a null collection
            // (the register says there are no members) iterates nothing.
            let huishoudens = unknown_fact("huishoudens");
            let context = ctx(vec![("items", huishoudens.clone())]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            let result = execute_operation(&op, &context, 0).unwrap();
            assert_eq!(result.missing_facts(), huishoudens.missing_facts());
        }

        #[test]
        fn test_foreach_untranslatable_in_collection_taints() {
            let taint = Value::Untranslatable {
                article: "22a".to_string(),
                construct: "kostendelersnorm".to_string(),
            };
            let context = ctx(vec![("items", taint.clone())]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), taint);
        }

        #[test]
        fn test_foreach_untranslatable_in_body_taints_whole_result() {
            let taint = Value::Untranslatable {
                article: "7".to_string(),
                construct: "naar redelijkheid".to_string(),
            };
            let context = ctx(vec![
                ("items", Value::Array(vec![Value::Int(1), Value::Int(2)])),
                ("onbekend", taint.clone()),
            ]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("onbekend"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), taint);
        }

        #[test]
        fn test_foreach_untranslatable_filter_taints() {
            let taint = Value::Untranslatable {
                article: "1".to_string(),
                construct: "in redelijkheid".to_string(),
            };
            let context = ctx(vec![
                ("items", Value::Array(vec![Value::Int(1)])),
                ("onbekend", taint.clone()),
            ]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: Some(var("onbekend")),
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), taint);
        }

        #[test]
        fn test_foreach_unknown_filter_is_unknown_not_skipped() {
            // Excluding an element the engine cannot judge would report a
            // confident wrong total. The element is an unknown element instead,
            // and the sum is unknown for the facts the filter missed (RFC-036).
            let onbekend = unknown_fact("verzekerd");
            let context = ctx(vec![
                ("items", Value::Array(vec![Value::Int(1), Value::Int(2)])),
                ("onbekend", onbekend.clone()),
            ]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: Some(var("onbekend")),
                combine: Some(CombineOp::Add),
            };
            let result = execute_operation(&op, &context, 0).unwrap();
            assert_eq!(result.missing_facts(), onbekend.missing_facts());
        }

        #[test]
        fn test_foreach_unknown_filter_loses_to_a_definite_true_under_or() {
            // Kleene through the filter too: `OR` over [unknown-element, true]
            // is true whatever the unknown element would have contributed.
            let context = ctx(vec![(
                "items",
                Value::Array(vec![
                    obj(vec![
                        ("keur", unknown_fact("keur")),
                        ("ja", Value::Bool(true)),
                    ]),
                    obj(vec![("keur", Value::Bool(true)), ("ja", Value::Bool(true))]),
                ]),
            )]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x.ja"),
                filter: Some(var("x.keur")),
                combine: Some(CombineOp::Or),
            };
            assert_eq!(
                execute_operation(&op, &context, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_foreach_absent_filter_verdict_is_an_error() {
            // A null filter is not a verdict about the element: the law has to
            // test for the absence before it selects on it (RFC-036).
            let context = ctx(vec![
                ("items", Value::Array(vec![Value::Int(1)])),
                ("geen", Value::Null),
            ]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: Some(var("geen")),
                combine: Some(CombineOp::Add),
            };
            let err = execute_operation(&op, &context, 0).unwrap_err();
            assert!(
                matches!(&err, EngineError::AbsentOperand { operation } if operation == "FOREACH filter"),
                "got {err:?}"
            );
        }

        #[test]
        fn test_foreach_error_in_body_aborts_without_partial_result() {
            // A sum over some of the children is not a legal determination.
            let context = ctx(vec![("items", Value::Array(vec![Value::Int(1)]))]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("bestaat_niet"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert!(matches!(
                execute_operation(&op, &context, 0),
                Err(EngineError::VariableNotFound(_))
            ));
        }

        #[test]
        fn test_foreach_binding_shadows_outer_variable() {
            let context = ctx(vec![
                ("items", Value::Array(vec![Value::Int(9)])),
                ("x", Value::Int(1)),
            ]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(9));
        }

        #[test]
        fn test_foreach_rejects_invalid_binding_name() {
            for bad in ["", "X", "1kind", "kind-naam", "kind naam"] {
                let context = ctx(vec![("items", Value::Array(vec![Value::Int(1)]))]);
                let op = ActionOperation::Foreach {
                    collection: var("items"),
                    as_name: Some(bad.to_string()),
                    body: lit(1i64),
                    filter: None,
                    combine: None,
                };
                assert!(
                    matches!(
                        execute_operation(&op, &context, 0),
                        Err(EngineError::InvalidOperation(_))
                    ),
                    "expected {bad:?} to be rejected as a binding name"
                );
            }
        }

        #[test]
        fn test_foreach_accepts_valid_binding_names() {
            for good in ["x", "kind", "_kind", "kind_2", "medebewoner"] {
                let context = ctx(vec![("items", Value::Array(vec![Value::Int(1)]))]);
                let op = ActionOperation::Foreach {
                    collection: var("items"),
                    as_name: Some(good.to_string()),
                    body: lit(1i64),
                    filter: None,
                    combine: Some(CombineOp::Add),
                };
                assert_eq!(
                    execute_operation(&op, &context, 0).unwrap(),
                    Value::Int(1),
                    "expected {good:?} to be accepted as a binding name"
                );
            }
        }

        #[test]
        fn test_foreach_rejects_collection_over_max_array_size() {
            let too_many: Vec<Value> = (0..=crate::config::MAX_ARRAY_SIZE as i64)
                .map(Value::Int)
                .collect();
            assert!(matches!(
                sum_of(too_many, Some(CombineOp::Add)),
                Err(EngineError::InvalidOperation(_))
            ));
        }

        #[test]
        fn test_foreach_accepts_collection_at_max_array_size() {
            // The limit is inclusive; one more is the error above.
            let exactly = vec![Value::Int(1); crate::config::MAX_ARRAY_SIZE];
            assert_eq!(
                sum_of(exactly, Some(CombineOp::Add)).unwrap(),
                Value::Int(crate::config::MAX_ARRAY_SIZE as i64)
            );
        }

        #[test]
        fn test_foreach_add_propagates_an_unknown_element() {
            // A body that cannot produce a value yet makes the total unknown.
            // This is the same rule RFC-016 states for an undecidable filter:
            // the element is in the collection and its contribution is not
            // known, so summing the rest would report a number that is short by
            // an unknown amount.
            let bijdrage = unknown_fact("bijdrage");
            let result = sum_of(
                vec![Value::Int(10), bijdrage.clone(), Value::Int(20)],
                Some(CombineOp::Add),
            )
            .unwrap();
            assert_eq!(result.missing_facts(), bijdrage.missing_facts());
        }

        #[test]
        fn test_foreach_nested_min_over_empty_is_an_absence_the_sum_rejects() {
            // An inner MIN/MAX over an empty collection is null: there is no
            // highest value of nothing, and that is an absence, not an unknown.
            // An outer ADD may neither leave it out (the total would look
            // complete while one member was never counted) nor count it as an
            // amount: the law has to say what an empty household contributes
            // (RFC-036), so the sum is an error until it does.
            let context = ctx(vec![(
                "huishoudens",
                Value::Array(vec![
                    obj(vec![("leden", Value::Array(vec![Value::Int(100)]))]),
                    obj(vec![("leden", Value::Array(vec![]))]),
                ]),
            )]);
            let inner = ActionOperation::Foreach {
                collection: var("huishouden.leden"),
                as_name: Some("lid".to_string()),
                body: var("lid"),
                filter: None,
                combine: Some(CombineOp::Max),
            };
            let outer = ActionOperation::Foreach {
                collection: var("huishoudens"),
                as_name: Some("huishouden".to_string()),
                body: ActionValue::Operation(Box::new(inner)),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            let err = execute_operation(&outer, &context, 0).unwrap_err();
            assert!(
                matches!(&err, EngineError::AbsentOperand { operation } if operation == "FOREACH combine ADD"),
                "got {err:?}"
            );
        }

        #[test]
        fn test_foreach_element_field_cannot_overwrite_the_binding() {
            // Without `as`, over `{item: 7, bedrag: 5}`: the binding must stay
            // the element, or `$item.bedrag` becomes a type error on an
            // integer.
            let context = ctx(vec![(
                "kinderen",
                Value::Array(vec![obj(vec![
                    ("item", Value::Int(7)),
                    ("bedrag", Value::Int(5)),
                ])]),
            )]);
            let op = ActionOperation::Foreach {
                collection: var("kinderen"),
                as_name: None,
                body: var("item.bedrag"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), Value::Int(5));
        }

        #[test]
        fn test_foreach_element_field_named_referencedate_stays_unreachable_bare() {
            // `referencedate` resolves ahead of local scope, so the flattened
            // form reads the calculation date however the element is shaped.
            // Worth pinning: a law author reading `$referencedate` inside a
            // FOREACH gets the peildatum, never the element's own field, and
            // dot notation is the only way at the latter.
            let context = ctx(vec![(
                "items",
                Value::Array(vec![obj(vec![(
                    "referencedate",
                    Value::String("1990-01-01".to_string()),
                )])]),
            )]);
            let bare = ActionOperation::Foreach {
                collection: var("items"),
                as_name: None,
                body: var("referencedate.iso"),
                filter: None,
                combine: None,
            };
            // The context's own reference date, not the element's field.
            assert_eq!(
                execute_operation(&bare, &context, 0).unwrap(),
                Value::Array(vec![Value::String("2025-06-15".to_string())])
            );

            let dotted = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x.referencedate"),
                filter: None,
                combine: None,
            };
            assert_eq!(
                execute_operation(&dotted, &context, 0).unwrap(),
                Value::Array(vec![Value::String("1990-01-01".to_string())])
            );
        }

        #[test]
        fn test_foreach_untranslatable_taints_the_array_too() {
            // Without `combine` there is no aggregation to catch the taint on
            // the way out, so FOREACH has to stop at the element itself. An
            // array holding an untranslatable would read downstream as a list
            // of real values.
            let taint = Value::Untranslatable {
                article: "1".to_string(),
                construct: "naar omstandigheden".to_string(),
            };
            let context = ctx(vec![
                ("items", Value::Array(vec![Value::Int(1)])),
                ("onbekend", taint.clone()),
            ]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("onbekend"),
                filter: None,
                combine: None,
            };
            assert_eq!(execute_operation(&op, &context, 0).unwrap(), taint);
        }

        /// Run an operation with tracing on, returning its trace node.
        fn traced_operation(op: &ActionOperation, context: &RuleContext) -> crate::trace::PathNode {
            let trace = std::rc::Rc::new(std::cell::RefCell::new(
                crate::trace::TraceBuilder::new_untimed(),
            ));
            let mut traced = context.clone();
            traced.set_trace(std::rc::Rc::clone(&trace));
            trace.borrow_mut().push("root", PathNodeType::Action);
            execute_operation(op, &traced, 0).expect("foreach should succeed");
            let root = trace.borrow_mut().pop().expect("root node should pop");
            root.children
                .into_iter()
                .next()
                .expect("FOREACH should record a node")
        }

        #[test]
        fn test_foreach_traces_every_iteration_and_counts_the_skipped() {
            // RFC-013: a reduction that shows only its total does not explain
            // how it was reached. The skip count in particular is the only
            // place the filter's effect is visible in the trace.
            let context = ctx(vec![(
                "items",
                Value::Array(vec![Value::Int(1), Value::Int(5), Value::Int(9)]),
            )]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: Some(ActionValue::Operation(Box::new(
                    ActionOperation::GreaterThan {
                        subject: var("x"),
                        value: lit(4i64),
                    },
                ))),
                combine: Some(CombineOp::Add),
            };

            let node = traced_operation(&op, &context);
            assert_eq!(node.name, "FOREACH");
            assert_eq!(
                node.message.as_deref(),
                Some("FOREACH over 2 element(s), 1 skipped by filter = 14")
            );

            // One node per element, skipped ones included, each naming its index.
            let items: Vec<&crate::trace::PathNode> = node
                .children
                .iter()
                .filter(|c| c.name.starts_with("ITEM_"))
                .collect();
            assert_eq!(items.len(), 3);
            assert_eq!(items[0].name, "ITEM_0");
            assert_eq!(
                items[0].message.as_deref(),
                Some("ITEM 0: skipped by filter")
            );
            assert_eq!(items[2].name, "ITEM_2");
            assert!(items[2]
                .message
                .as_deref()
                .is_some_and(|m| m.starts_with("ITEM 2: ")));
        }

        #[test]
        fn test_foreach_trace_message_without_a_filter_omits_the_skip_count() {
            let context = ctx(vec![("items", Value::Array(vec![Value::Int(1)]))]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            let node = traced_operation(&op, &context);
            assert_eq!(
                node.message.as_deref(),
                Some("FOREACH over 1 element(s) = 1")
            );
        }

        #[test]
        fn test_if_keeps_its_own_trace_message() {
            // The dispatcher normally overwrites a handler's trace message with
            // "Compute OP(...) = value". IF and FOREACH are the exceptions: what
            // they write from inside says which case matched, or how many
            // elements were seen, and the value alone does not.
            //
            // This lives in the FOREACH module because adding the FOREACH arm is
            // what turned that special case from an `if` into a `match`, and a
            // match arm is the kind of thing a refactor drops silently.
            let context = ctx(vec![("bedrag", Value::Int(50))]);

            let matched = ActionOperation::If {
                cases: vec![Case {
                    when: ActionValue::Operation(Box::new(ActionOperation::GreaterThan {
                        subject: var("bedrag"),
                        value: lit(10i64),
                    })),
                    then: lit(1i64),
                }],
                default: Some(lit(0i64)),
            };
            assert_eq!(
                traced_operation(&matched, &context).message.as_deref(),
                Some("IF(case 0 matched) = 1")
            );

            let defaulted = ActionOperation::If {
                cases: vec![Case {
                    when: lit(false),
                    then: lit(1i64),
                }],
                default: Some(lit(0i64)),
            };
            assert_eq!(
                traced_operation(&defaulted, &context).message.as_deref(),
                Some("IF(took default) = 0")
            );
        }

        #[test]
        fn test_foreach_body_and_filter_count_against_the_depth_limit() {
            // The body and filter are evaluated one level deeper than the
            // FOREACH itself. Without that increment a deeply nested law would
            // recurse past MAX_OPERATION_DEPTH instead of being rejected.
            let context = ctx(vec![("items", Value::Array(vec![Value::Int(1)]))]);
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };

            // At the ceiling the body's own evaluation is over it.
            assert!(matches!(
                execute_operation(&op, &context, MAX_OPERATION_DEPTH),
                Err(EngineError::MaxDepthExceeded(_))
            ));

            // The filter is evaluated one level deeper too, and it runs
            // before the body. A literal body keeps the body path within the
            // limit, so only the filter's own increment can trip this.
            let with_filter = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: lit(1i64),
                filter: Some(ActionValue::Operation(Box::new(ActionOperation::Not {
                    value: lit(false),
                }))),
                combine: Some(CombineOp::Add),
            };
            // One below the ceiling is the depth that pins the filter's own
            // increment: the FOREACH itself still fits, and only because the
            // filter is evaluated a level deeper does its nested operation run
            // over the limit.
            assert!(matches!(
                execute_operation(&with_filter, &context, MAX_OPERATION_DEPTH - 1),
                Err(EngineError::MaxDepthExceeded(_))
            ));
            // A step further down and both fit, so the rejection above is the
            // depth limit and not the shape of the operation.
            assert_eq!(
                execute_operation(&with_filter, &context, MAX_OPERATION_DEPTH - 2).unwrap(),
                Value::Int(1)
            );
        }

        #[test]
        fn test_foreach_needs_a_scoped_resolver() {
            // A resolver without child scopes says so, rather than failing later
            // with a confusing "variable not found" on the loop variable.
            let resolver = TestResolver::new().with_var("items", Value::Array(vec![Value::Int(1)]));
            let op = ActionOperation::Foreach {
                collection: var("items"),
                as_name: Some("x".to_string()),
                body: var("x"),
                filter: None,
                combine: Some(CombineOp::Add),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            assert!(
                matches!(&err, EngineError::InvalidOperation(m) if m.contains("child scopes")),
                "got {err:?}"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Null Checking Tests (IS_NULL / NOT_NULL)
    // -------------------------------------------------------------------------

    mod null_checks {
        use super::*;

        #[test]
        fn test_is_null_on_missing_value() {
            // A parameter that resolves to Null: "niet opgegeven" is true.
            let resolver = TestResolver::new().with_var("inkomen", Value::Null);
            let op = ActionOperation::IsNull {
                subject: var("inkomen"),
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_is_null_false_on_present_value() {
            // Zero is a value, not a missing value: IS_NULL must not confuse the two.
            let resolver = TestResolver::new().with_var("inkomen", 0i64);
            let op = ActionOperation::IsNull {
                subject: var("inkomen"),
            };

            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_not_null_is_the_complement() {
            // NOT_NULL answers the opposite question on the same two inputs.
            let resolver = TestResolver::new()
                .with_var("leeg", Value::Null)
                .with_var("gevuld", 0i64);

            let on_null = ActionOperation::NotNull {
                subject: var("leeg"),
            };
            assert_eq!(
                execute_operation(&on_null, &resolver, 0).unwrap(),
                Value::Bool(false)
            );

            let on_value = ActionOperation::NotNull {
                subject: var("gevuld"),
            };
            assert_eq!(
                execute_operation(&on_value, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_null_check_propagates_untranslatable() {
            // RFC-012: an untranslatable subject is neither null nor not-null;
            // the taint flows through instead of collapsing to a boolean.
            let tainted = Value::Untranslatable {
                article: "3".to_string(),
                construct: "naar het oordeel van".to_string(),
            };
            let resolver = TestResolver::new().with_var("onvertaalbaar", tainted.clone());
            let op = ActionOperation::IsNull {
                subject: var("onvertaalbaar"),
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // Date Operations Tests
    // -------------------------------------------------------------------------

    mod date_operations {
        use super::*;

        #[test]
        fn test_age_exact_birthday() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("1990-03-15"),
                reference_date: lit("2025-03-15"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_age_before_birthday() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("1990-03-15"),
                reference_date: lit("2025-03-14"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(34));
        }

        #[test]
        fn test_age_feb29_birthday_on_non_leap_year() {
            // Per Dutch law (BW art. 1:2): Feb 28 counts as birthday
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("2000-02-29"),
                reference_date: lit("2001-02-28"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(1));
        }

        #[test]
        fn test_age_feb29_birthday_before_feb28() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("2000-02-29"),
                reference_date: lit("2001-02-27"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(0));
        }

        #[test]
        fn test_age_feb29_birthday_on_leap_year() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Age {
                date_of_birth: lit("2000-02-29"),
                reference_date: lit("2004-02-29"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(4));
        }

        #[test]
        fn test_age_with_variables() {
            let resolver = TestResolver::new()
                .with_var("birth_date", "1990-03-15")
                .with_var("ref_date", "2025-03-15");

            let op = ActionOperation::Age {
                date_of_birth: var("birth_date"),
                reference_date: var("ref_date"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(35));
        }

        #[test]
        fn test_age_with_object_date() {
            let mut date_obj = BTreeMap::new();
            date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            date_obj.insert("year".to_string(), Value::Int(2025));
            date_obj.insert("month".to_string(), Value::Int(1));
            date_obj.insert("day".to_string(), Value::Int(1));

            let resolver = TestResolver::new()
                .with_var("referencedate", Value::Object(date_obj))
                .with_var("geboortedatum", Value::String("2005-01-01".to_string()));

            let op = ActionOperation::Age {
                date_of_birth: var("geboortedatum"),
                reference_date: var("referencedate"),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::Int(20));
        }

        #[test]
        fn test_date_add_days() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-10"),
                years: None,
                months: None,
                days: Some(lit(5i64)),
                weeks: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-15".to_string()));
        }

        #[test]
        fn test_date_add_weeks() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-01"),
                years: None,
                months: None,
                days: None,
                weeks: Some(lit(2i64)),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-15".to_string()));
        }

        #[test]
        fn test_date_add_days_and_weeks() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-01"),
                years: None,
                months: None,
                days: Some(lit(3i64)),
                weeks: Some(lit(1i64)),
            };

            // 1 week + 3 days = 10 days from Jan 1 = Jan 11
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-11".to_string()));
        }

        #[test]
        fn test_date_add_negative_days() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-15"),
                years: None,
                months: None,
                days: Some(lit(-5i64)),
                weeks: None,
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-01-10".to_string()));
        }

        #[test]
        fn test_date_add_months() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-03-15"),
                years: None,
                months: Some(lit(2i64)),
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-05-15".to_string()));
        }

        #[test]
        fn test_date_add_months_end_of_month_clamping() {
            // Jan 31 + 1 month = Feb 28 (not March 1)
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-31"),
                years: None,
                months: Some(lit(1i64)),
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-02-28".to_string()));
        }

        #[test]
        fn test_date_add_months_end_of_month_leap_year() {
            // Jan 31 + 1 month in leap year = Feb 29
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2024-01-31"),
                years: None,
                months: Some(lit(1i64)),
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2024-02-29".to_string()));
        }

        #[test]
        fn test_date_add_months_negative() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-03-31"),
                years: None,
                months: Some(lit(-1i64)),
                weeks: None,
                days: None,
            };
            // Mar 31 - 1 month = Feb 28
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-02-28".to_string()));
        }

        #[test]
        fn test_date_add_months_six_months() {
            // "binnen zes maanden" from Aug 31
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-08-31"),
                years: None,
                months: Some(lit(6i64)),
                weeks: None,
                days: None,
            };
            // Aug 31 + 6 months = Feb 28
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2026-02-28".to_string()));
        }

        #[test]
        fn test_date_add_years() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-06-15"),
                years: Some(lit(2i64)),
                months: None,
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2027-06-15".to_string()));
        }

        #[test]
        fn test_date_add_years_leap_day() {
            // Feb 29 + 1 year = Feb 28 (non-leap year)
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2024-02-29"),
                years: Some(lit(1i64)),
                months: None,
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-02-28".to_string()));
        }

        #[test]
        fn test_date_add_years_leap_day_to_leap_year() {
            // Feb 29 + 4 years = Feb 29 (another leap year)
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2024-02-29"),
                years: Some(lit(4i64)),
                months: None,
                weeks: None,
                days: None,
            };
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2028-02-29".to_string()));
        }

        #[test]
        fn test_date_add_combined_years_months_weeks_days() {
            // 2025-01-15 + 1 year + 2 months + 1 week + 3 days
            let resolver = TestResolver::new();
            let op = ActionOperation::DateAdd {
                date: lit("2025-01-15"),
                years: Some(lit(1i64)),
                months: Some(lit(2i64)),
                weeks: Some(lit(1i64)),
                days: Some(lit(3i64)),
            };
            // +1y = 2026-01-15, +2m = 2026-03-15, +1w = 2026-03-22, +3d = 2026-03-25
            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2026-03-25".to_string()));
        }

        #[test]
        fn test_date_construct() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Date {
                year: lit(2025i64),
                month: lit(3i64),
                day: lit(15i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2025-03-15".to_string()));
        }

        #[test]
        fn test_date_construct_leap_year() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Date {
                year: lit(2024i64),
                month: lit(2i64),
                day: lit(29i64),
            };

            let result = execute_operation(&op, &resolver, 0).unwrap();
            assert_eq!(result, Value::String("2024-02-29".to_string()));
        }

        #[test]
        fn test_date_construct_invalid() {
            let resolver = TestResolver::new();
            let op = ActionOperation::Date {
                year: lit(2025i64),
                month: lit(2i64),
                day: lit(30i64), // Feb 30 doesn't exist
            };

            let result = execute_operation(&op, &resolver, 0);
            assert!(matches!(result, Err(EngineError::InvalidOperation(_))));
        }

        #[test]
        fn test_day_of_week() {
            let resolver = TestResolver::new();

            // 2025-01-06 is a Monday (weekday 0)
            let op = ActionOperation::DayOfWeek {
                date: lit("2025-01-06"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(0));

            // 2025-01-12 is a Sunday (weekday 6)
            let op2 = ActionOperation::DayOfWeek {
                date: lit("2025-01-12"),
            };
            assert_eq!(
                execute_operation(&op2, &resolver, 0).unwrap(),
                Value::Int(6)
            );
        }
    }

    // -------------------------------------------------------------------------
    // Date comparison Tests (RFC-021 route A: type-safe ordered comparison)
    // -------------------------------------------------------------------------

    mod date_comparison {
        use super::*;

        #[test]
        fn test_greater_than_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-01-15"),
                value: lit("2025-01-10"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThan {
                subject: lit("2024-12-31"),
                value: lit("2025-01-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_greater_than_or_equal_dates_equal() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThanOrEqual {
                subject: lit("2025-06-01"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_less_than_or_equal_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThanOrEqual {
                subject: lit("2025-06-02"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_dates() {
            // ISO date string equality is equivalent to date equality (canonical format).
            let resolver = TestResolver::new();
            let op = ActionOperation::Equals {
                subject: lit("2025-03-01"),
                value: lit("2025-03-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_compare_peildatum_against_threshold() {
            // The thread's motivating case: peildatum (referencedate.iso) vs a date literal.
            let resolver = TestResolver::new().with_var("peildatum", "2025-07-01");
            let op = ActionOperation::GreaterThan {
                subject: var("peildatum"),
                value: lit("2025-01-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_numeric_comparison_still_works() {
            // Route A must not regress integer/float comparison.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(50i64),
                value: lit(42i64),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_mixed_number_and_date_errors() {
            // A number vs a date is a type error, not a silent coercion.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(2025i64),
                value: lit("2025-01-01"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_ordered_comparison_with_an_absent_operand_is_an_error() {
            // "More than nothing" is not a question the law asked: ordering an
            // absence is an error until the law tests for it (RFC-036).
            let resolver = TestResolver::new().with_var("geen", Value::Null);
            for (op, name) in [
                (
                    ActionOperation::GreaterThan {
                        subject: var("geen"),
                        value: lit(10i64),
                    },
                    "GREATER_THAN",
                ),
                (
                    ActionOperation::LessThan {
                        subject: lit(10i64),
                        value: var("geen"),
                    },
                    "LESS_THAN",
                ),
                (
                    ActionOperation::GreaterThanOrEqual {
                        subject: var("geen"),
                        value: lit("2025-01-01"),
                    },
                    "GREATER_THAN_OR_EQUAL",
                ),
                (
                    ActionOperation::LessThanOrEqual {
                        subject: var("geen"),
                        value: var("geen"),
                    },
                    "LESS_THAN_OR_EQUAL",
                ),
            ] {
                let err = execute_operation(&op, &resolver, 0).unwrap_err();
                assert!(
                    matches!(&err, EngineError::AbsentOperand { operation } if operation == name),
                    "{name}: got {err:?}"
                );
            }
        }

        #[test]
        fn test_ordered_comparison_with_an_unknown_operand_is_unknown() {
            // A fact nobody has yet cannot be ordered; the result carries the
            // facts of both unknown operands, in order, once each (RFC-036).
            let resolver = TestResolver::new()
                .with_var("huur", unknown_fact("huur"))
                .with_var("grens", unknown_fact("grens"));
            let gt = ActionOperation::GreaterThan {
                subject: var("huur"),
                value: lit(10i64),
            };
            assert_eq!(
                missing_names(&execute_operation(&gt, &resolver, 0).unwrap()),
                vec!["huur"]
            );
            let lt = ActionOperation::LessThan {
                subject: var("grens"),
                value: var("huur"),
            };
            assert_eq!(
                missing_names(&execute_operation(&lt, &resolver, 0).unwrap()),
                vec!["grens", "huur"]
            );
            // Untranslatable beats unknown.
            let tainted = Value::Untranslatable {
                article: "3".to_string(),
                construct: "redelijk".to_string(),
            };
            let resolver = resolver.with_var("vaag", tainted.clone());
            let mixed = ActionOperation::GreaterThan {
                subject: var("huur"),
                value: var("vaag"),
            };
            assert_eq!(execute_operation(&mixed, &resolver, 0).unwrap(), tainted);
        }

        #[test]
        fn test_non_date_string_errors() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("hello"),
                value: lit("world"),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("number or date"), "unexpected message: {msg}");
        }

        #[test]
        fn test_compare_referencedate_object_against_string() {
            // $referencedate resolves to an object; comparing it against a date string
            // must work via parse_date's object support.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-07-01"));
            let op = ActionOperation::GreaterThan {
                subject: var("referencedate"),
                value: lit("2025-01-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_equals_referencedate_object_equals_date_string() {
            // RFC-021: the object form and the string form of the same date are equal.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-03-01"));
            let op = ActionOperation::Equals {
                subject: var("referencedate"),
                value: lit("2025-03-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_equals_referencedate_object_differs_from_other_date() {
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-03-01"));
            let op = ActionOperation::Equals {
                subject: var("referencedate"),
                value: lit("2025-03-02"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_non_date_strings_unaffected() {
            // The date-aware fallback must not change plain string equality.
            let resolver = TestResolver::new();
            let same = ActionOperation::Equals {
                subject: lit("aanvraag"),
                value: lit("aanvraag"),
            };
            let diff = ActionOperation::Equals {
                subject: lit("aanvraag"),
                value: lit("bezwaar"),
            };
            assert_eq!(
                execute_operation(&same, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                execute_operation(&diff, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_strict_greater_than_is_false_on_equal_dates() {
            // Boundary day: a deadline check must not flip on the cutoff date
            // itself. Pins the strictness of the date path.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-06-01"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_strict_less_than_is_false_on_equal_dates() {
            let resolver = TestResolver::new();
            let op = ActionOperation::LessThan {
                subject: lit("2025-06-01"),
                value: lit("2025-06-01"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_not_equals_dates_mixed_forms() {
            // NOT_EQUALS inverts the date-aware fallback for the mixed form.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-03-01"));
            let same = ActionOperation::NotEquals {
                subject: var("referencedate"),
                value: lit("2025-03-01"),
            };
            let diff = ActionOperation::NotEquals {
                subject: var("referencedate"),
                value: lit("2025-03-02"),
            };
            assert_eq!(
                execute_operation(&same, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
            assert_eq!(
                execute_operation(&diff, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_equals_objects_sharing_iso_stay_structural() {
            // Object↔object keeps structural equality: two objects that merely
            // share an `iso` field are NOT equal (the date fallback is scoped to
            // the mixed string/object form).
            let mut left = BTreeMap::new();
            left.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            left.insert("type".to_string(), Value::String("aanvraag".to_string()));
            let mut right = BTreeMap::new();
            right.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
            right.insert("type".to_string(), Value::String("bezwaar".to_string()));

            let resolver = TestResolver::new()
                .with_var("left", Value::Object(left))
                .with_var("right", Value::Object(right));
            let op = ActionOperation::Equals {
                subject: var("left"),
                value: var("right"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_identical_referencedate_objects_still_equal() {
            // Two structurally identical reference-date objects stay equal via
            // plain structural equality; the fallback is not needed for them.
            let resolver = TestResolver::new()
                .with_var("a", date_obj("2025-03-01"))
                .with_var("b", date_obj("2025-03-01"));
            let op = ActionOperation::Equals {
                subject: var("a"),
                value: var("b"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn test_mixed_error_names_both_kinds() {
            // A number against a date reports the mixed pair, not a misleading
            // complaint about one (individually valid) operand.
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit(2025i64),
                value: lit("2025-01-01"),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("two numbers or two dates")
                    && msg.contains("a number compared against a date"),
                "unexpected message: {msg}"
            );
        }

        #[test]
        fn test_error_blames_the_invalid_operand() {
            // A valid date compared against a bool must blame the bool, not the
            // (perfectly fine) date subject.
            let resolver = TestResolver::new().with_var("flag", Value::Bool(true));
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-01-01"),
                value: var("flag"),
            };
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("number or date") && msg.contains("bool"),
                "unexpected message: {msg}"
            );
        }

        #[test]
        fn test_tainted_date_operand_propagates() {
            // RFC-012: an Untranslatable operand flows through the ordered
            // comparison as a value instead of failing the calculation.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = ActionOperation::GreaterThan {
                subject: var("tainted"),
                value: lit("2025-01-01"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // DATE_DIFF Tests (RFC-021 route B: explicit unit argument)
    // -------------------------------------------------------------------------

    mod date_diff {
        use super::*;

        #[test]
        fn test_diff_in_days() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-01-11"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(10)
            );
        }

        #[test]
        fn test_diff_in_days_negative_when_to_before_from() {
            // Sign convention: positive when `to >= from`, negative otherwise.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-11"),
                to: lit("2025-01-01"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(-10)
            );
        }

        #[test]
        fn test_diff_in_years() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-06-01"),
                to: lit("2025-06-01"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(25)
            );
        }

        #[test]
        fn test_diff_in_years_before_anniversary() {
            // 2025-05-31 has not yet reached the June 1 anniversary → 24 complete years.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-06-01"),
                to: lit("2025-05-31"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(24)
            );
        }

        #[test]
        fn test_diff_in_months() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-04-01"),
                unit: lit("months"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
        }

        #[test]
        fn test_diff_in_months_end_of_month_clamp() {
            // Jan 31 → Feb 28 counts as one complete month.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-31"),
                to: lit("2025-02-28"),
                unit: lit("months"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }

        #[test]
        fn test_diff_in_months_counts_only_complete_months() {
            // 15 January → 10 April is two whole months plus a bit: the day of
            // the month has not been reached again, so the third month does not
            // count. Termijnen in maanden lopen tot dezelfde dag in de maand.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-15"),
                to: lit("2025-04-10"),
                unit: lit("months"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn test_diff_in_months_spans_calendar_years() {
            // Months keep counting across the year boundary: 2 years and 3
            // months is 27 months, not 3.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2023-01-01"),
                to: lit("2025-04-01"),
                unit: lit("months"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(27)
            );
        }

        #[test]
        fn test_diff_in_years_february_start_is_not_treated_as_feb29() {
            // The Feb-29 rule is for Feb 29 only. A 10 February start reaches its
            // anniversary on 10 February, so 15 February is past it: 25 whole
            // years, not 24.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-02-10"),
                to: lit("2025-02-15"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(25)
            );
        }

        #[test]
        fn test_diff_in_years_feb29_anniversary_falls_on_feb29_in_a_leap_year() {
            // The fallback to 28 February exists only for years that have no 29
            // February. 2024 has one, so on 28 February 2024 the anniversary has
            // not been reached yet: 23 whole years.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-02-29"),
                to: lit("2024-02-28"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(23)
            );
        }

        #[test]
        fn test_diff_with_variables() {
            let resolver = TestResolver::new()
                .with_var("start", "2025-01-01")
                .with_var("eind", "2025-12-31");
            let op = ActionOperation::DateDiff {
                from: var("start"),
                to: var("eind"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(364)
            );
        }

        #[test]
        fn test_diff_invalid_unit_errors() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-02-01"),
                unit: lit("weeks"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_diff_invalid_date_errors() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("not-a-date"),
                to: lit("2025-02-01"),
                unit: lit("days"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_diff_non_canonical_date_errors() {
            // Non-zero-padded dates are rejected so EQUALS (string equality) and the
            // ordering operators can never disagree on the same literal.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-1-1"),
                to: lit("2025-02-01"),
                unit: lit("days"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_diff_with_referencedate_object() {
            // `to` is the $referencedate object form, `from` a date string.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-01-11"));
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: var("referencedate"),
                unit: lit("days"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(10)
            );
        }

        #[test]
        fn test_diff_in_days_across_leap_day() {
            // 2024 is a leap year, so Feb has 29 days: Feb 28 → Mar 1 is 2 days.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2024-02-28"),
                to: lit("2024-03-01"),
                unit: lit("days"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(2));
        }

        #[test]
        fn test_diff_in_years_feb29_birthday_non_leap_year() {
            // A Feb 29 start reaches its anniversary on Feb 28 in a non-leap year
            // (BW art. 1:2), so 2000-02-29 → 2025-02-28 is 25 whole years.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2000-02-29"),
                to: lit("2025-02-28"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(25)
            );
        }

        #[test]
        fn test_diff_in_months_negative_when_to_before_from() {
            // The sign convention holds for calendar units too, including the
            // end-of-month clamp in the reverse direction.
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-03-31"),
                to: lit("2025-02-28"),
                unit: lit("months"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(-1)
            );
        }

        #[test]
        fn test_diff_in_years_negative_when_to_before_from() {
            let resolver = TestResolver::new();
            let op = ActionOperation::DateDiff {
                from: lit("2025-06-01"),
                to: lit("2020-06-01"),
                unit: lit("years"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(-5)
            );
        }

        #[test]
        fn test_tainted_date_operand_propagates() {
            // RFC-012: an Untranslatable `from` flows through as a value.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = ActionOperation::DateDiff {
                from: var("tainted"),
                to: lit("2025-01-01"),
                unit: lit("days"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }

        #[test]
        fn test_tainted_unit_propagates() {
            // The schema allows `in: $variable`; when that variable resolves to
            // an Untranslatable, the taint propagates instead of erroring.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted_unit", tainted.clone());
            let op = ActionOperation::DateDiff {
                from: lit("2025-01-01"),
                to: lit("2025-01-11"),
                unit: var("tainted_unit"),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // DATE_PART Tests (the inverse of DATE; the unit is a component, not a count)
    // -------------------------------------------------------------------------

    mod date_part {
        use super::*;

        fn date_part(date: ActionValue, part: &str) -> ActionOperation {
            ActionOperation::DatePart {
                date,
                part: part.to_string(),
            }
        }

        #[test]
        fn test_part_year() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(2025)
            );
        }

        #[test]
        fn test_part_month() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "month");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
        }

        #[test]
        fn test_part_day() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "day");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(14)
            );
        }

        #[test]
        fn test_part_day_on_first_of_month() {
            // Awir art. 49 asks whether a change fell *after* the first of the
            // month, so day 1 has to come back as 1 and not as 0.
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-01"), "day");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }

        #[test]
        fn test_part_of_leap_day() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2024-02-29"), "day");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(29)
            );
        }

        #[test]
        fn test_part_reads_referencedate_object() {
            // `$referencedate` resolves to an object with an `iso` field; the
            // shared date parser has to accept it here too.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-07-04"));
            let op = date_part(var("referencedate"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(2025)
            );
        }

        #[test]
        fn test_part_of_nested_operation() {
            // The canonical form nests inside `date:`, so a computed date has to
            // work as the operand.
            let resolver = TestResolver::new();
            let op = date_part(
                ActionValue::Operation(Box::new(ActionOperation::DateAdd {
                    date: lit("2007-11-30"),
                    years: Some(lit(18)),
                    months: None,
                    weeks: None,
                    days: None,
                })),
                "year",
            );
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Int(2025)
            );
        }

        #[test]
        fn test_plural_unit_from_date_diff_is_rejected_by_name() {
            // The weak spot of the design: singular here, plural in DATE_DIFF.
            // An author who copies the wrong one gets told which words are valid.
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "months");
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("DATE_PART"),
                "message names the operation: {msg}"
            );
            assert!(msg.contains("'months'"), "message quotes the input: {msg}");
            assert!(
                msg.contains("'month'"),
                "message lists the valid words: {msg}"
            );
        }

        #[test]
        fn test_weekday_is_not_a_date_part() {
            // A weekday is no part of the year-month-day decomposition, so it is
            // out of scope here and stays DAY_OF_WEEK.
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-03-14"), "weekday");
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_non_canonical_date_rejected() {
            let resolver = TestResolver::new();
            let op = date_part(lit("2025-3-14"), "day");
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_tainted_date_propagates() {
            // RFC-012: an Untranslatable date flows through as a value.
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = date_part(var("tainted"), "year");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }
    }

    // -------------------------------------------------------------------------
    // START_OF Tests (RFC-024: truncation is a named operation, never implicit)
    // -------------------------------------------------------------------------

    mod start_of {
        use super::*;

        fn start_of(date: ActionValue, unit: &str) -> ActionOperation {
            ActionOperation::StartOf {
                date,
                unit: unit.to_string(),
            }
        }

        fn nested(op: ActionOperation) -> ActionValue {
            ActionValue::Operation(Box::new(op))
        }

        /// The canonical "first day of the month following" form: truncate first,
        /// add afterwards.
        fn first_of_next_month(date: ActionValue) -> ActionOperation {
            ActionOperation::DateAdd {
                date: nested(start_of(date, "month")),
                years: None,
                months: Some(lit(1)),
                weeks: None,
                days: None,
            }
        }

        #[test]
        fn test_start_of_month() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-14"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-03-01".to_string())
            );
        }

        #[test]
        fn test_start_of_month_is_idempotent_on_the_first() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-01"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-03-01".to_string())
            );
        }

        #[test]
        fn test_start_of_month_on_leap_day() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2024-02-29"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2024-02-01".to_string())
            );
        }

        #[test]
        fn test_start_of_year() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-12-31"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-01-01".to_string())
            );
        }

        #[test]
        fn test_start_of_year_is_idempotent_on_new_year() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-01-01"), "year");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-01-01".to_string())
            );
        }

        #[test]
        fn test_start_of_returns_canonical_form() {
            // Single-digit months and days come back zero-padded, so the result
            // compares equal under EQUALS as well as under ordering (RFC-021).
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-01-09"), "month");
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-01-01".to_string())
            );
        }

        #[test]
        fn test_day_is_not_a_truncation_unit() {
            // Truncating to a day is the identity; offering it would invite the
            // reading that START_OF normalizes dates.
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-14"), "day");
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_plural_unit_from_date_diff_is_rejected_by_name() {
            let resolver = TestResolver::new();
            let op = start_of(lit("2025-03-14"), "months");
            let err = execute_operation(&op, &resolver, 0).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("START_OF"),
                "message names the operation: {msg}"
            );
            assert!(msg.contains("'months'"), "message quotes the input: {msg}");
            assert!(
                msg.contains("'month'"),
                "message lists the valid words: {msg}"
            );
        }

        #[test]
        fn test_tainted_date_propagates() {
            let tainted = Value::Untranslatable {
                article: "1".to_string(),
                construct: "test".to_string(),
            };
            let resolver = TestResolver::new().with_var("tainted", tainted.clone());
            let op = start_of(var("tainted"), "month");
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), tainted);
        }

        // --- the canonical "first day of the month following" pattern ---

        #[test]
        fn test_first_of_next_month_midmonth() {
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-03-15"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-04-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_on_the_first() {
            // Awir art. 49 default branch: a change on the first counts in that
            // month itself, so this form is only reached from the other branch —
            // and it still has to move exactly one month.
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-03-01"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-04-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_on_the_last_day() {
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-03-31"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-04-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_on_31_january_never_touches_the_clamp() {
            // Truncate-then-add gives 1 February. Add-then-truncate would route
            // through the DATE_ADD clamp (31 Jan + 1 month = 28 Feb) and land on
            // the same day by coincidence; this test pins the order that does not
            // depend on that coincidence.
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-01-31"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2025-02-01".to_string())
            );
        }

        #[test]
        fn test_first_of_next_month_crosses_the_year() {
            let resolver = TestResolver::new();
            let op = first_of_next_month(lit("2025-12-17"));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2026-01-01".to_string())
            );
        }

        #[test]
        fn test_premieplicht_from_the_month_after_turning_eighteen() {
            // Zvw art. 16 lid 2 on a leap-day birth: 18 years later is 28 February
            // 2026 (the DATE_ADD clamp, BW art. 1:2), truncated 1 February 2026,
            // plus one month 1 March 2026.
            let resolver = TestResolver::new();
            let op = first_of_next_month(nested(ActionOperation::DateAdd {
                date: lit("2008-02-29"),
                years: Some(lit(18)),
                months: None,
                weeks: None,
                days: None,
            }));
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::String("2026-03-01".to_string())
            );
        }

        #[test]
        fn test_start_of_year_feeds_date_part() {
            // START_OF is derivable from DATE_PART + DATE (RFC-024 rejects that
            // as the way to write it); the two still have to agree.
            let resolver = TestResolver::new();
            let op = ActionOperation::DatePart {
                date: nested(start_of(lit("2025-12-31"), "year")),
                part: "month".to_string(),
            };
            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(1));
        }
    }

    // -------------------------------------------------------------------------
    // Canonical date-form Tests (RFC-021: keep EQUALS and ordering consistent)
    // -------------------------------------------------------------------------

    mod canonical_dates {
        use super::*;

        #[test]
        fn test_non_canonical_date_rejected_by_comparison() {
            let resolver = TestResolver::new();
            let op = ActionOperation::GreaterThan {
                subject: lit("2025-1-1"),
                value: lit("2025-01-01"),
            };
            assert!(execute_operation(&op, &resolver, 0).is_err());
        }

        #[test]
        fn test_equals_non_canonical_string_stays_structural() {
            // The EQUALS date fallback does not fire when a side fails canonical
            // parsing: "2025-1-1" against the object form of 2025-01-01 is simply
            // unequal, without an error (RFC-021 implementation notes). Pins the
            // one place where a non-canonical date is not rejected loudly.
            let resolver = TestResolver::new().with_var("referencedate", date_obj("2025-01-01"));
            let op = ActionOperation::Equals {
                subject: lit("2025-1-1"),
                value: var("referencedate"),
            };
            assert_eq!(
                execute_operation(&op, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn test_equals_and_ordering_agree_on_canonical_dates() {
            // For canonical (zero-padded) ISO dates, "a == b" iff neither "a > b" nor "a < b".
            let resolver = TestResolver::new();
            let a = "2025-03-01";
            let b = "2025-03-01";

            let eq = ActionOperation::Equals {
                subject: lit(a),
                value: lit(b),
            };
            let gt = ActionOperation::GreaterThan {
                subject: lit(a),
                value: lit(b),
            };
            let lt = ActionOperation::LessThan {
                subject: lit(a),
                value: lit(b),
            };
            assert_eq!(
                execute_operation(&eq, &resolver, 0).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                execute_operation(&gt, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
            assert_eq!(
                execute_operation(&lt, &resolver, 0).unwrap(),
                Value::Bool(false)
            );
        }
    }

    // -------------------------------------------------------------------------
    // Absent versus unknown (RFC-036)
    // -------------------------------------------------------------------------

    /// One test per row of the RFC-036 operation table, each with the Unknown
    /// case and the Null case: an unknown operand propagates (as the union of
    /// missing facts), an absent operand is a value that can be tested but not
    /// calculated with or decided on.
    mod absent_and_unknown {
        use super::*;

        fn resolver() -> TestResolver {
            TestResolver::new()
                .with_var("geen", Value::Null)
                .with_var("huur", unknown_fact("huur"))
                .with_var("partner", unknown_fact("partner_bsn"))
                .with_var("vaag", taint())
        }

        fn taint() -> Value {
            Value::Untranslatable {
                article: "3".to_string(),
                construct: "naar redelijkheid".to_string(),
            }
        }

        fn absent(err: EngineError, op: &str) {
            assert!(
                matches!(&err, EngineError::AbsentOperand { operation } if operation == op),
                "expected AbsentOperand for {op}, got {err:?}"
            );
        }

        #[test]
        fn equals_tests_absence_and_propagates_unknown() {
            let r = resolver();
            let run = |op: ActionOperation| execute_operation(&op, &r, 0);
            // Null is a value here: this is the absence test the corpus uses.
            assert_eq!(
                run(ActionOperation::Equals {
                    subject: var("geen"),
                    value: lit(Value::Null),
                })
                .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                run(ActionOperation::Equals {
                    subject: var("geen"),
                    value: lit(5i64),
                })
                .unwrap(),
                Value::Bool(false)
            );
            assert_eq!(
                run(ActionOperation::NotEquals {
                    subject: var("geen"),
                    value: lit(Value::Null),
                })
                .unwrap(),
                Value::Bool(false)
            );
            // Whether an unknown equals anything, null included, is unknown.
            let unknown = run(ActionOperation::Equals {
                subject: var("huur"),
                value: lit(Value::Null),
            })
            .unwrap();
            assert_eq!(missing_names(&unknown), vec!["huur"]);
            let both = run(ActionOperation::NotEquals {
                subject: var("huur"),
                value: var("partner"),
            })
            .unwrap();
            assert_eq!(missing_names(&both), vec!["huur", "partner_bsn"]);
            // Untranslatable beats unknown.
            assert_eq!(
                run(ActionOperation::Equals {
                    subject: var("huur"),
                    value: var("vaag"),
                })
                .unwrap(),
                taint()
            );
        }

        #[test]
        fn arithmetic_propagates_unknown_and_rejects_absence() {
            let r = resolver();
            type Case = (&'static str, fn(Vec<ActionValue>) -> ActionOperation);
            let cases: Vec<Case> = vec![
                ("ADD", |values| ActionOperation::Add { values }),
                ("SUBTRACT", |values| ActionOperation::Subtract { values }),
                ("MULTIPLY", |values| ActionOperation::Multiply { values }),
                ("DIVIDE", |values| ActionOperation::Divide { values }),
                ("MIN/MAX", |values| ActionOperation::Min { values }),
                ("MIN/MAX", |values| ActionOperation::Max { values }),
            ];
            for (name, build) in cases {
                // Unknown: the union of the unknown operands' facts, once each.
                let op = build(vec![lit(5i64), var("huur"), var("partner"), var("huur")]);
                let result = execute_operation(&op, &r, 0).unwrap();
                assert_eq!(
                    missing_names(&result),
                    vec!["huur", "partner_bsn"],
                    "{name}"
                );
                // Untranslatable beats unknown.
                let op = build(vec![var("huur"), var("vaag")]);
                assert_eq!(execute_operation(&op, &r, 0).unwrap(), taint(), "{name}");
                // Null: an absence is not an amount.
                let op = build(vec![lit(5i64), var("geen")]);
                absent(execute_operation(&op, &r, 0).unwrap_err(), name);
            }
        }

        #[test]
        fn rounding_propagates_unknown_and_rejects_absence() {
            let r = resolver();
            type Case = (&'static str, fn(ActionValue) -> ActionOperation);
            let cases: Vec<Case> = vec![
                ("ROUND", |value| ActionOperation::Round {
                    value,
                    precision: 0,
                }),
                ("CEIL", |value| ActionOperation::Ceil {
                    value,
                    precision: 0,
                }),
                ("FLOOR", |value| ActionOperation::Floor {
                    value,
                    precision: 0,
                }),
            ];
            for (name, build) in cases {
                let result = execute_operation(&build(var("huur")), &r, 0).unwrap();
                assert_eq!(missing_names(&result), vec!["huur"], "{name}");
                absent(
                    execute_operation(&build(var("geen")), &r, 0).unwrap_err(),
                    name,
                );
            }
        }

        #[test]
        fn and_is_kleene_over_unknown_and_rejects_absence() {
            let r = resolver();
            let run = |conditions: Vec<ActionValue>| {
                execute_operation(&ActionOperation::And { conditions }, &r, 0)
            };
            // A definite false decides, whatever the unknown turns out to be.
            assert_eq!(
                run(vec![var("huur"), lit(false)]).unwrap(),
                Value::Bool(false)
            );
            // Otherwise unknown, for every unknown operand.
            assert_eq!(
                missing_names(&run(vec![lit(true), var("huur"), var("partner")]).unwrap()),
                vec!["huur", "partner_bsn"]
            );
            // Untranslatable beats unknown, a false beats both.
            assert_eq!(run(vec![var("huur"), var("vaag")]).unwrap(), taint());
            assert_eq!(
                run(vec![var("vaag"), var("huur"), lit(false)]).unwrap(),
                Value::Bool(false)
            );
            // An absence is not a verdict.
            absent(run(vec![lit(true), var("geen")]).unwrap_err(), "AND");
            // The guard idiom: a false earlier stops the evaluation, so the law
            // can test for absence before it orders the absent value.
            let guarded = vec![
                ActionValue::Operation(Box::new(ActionOperation::NotEquals {
                    subject: var("geen"),
                    value: lit(Value::Null),
                })),
                ActionValue::Operation(Box::new(ActionOperation::GreaterThan {
                    subject: var("geen"),
                    value: lit(500i64),
                })),
            ];
            assert_eq!(run(guarded).unwrap(), Value::Bool(false));
        }

        #[test]
        fn or_is_kleene_over_unknown_and_rejects_absence() {
            let r = resolver();
            let run = |conditions: Vec<ActionValue>| {
                execute_operation(&ActionOperation::Or { conditions }, &r, 0)
            };
            assert_eq!(
                run(vec![var("huur"), lit(true)]).unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                missing_names(&run(vec![lit(false), var("partner"), var("huur")]).unwrap()),
                vec!["partner_bsn", "huur"]
            );
            assert_eq!(run(vec![var("huur"), var("vaag")]).unwrap(), taint());
            assert_eq!(
                run(vec![var("vaag"), var("huur"), lit(true)]).unwrap(),
                Value::Bool(true)
            );
            absent(run(vec![lit(false), var("geen")]).unwrap_err(), "OR");
            // The guard idiom for OR: a true earlier stops the evaluation.
            let guarded = vec![
                ActionValue::Operation(Box::new(ActionOperation::Equals {
                    subject: var("geen"),
                    value: lit(Value::Null),
                })),
                ActionValue::Operation(Box::new(ActionOperation::GreaterThan {
                    subject: var("geen"),
                    value: lit(500i64),
                })),
            ];
            assert_eq!(run(guarded).unwrap(), Value::Bool(true));
        }

        #[test]
        fn not_propagates_unknown_and_rejects_absence() {
            let r = resolver();
            let result =
                execute_operation(&ActionOperation::Not { value: var("huur") }, &r, 0).unwrap();
            assert_eq!(missing_names(&result), vec!["huur"]);
            assert_eq!(
                execute_operation(&ActionOperation::Not { value: var("vaag") }, &r, 0).unwrap(),
                taint()
            );
            absent(
                execute_operation(&ActionOperation::Not { value: var("geen") }, &r, 0).unwrap_err(),
                "NOT",
            );
        }

        #[test]
        fn if_with_an_unknown_condition_is_unknown_and_evaluates_nothing_else() {
            let r = resolver();
            // The `then`, the later case and the default all reference a variable
            // that does not exist: evaluating any of them would be an error, so
            // the unknown result proves none of them was evaluated.
            let op = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(false),
                        then: var("bestaat_niet"),
                    },
                    Case {
                        when: var("huur"),
                        then: var("bestaat_niet"),
                    },
                    Case {
                        when: var("bestaat_niet"),
                        then: lit("later"),
                    },
                ],
                default: Some(var("bestaat_niet")),
            };
            let result = execute_operation(&op, &r, 0).unwrap();
            assert_eq!(missing_names(&result), vec!["huur"]);
            // An earlier match still wins over a later unknown condition.
            let earlier = ActionOperation::If {
                cases: vec![
                    Case {
                        when: lit(true),
                        then: lit("eerst"),
                    },
                    Case {
                        when: var("huur"),
                        then: lit("ja"),
                    },
                ],
                default: Some(lit("nee")),
            };
            assert_eq!(
                execute_operation(&earlier, &r, 0).unwrap(),
                Value::String("eerst".to_string())
            );
            // An absent condition is an error, not a fall-through to the default.
            let absent_case = ActionOperation::If {
                cases: vec![Case {
                    when: var("geen"),
                    then: lit("ja"),
                }],
                default: Some(lit("nee")),
            };
            absent(execute_operation(&absent_case, &r, 0).unwrap_err(), "IF");
        }

        #[test]
        fn null_checks_answer_on_absence_and_are_unknown_on_unknown() {
            let r = resolver();
            let is_null = |subject: ActionValue| {
                execute_operation(&ActionOperation::IsNull { subject }, &r, 0).unwrap()
            };
            let not_null = |subject: ActionValue| {
                execute_operation(&ActionOperation::NotNull { subject }, &r, 0).unwrap()
            };
            assert_eq!(is_null(var("geen")), Value::Bool(true));
            assert_eq!(not_null(var("geen")), Value::Bool(false));
            assert_eq!(is_null(lit(0i64)), Value::Bool(false));
            // Whether the register holds a value is exactly the fact nobody has.
            assert_eq!(missing_names(&is_null(var("huur"))), vec!["huur"]);
            assert_eq!(missing_names(&not_null(var("huur"))), vec!["huur"]);
            assert_eq!(is_null(var("vaag")), taint());
        }

        #[test]
        fn membership_matches_definitely_or_stays_unknown() {
            let r = resolver();
            let is_in = |subject: ActionValue, values: Vec<ActionValue>| {
                execute_operation(
                    &ActionOperation::In {
                        subject,
                        value: None,
                        values: Some(values),
                    },
                    &r,
                    0,
                )
                .unwrap()
            };
            let not_in = |subject: ActionValue, values: Vec<ActionValue>| {
                execute_operation(
                    &ActionOperation::NotIn {
                        subject,
                        value: None,
                        values: Some(values),
                    },
                    &r,
                    0,
                )
                .unwrap()
            };
            // A definite element that matches settles it, unknown elements or not.
            assert_eq!(
                is_in(lit(2i64), vec![var("huur"), lit(2i64)]),
                Value::Bool(true)
            );
            assert_eq!(
                not_in(lit(2i64), vec![var("huur"), lit(2i64)]),
                Value::Bool(false)
            );
            // No definite match and an unknown in play: unknown, for those facts.
            assert_eq!(
                missing_names(&is_in(var("huur"), vec![lit(1i64), lit(2i64)])),
                vec!["huur"]
            );
            assert_eq!(
                missing_names(&not_in(lit(3i64), vec![lit(1i64), var("partner")])),
                vec!["partner_bsn"]
            );
            // Two unknowns are not a match for each other.
            assert_eq!(
                missing_names(&is_in(var("huur"), vec![var("partner")])),
                vec!["huur", "partner_bsn"]
            );
            // No unknown anywhere: a plain false / true.
            assert_eq!(is_in(lit(3i64), vec![lit(1i64)]), Value::Bool(false));
            assert_eq!(not_in(lit(3i64), vec![lit(1i64)]), Value::Bool(true));
            // Null is structural, as for EQUALS.
            assert_eq!(
                is_in(var("geen"), vec![lit(Value::Null)]),
                Value::Bool(true)
            );
            assert_eq!(is_in(var("geen"), vec![lit(1i64)]), Value::Bool(false));
        }

        /// `LIST [item]` as an operand.
        fn list(item: ActionValue) -> ActionValue {
            ActionValue::Operation(Box::new(ActionOperation::List { items: vec![item] }))
        }

        #[test]
        fn an_unknown_inside_a_container_propagates_through_equality() {
            // Review probe P1: two LISTs whose only element is unknown compared
            // equal, because the structural comparison fell through to
            // `PartialEq`, which equates two Unknowns. An Unknown at any depth
            // makes the comparison unknown, for the union of the facts.
            let mut dossier = BTreeMap::new();
            dossier.insert("status".to_string(), unknown_fact("beschikking"));
            dossier.insert("nummer".to_string(), Value::Int(7));
            let r = resolver().with_var("dossier", Value::Object(dossier));
            let run = |op: ActionOperation| execute_operation(&op, &r, 0).unwrap();

            let equal = run(ActionOperation::Equals {
                subject: list(var("huur")),
                value: list(var("partner")),
            });
            assert_eq!(missing_names(&equal), vec!["huur", "partner_bsn"]);
            let not_equal = run(ActionOperation::NotEquals {
                subject: list(var("huur")),
                value: list(var("partner")),
            });
            assert_eq!(missing_names(&not_equal), vec!["huur", "partner_bsn"]);
            // One side definite, the other with an unknown inside: still unknown.
            let mixed = run(ActionOperation::Equals {
                subject: list(lit(650i64)),
                value: list(var("huur")),
            });
            assert_eq!(missing_names(&mixed), vec!["huur"]);
            // A record with an unknown field, compared with itself.
            let record = run(ActionOperation::Equals {
                subject: var("dossier"),
                value: var("dossier"),
            });
            assert_eq!(missing_names(&record), vec!["beschikking"]);
            // Two levels deep.
            let nested = run(ActionOperation::Equals {
                subject: list(list(var("huur"))),
                value: list(list(lit(1i64))),
            });
            assert_eq!(missing_names(&nested), vec!["huur"]);
            // Containers without an unknown compare structurally as before;
            // an absence inside a container is a value like any other.
            assert_eq!(
                run(ActionOperation::Equals {
                    subject: list(lit(1i64)),
                    value: list(lit(1i64)),
                }),
                Value::Bool(true)
            );
            assert_eq!(
                run(ActionOperation::Equals {
                    subject: list(var("geen")),
                    value: list(lit(Value::Null)),
                }),
                Value::Bool(true)
            );
        }

        #[test]
        fn membership_looks_inside_the_subject_and_the_elements() {
            // Review probe P1: `IN` with a LIST subject against a list of LISTs
            // matched structurally on two Unknowns.
            let r = resolver();
            let is_in = |subject: ActionValue, values: Vec<ActionValue>| {
                execute_operation(
                    &ActionOperation::In {
                        subject,
                        value: None,
                        values: Some(values),
                    },
                    &r,
                    0,
                )
                .unwrap()
            };
            let not_in = |subject: ActionValue, values: Vec<ActionValue>| {
                execute_operation(
                    &ActionOperation::NotIn {
                        subject,
                        value: None,
                        values: Some(values),
                    },
                    &r,
                    0,
                )
                .unwrap()
            };
            assert_eq!(
                missing_names(&is_in(list(var("huur")), vec![list(var("partner"))])),
                vec!["huur", "partner_bsn"]
            );
            assert_eq!(
                missing_names(&not_in(list(var("huur")), vec![list(var("partner"))])),
                vec!["huur", "partner_bsn"]
            );
            // A subject with an unknown inside can never match definitely.
            assert_eq!(
                missing_names(&is_in(list(var("huur")), vec![list(lit(1i64))])),
                vec!["huur"]
            );
            // An element with an unknown inside is not a definite match either,
            // but another element that is definite still decides.
            assert_eq!(
                missing_names(&is_in(list(lit(1i64)), vec![list(var("huur"))])),
                vec!["huur"]
            );
            assert_eq!(
                is_in(list(lit(1i64)), vec![list(var("huur")), list(lit(1i64))]),
                Value::Bool(true)
            );
            assert_eq!(
                not_in(list(lit(1i64)), vec![list(var("huur")), list(lit(1i64))]),
                Value::Bool(false)
            );
        }

        #[test]
        fn membership_taint_in_an_element_beats_unknown_and_false() {
            // Untranslatable beats unknown per element too: an untranslatable
            // element could have been the match, so without a definite match
            // the result is that taint, not a confident false and not unknown.
            let r = resolver();
            let is_in = |subject: ActionValue, values: Vec<ActionValue>| {
                execute_operation(
                    &ActionOperation::In {
                        subject,
                        value: None,
                        values: Some(values),
                    },
                    &r,
                    0,
                )
                .unwrap()
            };
            let not_in = |subject: ActionValue, values: Vec<ActionValue>| {
                execute_operation(
                    &ActionOperation::NotIn {
                        subject,
                        value: None,
                        values: Some(values),
                    },
                    &r,
                    0,
                )
                .unwrap()
            };
            assert_eq!(is_in(lit(3i64), vec![lit(1i64), var("vaag")]), taint());
            assert_eq!(not_in(lit(3i64), vec![var("vaag")]), taint());
            assert_eq!(is_in(var("huur"), vec![var("vaag"), lit(1i64)]), taint());
            // A definite match still wins over a tainted element.
            assert_eq!(
                is_in(var("geen"), vec![var("vaag"), lit(Value::Null)]),
                Value::Bool(true)
            );
            assert_eq!(
                is_in(lit(2i64), vec![var("vaag"), var("huur"), lit(2i64)]),
                Value::Bool(true)
            );
        }

        #[test]
        fn foreach_arrays_with_unknown_elements_compare_as_unknown() {
            // Review probe P1: FOREACH without `combine` yields an array that may
            // hold unknown elements; comparing two such arrays is unknown.
            let mut params = BTreeMap::new();
            params.insert("items".to_string(), Value::Array(vec![Value::Int(1)]));
            params.insert("a".to_string(), unknown_fact("a"));
            params.insert("b".to_string(), unknown_fact("b"));
            let context = RuleContext::new(params, "2025-01-01").expect("valid date");
            let each = |body: ActionValue| {
                ActionValue::Operation(Box::new(ActionOperation::Foreach {
                    collection: var("items"),
                    as_name: Some("x".to_string()),
                    body,
                    filter: None,
                    combine: None,
                }))
            };
            let op = ActionOperation::Equals {
                subject: each(var("a")),
                value: each(var("b")),
            };
            let result = execute_operation(&op, &context, 0).unwrap();
            assert_eq!(missing_names(&result), vec!["a", "b"]);
            // The arrays themselves are still arrays, unknown elements and all.
            let listed = execute_operation(
                &ActionOperation::List {
                    items: vec![each(var("a"))],
                },
                &context,
                0,
            )
            .unwrap();
            assert!(listed.contains_unknown());
            assert!(!listed.is_unknown());
        }

        #[test]
        fn list_may_hold_unknown_and_absent_elements() {
            let r = resolver();
            let op = ActionOperation::List {
                items: vec![lit(1i64), var("huur"), var("geen")],
            };
            assert_eq!(
                execute_operation(&op, &r, 0).unwrap(),
                Value::Array(vec![Value::Int(1), unknown_fact("huur"), Value::Null])
            );
        }

        #[test]
        fn date_operations_propagate_unknown_and_reject_absence() {
            let r = resolver().with_var("datum", "2025-01-01");
            type Case = (&'static str, fn(ActionValue) -> ActionOperation);
            let cases: Vec<Case> = vec![
                ("AGE", |v| ActionOperation::Age {
                    date_of_birth: v,
                    reference_date: lit("2025-01-01"),
                }),
                ("DATE_ADD", |v| ActionOperation::DateAdd {
                    date: v,
                    years: None,
                    months: None,
                    weeks: None,
                    days: None,
                }),
                ("DATE_ADD", |v| ActionOperation::DateAdd {
                    date: lit("2025-01-01"),
                    years: None,
                    months: Some(v),
                    weeks: None,
                    days: None,
                }),
                ("DATE", |v| ActionOperation::Date {
                    year: lit(2025i64),
                    month: v,
                    day: lit(1i64),
                }),
                ("DAY_OF_WEEK", |v| ActionOperation::DayOfWeek { date: v }),
                ("DATE_DIFF", |v| ActionOperation::DateDiff {
                    from: lit("2025-01-01"),
                    to: v,
                    unit: lit("days"),
                }),
                ("DATE_DIFF", |v| ActionOperation::DateDiff {
                    from: lit("2025-01-01"),
                    to: lit("2025-02-01"),
                    unit: v,
                }),
            ];
            for (name, build) in cases {
                let result = execute_operation(&build(var("huur")), &r, 0).unwrap();
                assert_eq!(missing_names(&result), vec!["huur"], "{name}");
                absent(
                    execute_operation(&build(var("geen")), &r, 0).unwrap_err(),
                    name,
                );
            }
        }

        #[test]
        fn a_decided_result_gets_the_dispatchers_message() {
            // The counterpart of the test below: the handler's message survives
            // only for an unknown result. AND writes one of its own when every
            // condition holds, and for a decided result the reader still gets
            // the uniform "Compute OP(...) = value".
            let mut context = RuleContext::new(BTreeMap::new(), "2025-01-01").unwrap();
            let trace = std::rc::Rc::new(std::cell::RefCell::new(
                crate::trace::TraceBuilder::new_untimed(),
            ));
            context.set_trace(std::rc::Rc::clone(&trace));
            trace.borrow_mut().push("root", PathNodeType::Action);
            let op = ActionOperation::And {
                conditions: vec![lit(true), lit(true)],
            };
            execute_operation(&op, &context, 0).unwrap();
            let root = trace.borrow_mut().pop().unwrap();
            let node = root.children.into_iter().next().unwrap();
            assert_eq!(node.message.as_deref(), Some("Compute AND(...) = True"));
        }

        #[test]
        fn the_trace_says_why_a_result_is_unknown() {
            // The dispatcher normally overwrites a handler's message with
            // "Compute OP(...) = value"; for an unknown result the handler's
            // message, which names the missing facts, is what the reader needs.
            let mut context = RuleContext::new(BTreeMap::new(), "2025-01-01").unwrap();
            context.set_local("huur", unknown_fact("huur"));
            let trace = std::rc::Rc::new(std::cell::RefCell::new(
                crate::trace::TraceBuilder::new_untimed(),
            ));
            context.set_trace(std::rc::Rc::clone(&trace));
            trace.borrow_mut().push("root", PathNodeType::Action);
            let op = ActionOperation::Add {
                values: vec![lit(100i64), var("huur")],
            };
            execute_operation(&op, &context, 0).unwrap();
            let root = trace.borrow_mut().pop().unwrap();
            let node = root.children.into_iter().next().unwrap();
            assert_eq!(
                node.message.as_deref(),
                Some("ADD with an unknown operand: unknown (missing: testwet.huur)")
            );
        }
    }

    // -------------------------------------------------------------------------
    // Tracing opt-in Tests
    // -------------------------------------------------------------------------

    mod tracing {
        use super::*;
        use std::cell::RefCell;

        /// A resolver that offers a trace hook but never switches tracing on:
        /// it keeps `has_trace` at the trait's default.
        #[derive(Default)]
        struct RecordingResolver {
            pushed: RefCell<Vec<String>>,
        }

        impl ValueResolver for RecordingResolver {
            fn resolve(&self, name: &str) -> Result<Value> {
                Err(EngineError::VariableNotFound(name.to_string()))
            }

            fn trace_push(&self, name: &str, _node_type: PathNodeType) {
                self.pushed.borrow_mut().push(name.to_string());
            }
        }

        #[test]
        fn test_no_tracing_unless_the_resolver_opts_in() {
            // Tracing is opt-in: a resolver that implements the hooks but leaves
            // `has_trace` at its default gets none of the trace bookkeeping, and
            // the operation result is the same either way.
            let resolver = RecordingResolver::default();
            let op = ActionOperation::Add {
                values: vec![lit(1i64), lit(2i64)],
            };

            assert_eq!(execute_operation(&op, &resolver, 0).unwrap(), Value::Int(3));
            assert!(
                resolver.pushed.borrow().is_empty(),
                "expected no trace nodes, got {:?}",
                resolver.pushed.borrow()
            );
        }
    }

    // -------------------------------------------------------------------------
    // parse_date Tests (Object input)
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_date_with_object() {
        let mut date_obj = BTreeMap::new();
        date_obj.insert("iso".to_string(), Value::String("2025-01-01".to_string()));
        date_obj.insert("year".to_string(), Value::Int(2025));

        let result = parse_date(&Value::Object(date_obj)).unwrap();
        assert_eq!(result.to_string(), "2025-01-01");
    }

    #[test]
    fn test_parse_date_object_without_iso_field() {
        let mut date_obj = BTreeMap::new();
        date_obj.insert("year".to_string(), Value::Int(2025));

        let result = parse_date(&Value::Object(date_obj));
        assert!(matches!(result, Err(EngineError::TypeMismatch { .. })));
    }

    // -------------------------------------------------------------------------
    // values_equal Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_values_equal_exact_large_integers() {
        // Decimal carries integers exactly, so large-integer Int/Decimal pairs
        // compare without the ±2^53 precision loss f64 would introduce.
        let large: i64 = 9_007_199_254_740_993; // 2^53 + 1
        assert!(values_equal(
            &Value::Int(large),
            &Value::Decimal(Decimal::from(large))
        ));
        assert!(!values_equal(
            &Value::Int(large),
            &Value::Decimal(Decimal::from(large + 1))
        ));

        assert!(values_equal(&Value::Int(42), &Value::Decimal(dec!(42))));
        assert!(values_equal(&Value::Decimal(dec!(42)), &Value::Int(42)));
    }

    #[test]
    fn test_compound_equal_reports_the_stored_rule_with_its_operation() {
        // EQUALS on records and the comparison of stored values are one
        // rule (`stored::equal_as_written`); the engine adds the operation
        // and the place to the message.
        let record = |v: Value| Value::Object(BTreeMap::from([("n".to_string(), v)]));
        let err = compound_equal(
            &record(Value::Int(5)),
            &record(Value::Decimal(dec!(5.0))),
            "NOT_EQUALS",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("records NOT_EQUALS compares"), "{err}");
        assert!(err.contains("5 against 5.0 at .n"), "{err}");
        assert!(compound_equal(
            &record(Value::Decimal(dec!(5.0))),
            &record(Value::Decimal(dec!(5.00))),
            "EQUALS"
        )
        .unwrap());
        assert!(!compound_equal(&record(Value::Int(5)), &record(Value::Int(6)), "EQUALS").unwrap());
    }
}
