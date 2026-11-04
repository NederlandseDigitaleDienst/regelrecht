//! Which actions a requested output depends on (RFC-043).
//!
//! An article executes only the actions in the dependency closure of the
//! outputs it was asked for. The closure follows `$name` references between
//! the article's own outputs; anything else a reference can name (an input, a
//! parameter, a definition) is not an action and is left to resolution.
//!
//! Over-including is always safe: an action that runs without being needed
//! costs work, never a wrong value. Under-including is what must not happen,
//! so the walk goes through [`crate::article::ActionOperation::operands`], which is exhaustive
//! over the operation variants.

use crate::article::{foreach_binding, Action, ActionOperation, ActionValue, Article};
use crate::types::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The outputs an article's actions produce, in declaration order.
pub(crate) fn action_outputs(article: &Article) -> impl Iterator<Item = &str> {
    article
        .get_execution_spec()
        .and_then(|e| e.actions.as_ref())
        .into_iter()
        .flatten()
        .filter_map(|a| a.output.as_deref())
}

/// The outputs of `actions` that computing `requested` needs: those requested
/// and everything they read, transitively. A requested name no action
/// produces (a hook output) adds nothing: no action can produce it.
pub(crate) fn required_outputs(actions: &[Action], requested: &[&str]) -> BTreeSet<String> {
    required_outputs_with(actions, requested, &BTreeMap::new())
}

/// [`required_outputs`], where computing an output also reads the names
/// `also_reads` gives for it (the parameters of an override that replaces it).
pub(crate) fn required_outputs_with(
    actions: &[Action],
    requested: &[&str],
    also_reads: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    // Every action that writes a name counts: an output assigned twice reads
    // what both assignments read.
    let mut produced: BTreeMap<&str, Vec<&Action>> = BTreeMap::new();
    for action in actions {
        if let Some(name) = action.output.as_deref() {
            produced.entry(name).or_default().push(action);
        }
    }
    let mut required = BTreeSet::new();
    let mut pending: Vec<&str> = requested
        .iter()
        .copied()
        .filter(|name| produced.contains_key(name))
        .collect();
    while let Some(name) = pending.pop() {
        if !required.insert(name.to_string()) {
            continue;
        }
        let reads = produced
            .get(name)
            .into_iter()
            .flatten()
            .flat_map(|action| referenced_names(action))
            .chain(also_reads.get(name).into_iter().flatten().cloned());
        for reference in reads {
            if let Some((key, _)) = produced.get_key_value(reference.as_str()) {
                pending.push(key);
            }
        }
    }
    required
}

/// The order to run `actions` in: every action after the actions producing
/// what it reads. Where nothing orders two actions, declaration order does,
/// so the order of the actions in the file never changes a value, except
/// among the assignments of an output assigned more than once: those run
/// together, in declaration order, and a reader of that output runs after
/// all of them.
///
/// A name that is both an input and an output of the article is the output
/// to every action but its first assignment: a reader runs after the output
/// is set, and an output wins over an input when a name is resolved. The
/// first assignment reads the input (the pass-through idiom `x: $x`); later
/// assignments and every other action read the output. So the input of that
/// name only ever reaches the first assignment, whatever the file's order.
///
/// With `outputs` (the closure of [`required_outputs`]), only the actions
/// producing those outputs are ordered, so a cycle among outputs nobody asked
/// for cannot fail the request (RFC-043). An action without `output` keeps
/// its place at the front, where the loop rejects it. `Err` names an output
/// that depends on itself through other outputs of the article.
pub(crate) fn execution_order(
    actions: &[Action],
    outputs: Option<&BTreeSet<String>>,
) -> Result<Vec<usize>, String> {
    let mut order: Vec<usize> = Vec::with_capacity(actions.len());
    let mut names: Vec<&str> = Vec::new();
    let mut writes: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, action) in actions.iter().enumerate() {
        match action.output.as_deref() {
            None => order.push(index),
            Some(name) => {
                let entry = writes.entry(name).or_default();
                if entry.is_empty() {
                    names.push(name);
                }
                entry.push(index);
            }
        }
    }
    let depends_on = |name: &str| -> Vec<&str> {
        let reads: BTreeSet<String> = writes[name]
            .iter()
            .flat_map(|&index| referenced_names(&actions[index]))
            .collect();
        // In declaration order, so the order among independent outputs is
        // the file's.
        names
            .iter()
            .copied()
            .filter(|other| *other != name && reads.contains(*other))
            .collect()
    };

    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit<'a>(
        name: &'a str,
        depends_on: &dyn Fn(&str) -> Vec<&'a str>,
        marks: &mut BTreeMap<&'a str, Mark>,
        sorted: &mut Vec<&'a str>,
    ) -> Result<(), String> {
        match marks.get(name) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => return Err(name.to_string()),
            None => {}
        }
        marks.insert(name, Mark::Visiting);
        for dependency in depends_on(name) {
            visit(dependency, depends_on, marks, sorted)?;
        }
        marks.insert(name, Mark::Done);
        sorted.push(name);
        Ok(())
    }
    let mut marks = BTreeMap::new();
    let mut sorted = Vec::with_capacity(names.len());
    // The closure is transitive, so the walk from a requested output stays
    // inside it.
    for &name in names
        .iter()
        .filter(|name| outputs.is_none_or(|outputs| outputs.contains(**name)))
    {
        visit(name, &depends_on, &mut marks, &mut sorted)?;
    }
    order.extend(sorted.iter().flat_map(|name| writes[name].iter().copied()));
    Ok(order)
}

