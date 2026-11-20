//! Construct mutation: the BDD runners load every synthetic `test_*`
//! regelwerk with one construct of the law format broken, to show whether a
//! scenario's assertion depends on that construct.
//!
//! Line coverage says a construct was evaluated. It does not say that a
//! scenario would turn red if the construct did something else. Breaking the
//! construct and running the suites answers that question: a construct is
//! *pinned* when at least one scenario fails, and *unpinned* when every
//! scenario stays green. `script/construct-mutants.mjs` runs the suites once
//! per construct; `tests/construct_coverage.rs` checks that every construct in
//! the schema is in [`CONSTRUCTS`] or in [`NOT_MUTATED`] with a reason.
//!
//! Only regelwerken whose `$id` starts with `test_` are changed: those are the
//! synthetic laws of the conformance bucket and the cell's suite. The real
//! laws in the corpus stay as they are, so a red scenario means a synthetic
//! test noticed, not that some corpus law happened to break.

use serde_yaml_ng::{Mapping, Value};

/// The environment variable that selects the construct to break. The value
/// `list` makes the engine's BDD runner print [`CONSTRUCTS`] and exit.
pub const VAR: &str = "BDD_CONSTRUCT_MUTATION";

/// One construct of the law format and how the runner breaks it.
pub struct Construct {
    /// The name the report and `construct_coverage.rs` use: an operation
    /// name (`ADD`) or `<schema definition>.<property>`.
    pub name: &'static str,
    /// What the mutation does, for the report.
    pub how: &'static str,
    rewrite: Rewrite,
}

