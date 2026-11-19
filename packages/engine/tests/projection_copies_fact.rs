//! A projection that writes neither `state` nor `steps` copies the fact
//! `per` names (schema v0.8.25): its state holds every field of that fact,
//! with the type the article declares, and its one step copies them from
//! the root.
//!
//! A fictional register records where someone is registered (article 1),
//! corrects the postcode of such a registration (article 2), and records
//! that a registration was wrong from the start, which revokes it
//! (article 3). The same aggregate is written twice: once as a copy, once
//! written out by hand. Every name here is made up.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::{Aggregate, ChronicleRecord, LawExecutionService, Value};

fn schema(version: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{version}/schema/{version}/schema.json"
    )
}

/// Article 1 requires the registration, with a nullable number, a string
/// that may be empty and a table; article 2 the correction of its postcode;
/// article 3 the finding that it was wrong, which revokes it; article 4 a
/// fact with a field called `id`, and article 5 one with a list without
/// columns. A fact without fields does not load at
/// all (RFC-051), so no copy meets one.
fn register() -> String {
    format!(
        r#"
$schema: {}
$id: test_kopie_register
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De inschrijving van een bewoner op een adres.
    machine_readable:
      requires:
        - fact: ingeschreven
          parameters: [bewoner, postcode, huisnummer, toevoeging, kamers]
      execution:
        parameters:
          - {{name: bewoner, type: string}}
          - {{name: postcode, type: string}}
          - {{name: huisnummer, type: number, nullable: true}}
          - {{name: toevoeging, type: string, nullable: true}}
          - name: kamers
            type: array
            columns:
              - {{name: naam, type: string}}
              - {{name: oppervlakte, type: number}}
  - number: '2'
    text: De postcode van een inschrijving wordt verbeterd.
    machine_readable:
      requires:
        - fact: postcode_verbeterd
          parameters: [postcode]
      execution:
        parameters:
          - {{name: postcode, type: string}}
  - number: '3'
    text: Een inschrijving was ten onrechte.
    machine_readable:
      requires:
        - fact: ten_onrechte
          parameters: [reden]
          effects:
            - revokes: refers_to
      execution:
        parameters:
          - {{name: reden, type: string}}
  - number: '4'
    text: Een feit met een veld id.
    machine_readable:
      requires:
        - fact: met_id
          parameters: [id]
      execution:
        parameters:
          - {{name: id, type: string}}
  - number: '5'
    text: Een feit met een lijst zonder kolommen.
    machine_readable:
      requires:
        - fact: met_lijst
          parameters: [regels]
      execution:
        parameters:
          - {{name: regels, type: array}}
"#,
        schema("v0.8.24")
    )
}

/// A reduction `id` whose article `inschrijving` plays one aggregate per
/// registration as `projection` (YAML under `projection:`), and whose
/// article `inschrijvingen` reads the aggregates of one resident.
fn reduction(version: &str, id: &str, projection: &str) -> String {
    format!(
        r#"
$schema: {}
$id: {id}
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: inschrijving
    text: Per inschrijving de velden.
    machine_readable:
      projection:
{projection}
  - number: inschrijvingen
    text: De inschrijvingen van een bewoner, als rijen.
    machine_readable:
      execution:
        parameters:
          - {{name: bewoner, type: string}}
        input:
          - name: rijen
            type: array
            source:
              projections: {id}#inschrijving
              where:
                bewoner: $bewoner
        output:
          - {{name: inschrijvingen, type: array}}
        actions:
          - {{output: inschrijvingen, value: $rijen}}
"#,
        schema(version)
    )
}

/// The copy: `per` alone.
const COPY: &str = "        per: test_kopie_register#1#ingeschreven";

/// What the copy stands for, written out by hand.
const BY_HAND: &str = r"        per: test_kopie_register#1#ingeschreven
        state:
          - {name: bewoner, type: string}
          - {name: postcode, type: string}
          - {name: huisnummer, type: number, nullable: true}
          - {name: toevoeging, type: string}
          - name: kamers
            type: array
            columns:
              - {name: naam, type: string}
              - {name: oppervlakte, type: number}
        steps:
          - record: test_kopie_register#1#ingeschreven
            actions:
              - {output: bewoner, value: $record.bewoner}
              - {output: postcode, value: $record.postcode}
              - {output: huisnummer, value: $record.huisnummer}
              - {output: toevoeging, value: $record.toevoeging}
              - {output: kamers, value: $record.kamers}";

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&register()).unwrap();
    service
        .load_law(&reduction("v0.8.25", "reductie_kopie", COPY))
        .unwrap();
    service
        .load_law(&reduction("v0.8.25", "reductie_met_de_hand", BY_HAND))
        .unwrap();
    service
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 3, d).unwrap()
}

