//! Every row of the signature table has a conformance scenario (RFC-055).
//!
//! The scenarios in `bdd/conformance/signatures.feature` are written by hand,
//! from the RFC and not from the table, so that they check the table instead
//! of repeating it. That leaves completeness unguarded: a row added to the
//! table without a scenario would pass every test. This test closes that gap.
//!
//! A scenario that shows an operation accepting a row carries the tag
//! `@signature:<name>:<row>` on the line above it. The tag counts only on a
//! scenario that evaluates something and asserts an output, so a tag in a
//! comment, or on a scenario that merely succeeds, covers nothing. Each
//! covered name also needs a scenario that fails with its refusal.

#![allow(clippy::panic)]

use std::path::Path;

use regelrecht_engine::article::CombineOp;
use regelrecht_engine::signatures::{
    Operands, Positional, Signature, combine_signatures, condition, signatures,
};
use regelrecht_engine::types::Operation;

/// The operations whose signatures the engine enforces and the suite covers:
/// every operation of the schema. A new operation is added here with its
/// rows and its scenarios.
const COVERED: &[Operation] = &[
    Operation::Add,
    Operation::Subtract,
    Operation::Multiply,
    Operation::Divide,
    Operation::Min,
    Operation::Max,
    Operation::Round,
    Operation::Ceil,
    Operation::Floor,
    Operation::Equals,
    Operation::GreaterThan,
    Operation::LessThan,
    Operation::GreaterThanOrEqual,
    Operation::LessThanOrEqual,
    Operation::And,
    Operation::Or,
    Operation::Not,
    Operation::If,
    Operation::In,
    Operation::List,
    Operation::Record,
    Operation::ForEach,
    Operation::Date,
    Operation::DateAdd,
    Operation::DateDiff,
    Operation::Age,
    Operation::DatePart,
    Operation::DayOfWeek,
    Operation::StartOf,
];

/// The name of a row in a tag: its shape and the kinds it takes, so two rows
/// of one operation never share a name.
fn row_name(operands: Operands) -> String {
    match operands {
        Operands::Positional(Positional::Each(kind) | Positional::Any(kind)) => kind.to_string(),
        Operands::Positional(Positional::One(kind)) => kind.to_string(),
        Operands::Positional(Positional::Two(first, second)) => format!("{first}-{second}"),
        Operands::Positional(Positional::Three(kind)) => format!("{kind}-{kind}-{kind}"),
        Operands::Positional(Positional::Alike) => "alike".to_string(),
        Operands::MemberOf => "member-of".to_string(),
        Operands::Cases => "cases".to_string(),
        Operands::Values => "values".to_string(),
        Operands::Collection => "collection".to_string(),
        Operands::DateAndCounts => "date-and-counts".to_string(),
        Operands::DateAndPart => "date-and-part".to_string(),
        Operands::DatesAndUnit => "dates-and-unit".to_string(),
    }
}

/// Every covered place a row applies to: the name its tag carries, the name
/// its refusal carries in an error, and its rows.
fn covered_rows() -> Vec<(String, String, &'static [Signature])> {
    let mut rows: Vec<(String, String, &'static [Signature])> = COVERED
        .iter()
        .map(|operation| {
            let name = operation.name().to_string();
            (name.clone(), name, signatures(*operation))
        })
        .collect();
    rows.push((
        "FOREACH-filter".to_string(),
        "FOREACH filter".to_string(),
        condition(),
    ));
    for combine in [
        CombineOp::Add,
        CombineOp::And,
        CombineOp::Or,
        CombineOp::Min,
        CombineOp::Max,
        CombineOp::Only,
        CombineOp::Distinct,
    ] {
        rows.push((
            format!("combine-{}", combine.as_str()),
            format!("FOREACH combine {}", combine.as_str()),
            combine_signatures(combine),
        ));
    }
    rows
}

/// One scenario of the feature: the tags on the line above it and its steps.
struct Scenario {
    tags: Vec<String>,
    steps: Vec<String>,
}

impl Scenario {
    /// Whether the scenario evaluates something and asserts an output: what a
    /// tag needs to count as showing a row at work.
    fn asserts_an_output(&self) -> bool {
        self.steps
            .iter()
            .any(|step| step.starts_with("When I evaluate"))
            && self
                .steps
                .iter()
                .any(|step| step.starts_with("Then output "))
    }

    /// Whether the scenario fails with `message`.
    fn fails_with(&self, message: &str) -> bool {
        self.steps.iter().any(|step| {
            step.strip_prefix("Then the execution fails with \"")
                .is_some_and(|rest| rest.starts_with(message))
        })
    }
}

/// The scenarios of the feature. A tag belongs to the scenario on the next
/// line; a comment line holds no tag.
fn scenarios() -> Vec<Scenario> {
    let feature =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bdd/conformance/signatures.feature");
    let text = std::fs::read_to_string(&feature)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", feature.display()));
    let mut scenarios: Vec<Scenario> = Vec::new();
    let mut pending_tags: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('@') {
            pending_tags.extend(line.split_whitespace().map(str::to_string));
        } else if line.starts_with("Scenario:") {
            scenarios.push(Scenario {
                tags: std::mem::take(&mut pending_tags),
                steps: Vec::new(),
            });
        } else if line.starts_with('#') || line.is_empty() {
            pending_tags.clear();
        } else if let Some(scenario) = scenarios.last_mut() {
            scenario.steps.push(line.to_string());
        }
    }
    scenarios
}

