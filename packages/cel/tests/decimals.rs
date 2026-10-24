//! The chronicle keeps a number as the engine computed it: a decision
//! output that is a decimal comes back from the file as the same decimal,
//! with all its digits, and an integer as an integer. So the engine can
//! compare a stored output with one computed anew.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_cel::{Cell, Chronicle, Gram, Reference};
use regelrecht_engine::{LawExecutionService, Value};
use regelrecht_law_model::stored;
use serde_json::{json, Map};

mod common;
use common::cell_tempdir;

/// A procedure for a beschikking.
const GENERAL_LAW: &str = r#"
$id: wet_algemeen_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
procedure:
  - id: beschikking
    default: true
    applies_to: {legal_character: BESCHIKKING}
    stages:
      - name: AANVRAAG
      - name: BESLUIT
      - name: BEKENDMAKING
articles: []
"#;

/// Article 1 establishes an application and decides on it: a share, a
/// division that is a decimal, and a count, which is an integer. Article 2
/// compares two records, as an article that reads a stored decision would;
/// article 4 calls the article a decision value names anew
/// (`source.article`) and says whether the outcome is another.
const LAW: &str = r#"
$id: wet_aandeel_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Op aanvraag wordt het aandeel vastgesteld.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_aandeel_fictief#1
            parameters: [teller, noemer]
          - type: BESCHIKKING
            outputs: [aandeel, aantal]
        parameters:
          - {name: teller, type: number}
          - {name: noemer, type: number}
        output:
          - {name: aandeel, type: number}
          - {name: aantal, type: number}
        actions:
          - output: aandeel
            value: {operation: DIVIDE, values: [$teller, $noemer]}
          - output: aantal
            value: {operation: ADD, values: [$teller, $noemer]}
  - number: '2'
    text: Twee besluiten worden vergeleken.
    machine_readable:
      execution:
        parameters:
          - {name: opgeslagen, type: object}
          - {name: vers, type: object}
        output:
          - {name: gelijk, type: boolean}
        actions:
          - output: gelijk
            value: {operation: EQUALS, subject: $opgeslagen, value: $vers}
  - number: '3'
    text: Een aanvraag gaat vergezeld van een bijlage.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            parameters: [bijlage]
            outputs: [gemeld]
        parameters:
          - {name: bijlage, type: object}
        output:
          - {name: gemeld, type: boolean}
        actions:
          - {output: gemeld, value: true}
  - number: '4'
    text: Een besluit wordt opnieuw genomen en vergeleken met het oude.
    machine_readable:
      execution:
        parameters:
          - {name: besluit, type: decision}
        input:
          - name: opnieuw
            type: decision
            nullable: true
            source:
              article: $besluit.basis
              parameters_from: [$besluit.fields]
        output:
          - {name: anders, type: boolean}
        actions:
          - output: anders
            value:
              operation: NOT_EQUALS
              subject: $opnieuw.outputs
              value: $besluit.outputs
  - number: '5'
    text: Op verzoek wordt een besluit opnieuw genomen en vergeleken met het oude.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_aandeel_fictief#5
            parameters: [besluit]
          - type: BESCHIKKING
            outputs: [anders]
        parameters:
          - {name: besluit, type: decision}
        input:
          - name: opnieuw
            type: decision
            nullable: true
            source:
              article: $besluit.basis
              parameters_from: [$besluit.fields]
        output:
          - {name: anders, type: boolean}
        actions:
          - output: anders
            value:
              operation: NOT_EQUALS
              subject: $opnieuw.outputs
              value: $besluit.outputs
"#;

const ID: &str = "wet_aandeel_fictief";
const ON: &str = "2026-10-13";
const ACTOR: &str = "fictieve_gemeente";

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(GENERAL_LAW).unwrap();
    service.load_law(LAW).unwrap();
    service
}

fn day() -> NaiveDate {
    NaiveDate::parse_from_str(ON, "%Y-%m-%d").unwrap()
}

