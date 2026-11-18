//! A value that enters a declared field is held against the type the field
//! declares (RFC-055, "Where a value meets a declaration"): a parameter when
//! the article is called, an input when it is first read. A string in the
//! form `YYYY-MM-DD` becomes a date in a field declared `date`.
//!
//! The law here is a toy, written for the engine.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use regelrecht_engine::{EngineError, LawExecutionService, Value};
use serde_json::json;

const DATE: &str = "2025-06-01";

const LAW: &str = r#"
$id: wet_velden
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De registratie bevat de geboortedatum en de leeftijd.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: dag
            type: date
          - name: namen
            type: array
          - name: code
            type: string
        input:
          - name: geboortedatum
            type: date
            source: {}
          - name: sterfdatum
            type: date
            nullable: true
            source: {}
          - name: leeftijd
            type: number
            source: {}
        output:
          - name: dag_terug
            type: date
          - name: geboortejaar
            type: number
          - name: sterfdatum_terug
            type: date
            nullable: true
          - name: leeftijd_terug
            type: number
          - name: namen_terug
            type: array
          - name: code_terug
            type: string
        actions:
          - output: dag_terug
            value: $dag
          - output: geboortejaar
            value:
              operation: DATE_PART
              date: $geboortedatum
              in: year
          - output: leeftijd_terug
            value: $leeftijd
          - output: sterfdatum_terug
            value: $sterfdatum
          - output: namen_terug
            value: $namen
          - output: code_terug
            value: $code
"#;

fn params(value: serde_json::Value) -> BTreeMap<String, Value> {
    match Value::from(value) {
        Value::Object(map) => map,
        other => panic!("not an object: {other:?}"),
    }
}

/// A service whose register holds one row for person `1`.
fn service(row: serde_json::Value) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(LAW).unwrap();
    let mut row = params(row);
    row.insert("bsn".to_string(), Value::String("1".to_string()));
    service
        .register_dict_source_for_law("wet_velden", "register", "bsn", vec![row], 10)
        .unwrap();
    service
}

fn output(
    service: &LawExecutionService,
    name: &str,
    parameters: serde_json::Value,
) -> Result<Value, EngineError> {
    service
        .evaluate_law_output("wet_velden", name, params(parameters), DATE)
        .map(|result| result.outputs[name].clone())
}

#[test]
fn a_date_written_as_text_is_a_date_once_it_is_in_a_date_parameter() {
    let service = service(json!({ "leeftijd": 30 }));
    let dag = output(
        &service,
        "dag_terug",
        json!({ "bsn": "1", "dag": "2025-03-01" }),
    )
    .unwrap();
    assert_eq!(dag, Value::Date("2025-03-01".parse().unwrap()));
}

#[test]
fn text_that_is_no_calendar_day_is_refused_in_a_date_parameter() {
    let service = service(json!({ "leeftijd": 30 }));
    for no_date in ["2025-02-30", "1 maart 2025", ""] {
        let error =
            output(&service, "dag_terug", json!({ "bsn": "1", "dag": no_date })).unwrap_err();
        assert!(
            matches!(&error, EngineError::InvalidDate(message)
                if message.contains("parameter 'dag' of wet_velden")),
            "{no_date:?}: {error}"
        );
    }
}

#[test]
fn a_value_of_another_type_is_refused_in_a_parameter() {
    let service = service(json!({ "leeftijd": 30 }));
    for (parameters, expected, actual) in [
        (json!({ "bsn": 1 }), "string for parameter 'bsn'", "number"),
        (
            json!({ "bsn": "1", "dag": 20250301 }),
            "date for parameter 'dag'",
            "number",
        ),
        (
            json!({ "bsn": "1", "dag": true }),
            "date for parameter 'dag'",
            "boolean",
        ),
        (
            json!({ "bsn": "1", "namen": "Ada" }),
            "array for parameter 'namen'",
            "string",
        ),
    ] {
        let error = output(&service, "leeftijd_terug", parameters.clone()).unwrap_err();
        assert!(
            matches!(&error, EngineError::TypeMismatch { expected: e, actual: a }
                if e.contains(expected) && a == actual),
            "{parameters}: {error}"
        );
    }
}

