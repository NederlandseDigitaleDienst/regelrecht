//! Every row of the signature table has a conformance scenario (RFC-055).
//!
//! The scenarios in `bdd/conformance/signatures.feature` are written by hand,
//! from the RFC and not from the table, so that they check the table instead
//! of repeating it. That leaves completeness unguarded: a row added to the
//! table without a scenario would pass every test. This test closes that gap.
//! A scenario that shows an operation accepting a row carries the tag
//! `@signature:<OPERATION>:<row>`, and every row of an operation listed here
//! must have one.

use std::path::Path;

use regelrecht_engine::signatures::{Operands, signatures};
use regelrecht_engine::types::Operation;

/// The operations whose signatures the engine enforces and the suite covers.
/// The list grows with each step of RFC-055 until it holds every operation of
/// the schema.
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
];

/// The name of a row in a tag: the kinds it takes, or the shape it names.
fn row_name(operands: Operands) -> String {
    match operands {
        Operands::Each(kind) | Operands::Any(kind) | Operands::One(kind) => kind.to_string(),
        Operands::Two(first, second) => format!("{first}-{second}"),
        Operands::Alike => "alike".to_string(),
        Operands::MemberOf => "member-of".to_string(),
        Operands::Cases => "cases".to_string(),
        Operands::Values => "values".to_string(),
        Operands::Collection => "collection".to_string(),
        Operands::DateAndCounts => "date-and-counts".to_string(),
        Operands::DateAndPart => "date-and-part".to_string(),
        Operands::DatesAndUnit => "dates-and-unit".to_string(),
    }
}

#[test]
fn every_row_of_a_covered_operation_has_a_scenario() {
    let feature =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bdd/conformance/signatures.feature");
    let text = std::fs::read_to_string(&feature)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", feature.display()));
    let tags: Vec<&str> = text
        .split_whitespace()
        .filter(|word| word.starts_with("@signature:"))
        .collect();

    let mut missing = Vec::new();
    let mut expected = Vec::new();
    for operation in COVERED {
        for signature in signatures(*operation) {
            let tag = format!(
                "@signature:{}:{}",
                operation.name(),
                row_name(signature.operands)
            );
            if !tags.contains(&tag.as_str()) {
                missing.push(tag.clone());
            }
            expected.push(tag);
        }
    }
    assert!(
        missing.is_empty(),
        "rows of the signature table without a scenario in {}: {missing:?}",
        feature.display()
    );

    // A tag that names no row is a scenario for a row that is gone or
    // misspelled; it would count as coverage for nothing.
    let unknown: Vec<&&str> = tags
        .iter()
        .filter(|tag| !expected.iter().any(|known| known == **tag))
        .collect();
    assert!(
        unknown.is_empty(),
        "tags that name no row of the table: {unknown:?}"
    );
}

#[test]
fn every_covered_operation_has_a_scenario_that_refuses_operands() {
    let feature =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bdd/conformance/signatures.feature");
    let text = std::fs::read_to_string(&feature)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", feature.display()));
    let without: Vec<&str> = COVERED
        .iter()
        .map(|operation| operation.name())
        .filter(|name| !text.contains(&format!("\"{name} is not defined for (")))
        .collect();
    assert!(
        without.is_empty(),
        "operations without a scenario that refuses operands: {without:?}"
    );
}