fn numbers(teller: i64, noemer: i64) -> Map<String, serde_json::Value> {
    json!({"teller": teller, "noemer": noemer})
        .as_object()
        .unwrap()
        .clone()
}

/// Decide on `teller / noemer` in a chronicle file, open the file again
/// and return the decision as a decision value `{basis, fields,
/// findings, outputs}`, read back the way the engine reads it.
fn stored_decision(service: &mut LawExecutionService, teller: i64, noemer: i64) -> Value {
    let dir = cell_tempdir();
    let path = dir.path().join("kroniek.jsonl");
    let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
    let app = cell
        .record_submission(service, &[], ID, "1", None, &numbers(teller, noemer), day())
        .unwrap();
    cell.decide(
        service,
        &[],
        ID,
        "1",
        &app.id,
        &Map::new(),
        &Map::new(),
        day(),
    )
    .unwrap();

    let reopened = Cell::open(ACTOR, &path, dir.path()).unwrap();
    let [application, decision] = reopened.grams() else {
        panic!("not an application and a decision")
    };
    let value = json!({
        "basis": decision.basis,
        "taken_on": application.basis,
        "fields": application.fields,
        "findings": {},
        "outputs": decision.outputs,
    });
    stored::from_stored(&value).unwrap()
}

/// The outputs of the decision on `teller / noemer`, stored in a chronicle
/// file and read back.
fn stored(service: &mut LawExecutionService, teller: i64, noemer: i64) -> BTreeMap<String, Value> {
    match stored_decision(service, teller, noemer) {
        Value::Object(mut value) => match value.remove("outputs") {
            Some(Value::Object(outputs)) => outputs,
            other => panic!("no outputs: {other:?}"),
        },
        other => panic!("not a decision value: {other:?}"),
    }
}

/// The outputs of article 1 for `teller / noemer`, computed anew.
fn fresh(service: &LawExecutionService, teller: i64, noemer: i64) -> BTreeMap<String, Value> {
    let parameters = [
        ("teller".to_string(), Value::Int(teller)),
        ("noemer".to_string(), Value::Int(noemer)),
    ]
    .into();
    service
        .evaluate_law(ID, &["aandeel", "aantal"], parameters, ON)
        .unwrap()
        .outputs
        .into_iter()
        .collect()
}

/// Whether article 2 finds `{name: a}` equal to `{name: b}`, or the
/// engine's error when it refuses to say.
fn equals(service: &LawExecutionService, name: &str, a: &Value, b: &Value) -> Result<bool, String> {
    let record = |v: &Value| Value::Object([(name.to_string(), v.clone())].into());
    let parameters = [
        ("opgeslagen".to_string(), record(a)),
        ("vers".to_string(), record(b)),
    ]
    .into();
    let result = service
        .evaluate_law(ID, &["gelijk"], parameters, ON)
        .map_err(|e| e.to_string())?;
    Ok(result.outputs["gelijk"].as_bool().unwrap())
}

/// Article 2 refuses to compare `{name: a}` with `{name: b}`: an integer
/// against a decimal of the same value.
fn refuses(service: &LawExecutionService, name: &str, a: &Value, b: &Value) {
    let err = equals(service, name, a, b).expect_err("an integer against the same decimal");
    assert!(
        err.contains("numbers written alike") && err.contains(&format!(".{name}")),
        "{err}"
    );
}

fn decimal(value: &Value) -> String {
    match value {
        Value::Decimal(d) => d.to_string(),
        other => panic!("{other:?} is not a decimal"),
    }
}

#[test]
fn a_whole_decimal_comes_back_as_a_decimal() {
    let mut service = service();
    let stored = stored(&mut service, 10, 2);
    assert_eq!(
        decimal(&stored["aandeel"]),
        decimal(&fresh(&service, 10, 2)["aandeel"])
    );
    assert_eq!(
        equals(
            &service,
            "aandeel",
            &stored["aandeel"],
            &fresh(&service, 10, 2)["aandeel"]
        ),
        Ok(true)
    );
}

