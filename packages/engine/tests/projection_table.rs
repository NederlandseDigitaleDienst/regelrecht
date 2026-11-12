//! A table in the state of a projection (schema v0.8.20).
//!
//! A fictional law records one fact per polling station: the result, with a
//! table of the lists and the votes each received. A later fact corrects a
//! result and refers to it. The reduction plays one aggregate per result,
//! with the table in its state; a read model iterates over the tables of
//! every aggregate with a nested FOREACH. Every name here is made up.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::{ChronicleRecord, LawExecutionService, ProjectionEvent, Value};

const SCHEMA: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.20/schema/v0.8.20/schema.json";

/// The columns of a table of lists, as the law declares them and the state
/// declares them too.
const COLUMNS: &str = "[{name: aanduiding, type: string}, {name: aantal_stemmen, type: number}]";

/// Article 1 requires the result of a polling station, with its table of
/// lists; article 2 requires a correction of a result, with a new table.
fn law(columns: &str) -> String {
    format!(
        r#"
$schema: {SCHEMA}
$id: test_uitslagen
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het stembureau stelt de uitslag vast, met per lijst het aantal stemmen.
    machine_readable:
      requires:
        - fact: uitslag
          parameters: [stembureau, lijsten]
      execution:
        parameters:
          - {{name: stembureau, type: string}}
          - {{name: lijsten, type: array, columns: {columns}}}
  - number: '2'
    text: Een vastgestelde uitslag kan worden gecorrigeerd.
    machine_readable:
      requires:
        - fact: correctie
          parameters: [lijsten]
      execution:
        parameters:
          - {{name: lijsten, type: array, columns: {columns}}}
"#
    )
}

/// One aggregate per result, its table replaced by a correction, and two
/// read models over the aggregates: the votes of one list over every
/// polling station, and the number of lists that received any vote.
fn reduction(version: &str, state_columns: &str) -> String {
    format!(
        r#"
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{version}/schema/{version}/schema.json
$id: reductie_uitslagen
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: uitslag
    text: t
    machine_readable:
      projection:
        per: test_uitslagen#1
        state:
          - {{name: stembureau, type: string}}
          - {{name: lijsten, type: table, columns: {state_columns}}}
        steps:
          - record: test_uitslagen#1
            actions:
              - {{output: stembureau, value: $record.stembureau}}
              - {{output: lijsten, value: $record.lijsten}}
          - record: test_uitslagen#2
            actions:
              - {{output: lijsten, value: $record.lijsten}}
  - number: stemmen_op_lijst
    text: t
    machine_readable:
      execution:
        parameters: [{{name: aanduiding, type: string}}]
        input:
          - name: uitslagen
            type: array
            source: {{projections: reductie_uitslagen#uitslag}}
        output:
          - {{name: stemmen_op_lijst, type: number}}
          - {{name: lijsten_met_stemmen, type: number}}
        actions:
          - output: stemmen_op_lijst
            value:
              operation: FOREACH
              collection: $uitslagen
              as: u
              combine: ADD
              body:
                operation: FOREACH
                collection: $u.lijsten
                as: l
                filter: {{operation: EQUALS, subject: $l.aanduiding, value: $aanduiding}}
                body: $l.aantal_stemmen
                combine: ADD
          - output: lijsten_met_stemmen
            value:
              operation: FOREACH
              collection: $uitslagen
              as: u
              combine: ADD
              body:
                operation: FOREACH
                collection: $u.lijsten
                as: l
                filter: {{operation: GREATER_THAN, subject: $l.aantal_stemmen, value: 0}}
                body: 1
                combine: ADD
"#
    )
}

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&law(COLUMNS)).unwrap();
    service.load_law(&reduction("v0.8.20", COLUMNS)).unwrap();
    service
}

/// A row of a table of lists.
fn list(aanduiding: &str, stemmen: i64) -> Value {
    Value::Object(BTreeMap::from([
        (
            "aanduiding".to_string(),
            Value::String(aanduiding.to_string()),
        ),
        ("aantal_stemmen".to_string(), Value::Int(stemmen)),
    ]))
}

