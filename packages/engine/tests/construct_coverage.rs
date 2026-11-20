//! Every construct of the law format is either broken by the construct
//! mutation run (`common/construct_mutation.rs`) or listed with the reason it
//! is not.
//!
//! The mutation run itself is too slow for every pull request; it runs from
//! `script/construct-mutants.mjs`. This test is the cheap half: it fails the
//! moment the schema gains an operation or a property that neither list
//! names, so a new construct cannot arrive without someone deciding how its
//! tests are measured. `conformance_coverage.rs` asks the same question of the
//! conformance manifest, for operations only.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

// The runner reads `selected` and `list_requested`; this test does not.
#[allow(dead_code)]
#[path = "common/construct_mutation.rs"]
mod construct_mutation;

use construct_mutation::{CONSTRUCTS, NOT_MUTATED, mutated};
use regelrecht_engine::types::Operation;
use std::collections::BTreeSet;
use std::path::Path;

/// The schema definitions whose properties count as constructs, with
/// `execution` (a property of `machineReadableSection`) as one more.
const DEFINITIONS: &[&str] = &[
    "machineReadableSection",
    "baseField",
    "inputField",
    "parameterField",
    "outputField",
    "action",
    "foreachOperation",
    "producedItem",
];

fn schema() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema/latest/schema.json");
    let text = std::fs::read_to_string(&path).expect("schema/latest/schema.json is readable");
    serde_json::from_str(&text).expect("schema.json is JSON")
}

/// The properties a schema node declares, including those in its `allOf`.
fn properties(node: &serde_json::Value) -> Vec<String> {
    let own = node["properties"]
        .as_object()
        .into_iter()
        .flat_map(|p| p.keys());
    let inherited = node["allOf"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["properties"].as_object())
        .flat_map(|p| p.keys());
    own.chain(inherited).cloned().collect()
}

/// Every construct name the schema implies.
fn schema_constructs() -> BTreeSet<String> {
    let schema = schema();
    let definitions = &schema["definitions"];
    let mut names: BTreeSet<String> = Operation::SCHEMA_OPERATIONS
        .iter()
        .map(|op| op.name().to_string())
        .collect();
    for definition in DEFINITIONS {
        let node = &definitions[*definition];
        assert!(node.is_object(), "schema has no definition {definition}");
        names.extend(
            properties(node)
                .into_iter()
                .map(|p| format!("{definition}.{p}")),
        );
    }
    let execution = &definitions["machineReadableSection"]["properties"]["execution"];
    names.extend(
        properties(execution)
            .into_iter()
            .map(|p| format!("execution.{p}")),
    );
    names
}

fn classified() -> BTreeSet<&'static str> {
    CONSTRUCTS
        .iter()
        .map(|c| c.name)
        .chain(NOT_MUTATED.iter().map(|(name, _)| *name))
        .collect()
}

#[test]
fn every_schema_construct_is_mutated_or_has_a_reason() {
    let classified = classified();
    let missing: Vec<String> = schema_constructs()
        .into_iter()
        .filter(|name| !classified.contains(name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "Constructs in the schema that neither CONSTRUCTS nor NOT_MUTATED in \
         tests/common/construct_mutation.rs names: {missing:?}. Add a mutation, \
         or list the construct in NOT_MUTATED with the reason it has none."
    );
}

#[test]
fn every_listed_construct_is_in_the_schema() {
    let schema = schema_constructs();
    let stale: Vec<&str> = classified()
        .into_iter()
        .filter(|name| !schema.contains(*name))
        .collect();
    assert!(
        stale.is_empty(),
        "construct_mutation.rs names constructs the schema no longer has: {stale:?}"
    );
}

#[test]
fn no_construct_is_both_mutated_and_excused() {
    let mutated: BTreeSet<&str> = CONSTRUCTS.iter().map(|c| c.name).collect();
    let both: Vec<&str> = NOT_MUTATED
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| mutated.contains(name))
        .collect();
    assert!(both.is_empty(), "in CONSTRUCTS and NOT_MUTATED: {both:?}");
}

#[test]
fn every_reason_says_something() {
    for (name, reason) in NOT_MUTATED {
        assert!(!reason.trim().is_empty(), "{name} has no reason");
    }
}

// The mutations themselves: each must change what it names and keep the law
// loadable YAML, or the run measures the wrong thing.

fn construct(name: &str) -> &'static construct_mutation::Construct {
    CONSTRUCTS
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no construct {name}"))
}

fn law(machine_readable: &str) -> String {
    let indented: String = machine_readable
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect();
    format!("$id: test_mutation\narticles:\n  - number: '1'\n    machine_readable:\n{indented}")
}