#[test]
fn a_list_enters_a_parameter_declared_an_array() {
    let service = service(json!({ "leeftijd": 30 }));
    let namen = output(
        &service,
        "namen_terug",
        json!({ "bsn": "1", "namen": ["Ada", "Bob"] }),
    )
    .unwrap();
    assert_eq!(namen, Value::from(json!(["Ada", "Bob"])));
}

#[test]
fn a_date_a_source_delivers_as_text_is_a_date_once_read() {
    let service = service(json!({ "leeftijd": 30, "geboortedatum": "1995-04-12" }));
    let jaar = output(&service, "geboortejaar", json!({ "bsn": "1" })).unwrap();
    assert_eq!(jaar, Value::Int(1995));
}

#[test]
fn a_source_value_of_another_type_is_refused_when_the_input_is_read() {
    let service = service(json!({ "leeftijd": "dertig", "geboortedatum": "12-04-1995" }));
    let error = output(&service, "leeftijd_terug", json!({ "bsn": "1" })).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("expected number for input 'leeftijd' of wet_velden, got string"),
        "{error}"
    );
    let error = output(&service, "geboortejaar", json!({ "bsn": "1" })).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("input 'geboortedatum' of wet_velden is declared a date"),
        "{error}"
    );
}

#[test]
fn an_input_that_is_not_read_is_not_checked() {
    // `leeftijd` holds text, and nothing that is asked for reads it (RFC-043).
    let service = service(json!({ "leeftijd": "dertig", "geboortedatum": "1995-04-12" }));
    let jaar = output(&service, "geboortejaar", json!({ "bsn": "1" })).unwrap();
    assert_eq!(jaar, Value::Int(1995));
}

#[test]
fn an_absence_is_left_to_the_nullability_check() {
    let service = service(json!({ "leeftijd": 30, "sterfdatum": null }));
    let dag = output(&service, "sterfdatum_terug", json!({ "bsn": "1" })).unwrap();
    assert_eq!(dag, Value::Null);
}

#[test]
fn a_value_passed_under_the_name_of_an_input_is_held_against_the_input_when_read() {
    let service = service(json!({ "leeftijd": 30, "geboortedatum": "1995-04-12" }));
    // Passed, it takes the place of what the register holds.
    let jaar = output(
        &service,
        "geboortejaar",
        json!({ "bsn": "1", "geboortedatum": "2001-01-01" }),
    )
    .unwrap();
    assert_eq!(jaar, Value::Int(2001));
    let error = output(
        &service,
        "leeftijd_terug",
        json!({ "bsn": "1", "leeftijd": "dertig" }),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("expected number for input 'leeftijd' of wet_velden, got string"),
        "{error}"
    );
    // Nothing that is asked for reads `leeftijd`, so what was passed for it
    // is not looked at (RFC-043).
    let jaar = output(
        &service,
        "geboortejaar",
        json!({ "bsn": "1", "leeftijd": "dertig" }),
    )
    .unwrap();
    assert_eq!(jaar, Value::Int(1995));
}

#[test]
fn the_reference_date_enters_a_date_parameter_as_its_day() {
    let service = service(json!({ "leeftijd": 30 }));
    let reference = json!({ "iso": "2025-06-01", "year": 2025, "month": 6, "day": 1 });
    let dag = output(
        &service,
        "dag_terug",
        json!({ "bsn": "1", "dag": reference }),
    )
    .unwrap();
    assert_eq!(dag, Value::Date("2025-06-01".parse().unwrap()));
    // A record without a day is no date.
    let error = output(
        &service,
        "dag_terug",
        json!({ "bsn": "1", "dag": { "year": 2025 } }),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("expected date for parameter 'dag' of wet_velden, got object"),
        "{error}"
    );
}

#[test]
fn a_date_enters_a_string_parameter_as_its_text() {
    let service = service(json!({ "leeftijd": 30 }));
    let mut parameters = params(json!({ "bsn": "1" }));
    parameters.insert(
        "code".to_string(),
        Value::Date("2025-03-01".parse().unwrap()),
    );
    let code = service
        .evaluate_law_output("wet_velden", "code_terug", parameters, DATE)
        .unwrap()
        .outputs["code_terug"]
        .clone();
    assert_eq!(code, Value::String("2025-03-01".to_string()));
}