/// The name a `$reference` reads, by its base (`$a.b` gives `a`); `None` for
/// a string that is not a reference.
pub(crate) fn reference_base(reference: &str) -> Option<&str> {
    let path = reference.strip_prefix('$')?;
    Some(path.split('.').next().unwrap_or(path))
}

/// Every `$name` an action refers to, by its base name. A name a FOREACH
/// binds (`as`, or `item` without it) is its element inside `body` and
/// `filter`, not a reference to an output or input of that name. A FOREACH
/// without `as` also exposes the fields of an object element as bare names;
/// those are not known here and still count: that over-includes, which is
/// safe for the closure, but a field named like an output that reads the
/// FOREACH's own output is reported as a cycle. With `as` no field is bound
/// by its own name (RFC-049), so every other bare name is a real reference.
pub(crate) fn referenced_names(action: &Action) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    action
        .operands()
        .for_each(|operand| visit_value(operand, &BTreeSet::new(), &mut names));
    names
}

fn visit_value(value: &ActionValue, bound: &BTreeSet<&str>, names: &mut BTreeSet<String>) {
    match value {
        ActionValue::Literal(literal) => visit_literal(literal, bound, names),
        ActionValue::Operation(op) => match op.as_ref() {
            ActionOperation::Foreach {
                collection,
                as_name,
                body,
                filter,
                ..
            } => {
                visit_value(collection, bound, names);
                let mut inner = bound.clone();
                inner.insert(foreach_binding(as_name.as_deref()));
                std::iter::once(body)
                    .chain(filter)
                    .for_each(|operand| visit_value(operand, &inner, names));
            }
            // A FOLD binds its element and its state in `body`, and in what
            // `body` nests (RFC-056).
            ActionOperation::Fold {
                collection,
                as_name,
                state,
                initial,
                body,
            } => {
                visit_value(collection, bound, names);
                visit_value(initial, bound, names);
                let mut inner = bound.clone();
                inner.insert(as_name.as_str());
                inner.insert(state.as_str());
                visit_value(body, &inner, names);
            }
            other => other
                .operands()
                .into_iter()
                .for_each(|operand| visit_value(operand, bound, names)),
        },
    }
}