fn mutate(name: &str, machine_readable: &str) -> (serde_yaml_ng::Value, usize) {
    let (yaml, sites) = mutated(&law(machine_readable), construct(name)).unwrap();
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).unwrap();
    (value["articles"][0]["machine_readable"].clone(), sites)
}

#[test]
fn an_operation_is_replaced_by_its_counterpart_at_every_depth() {
    let (mr, sites) = mutate(
        "ADD",
        "execution:\n  actions:\n    - output: x\n      operation: ADD\n      values:\n        - operation: ADD\n          values: [1, 2]\n        - 3\n",
    );
    assert_eq!(sites, 2);
    let action = &mr["execution"]["actions"][0];
    assert_eq!(action["operation"].as_str(), Some("SUBTRACT"));
    assert_eq!(action["values"][0]["operation"].as_str(), Some("SUBTRACT"));
}

#[test]
fn a_regelwerk_outside_the_synthetic_set_is_left_alone() {
    let real = law(
        "execution:\n  actions:\n    - output: x\n      operation: ADD\n      values: [1, 2]\n",
    )
    .replace("test_mutation", "zorgtoeslagwet");
    let (yaml, sites) = mutated(&real, construct("ADD")).unwrap();
    assert_eq!(sites, 0);
    assert_eq!(yaml, real);
}

#[test]
fn not_is_replaced_by_its_operand() {
    let (mr, sites) = mutate(
        "NOT",
        "execution:\n  actions:\n    - output: x\n      value:\n        operation: NOT\n        value: $flag\n",
    );
    assert_eq!(sites, 1);
    assert_eq!(
        mr["execution"]["actions"][0]["value"].as_str(),
        Some("$flag")
    );
}

#[test]
fn if_loses_its_first_case_and_falls_back_to_the_default() {
    let (mr, sites) = mutate(
        "IF",
        "execution:\n  actions:\n    - output: x\n      value:\n        operation: IF\n        cases:\n          - when: true\n            then: 1\n        default: 2\n",
    );
    assert_eq!(sites, 1);
    assert_eq!(mr["execution"]["actions"][0]["value"].as_i64(), Some(2));
}

#[test]
fn an_if_without_a_default_and_one_case_is_not_a_site() {
    let (_, sites) = mutate(
        "IF",
        "execution:\n  actions:\n    - output: x\n      value:\n        operation: IF\n        cases:\n          - when: true\n            then: 1\n",
    );
    assert_eq!(sites, 0);
}

#[test]
fn hooks_are_removed() {
    let (mr, sites) = mutate(
        "machineReadableSection.hooks",
        "hooks:\n  - hook_point: post_actions\n    applies_to: {}\n",
    );
    assert_eq!(sites, 1);
    assert!(mr.get("hooks").is_none());
}

#[test]
fn open_terms_lose_their_default() {
    let (mr, sites) = mutate(
        "machineReadableSection.open_terms",
        "open_terms:\n  - id: term\n    default:\n      actions: []\n",
    );
    assert_eq!(sites, 1);
    assert!(mr["open_terms"][0].get("default").is_none());
}

#[test]
fn numeric_definitions_go_up_by_one_in_both_forms() {
    let (mr, sites) = mutate(
        "machineReadableSection.definitions",
        "definitions:\n  bare: 10\n  structured:\n    value: 2.5\n  text: Minister\n",
    );
    assert_eq!(sites, 1, "one definitions block is one site");
    assert_eq!(mr["definitions"]["bare"].as_i64(), Some(11));
    assert_eq!(mr["definitions"]["structured"]["value"].as_f64(), Some(3.5));
    assert_eq!(mr["definitions"]["text"].as_str(), Some("Minister"));
}

#[test]
fn a_date_part_reads_the_next_component() {
    let (mr, _) = mutate(
        "DATE_PART",
        "execution:\n  actions:\n    - output: x\n      value:\n        operation: DATE_PART\n        date: $d\n        in: day\n",
    );
    assert_eq!(
        mr["execution"]["actions"][0]["value"]["in"].as_str(),
        Some("year")
    );
}

#[test]
fn foreach_combine_moves_to_its_counterpart() {
    let (mr, sites) = mutate(
        "foreachOperation.combine",
        "execution:\n  actions:\n    - output: x\n      value:\n        operation: FOREACH\n        collection: $c\n        as: i\n        body: $i\n        combine: ADD\n",
    );
    assert_eq!(sites, 1);
    assert_eq!(
        mr["execution"]["actions"][0]["value"]["combine"].as_str(),
        Some("MAX")
    );
}
