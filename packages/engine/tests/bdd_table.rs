//! Unit tests for the BDD data-table parsing shared with the editor's JS
//! mirror (`frontend/src/gherkin/actions.js`). The `bdd` target itself runs
//! `harness = false` with `test = false`, so these live in their own target.

#![allow(dead_code, clippy::panic, clippy::expect_used, clippy::unwrap_used)]

#[path = "bdd/helpers/value_conversion.rs"]
mod value_conversion;

#[path = "bdd/helpers/table.rs"]
mod table;

use regelrecht_engine::Value;
use table::{Rows, rows_to_params, rows_to_records};
use value_conversion::convert_gherkin_value;

fn rows(raw: &[&[&str]]) -> Rows {
    raw.iter()
        .map(|row| row.iter().map(|c| (*c).to_string()).collect())
        .collect()
}

#[test]
fn records_use_the_header_row() {
    let records = rows_to_records(
        &rows(&[
            &["naam", "leeftijd", "verzekerd"],
            &["Jansen", "30", "true"],
        ]),
        convert_gherkin_value,
    );

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].get("naam"),
        Some(&Value::String("Jansen".to_string()))
    );
    assert_eq!(records[0].get("leeftijd"), Some(&Value::Int(30)));
    assert_eq!(records[0].get("verzekerd"), Some(&Value::Bool(true)));
}

#[test]
fn an_empty_cell_leaves_the_key_out_of_the_record() {
    // RFC-036: an empty cell is "nobody has this fact" and the engine resolves
    // the input as unknown; the literal `null` is "the register says none" and
    // stays a Null value. A whitespace-only cell counts as empty.
    let records = rows_to_records(
        &rows(&[
            &["bsn", "huur", "partner_bsn", "beschikking"],
            &["1", "", "null", "   "],
        ]),
        convert_gherkin_value,
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].get("bsn"), Some(&Value::Int(1)));
    assert!(!records[0].contains_key("huur"));
    assert_eq!(records[0].get("partner_bsn"), Some(&Value::Null));
    assert!(!records[0].contains_key("beschikking"));
    // The cell converter itself is unchanged: on its own, "" is still Null.
    assert_eq!(convert_gherkin_value(""), Value::Null);
}

#[test]
fn an_empty_parameter_cell_leaves_the_parameter_out() {
    // RFC-036: the empty cell means one thing in every table. In a parameter
    // table it is "not passed" (an optional parameter is then unknown for lack
    // of it, a required one is the caller's omission); the word `null` passes
    // an absence. A whitespace-only cell counts as empty.
    let params = rows_to_params(
        &rows(&[
            &["bsn", "999993653"],
            &["aanvraag_bedrag", ""],
            &["partner_bsn", "null"],
            &["huur", "   "],
        ]),
        convert_gherkin_value,
    );

    assert_eq!(params.get("bsn"), Some(&Value::Int(999993653)));
    assert!(!params.contains_key("aanvraag_bedrag"));
    assert_eq!(params.get("partner_bsn"), Some(&Value::Null));
    assert!(!params.contains_key("huur"));
    assert_eq!(params.len(), 2);
}

#[test]
#[should_panic(expected = "data table row 1 has 2 cells, header row has 3")]
fn a_short_row_is_rejected_instead_of_dropping_a_column() {
    rows_to_records(
        &rows(&[&["naam", "leeftijd", "verzekerd"], &["Jansen", "30"]]),
        convert_gherkin_value,
    );
}

#[test]
#[should_panic(expected = "data table row 2 has 4 cells, header row has 3")]
fn a_long_row_is_rejected_too() {
    rows_to_records(
        &rows(&[
            &["naam", "leeftijd", "verzekerd"],
            &["Jansen", "30", "true"],
            &["De Vries", "40", "false", "extra"],
        ]),
        convert_gherkin_value,
    );
}
