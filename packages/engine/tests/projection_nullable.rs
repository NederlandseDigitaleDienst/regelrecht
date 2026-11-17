//! A property of the state of a projection that may be empty (schema
//! v0.8.20, `nullable`).
//!
//! A fictional law records one fact per result of an election in a
//! district, with the number of inhabitants where the district has one; a
//! later fact sets that number for a result that lacked it. The reduction
//! plays one aggregate per result, with the number nullable in its state; a
//! read model adds up the numbers that are there and counts the results
//! without one. Every name here is made up.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::{ChronicleRecord, LawExecutionService, Value};

fn schema(version: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{version}/schema/{version}/schema.json"
    )
}

/// Article 1 requires the result of a district, with its number of
/// inhabitants or null where it has none; article 2 requires the number set
/// later for a result, never null.
fn law() -> String {
    format!(
        r#"
$schema: {}
$id: test_inwoners
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het hoofdstembureau stelt de uitslag van de kieskring vast, met het inwonertal als dat er is.
    machine_readable:
      requires:
        - fact: uitslag
          parameters: [kieskring, inwonertal]
      execution:
        parameters:
          - {{name: kieskring, type: string}}
          - {{name: inwonertal, type: number, nullable: true}}
  - number: '2'
    text: Het inwonertal van een kieskring wordt later vastgesteld.
    machine_readable:
      requires:
        - fact: inwonertal_vastgesteld
          parameters: [inwonertal]
      execution:
        parameters:
          - {{name: inwonertal, type: number}}
"#,
        schema("v0.8.20")
    )
}