#[test]
fn a_fraction_keeps_all_its_digits() {
    let mut service = service();
    let stored = stored(&mut service, 1, 3);
    assert_eq!(
        decimal(&stored["aandeel"]),
        "0.3333333333333333333333333333"
    );
    assert_eq!(
        equals(
            &service,
            "aandeel",
            &stored["aandeel"],
            &fresh(&service, 1, 3)["aandeel"]
        ),
        Ok(true)
    );
}

#[test]
fn an_integer_comes_back_as_an_integer() {
    let mut service = service();
    let stored = stored(&mut service, 10, 2);
    assert!(
        matches!(stored["aantal"], Value::Int(12)),
        "{:?}",
        stored["aantal"]
    );
    assert_eq!(
        equals(
            &service,
            "aantal",
            &stored["aantal"],
            &fresh(&service, 10, 2)["aantal"]
        ),
        Ok(true)
    );
}

#[test]
fn an_integer_against_a_decimal_is_an_error() {
    // The chronicle does not make a decimal of an integer, nor the other
    // way round, so the engine's EQUALS on records still sees the
    // difference: an integer 5 against a decimal 5.0 is an error, not an
    // answer either way.
    let mut service = service();
    let stored = stored(&mut service, 10, 2);
    let aandeel = &fresh(&service, 10, 2)["aandeel"];
    refuses(&service, "n", &Value::Int(5), aandeel);
    refuses(&service, "n", &Value::Int(5), &stored["aandeel"]);
    refuses(
        &service,
        "n",
        &stored["aantal"],
        &Value::Decimal("12.0".parse().unwrap()),
    );
}

#[test]
fn a_stored_decision_called_anew_gives_no_false_difference() {
    // Article 4 calls the article the stored decision names anew, with the
    // fields of its application, and compares the outcomes as records: a
    // whole decimal, a fraction with all its digits, and an integer.
    for (teller, noemer) in [(10, 2), (1, 3)] {
        let mut service = service();
        let besluit = stored_decision(&mut service, teller, noemer);
        let parameters = [("besluit".to_string(), besluit)].into();
        let result = service
            .evaluate_law(ID, &["anders"], parameters, ON)
            .unwrap();
        assert_eq!(
            result.outputs["anders"],
            Value::Bool(false),
            "{teller}/{noemer}"
        );
    }
    // As plain JSON numbers, which the chronicle wrote before, the same
    // decisions differ falsely: 1/3 lost digits, and 5.0 came back as 5.
    let mut service = service();
    for (teller, noemer) in [(10, 2), (1, 3)] {
        let Value::Object(mut besluit) = stored_decision(&mut service, teller, noemer) else {
            panic!("not a decision value")
        };
        let plain = serde_json::Value::from(&besluit["outputs"]);
        besluit.insert("outputs".into(), Value::from(plain));
        let parameters = [("besluit".to_string(), Value::Object(besluit))].into();
        match service.evaluate_law(ID, &["anders"], parameters, ON) {
            // 1/3: a float with fewer digits, so another value.
            Ok(result) => assert_eq!(
                result.outputs["anders"],
                Value::Bool(true),
                "{teller}/{noemer}"
            ),
            // 10/2: the integer 5 against the decimal 5.0.
            Err(e) => assert!(
                (teller, noemer) == (10, 2) && e.to_string().contains("numbers written alike"),
                "{teller}/{noemer}: {e}"
            ),
        }
    }
}