fn fact(
    id: &str,
    (article, name): (&str, &str),
    recorded_on: NaiveDate,
    refers_to: Option<&str>,
    fields: Vec<(&str, Value)>,
) -> ChronicleRecord {
    ChronicleRecord {
        id: id.to_string(),
        kind: "EXECUTOGRAM".to_string(),
        basis: format!("test_kopie_register#{article}"),
        fact: Some(name.to_string()),
        recorded_on,
        refers_to: refers_to.map(str::to_string),
        fields: fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        outputs: BTreeMap::new(),
    }
}

fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

fn registration(
    id: &str,
    recorded_on: NaiveDate,
    bewoner: &str,
    postcode: &str,
) -> ChronicleRecord {
    let room = |naam: &str, oppervlakte: i64| {
        Value::Object(BTreeMap::from([
            ("naam".to_string(), text(naam)),
            ("oppervlakte".to_string(), Value::Int(oppervlakte)),
        ]))
    };
    fact(
        id,
        ("1", "ingeschreven"),
        recorded_on,
        None,
        vec![
            ("bewoner", text(bewoner)),
            ("postcode", text(postcode)),
            ("huisnummer", Value::Null),
            ("toevoeging", text("a")),
            (
                "kamers",
                Value::Array(vec![room("woonkamer", 30), room("keuken", 8)]),
            ),
        ],
    )
}

/// A chronicle with two registrations of one resident: the first gets its
/// postcode corrected, the second is found wrong from the start, and a
/// registration of another resident.
fn chronicle() -> Vec<ChronicleRecord> {
    vec![
        registration("r1", day(2), "bewoner_a", "1234AB"),
        registration("r2", day(3), "bewoner_a", "5678CD"),
        registration("r3", day(3), "bewoner_b", "9999ZZ"),
        fact(
            "c1",
            ("2", "postcode_verbeterd"),
            day(5),
            Some("r1"),
            vec![("postcode", text("1234AC"))],
        ),
        fact(
            "w1",
            ("3", "ten_onrechte"),
            day(7),
            Some("r2"),
            vec![("reden", text("verhuisd voor de inschrijving"))],
        ),
    ]
}

/// What an aggregate holds, in a form two of them compare on.
fn comparable(aggregates: Vec<Aggregate>) -> Vec<(String, String)> {
    aggregates
        .into_iter()
        .map(|a| {
            (
                a.root,
                format!("{:?} {:?} {:?}", a.state, a.set_by, a.events),
            )
        })
        .collect()
}

fn aggregates(service: &LawExecutionService, reduction: &str, on: &str) -> Vec<(String, String)> {
    comparable(
        service
            .aggregates(
                &format!("{reduction}#inschrijving"),
                &chronicle(),
                &BTreeMap::new(),
                on,
            )
            .unwrap(),
    )
}

/// On every day, before and after the correction and the revocation, the
/// copy gives the aggregates the reduction written out by hand gives: the
/// same state, the same record behind each property, the same events.
#[test]
fn a_copy_plays_as_the_projection_written_out() {
    let service = service();
    for on in [
        "2025-03-01",
        "2025-03-02",
        "2025-03-04",
        "2025-03-06",
        "2025-03-08",
    ] {
        assert_eq!(
            aggregates(&service, "reductie_kopie", on),
            aggregates(&service, "reductie_met_de_hand", on),
            "the copy and the written-out projection differ on {on}"
        );
    }
}

/// The copy holds every field of the root with its type, a correction that
/// refers to the root sets nothing, and a revoked root keeps the start
/// values.
#[test]
fn a_copy_holds_the_fields_of_the_root_alone() {
    let service = service();
    let played = service
        .aggregates(
            "reductie_kopie#inschrijving",
            &chronicle(),
            &BTreeMap::new(),
            "2025-03-08",
        )
        .unwrap();
    let of = |root: &str| played.iter().find(|a| a.root == root).unwrap();

    let first = of("r1");
    assert_eq!(first.state["postcode"], text("1234AB"));
    assert_eq!(first.state["huisnummer"], Value::Null);
    assert_eq!(first.state["toevoeging"], text("a"));
    assert!(matches!(&first.state["kamers"], Value::Array(rows) if rows.len() == 2));
    assert_eq!(first.set_by["postcode"].as_deref(), Some("r1"));

    // Revoked from the start: no step applies, every property keeps its
    // start value.
    let revoked = of("r2");
    assert_eq!(revoked.state["bewoner"], Value::Null);
    assert_eq!(revoked.state["kamers"], Value::Array(Vec::new()));
    assert!(revoked.set_by.values().all(Option::is_none));
}

/// A read model reads the aggregates of a copy as those of any projection,
/// filtered on a field of the root.
#[test]
fn a_read_model_reads_a_copy() {
    let service = service();
    let rows = |reduction: &str, on: &str| {
        service
            .evaluate_projection(
                reduction,
                &["inschrijvingen"],
                &chronicle(),
                BTreeMap::from([("bewoner".to_string(), text("bewoner_a"))]),
                on,
                false,
            )
            .unwrap()
            .result
            .outputs["inschrijvingen"]
            .clone()
    };
    for on in ["2025-03-04", "2025-03-08"] {
        assert_eq!(rows("reductie_kopie", on), rows("reductie_met_de_hand", on));
    }
    let Value::Array(after) = rows("reductie_kopie", "2025-03-08") else {
        panic!("the read model gives rows");
    };
    let postcodes: Vec<&Value> = after
        .iter()
        .map(|row| match row {
            Value::Object(row) => &row["postcode"],
            other => panic!("a row is an object, not {other:?}"),
        })
        .collect();
    assert_eq!(postcodes, [&text("1234AB"), &Value::Null]);
}