/// One aggregate per result, with `inwonertal` as `state_property` (a YAML
/// flow mapping without the name), and a read model that adds up the
/// numbers that are there and counts the results without one.
fn reduction(version: &str, state_property: &str) -> String {
    format!(
        r#"
$schema: {}
$id: reductie_inwoners
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: uitslag
    text: t
    machine_readable:
      projection:
        per: test_inwoners#1
        state:
          - {{name: kieskring, type: string}}
          - {{name: inwonertal, {state_property}}}
        steps:
          - record: test_inwoners#1
            actions:
              - {{output: kieskring, value: $record.kieskring}}
              - {{output: inwonertal, value: $record.inwonertal}}
          - record: test_inwoners#2
            actions:
              - {{output: inwonertal, value: $record.inwonertal}}
  - number: inwoners
    text: t
    machine_readable:
      execution:
        input:
          - name: uitslagen
            type: array
            source: {{projections: reductie_inwoners#uitslag}}
        output:
          - {{name: totaal_inwoners, type: number}}
          - {{name: zonder_inwonertal, type: number}}
        actions:
          - output: totaal_inwoners
            value:
              operation: FOREACH
              collection: $uitslagen
              as: u
              filter:
                operation: NOT
                value: {{operation: EQUALS, subject: $u.inwonertal, value: null}}
              body: $u.inwonertal
              combine: ADD
          - output: zonder_inwonertal
            value:
              operation: FOREACH
              collection: $uitslagen
              as: u
              filter: {{operation: EQUALS, subject: $u.inwonertal, value: null}}
              body: 1
              combine: ADD
"#,
        schema(version)
    )
}

const NULLABLE: &str = "type: number, nullable: true";

fn service(state_property: &str) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&law()).unwrap();
    service
        .load_law(&reduction("v0.8.20", state_property))
        .unwrap();
    service
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
        basis: format!("test_inwoners#{article}"),
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

/// The result of district `kieskring`, with `inwonertal` (null for none).
fn result(id: &str, kieskring: &str, inwonertal: Value) -> ChronicleRecord {
    fact(
        id,
        "1",
        "uitslag",
        None,
        vec![
            ("kieskring", Value::String(kieskring.to_string())),
            ("inwonertal", inwonertal),
        ],
    )
}

/// The number of inhabitants set later for result `of`.
fn set_later(id: &str, of: &str, inwonertal: i64) -> ChronicleRecord {
    fact(
        id,
        "2",
        "inwonertal_vastgesteld",
        Some(of),
        vec![("inwonertal", Value::Int(inwonertal))],
    )
}

fn read(service: &LawExecutionService, records: &[ChronicleRecord]) -> (Value, Value) {
    let outcome = service
        .evaluate_projection(
            "reductie_inwoners",
            &["totaal_inwoners", "zonder_inwonertal"],
            records,
            BTreeMap::new(),
            "2025-03-01",
            false,
        )
        .unwrap();
    (
        outcome.result.outputs["totaal_inwoners"].clone(),
        outcome.result.outputs["zonder_inwonertal"].clone(),
    )
}

fn inwonertal_of(service: &LawExecutionService, records: &[ChronicleRecord], root: &str) -> Value {
    service
        .aggregates(
            "reductie_inwoners#uitslag",
            records,
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap()
        .into_iter()
        .find(|a| a.root == root)
        .unwrap()
        .state["inwonertal"]
        .clone()
}

/// A nullable number stays empty when the fact holds none, a later step
/// fills it, and the read model skips the empty one and counts it apart.
#[test]
fn a_nullable_number_stays_empty_until_a_later_step_fills_it() {
    let service = service(NULLABLE);
    let records = [
        result("a", "noord", Value::Null),
        result("b", "zuid", Value::Int(1000)),
    ];
    assert_eq!(inwonertal_of(&service, &records, "a"), Value::Null);
    assert_eq!(read(&service, &records), (Value::Int(1000), Value::Int(1)));

    let later = [
        result("a", "noord", Value::Null),
        result("b", "zuid", Value::Int(1000)),
        set_later("c", "a", 500),
    ];
    assert_eq!(inwonertal_of(&service, &later, "a"), Value::Int(500));
    assert_eq!(read(&service, &later), (Value::Int(1500), Value::Int(0)));
}

/// A nullable number starts as null, not as 0: here no step sets it.
#[test]
fn a_nullable_number_starts_as_null() {
    let untouched = reduction("v0.8.20", NULLABLE)
        .replace("              - {output: inwonertal, value: $record.inwonertal}\n          - record: test_inwoners#2", "          - record: test_inwoners#2");
    let mut service = LawExecutionService::new();
    service.load_law(&law()).unwrap();
    service.load_law(&untouched).unwrap();
    let records = [result("a", "noord", Value::Int(7))];
    assert_eq!(inwonertal_of(&service, &records, "a"), Value::Null);
}

/// Without `nullable` a number keeps starting at 0, and a null that reaches
/// it when the step plays is refused, as before schema v0.8.20.
#[test]
fn a_number_that_is_not_nullable_refuses_null_when_played() {
    let mut service = LawExecutionService::new();
    service
        .load_law(&law().replace(", nullable: true", ""))
        .unwrap();
    service
        .load_law(&reduction("v0.8.20", "type: number"))
        .unwrap();
    let err = service
        .aggregates(
            "reductie_inwoners#uitslag",
            &[result("a", "noord", Value::Null)],
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("it sets 'inwonertal' from $record.inwonertal to Null, which is no Number"),
        "{err}"
    );
}

/// The loader refuses a step that fills a property that is not nullable
/// from a fact field that is: the step would fail on the first record
/// without a value. A string or a date may always be null.
#[test]
fn the_loader_refuses_a_nullable_field_in_a_property_that_is_not() {
    let mut service = LawExecutionService::new();
    service.load_law(&law()).unwrap();
    let err = service
        .load_law(&reduction("v0.8.20", "type: number"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(
            "sets 'inwonertal' from $record.inwonertal, which may be null, but 'inwonertal' is not nullable"
        ),
        "{err}"
    );
    // A file on v0.8.19 keeps loading as it did; its steps are checked
    // against null only when they play.
    service
        .load_law(&reduction("v0.8.19", "type: number"))
        .unwrap();
}

/// `nullable` in the state came with schema v0.8.20, and an array in the
/// state is never null: it starts as an empty list.
#[test]
fn the_loader_refuses_nullable_where_it_does_not_apply() {
    let refused = |reduction_text: String, why: &str| {
        let mut service = LawExecutionService::new();
        service.load_law(&law()).unwrap();
        let err = service.load_law(&reduction_text).unwrap_err().to_string();
        assert!(err.contains(why), "{why}: {err}");
    };
    refused(
        reduction("v0.8.19", NULLABLE),
        "declares state property 'inwonertal' nullable, which came with schema v0.8.20",
    );
    refused(
        reduction("v0.8.20", NULLABLE).replace(
            "{name: kieskring, type: string}",
            "{name: kieskring, type: string}\n          - {name: rijen, type: array, nullable: true, columns: [{name: a, type: string}]}",
        ),
        "declares state property 'rijen' nullable; an array in the state is never null, it starts as an empty list",
    );
}

/// Every source a step reads is held to the same rule: a nullable output of
/// a decision, a nullable property of the state and an input the step
/// declares nullable may not fill a number that is not nullable.
#[test]
fn every_nullable_source_is_refused_for_a_property_that_is_not() {
    let extra = r#"  - number: '3'
    text: Het college stelt het inwonertal opnieuw vast.
    machine_readable:
      execution:
        produces: [{type: BESCHIKKING, outputs: [herzien]}]
        output: [{name: herzien, type: number, nullable: true}]
        actions: [{output: herzien, value: null}]
  - number: '4'
    text: Een inwonertal dat niet bekend is.
    machine_readable:
      execution:
        output: [{name: onbekend, type: number, nullable: true}]
        actions: [{output: onbekend, value: null}]
"#;
    let law_text = format!("{}{extra}", law().replace(", nullable: true", ""));
    let step_on_3 = "          - record: test_inwoners#3\n            actions:\n              - {output: inwonertal, value: $record.herzien}\n";
    let base = reduction("v0.8.20", "type: number");
    let cases = [
        (
            base.replace(
                "  - number: inwoners\n",
                &format!("{step_on_3}  - number: inwoners\n"),
            ),
            "sets 'inwonertal' from $record.herzien, which may be null",
        ),
        (
            base.replace(
                "          - {name: kieskring, type: string}\n",
                "          - {name: kieskring, type: string}\n          - {name: vorig, type: number, nullable: true}\n",
            )
            .replace(
                "              - {output: inwonertal, value: $record.inwonertal}\n  - number",
                "              - {output: inwonertal, value: $state.vorig}\n  - number",
            ),
            "sets 'inwonertal' from $state.vorig, which may be null",
        ),
        (
            base.replace(
                "          - record: test_inwoners#2\n            actions:\n              - {output: inwonertal, value: $record.inwonertal}\n",
                "          - record: test_inwoners#2\n            input:\n              - {name: n, type: number, nullable: true, source: {regulation: test_inwoners, output: onbekend}}\n            actions:\n              - {output: inwonertal, value: $n}\n",
            ),
            "sets 'inwonertal' from $n, which may be null",
        ),
    ];
    for (reduction_text, why) in cases {
        assert_ne!(reduction_text, base, "{why}: the case changes nothing");
        let mut service = LawExecutionService::new();
        service.load_law(&law_text).unwrap();
        let err = service.load_law(&reduction_text).unwrap_err().to_string();
        assert!(err.contains(why), "{why}: {err}");
        // The same source fills a nullable number.
        let mut service = LawExecutionService::new();
        service.load_law(&law_text).unwrap();
        service
            .load_law(&reduction_text.replace(
                "{name: inwonertal, type: number}",
                "{name: inwonertal, type: number, nullable: true}",
            ))
            .unwrap();
    }
}

/// The rule against a nullable source holds from schema v0.8.20 also when
/// the projection plays: a reduction on v0.8.19 plays a record with a value
/// as before, and a reduction on v0.8.20 that loaded before its law, so the
/// loader could not check it, is refused when it plays.
#[test]
fn the_rule_against_a_nullable_source_also_holds_when_it_plays() {
    let records = [result("a", "noord", Value::Int(1000))];
    let mut older = LawExecutionService::new();
    older.load_law(&law()).unwrap();
    older
        .load_law(&reduction("v0.8.19", "type: number"))
        .unwrap();
    assert_eq!(inwonertal_of(&older, &records, "a"), Value::Int(1000));

    let mut early = LawExecutionService::new();
    early
        .load_law(&reduction("v0.8.20", "type: number"))
        .unwrap();
    early.load_law(&law()).unwrap();
    let err = early
        .aggregates(
            "reductie_inwoners#uitslag",
            &records,
            &BTreeMap::new(),
            "2025-03-01",
        )
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("sets 'inwonertal' from $record.inwonertal, which may be null"),
        "{err}"
    );
}

/// `nullable: false` says nothing new, but it is still `nullable`: refused
/// under v0.8.19, and allowed on an array under v0.8.20.
#[test]
fn nullable_false_is_still_nullable() {
    let mut service = LawExecutionService::new();
    service.load_law(&law()).unwrap();
    let err = service
        .load_law(&reduction("v0.8.19", "type: number, nullable: false"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(
            "declares state property 'inwonertal' nullable, which came with schema v0.8.20"
        ),
        "{err}"
    );
    service
        .load_law(&reduction("v0.8.20", NULLABLE).replace(
            "{name: kieskring, type: string}",
            "{name: kieskring, type: string}\n          - {name: rijen, type: array, nullable: false, columns: [{name: a, type: string}]}",
        ))
        .unwrap();
}