fn fact(
    id: &str,
    article: &str,
    name: &str,
    refers_to: Option<&str>,
    fields: Vec<(&str, Value)>,
) -> ChronicleRecord {
    ChronicleRecord {
        id: id.to_string(),
        kind: "EXECUTOGRAM".to_string(),
        basis: format!("test_uitslagen#{article}"),
        fact: Some(name.to_string()),
        recorded_on: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        refers_to: refers_to.map(str::to_string),
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        outputs: BTreeMap::new(),
    }
}

/// The result of polling station `stembureau`, with `lijsten`.
fn result(id: &str, stembureau: &str, lijsten: Vec<Value>) -> ChronicleRecord {
    fact(
        id,
        "1",
        "uitslag",
        None,
        vec![
            ("stembureau", Value::String(stembureau.to_string())),
            ("lijsten", Value::Array(lijsten)),
        ],
    )
}

/// A correction of result `of`, with a new table.
fn correction(id: &str, of: &str, lijsten: Vec<Value>) -> ChronicleRecord {
    fact(
        id,
        "2",
        "correctie",
        Some(of),
        vec![("lijsten", Value::Array(lijsten))],
    )
}

/// The outputs of the read model over `records`, for list `aanduiding`.
fn read(
    service: &LawExecutionService,
    records: &[ChronicleRecord],
    aanduiding: &str,
) -> (Value, Value) {
    let outcome = service
        .evaluate_projection(
            "reductie_uitslagen",
            &["stemmen_op_lijst", "lijsten_met_stemmen"],
            records,
            BTreeMap::from([(
                "aanduiding".to_string(),
                Value::String(aanduiding.to_string()),
            )]),
            "2025-03-01",
            false,
        )
        .unwrap();
    (
        outcome.result.outputs["stemmen_op_lijst"].clone(),
        outcome.result.outputs["lijsten_met_stemmen"].clone(),
    )
}