/// The reduction may load before the register: the copy is written out
/// when the projection plays.
#[test]
fn a_copy_loads_before_the_law_of_its_fact() {
    let mut service = LawExecutionService::new();
    service
        .load_law(&reduction("v0.8.25", "reductie_kopie", COPY))
        .unwrap();
    service.load_law(&register()).unwrap();
    let played = service
        .aggregates(
            "reductie_kopie#inschrijving",
            &chronicle(),
            &BTreeMap::new(),
            "2025-03-08",
        )
        .unwrap();
    assert_eq!(played.len(), 3);
}

/// Load `projection` as the one article `inschrijving` of a reduction on
/// `version`, after the register, and return the error. Without the read
/// model, whose `where` names a field of the registration, the error is
/// the projection's own.
fn refused(version: &str, projection: &str) -> String {
    let aggregate_only = reduction(version, "reductie_kopie", projection);
    let aggregate_only = aggregate_only
        .split("  - number: inschrijvingen")
        .next()
        .unwrap();
    let mut service = LawExecutionService::new();
    service.load_law(&register()).unwrap();
    service
        .load_law(aggregate_only)
        .expect_err("the loader refuses the projection")
        .to_string()
}

fn assert_mentions(error: &str, parts: &[&str]) {
    for part in parts {
        assert!(error.contains(part), "'{part}' is missing from: {error}");
    }
}

#[test]
fn a_copy_of_a_fact_the_article_does_not_require_is_refused() {
    let error = refused(
        "v0.8.25",
        "        per: test_kopie_register#1#uitgeschreven",
    );
    assert_mentions(
        &error,
        &[
            "inschrijving",
            "test_kopie_register#1#uitgeschreven",
            "requires no fact 'uitgeschreven'",
        ],
    );
}

#[test]
fn a_copy_of_a_field_called_id_is_refused() {
    let error = refused("v0.8.25", "        per: test_kopie_register#4#met_id");
    assert_mentions(
        &error,
        &[
            "inschrijving",
            "test_kopie_register#4#met_id",
            "the name under which a row of the aggregates holds its root",
        ],
    );
}

/// A copied field meets the rules of a property written by hand: in the
/// state an array holds its columns.
#[test]
fn a_copy_of_an_array_without_columns_is_refused() {
    let error = refused("v0.8.25", "        per: test_kopie_register#5#met_lijst");
    assert_mentions(
        &error,
        &[
            "inschrijving",
            "test_kopie_register#5#met_lijst",
            "'regels'",
            "without columns",
        ],
    );
}

#[test]
fn a_copy_needs_per_to_name_a_fact() {
    let error = refused("v0.8.25", "        per: test_kopie_register#1");
    assert_mentions(&error, &["inschrijving", "names no fact"]);
}

/// A state without steps, or steps without a state, is neither a copy nor
/// a projection written out.
#[test]
fn half_a_projection_is_refused() {
    let state_only = "        per: test_kopie_register#1#ingeschreven\n        state:\n          - {name: bewoner, type: string}";
    assert_mentions(
        &refused("v0.8.25", state_only),
        &["inschrijving", "a state and no steps"],
    );
    let steps_only = "        per: test_kopie_register#1#ingeschreven\n        steps:\n          - record: test_kopie_register#1#ingeschreven\n            actions:\n              - {output: bewoner, value: $record.bewoner}";
    assert_mentions(
        &refused("v0.8.25", steps_only),
        &["inschrijving", "steps and no state"],
    );
}

#[test]
fn a_copy_before_schema_v0_8_25_is_refused() {
    let error = refused("v0.8.24", COPY);
    assert_mentions(&error, &["inschrijving", "v0.8.25"]);
}

/// Loaded before its register, a copy of a fact the register does not
/// require loads, and fails when it plays, naming the article and the fact.
#[test]
fn a_copy_of_an_unknown_fact_fails_when_it_plays() {
    let mut service = LawExecutionService::new();
    service
        .load_law(&reduction(
            "v0.8.25",
            "reductie_kopie",
            "        per: test_kopie_register#1#uitgeschreven",
        ))
        .unwrap();
    service.load_law(&register()).unwrap();
    let error = service
        .aggregates(
            "reductie_kopie#inschrijving",
            &chronicle(),
            &BTreeMap::new(),
            "2025-03-08",
        )
        .expect_err("the play refuses the copy")
        .to_string();
    assert_mentions(
        &error,
        &[
            "inschrijving",
            "test_kopie_register#1#uitgeschreven",
            "requires no fact",
        ],
    );
}