#[test]
fn every_row_of_a_covered_operation_has_a_scenario() {
    let scenarios = scenarios();
    let shown: Vec<&str> = scenarios
        .iter()
        .filter(|scenario| scenario.asserts_an_output())
        .flat_map(|scenario| scenario.tags.iter().map(String::as_str))
        .filter(|tag| tag.starts_with("@signature:"))
        .collect();

    let expected: Vec<String> = covered_rows()
        .iter()
        .flat_map(|(tag_name, _, rows)| {
            rows.iter()
                .map(move |row| format!("@signature:{tag_name}:{}", row_name(row.operands)))
        })
        .collect();
    let missing: Vec<&String> = expected
        .iter()
        .filter(|tag| !shown.contains(&tag.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "rows of the signature table without a scenario that asserts an output: {missing:?}"
    );

    // A tag that names no row is a scenario for a row that is gone or
    // misspelled; it would count as coverage for nothing.
    let unknown: Vec<&str> = scenarios
        .iter()
        .flat_map(|scenario| scenario.tags.iter().map(String::as_str))
        .filter(|tag| tag.starts_with("@signature:"))
        .filter(|tag| !expected.iter().any(|known| known == tag))
        .collect();
    assert!(
        unknown.is_empty(),
        "tags that name no row of the table: {unknown:?}"
    );
}

#[test]
fn every_covered_operation_has_a_scenario_that_refuses_operands() {
    let scenarios = scenarios();
    // A place whose rows take values of any kind (`LIST`, `RECORD`, a
    // `FOREACH` collection, `combine: ONLY`) has nothing to refuse.
    let refuses = |rows: &[Signature]| {
        rows.iter()
            .any(|row| !matches!(row.operands, Operands::Values | Operands::Collection))
    };
    let without: Vec<String> = covered_rows()
        .into_iter()
        .filter(|(_, _, rows)| refuses(rows))
        .map(|(_, error_name, _)| error_name)
        // `combine: DISTINCT` takes values of one kind, whatever it is: its
        // refusal is that of the equality it compares with.
        .chain(std::iter::once("FOREACH combine DISTINCT".to_string()))
        .filter(|name| {
            let refusal = format!("{name} is not defined for (");
            !scenarios
                .iter()
                .any(|scenario| scenario.fails_with(&refusal))
        })
        .collect();
    assert!(
        without.is_empty(),
        "operations without a scenario that refuses operands: {without:?}"
    );
}

#[test]
fn every_operation_of_the_schema_is_covered() {
    let missing: Vec<&str> = Operation::SCHEMA_OPERATIONS
        .iter()
        .filter(|operation| !COVERED.contains(operation))
        .map(|operation| operation.name())
        .collect();
    assert!(
        missing.is_empty(),
        "operations of the schema that the signature scenarios do not cover: {missing:?}"
    );
}

#[test]
fn a_tag_counts_only_on_a_scenario_that_asserts_an_output() {
    let succeeds_only = Scenario {
        tags: vec!["@signature:ADD:array".to_string()],
        steps: vec![
            "When I evaluate \"lijst\" of \"test_signaturen\"".to_string(),
            "Then the execution succeeds".to_string(),
        ],
    };
    assert!(!succeeds_only.asserts_an_output());
    let asserts = Scenario {
        tags: Vec::new(),
        steps: vec![
            "When I evaluate \"som\" of \"test_signaturen\"".to_string(),
            "Then output \"som\" equals 5".to_string(),
        ],
    };
    assert!(asserts.asserts_an_output());
    assert!(!asserts.fails_with("ADD is not defined for ("));
}