/// How a construct is broken. Each variant changes one mapping in place and
/// says whether it changed anything, so the runner can count the sites.
enum Rewrite {
    /// `operation: from` becomes `operation: to`, same operands.
    Operation(&'static str, &'static str),
    /// A rewrite of the whole node that carries `operation: <op>`.
    Node(&'static str, fn(&mut Value) -> bool),
    /// The key is removed from every mapping that holds it.
    DropKey(&'static str),
    /// A rewrite of every mapping, for constructs that are not an operation.
    Mapping(fn(&mut Mapping) -> bool),
}

/// Every construct the runner knows how to break.
pub const CONSTRUCTS: &[Construct] = &[
    op("EQUALS", "NOT_EQUALS"),
    op("GREATER_THAN", "GREATER_THAN_OR_EQUAL"),
    op("LESS_THAN", "LESS_THAN_OR_EQUAL"),
    op("GREATER_THAN_OR_EQUAL", "GREATER_THAN"),
    op("LESS_THAN_OR_EQUAL", "LESS_THAN"),
    op("ADD", "SUBTRACT"),
    op("SUBTRACT", "ADD"),
    op("MULTIPLY", "DIVIDE"),
    op("DIVIDE", "MULTIPLY"),
    op("MAX", "MIN"),
    op("MIN", "MAX"),
    op("ROUND", "FLOOR"),
    op("CEIL", "FLOOR"),
    op("FLOOR", "CEIL"),
    op("AND", "OR"),
    op("OR", "AND"),
    op("IN", "NOT_IN"),
    node("NOT", "replaced by its operand", unwrap_not),
    node("IF", "first case dropped", drop_first_case),
    node("LIST", "last item dropped", drop_last_item),
    node(
        "RECORD",
        "values of the first two fields swapped",
        swap_first_fields,
    ),
    node(
        "FOREACH",
        "collection replaced by an empty list",
        empty_collection,
    ),
    node("AGE", "date_of_birth and reference_date swapped", |n| {
        swap(n, "date_of_birth", "reference_date")
    }),
    node("DATE_ADD", "one day more", |n| add_one(n, "days")),
    node("DATE", "day plus one", |n| add_one(n, "day")),
    node("DAY_OF_WEEK", "date shifted by one day", shift_date),
    node("DATE_DIFF", "from and to swapped", |n| {
        swap(n, "from", "to")
    }),
    node("DATE_PART", "next component (year, month, day)", |n| {
        cycle(n, "in", &["year", "month", "day"])
    }),
    node("START_OF", "the other unit (year, month)", |n| {
        cycle(n, "in", &["year", "month"])
    }),
    Construct {
        name: "foreachOperation.filter",
        how: "filter removed",
        rewrite: Rewrite::Mapping(drop_foreach_filter),
    },
    Construct {
        name: "foreachOperation.combine",
        how: "ADD/MIN/MAX/AND/OR to its counterpart, ONLY/DISTINCT removed",
        rewrite: Rewrite::Mapping(change_combine),
    },
    drop_key("machineReadableSection.hooks", "hooks"),
    drop_key("machineReadableSection.overrides", "overrides"),
    drop_key("machineReadableSection.implements", "implements"),
    drop_key("machineReadableSection.markings", "markings"),
    Construct {
        name: "machineReadableSection.open_terms",
        how: "default of every open term removed",
        rewrite: Rewrite::Mapping(drop_open_term_defaults),
    },
    Construct {
        name: "machineReadableSection.definitions",
        how: "every numeric constant plus one",
        rewrite: Rewrite::Mapping(bump_definitions),
    },
    drop_key("baseField.nullable", "nullable"),
    drop_key("baseField.type_spec", "type_spec"),
];

/// Constructs in the schema that [`CONSTRUCTS`] does not break, each with the
/// reason. `construct_coverage.rs` fails on a schema construct in neither
/// list, so a new construct is a decision and not a silent gap.
pub const NOT_MUTATED: &[(&str, &str)] = &[
    // Not executed: the schema itself says the engine does not act on these.
    (
        "machineReadableSection.endpoint",
        "not executed: the engine calls by output name",
    ),
    (
        "machineReadableSection.requires",
        "not executed: read by law-model only",
    ),
    (
        "baseField.temporal",
        "not executed: declared, not read by the engine",
    ),
    (
        "action.resolve",
        "not executed: the engine does not run it yet",
    ),
    // Documentation and identity: no value depends on them.
    ("baseField.description", "documentation"),
    ("baseField.legal_basis", "documentation"),
    ("action.legal_basis", "documentation"),
    ("foreachOperation.legal_basis", "documentation"),
    (
        "baseField.name",
        "identity: renaming breaks every reference, not one construct",
    ),
    (
        "action.output",
        "identity: renaming breaks every reference, not one construct",
    ),
    (
        "foreachOperation.operation",
        "the discriminator; FOREACH itself is mutated",
    ),
    ("action.operation", "covered per operation"),
    ("action.values", "covered per operation"),
    (
        "action.value",
        "a literal or reference; covered by the operations around it",
    ),
    (
        "foreachOperation.collection",
        "covered by the FOREACH mutation",
    ),
    (
        "foreachOperation.as",
        "a binding name; renaming breaks every reference",
    ),
    (
        "foreachOperation.body",
        "covered by the operations inside it",
    ),
    // Executed, but no mutation that keeps the law loadable yet.
    (
        "baseField.type",
        "no mutation yet: a wrong type fails the load check, not a scenario",
    ),
    ("action.precision", "no mutation yet"),
    (
        "machineReadableSection.competent_authority",
        "no mutation yet",
    ),
    (
        "machineReadableSection.execution",
        "the container of the constructs below",
    ),
    (
        "machineReadableSection.projection",
        "no mutation yet: a reduction has no single seam",
    ),
    ("machineReadableSection.declares", "no mutation yet"),
    (
        "execution.produces",
        "no mutation yet: selects hooks and procedures",
    ),
    (
        "execution.parameters",
        "no mutation yet: removing one breaks the call, not a rule",
    ),
    ("execution.input", "no mutation yet"),
    ("execution.output", "no mutation yet"),
    ("execution.actions", "covered per operation"),
    (
        "inputField.source",
        "no mutation yet: cross-law references and data sources",
    ),
    ("parameterField.required", "no mutation yet"),
    ("parameterField.columns", "no mutation yet"),
    ("outputField.columns", "no mutation yet"),
    ("producedItem.type", "no mutation yet"),
    ("producedItem.requests", "no mutation yet"),
    ("producedItem.parameters", "no mutation yet"),
    ("producedItem.outputs", "no mutation yet"),
    ("producedItem.derived", "no mutation yet"),
    ("producedItem.effects", "no mutation yet"),
    ("producedItem.decision_type", "no mutation yet"),
    ("producedItem.procedure_id", "no mutation yet"),
    ("producedItem.moment", "no mutation yet"),
];

const fn op(from: &'static str, to: &'static str) -> Construct {
    Construct {
        name: from,
        how: to,
        rewrite: Rewrite::Operation(from, to),
    }
}

const fn node(name: &'static str, how: &'static str, f: fn(&mut Value) -> bool) -> Construct {
    Construct {
        name,
        how,
        rewrite: Rewrite::Node(name, f),
    }
}

const fn drop_key(name: &'static str, key: &'static str) -> Construct {
    Construct {
        name,
        how: "removed",
        rewrite: Rewrite::DropKey(key),
    }
}

/// The construct `BDD_CONSTRUCT_MUTATION` selects, if any.
///
/// # Panics
///
/// When the variable names no construct: a typo would otherwise run the
/// suites unchanged and report the construct as unpinned.
pub fn selected() -> Option<&'static Construct> {
    let name = std::env::var(VAR).ok()?;
    let name = name.trim();
    if name.is_empty() || name == "list" {
        return None;
    }
    let found = CONSTRUCTS.iter().find(|c| c.name == name);
    assert!(found.is_some(), "{VAR}={name}: no such construct");
    found
}

/// Whether the runner should only print the constructs and exit.
pub fn list_requested() -> bool {
    std::env::var(VAR).is_ok_and(|v| v.trim() == "list")
}

/// `content` with `construct` broken, and the number of sites changed. A
/// regelwerk whose `$id` does not start with `test_` comes back unchanged.
pub fn mutated(
    content: &str,
    construct: &Construct,
) -> Result<(String, usize), serde_yaml_ng::Error> {
    // The runners load the whole corpus for every feature; parsing each real
    // rulework only to find its `$id` would make every run several times
    // slower. The `$id` sits on a top-level line, so look there first.
    if !content.lines().any(declares_synthetic_id) {
        return Ok((content.to_string(), 0));
    }
    let mut law: Value = serde_yaml_ng::from_str(content)?;
    let synthetic = law
        .get("$id")
        .and_then(Value::as_str)
        .is_some_and(|id| id.starts_with("test_"));
    if !synthetic {
        return Ok((content.to_string(), 0));
    }
    let sites = match law.get_mut("articles") {
        Some(articles) => walk(articles, &construct.rewrite),
        None => 0,
    };
    Ok((serde_yaml_ng::to_string(&law)?, sites))
}

/// Whether `line` is the top-level `$id` of a synthetic rulework.
fn declares_synthetic_id(line: &str) -> bool {
    line.strip_prefix("$id:").is_some_and(|id| {
        id.trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .starts_with("test_")
    })
}

/// Apply `rewrite` to `value` and everything below it, counting changes.
/// A node is rewritten before its children are visited, so a rewrite that
/// replaces a node (NOT) still reaches the operations in its operand.
fn walk(value: &mut Value, rewrite: &Rewrite) -> usize {
    let here = usize::from(apply(value, rewrite));
    let below: usize = match value {
        Value::Mapping(map) => map.values_mut().map(|v| walk(v, rewrite)).sum(),
        Value::Sequence(items) => items.iter_mut().map(|v| walk(v, rewrite)).sum(),
        _ => 0,
    };
    here + below
}

fn apply(value: &mut Value, rewrite: &Rewrite) -> bool {
    match rewrite {
        Rewrite::Operation(from, to) => {
            if operation_of(value) == Some(from) {
                value["operation"] = Value::from(*to);
                return true;
            }
            false
        }
        Rewrite::Node(op, f) => operation_of(value) == Some(op) && f(value),
        Rewrite::DropKey(key) => match value {
            Value::Mapping(map) => map.remove(*key).is_some(),
            _ => false,
        },
        Rewrite::Mapping(f) => match value {
            Value::Mapping(map) => f(map),
            _ => false,
        },
    }
}

fn operation_of(value: &Value) -> Option<&str> {
    value.get("operation").and_then(Value::as_str)
}

fn unwrap_not(node: &mut Value) -> bool {
    let Some(operand) = node.get("value").cloned() else {
        return false;
    };
    *node = operand;
    true
}

/// Drop the first case of an IF, when something is left to decide the value.
fn drop_first_case(node: &mut Value) -> bool {
    let has_default = node.get("default").is_some();
    let Some(Value::Sequence(cases)) = node.get_mut("cases") else {
        return false;
    };
    if cases.len() > 1 || (has_default && !cases.is_empty()) {
        cases.remove(0);
        if cases.is_empty() {
            // `cases` must hold at least one case: the default takes over.
            let default = node.get("default").cloned().unwrap_or(Value::Null);
            *node = default;
        }
        return true;
    }
    false
}

fn drop_last_item(node: &mut Value) -> bool {
    match node.get_mut("items") {
        Some(Value::Sequence(items)) => items.pop().is_some(),
        _ => false,
    }
}

fn swap_first_fields(node: &mut Value) -> bool {
    let Some(Value::Mapping(fields)) = node.get_mut("fields") else {
        return false;
    };
    let mut values = fields.values_mut();
    let (Some(a), Some(b)) = (values.next(), values.next()) else {
        return false;
    };
    if a == b {
        return false;
    }
    std::mem::swap(a, b);
    true
}

fn empty_collection(node: &mut Value) -> bool {
    let Some(collection) = node.get_mut("collection") else {
        return false;
    };
    *collection = operation_node("LIST", [("items", Value::Sequence(Vec::new()))]);
    true
}

fn swap(node: &mut Value, a: &str, b: &str) -> bool {
    let Value::Mapping(map) = node else {
        return false;
    };
    let (Some(x), Some(y)) = (map.get(a).cloned(), map.get(b).cloned()) else {
        return false;
    };
    if x == y {
        return false;
    }
    map.insert(Value::from(a), y);
    map.insert(Value::from(b), x);
    true
}

/// `key` becomes `ADD(key, 1)`, or `1` when the node does not have it.
fn add_one(node: &mut Value, key: &str) -> bool {
    let Value::Mapping(map) = node else {
        return false;
    };
    let bumped = match map.get(key) {
        Some(current) => operation_node(
            "ADD",
            [(
                "values",
                Value::Sequence(vec![current.clone(), Value::from(1)]),
            )],
        ),
        None => Value::from(1),
    };
    map.insert(Value::from(key), bumped);
    true
}

/// The `date` operand becomes `DATE_ADD(date, days: 1)`.
fn shift_date(node: &mut Value) -> bool {
    let Some(date) = node.get_mut("date") else {
        return false;
    };
    *date = operation_node(
        "DATE_ADD",
        [("date", date.clone()), ("days", Value::from(1))],
    );
    true
}

/// `key` moves to the next value in `cycle`, wrapping around.
fn cycle(node: &mut Value, key: &str, cycle: &[&str]) -> bool {
    let Some(current) = node.get(key).and_then(Value::as_str) else {
        return false;
    };
    let Some(at) = cycle.iter().position(|v| *v == current) else {
        return false;
    };
    let next = cycle[(at + 1) % cycle.len()];
    node[key] = Value::from(next);
    true
}

fn drop_foreach_filter(map: &mut Mapping) -> bool {
    let is_foreach = map.get("operation").and_then(Value::as_str) == Some("FOREACH");
    is_foreach && map.remove("filter").is_some()
}

fn change_combine(map: &mut Mapping) -> bool {
    if map.get("operation").and_then(Value::as_str) != Some("FOREACH") {
        return false;
    }
    let counterpart = match map.get("combine").and_then(Value::as_str) {
        Some("ADD") => Some("MAX"),
        Some("MAX") => Some("MIN"),
        Some("MIN") => Some("MAX"),
        Some("AND") => Some("OR"),
        Some("OR") => Some("AND"),
        Some(_) => None,
        None => return false,
    };
    match counterpart {
        Some(to) => {
            map.insert(Value::from("combine"), Value::from(to));
        }
        None => {
            map.remove("combine");
        }
    }
    true
}

fn drop_open_term_defaults(map: &mut Mapping) -> bool {
    let Some(Value::Sequence(terms)) = map.get_mut("open_terms") else {
        return false;
    };
    let mut changed = false;
    for term in terms.iter_mut() {
        if let Value::Mapping(term) = term {
            changed |= term.remove("default").is_some();
        }
    }
    changed
}

fn bump_definitions(map: &mut Mapping) -> bool {
    let Some(Value::Mapping(definitions)) = map.get_mut("definitions") else {
        return false;
    };
    let mut changed = false;
    for definition in definitions.values_mut() {
        let number = match definition {
            Value::Mapping(structured) => structured.get_mut("value"),
            other => Some(other),
        };
        if let Some(number) = number {
            changed |= bump(number);
        }
    }
    changed
}

fn bump(value: &mut Value) -> bool {
    if let Some(i) = value.as_i64() {
        *value = Value::from(i.saturating_add(1));
        true
    } else if let Some(f) = value.as_f64() {
        *value = Value::from(f + 1.0);
        true
    } else {
        false
    }
}

fn operation_node<const N: usize>(operation: &str, fields: [(&str, Value); N]) -> Value {
    let mut map = Mapping::new();
    map.insert(Value::from("operation"), Value::from(operation));
    for (key, value) in fields {
        map.insert(Value::from(key), value);
    }
    Value::Mapping(map)
}