/// Each result is an aggregate whose state holds its table. The read model
/// sums the votes of one list over the tables of every aggregate, the inner
/// FOREACH reading the table of the outer row (`$u.lijsten`).
#[test]
fn a_read_model_iterates_over_the_table_of_each_aggregate() {
    let service = service();
    let records = [
        result("a", "noord", vec![list("L1", 120), list("L2", 80)]),
        result("b", "zuid", vec![list("L1", 30), list("L3", 0)]),
    ];
    assert_eq!(
        read(&service, &records, "L1"),
        (Value::Int(150), Value::Int(3))
    );
    assert_eq!(
        read(&service, &records, "L2"),
        (Value::Int(80), Value::Int(3))
    );
    assert_eq!(
        read(&service, &records, "L4"),
        (Value::Int(0), Value::Int(3))
    );

    let rows = service
        .projection_rows(
            "reductie_uitslagen#uitslag",
            &records,
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap();
    let Value::Array(rows) = rows else {
        panic!("rows: {rows:?}");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        Value::Object(BTreeMap::from([
            ("id".to_string(), Value::String("a".to_string())),
            ("stembureau".to_string(), Value::String("noord".to_string())),
            (
                "lijsten".to_string(),
                Value::Array(vec![list("L1", 120), list("L2", 80)])
            ),
        ]))
    );
}

/// Before any step the table is empty, so an aggregate whose root sets no
/// table yet counts nothing; a row lacking a column holds null there.
#[test]
fn a_table_starts_empty_and_a_missing_column_is_null() {
    let service = service();
    let partial = Value::Object(BTreeMap::from([(
        "aanduiding".to_string(),
        Value::String("L1".to_string()),
    )]));
    let records = [result("a", "noord", vec![partial])];
    let rows = service
        .projection_rows(
            "reductie_uitslagen#uitslag",
            &records,
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap();
    let Value::Array(rows) = rows else {
        panic!("rows: {rows:?}");
    };
    let Value::Object(row) = &rows[0] else {
        panic!("row: {:?}", rows[0]);
    };
    assert_eq!(
        row["lijsten"],
        Value::Array(vec![Value::Object(BTreeMap::from([
            ("aanduiding".to_string(), Value::String("L1".to_string())),
            ("aantal_stemmen".to_string(), Value::Null),
        ]))])
    );
    assert_eq!(read(&service, &[], "L1"), (Value::Int(0), Value::Int(0)));
}

/// A correction refers to the result it corrects, so it plays in that
/// result's aggregate, and its step replaces the whole table. The other
/// aggregate keeps its own table.
#[test]
fn a_later_record_replaces_the_table_of_its_aggregate() {
    let service = service();
    let records = [
        result("a", "noord", vec![list("L1", 120), list("L2", 80)]),
        result("b", "zuid", vec![list("L1", 30)]),
        correction(
            "c",
            "a",
            vec![list("L1", 102), list("L2", 98), list("L3", 5)],
        ),
    ];
    assert_eq!(
        read(&service, &records, "L1"),
        (Value::Int(132), Value::Int(4))
    );
    assert_eq!(
        read(&service, &records, "L2"),
        (Value::Int(98), Value::Int(4))
    );
    assert_eq!(
        read(&service, &records, "L3"),
        (Value::Int(5), Value::Int(4))
    );

    let aggregates = service
        .aggregates(
            "reductie_uitslagen#uitslag",
            &records,
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap();
    let a = aggregates.iter().find(|a| a.root == "a").unwrap();
    assert_eq!(a.set_by["lijsten"].as_deref(), Some("c"));
    assert_eq!(a.set_by["stembureau"].as_deref(), Some("a"));
    let changed: Vec<&str> = a
        .events
        .iter()
        .filter_map(|e| match e {
            ProjectionEvent::Applied {
                record, changes, ..
            } if changes.iter().any(|c| c.property == "lijsten") => Some(record.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(changed, ["a", "c"]);
}

/// A row with a column the table does not declare fails the step that
/// sets it, naming the row and the column.
#[test]
fn a_row_with_an_undeclared_column_is_refused_when_played() {
    let service = service();
    let row = Value::Object(BTreeMap::from([
        ("aanduiding".to_string(), Value::String("L1".to_string())),
        ("zetels".to_string(), Value::Int(2)),
    ]));
    let err = service
        .evaluate_projection(
            "reductie_uitslagen",
            &["stemmen_op_lijst"],
            &[result("a", "noord", vec![row])],
            BTreeMap::from([("aanduiding".to_string(), Value::String("L1".to_string()))]),
            "2025-03-01",
            false,
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("it sets 'lijsten' from $record.lijsten but row 1: it holds 'zetels', which is no column of the table"),
        "{err}"
    );
}

/// The loader holds the columns of the table to the columns of the fact
/// field the step fills it from: the same names, each of the same type.
#[test]
fn the_loader_holds_the_columns_to_the_fact() {
    for (state_columns, why) in [
        (
            "[{name: aanduiding, type: string}, {name: aantal_stemmen, type: amount}]",
            "fills table 'lijsten' from $record.lijsten, whose column 'aantal_stemmen' is a Number, not a Amount",
        ),
        (
            "[{name: aanduiding, type: string}]",
            "fills table 'lijsten' from $record.lijsten, which has a column 'aantal_stemmen' the table does not declare",
        ),
        (
            "[{name: aanduiding, type: string}, {name: aantal_stemmen, type: number}, {name: zetels, type: number}]",
            "fills table 'lijsten' from $record.lijsten, which has no column 'zetels'",
        ),
    ] {
        let mut service = LawExecutionService::new();
        service.load_law(&law(COLUMNS)).unwrap();
        let err = service
            .load_law(&reduction("v0.8.20", state_columns))
            .unwrap_err()
            .to_string();
        assert!(err.contains(why), "{state_columns}: {err}");
    }
    // A fact field that declares no columns leaves nothing to compare.
    let mut service = LawExecutionService::new();
    service
        .load_law(&law(COLUMNS).replace(&format!(", columns: {COLUMNS}"), ""))
        .unwrap();
    service
        .load_law(&reduction("v0.8.20", "[{name: aanduiding, type: string}]"))
        .unwrap();
}

/// A table in the state came with schema v0.8.20; a file on v0.8.19 that
/// has one is refused. The table declares its columns, each once and of a
/// scalar type, and only a parameter of type array declares columns.
#[test]
fn the_loader_refuses_a_malformed_table() {
    let load = |law_text: &str, reduction_text: &str| {
        let mut service = LawExecutionService::new();
        service.load_law(law_text)?;
        service.load_law(reduction_text)
    };
    let refused = |law_text: String, reduction_text: String, why: &str| {
        let err = load(&law_text, &reduction_text).unwrap_err().to_string();
        assert!(err.contains(why), "{why}: {err}");
    };
    refused(
        law(COLUMNS),
        reduction("v0.8.19", COLUMNS),
        "gives state property 'lijsten' the type table, which came with schema v0.8.20",
    );
    refused(
        law(COLUMNS),
        reduction("v0.8.20", COLUMNS).replace(&format!(", columns: {COLUMNS}}}"), "}"),
        "gives state property 'lijsten' the type table without its columns",
    );
    refused(
        law(COLUMNS),
        reduction(
            "v0.8.20",
            "[{name: a, type: string}, {name: a, type: number}]",
        ),
        "state property 'lijsten' names column 'a' twice",
    );
    refused(
        law(COLUMNS),
        reduction("v0.8.20", "[{name: a, type: array}]"),
        "state property 'lijsten' gives column 'a' the type Array",
    );
    refused(
        law(COLUMNS),
        reduction("v0.8.20", COLUMNS).replace(
            "{name: stembureau, type: string}",
            "{name: stembureau, type: string, columns: [{name: a, type: string}]}",
        ),
        "declares columns for state property 'stembureau', which is no table",
    );
    refused(
        law(COLUMNS).replace(
            "{name: stembureau, type: string}",
            "{name: stembureau, type: string, columns: [{name: a, type: string}]}",
        ),
        reduction("v0.8.20", COLUMNS),
        "parameter 'stembureau' declares columns, but is a String; only an array is a table",
    );
}

/// A table filled from another table of the state is held to that table's
/// columns, as a table filled from a fact is.
#[test]
fn a_table_filled_from_another_table_of_the_state_has_its_columns() {
    let with_copy = |columns: &str| {
        reduction("v0.8.20", COLUMNS)
            .replace(
                "          - {name: stembureau, type: string}\n",
                &format!(
                    "          - {{name: stembureau, type: string}}\n          - {{name: vorige, type: table, columns: {columns}}}\n"
                ),
            )
            .replace(
                "          - record: test_uitslagen#2\n            actions:\n",
                "          - record: test_uitslagen#2\n            actions:\n              - {output: vorige, value: $state.lijsten}\n",
            )
    };
    let mut service = LawExecutionService::new();
    service.load_law(&law(COLUMNS)).unwrap();
    service.load_law(&with_copy(COLUMNS)).unwrap();
    let records = [
        result("a", "noord", vec![list("L1", 120)]),
        correction("c", "a", vec![list("L1", 102)]),
    ];
    let aggregates = service
        .aggregates(
            "reductie_uitslagen#uitslag",
            &records,
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap();
    assert_eq!(
        aggregates[0].state["vorige"],
        Value::Array(vec![list("L1", 120)])
    );

    let mut service = LawExecutionService::new();
    service.load_law(&law(COLUMNS)).unwrap();
    let err = service
        .load_law(&with_copy("[{name: aanduiding, type: string}]"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("fills table 'vorige' from $state.lijsten, which has a column 'aantal_stemmen' the table does not declare"),
        "{err}"
    );
}

/// A fact field that declares no columns may hold anything; a value that is
/// no list of rows fails the step that puts it in a table.
#[test]
fn a_value_that_is_no_list_of_rows_is_refused_when_played() {
    let mut service = LawExecutionService::new();
    service
        .load_law(&law(COLUMNS).replace(&format!(", columns: {COLUMNS}"), ""))
        .unwrap();
    service.load_law(&reduction("v0.8.20", COLUMNS)).unwrap();
    let err = service
        .aggregates(
            "reductie_uitslagen#uitslag",
            &[result("a", "noord", vec![Value::Int(7)])],
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("it sets 'lijsten' from $record.lijsten but row 1: Int(7) is no row"),
        "{err}"
    );
}

/// A state property of a type the schema does not know is refused when the
/// reduction parses, naming the type.
#[test]
fn an_unknown_state_type_is_refused() {
    let mut service = LawExecutionService::new();
    service.load_law(&law(COLUMNS)).unwrap();
    let err = service
        .load_law(&reduction("v0.8.20", COLUMNS).replace(
            "{name: stembureau, type: string}",
            "{name: stembureau, type: euro}",
        ))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(
            "unknown state type `euro`, expected table, boolean, number, string, amount or date"
        ),
        "{err}"
    );
}