#[test]
fn a_decision_the_cell_reads_back_and_calls_anew_gives_no_false_difference() {
    // The path of a submission that refers to a decision (RFC-050): the
    // cell reopens the file, builds the decision value of the stored
    // decision itself and passes it to article 5, which calls article 1
    // anew (`source.article`) and compares the outputs as records.
    for (teller, noemer) in [(10, 2), (1, 3)] {
        let dir = cell_tempdir();
        let path = dir.path().join("kroniek.jsonl");
        let mut service = service();
        let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
        let app = cell
            .record_submission(
                &mut service,
                &[],
                ID,
                "1",
                None,
                &numbers(teller, noemer),
                day(),
            )
            .unwrap();
        let decided = cell
            .decide(
                &mut service,
                &[],
                ID,
                "1",
                &app.id,
                &Map::new(),
                &Map::new(),
                day(),
            )
            .unwrap()
            .remove(0);

        let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
        let verzoek = cell
            .record_submission(
                &mut service,
                &[],
                ID,
                "5",
                Some(Reference::Local(&decided.id)),
                &Map::new(),
                day(),
            )
            .unwrap();
        let [anew] = &cell
            .decide(
                &mut service,
                &[],
                ID,
                "5",
                &verzoek.id,
                &Map::new(),
                &Map::new(),
                day(),
            )
            .unwrap()[..]
        else {
            panic!("not one decision")
        };
        assert_eq!(anew.outputs["anders"], json!(false), "{teller}/{noemer}");
    }
}

#[test]
fn the_file_holds_the_digits() {
    let dir = cell_tempdir();
    let path = dir.path().join("kroniek.jsonl");
    let mut service = service();
    let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
    let app = cell
        .record_submission(&mut service, &[], ID, "1", None, &numbers(1, 3), day())
        .unwrap();
    cell.decide(
        &mut service,
        &[],
        ID,
        "1",
        &app.id,
        &Map::new(),
        &Map::new(),
        day(),
    )
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains(r#""aandeel":{"__decimal":"0.3333333333333333333333333333"}"#),
        "{text}"
    );
    assert!(text.contains(r#""aantal":4"#), "{text}");
}

#[test]
fn a_supplied_value_may_not_hold_the_decimal_key() {
    // An object field takes any object, so only the cell's own refusal
    // stands between this value and a decimal nobody supplied.
    let mut service = service();
    let dir = cell_tempdir();
    let mut cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let ok = json!({"bijlage": {"x": 1}});
    cell.record_submission(
        &mut service,
        &[],
        ID,
        "3",
        None,
        ok.as_object().unwrap(),
        day(),
    )
    .unwrap();
    let supplied = json!({"bijlage": {"x": {"__decimal": "1.5"}}});
    let err = cell
        .record_submission(
            &mut service,
            &[],
            ID,
            "3",
            None,
            supplied.as_object().unwrap(),
            day(),
        )
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("which the chronicle keeps for a decimal"),
        "{err}"
    );
}

#[test]
fn a_damaged_decimal_in_the_file_is_refused() {
    let dir = cell_tempdir();
    let path = dir.path().join("kroniek.jsonl");
    let mut service = service();
    let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
    let app = cell
        .record_submission(&mut service, &[], ID, "1", None, &numbers(1, 3), day())
        .unwrap();
    cell.decide(
        &mut service,
        &[],
        ID,
        "1",
        &app.id,
        &Map::new(),
        &Map::new(),
        day(),
    )
    .unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("0.3333", "0,3333")).unwrap();
    let err = Cell::open(ACTOR, &path, dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("line 2") && err.to_string().contains("no stored decimal"),
        "{err}"
    );
}

#[test]
fn a_gram_that_would_not_open_again_is_not_appended() {
    // Writing and opening share one rule: what `open` refuses, `append`
    // does not write.
    let dir = cell_tempdir();
    let path = dir.path().join("kroniek.jsonl");
    let mut chronicle = Chronicle::open(&path).unwrap();
    let gram = Gram {
        id: "g".into(),
        kind: "AANVRAAG".into(),
        basis: format!("{ID}#1"),
        fact: None,
        refers_to: None,
        received: None,
        after: None,
        recording_actor: ACTOR.into(),
        recorded_on: day(),
        moment: None,
        fields: [("teller".to_string(), json!({"__decimal": "0,5"}))].into(),
        missing: Vec::new(),
        outputs: BTreeMap::new(),
    };
    let err = chronicle.append(gram).unwrap_err();
    assert!(err.to_string().contains("no stored decimal"), "{err}");
    assert!(chronicle.grams().is_empty());
    assert!(!path.exists() || std::fs::read_to_string(&path).unwrap().is_empty());
}