fn visit_literal(literal: &Value, bound: &BTreeSet<&str>, names: &mut BTreeSet<String>) {
    match literal {
        Value::String(s) => names.extend(
            reference_base(s)
                .filter(|name| !bound.contains(name))
                .map(str::to_string),
        ),
        Value::Array(items) => items
            .iter()
            .for_each(|item| visit_literal(item, bound, names)),
        Value::Object(fields) => fields
            .values()
            .for_each(|field| visit_literal(field, bound, names)),
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn actions(yaml: &str) -> Vec<Action> {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    /// The known limit of the scan: a field of an object element, which a
    /// FOREACH exposes as a bare name, still counts as a reference to the
    /// output of that name. Where that output reads the FOREACH's own output,
    /// the order reports a cycle the file order did not have. Pinned here so
    /// a change to this behaviour is a deliberate one.
    #[test]
    fn a_field_of_a_foreach_element_named_like_an_output_counts_as_a_reference() {
        let acts = actions(
            r#"
- output: totaal
  value:
    operation: FOREACH
    collection: $posten
    body: $bedrag
    combine: ADD
- output: bedrag
  value:
    operation: MULTIPLY
    values: [$totaal, 2]
"#,
        );
        assert!(referenced_names(&acts[0]).contains("bedrag"));
        assert_eq!(execution_order(&acts, None), Err("totaal".to_string()));
    }

    #[test]
    fn a_foreach_binding_is_not_a_reference_inside_its_body() {
        let acts = actions(
            r#"
- output: totaal
  value:
    operation: FOREACH
    collection: $bedragen
    as: bedrag
    filter:
      operation: GREATER_THAN
      subject: $bedrag
      value: $drempel
    body: $bedrag
    combine: ADD
"#,
        );
        assert_eq!(
            referenced_names(&acts[0]),
            BTreeSet::from(["bedragen".to_string(), "drempel".to_string()])
        );
    }

    #[test]
    fn a_foreach_binding_does_not_hide_the_name_in_its_collection() {
        // The collection is read in the scope around the FOREACH, where the
        // binding does not exist yet.
        let acts = actions(
            r#"
- output: totaal
  value:
    operation: FOREACH
    collection: $item
    body: $item
"#,
        );
        assert_eq!(
            referenced_names(&acts[0]),
            BTreeSet::from(["item".to_string()])
        );
    }

    const CHAIN: &str = r#"
- output: leeftijd_ok
  value:
    operation: GREATER_THAN_OR_EQUAL
    subject: $leeftijd
    value: 18
- output: voldoet
  value:
    operation: AND
    conditions:
      - $leeftijd_ok
      - $is_verzekerde
- output: hoogte
  value:
    operation: IF
    cases:
      - when: $voldoet
        then:
          operation: MULTIPLY
          values: [$inkomen, 0.1]
    default: 0
- output: los
  value: $partner.inkomen
"#;

    #[test]
    fn closure_follows_output_references_only() {
        let acts = actions(CHAIN);
        let required = required_outputs(&acts, &["voldoet"]);
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["leeftijd_ok".to_string(), "voldoet".to_string()]
        );
    }

    #[test]
    fn closure_is_transitive_through_nested_operations() {
        let acts = actions(CHAIN);
        let required = required_outputs(&acts, &["hoogte"]);
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec![
                "hoogte".to_string(),
                "leeftijd_ok".to_string(),
                "voldoet".to_string()
            ]
        );
    }

    #[test]
    fn several_requested_outputs_union_their_closures() {
        let acts = actions(CHAIN);
        let required = required_outputs(&acts, &["leeftijd_ok", "los"]);
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["leeftijd_ok".to_string(), "los".to_string()]
        );
    }

    #[test]
    fn a_name_no_action_produces_adds_nothing() {
        let acts = actions(CHAIN);
        assert!(required_outputs(&acts, &["van_een_hook"]).is_empty());
        assert_eq!(
            required_outputs(&acts, &["van_een_hook", "leeftijd_ok"])
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["leeftijd_ok".to_string()]
        );
    }

    #[test]
    fn an_output_assigned_twice_reads_what_both_assignments_read() {
        let acts = actions(
            r#"
- output: a
  value: 1
- output: x
  value: $a
- output: x
  value:
    operation: ADD
    values: [$x, 1]
- output: los
  value: 2
"#,
        );
        let required = required_outputs(&acts, &["x"]);
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["a".to_string(), "x".to_string()]
        );
    }

    #[test]
    fn a_reference_inside_a_literal_list_or_object_counts() {
        let acts = actions(
            r#"
- output: a
  value: 1
- output: b
  value: 2
- output: lijst
  value: [$a, {veld: $b}]
"#,
        );
        let required = required_outputs(&acts, &["lijst"]);
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            vec!["a".to_string(), "b".to_string(), "lijst".to_string()]
        );
    }

    /// The action-level operation shape (`operation` with `subject`, `values`
    /// or `conditions` on the action itself) is walked like a nested one.
    #[test]
    fn the_action_level_operation_shape_is_walked() {
        let acts = actions(
            r#"
- output: a
  value: 1
- output: b
  value: 2
- output: c
  value: 3
- output: d
  value: 4
- output: vergelijk
  operation: EQUALS
  subject: $a
  value: 1
- output: som
  operation: ADD
  values: [$b, 1]
- output: en
  operation: AND
  conditions: [$c, $d]
"#,
        );
        let required = required_outputs(&acts, &["vergelijk", "som", "en"]);
        assert_eq!(
            required.into_iter().collect::<Vec<_>>(),
            ["a", "b", "c", "d", "en", "som", "vergelijk"]
                .map(String::from)
                .to_vec()
        );
    }

    #[test]
    fn what_a_replacing_override_reads_is_part_of_the_closure() {
        // `los` reads nothing and nobody reads it, but the override replacing
        // `hoogte` declares it as a parameter.
        let acts = actions(CHAIN);
        let mut also_reads = BTreeMap::new();
        also_reads.insert("hoogte".to_string(), BTreeSet::from(["los".to_string()]));
        let required = required_outputs_with(&acts, &["hoogte"], &also_reads);
        assert!(required.contains("los"), "{required:?}");
        assert!(!required_outputs(&acts, &["hoogte"]).contains("los"));
    }

    #[test]
    fn dotted_reference_counts_by_its_base() {
        let acts = actions(CHAIN);
        let names = referenced_names(&acts[3]);
        assert!(names.contains("partner"));
    }
}
