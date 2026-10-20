//! A cell records a fact that refers to no submission (RFC-051): an
//! executogram of a fact an article requires, under a small fictional law.
//! The article's `requires` entry for the fact says what it holds (schema
//! v0.8.7).

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_cel::{reading, Cell, Error, EXECUTOGRAM};
use regelrecht_engine::{AnswerRequest, Authority, LawExecutionService, OpenTermAnswerer};
use serde_json::{json, Map, Value};

mod common;
use common::{cell_tempdir, day, object_map};

const ACTOR: &str = "school_fictief";

/// A procedure for a beschikking, so a decision on a submission has stages.
const PROCEDURE_LAW: &str = r#"
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

/// Article 3 requires the fact of where a school stands, since a day;
/// article 4 requires none; article 5 a fact with a field `id` and article 6
/// one with a field `refers_to`, which no row can hold.
const SPECIAL_LAW: &str = r#"
$id: wet_vaststelling_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '3'
    text: Het bevoegd gezag legt vast waar de school staat, en sinds wanneer.
    machine_readable:
      requires:
        - fact: stand
          parameters: [plaats, sinds, opmerking]
      execution:
        parameters:
          - {name: plaats, type: string}
          - {name: sinds, type: date}
          - {name: opmerking, type: string, nullable: true}
  - number: '4'
    text: De plaats van een school is bepalend.
    machine_readable:
      execution:
        parameters:
          - {name: plaats, type: string}
        output:
          - {name: plaats_bekend, type: boolean}
        actions:
          - {output: plaats_bekend, value: true}
  - number: '5'
    text: Het bevoegd gezag legt een nummer vast.
    machine_readable:
      requires:
        - fact: nummer
          parameters: [id]
      execution:
        parameters:
          - {name: id, type: string}
  - number: '6'
    text: Het bevoegd gezag legt een verwijzing vast.
    machine_readable:
      requires:
        - fact: verwijzing
          parameters: [refers_to]
      execution:
        parameters:
          - {name: refers_to, type: string}
"#;

const BASIS: &str = "wet_vaststelling_fictief#3";
const FACT: &str = "wet_vaststelling_fictief#3#stand";

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(SPECIAL_LAW).unwrap();
    service
}

/// A cell of the fictional school, without reductions.
fn cell() -> (Cell, tempfile::TempDir) {
    let dir = cell_tempdir();
    (Cell::in_memory(ACTOR, dir.path()).unwrap(), dir)
}

fn refused(e: &Error, says: &str) {
    assert!(matches!(e, Error::Refused(m) if m.contains(says)), "{e}");
}

fn stand(plaats: &str, sinds: &str) -> Map<String, Value> {
    object_map(json!({ "plaats": plaats, "sinds": sinds }))
}

fn record(
    cell: &mut Cell,
    fact: &str,
    supplied: &Map<String, Value>,
    on: &str,
) -> Result<regelrecht_cel::Gram, Error> {
    cell.record_executogram(&service(), fact, supplied, day(on))
}

#[test]
fn an_executogram_records_a_fact_the_article_requires() {
    let (mut cell, _dir) = cell();
    let gram = record(
        &mut cell,
        FACT,
        &stand("Utrecht", "2020-08-01"),
        "2026-08-01",
    )
    .unwrap();
    assert_eq!(gram.kind, EXECUTOGRAM);
    assert_eq!(gram.basis, BASIS);
    assert_eq!(gram.fact.as_deref(), Some("stand"));
    assert_eq!(gram.refers_to, None);
    assert_eq!(gram.recording_actor, ACTOR);
    assert_eq!(gram.fields["plaats"], json!("Utrecht"));
    assert_eq!(gram.fields["opmerking"], Value::Null);
    assert!(gram.outputs.is_empty());
    assert!(gram.missing.is_empty());
    assert_eq!(cell.grams(), [gram]);
}

#[test]
fn an_executogram_is_whole_or_not_recorded() {
    let (mut cell, _dir) = cell();

    // An article that requires no such fact, and a reference that names no
    // fact.
    let e = record(
        &mut cell,
        "wet_vaststelling_fictief#4#stand",
        &object_map(json!({"plaats": "Utrecht"})),
        "2026-08-01",
    )
    .unwrap_err();
    refused(&e, "requires no fact 'stand'");
    let e = record(
        &mut cell,
        BASIS,
        &stand("Utrecht", "2020-08-01"),
        "2026-08-01",
    )
    .unwrap_err();
    refused(&e, "is no <regulation>#<article>#<fact>");
    let e = record(
        &mut cell,
        "wet_vaststelling_fictief#9#stand",
        &stand("Utrecht", "2020-08-01"),
        "2026-08-01",
    )
    .unwrap_err();
    refused(&e, "has no version in force");

    let mut unknown = stand("Utrecht", "2020-08-01");
    unknown.insert("straat".into(), json!("Lindelaan"));
    let e = record(&mut cell, FACT, &unknown, "2026-08-01").unwrap_err();
    refused(&e, "holds no 'straat'");

    let e = record(
        &mut cell,
        FACT,
        &object_map(json!({"plaats": "Utrecht", "sinds": 2020})),
        "2026-08-01",
    )
    .unwrap_err();
    refused(&e, "does not fit a non-nullable Date");

    let e = record(
        &mut cell,
        FACT,
        &object_map(json!({"plaats": "Utrecht"})),
        "2026-08-01",
    )
    .unwrap_err();
    refused(&e, "needs 'sinds', which is left out");

    // A field named `id`: the row gives the executogram's id under it.
    let e = record(
        &mut cell,
        "wet_vaststelling_fictief#5#nummer",
        &object_map(json!({"id": "x"})),
        "2026-08-01",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("holds a field 'id'")),
        "{e}"
    );

    // Nor one named `refers_to`: the row gives what the executogram refers
    // to under it (RFC-053).
    let e = record(
        &mut cell,
        "wet_vaststelling_fictief#6#verwijzing",
        &object_map(json!({"refers_to": "x"})),
        "2026-08-01",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("holds a field 'refers_to'")),
        "{e}"
    );

    // The article must be in force on the day.
    let e = record(
        &mut cell,
        FACT,
        &stand("Utrecht", "2020-08-01"),
        "2025-06-01",
    )
    .unwrap_err();
    refused(&e, "has no version in force on 2025-06-01");

    assert!(cell.grams().is_empty());
}

#[test]
fn an_executogram_reads_back_from_the_file() {
    let dir = cell_tempdir();
    let file = cell_tempdir();
    let path = file.path().join("kroniek.jsonl");
    let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
    let gram = record(
        &mut cell,
        FACT,
        &stand("Utrecht", "2020-08-01"),
        "2026-08-01",
    )
    .unwrap();
    let again = Cell::open(ACTOR, &path, dir.path()).unwrap();
    assert_eq!(again.grams(), [gram]);
}

#[test]
fn an_executogram_read_back_records_a_fact_and_nothing_else() {
    let dir = cell_tempdir();
    let file = cell_tempdir();
    let path = file.path().join("kroniek.jsonl");
    {
        let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
        record(
            &mut cell,
            FACT,
            &stand("Utrecht", "2020-08-01"),
            "2026-08-01",
        )
        .unwrap();
    }
    let line = std::fs::read_to_string(&path).unwrap();
    let opens = |tampered: String| {
        assert_ne!(tampered, line);
        std::fs::write(&path, tampered).unwrap();
        Cell::open(ACTOR, &path, dir.path()).map(|_| ())
    };
    let e = opens(line.replace("\"outputs\":{}", "\"outputs\":{\"x\":1}")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("records a fact and nothing else")),
        "{e}"
    );
    let e = opens(line.replace("\"outputs\":{}", "\"missing\":[\"sinds\"],\"outputs\":{}"))
        .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("records a fact and nothing else")),
        "{e}"
    );
    let e = opens(line.replace(
        "\"recording_actor\"",
        "\"refers_to\":\"x\",\"recording_actor\"",
    ))
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("records a fact and nothing else")),
        "{e}"
    );
    let e = opens(line.replace("\"fact\":\"stand\",", "")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("records a fact and nothing else")),
        "{e}"
    );
    let e = opens(line.replace("\"kind\":\"EXECUTOGRAM\"", "\"kind\":\"AANVRAAG\"")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("names a fact but is a AANVRAAG")),
        "{e}"
    );
    std::fs::write(&path, &line).unwrap();
    assert!(Cell::open(ACTOR, &path, dir.path()).is_ok());
}

#[test]
fn the_rows_are_the_executograms_of_that_fact() {
    let (mut cell, _dir) = cell();
    let first = record(
        &mut cell,
        FACT,
        &stand("Utrecht", "2020-08-01"),
        "2026-08-01",
    )
    .unwrap();
    record(&mut cell, FACT, &stand("Zeist", "2027-01-01"), "2026-09-01").unwrap();
    let rows =
        serde_json::to_value(reading::executogram_rows(&service(), cell.grams(), FACT).unwrap())
            .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["id"], json!(first.id));
    assert_eq!(rows[0]["plaats"], json!("Utrecht"));
    assert_eq!(rows[1]["plaats"], json!("Zeist"));
    let none = serde_json::to_value(
        reading::executogram_rows(&service(), cell.grams(), "wet_vaststelling_fictief#3#ander")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(none, json!([]));
}

/// A reduction of the fictional school that reads the executograms of the
/// fact `fact`, and gives the answer `plaats`.
fn reduction(fact: &str) -> String {
    format!(
        r#"
$id: reductie_plaats_fictief
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: De plaats is die van de stand.
    machine_readable:
      execution:
        input:
          - name: standen
            type: array
            source:
              facts: {fact}
        output:
          - {{name: plaats, type: string, nullable: true}}
        actions:
          - output: plaats
            value:
              operation: FOREACH
              collection: $standen
              as: s
              body: $s.plaats
              combine: ONLY
"#
    )
}

#[test]
fn a_reduction_reads_only_a_fact_its_article_requires() {
    let authority = Authority {
        name: "Testcel".into(),
        category: None,
    };
    let answers = [("plaats".to_string(), "reductie_plaats_fictief".to_string())].into();
    let reductions = ["reductie_plaats_fictief".to_string()].into();
    let reads = |fact: &str| {
        let mut service = service();
        service.load_law(&reduction(fact)).unwrap();
        let reader =
            reading::Reader::new(ACTOR, &authority, &answers, &reductions, Vec::new(), None);
        let request = AnswerRequest {
            name: "plaats",
            parameters: &std::collections::BTreeMap::new(),
            calculation_date: "2026-08-01",
        };
        reader.answer(&service, &request).map(|a| a.unwrap().value)
    };
    // No executogram yet: no plaats.
    assert_eq!(reads(FACT).unwrap(), regelrecht_engine::Value::Null);
    let e = reads("wet_vaststelling_fictief#4#stand").unwrap_err();
    assert!(e.to_string().contains("requires no fact 'stand'"), "{e}");
    // A stream id, as v0.8.6 had it, or an article alone is no fact
    // reference.
    assert!(service().load_law(&reduction("stand")).is_err());
    assert!(service().load_law(&reduction(BASIS)).is_err());
}

#[test]
fn a_reduction_that_asks_its_own_answer_is_refused() {
    // The answer x that fills `x` of article 1 asks article 1, which reads
    // `x` again: the cell refuses to answer itself.
    let law = r#"
$id: wet_kring_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het bevoegd gezag stelt vast of x geldt.
    machine_readable:
      open_terms:
        - {id: x, type: boolean, decided_per_case_by: het bevoegd gezag}
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_kring_fictief#1
            parameters: [dagen]
            outputs: [ontvangen]
          - type: BESCHIKKING
            outputs: [uit]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: ontvangen, type: boolean}
          - {name: uit, type: boolean}
        actions:
          - {output: ontvangen, value: true}
          - {output: uit, value: $x}
"#;
    // The policy of the school binds x to the answer x of its cell.
    let binding = r#"
$id: beleid_kring_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
competent_authority:
  name: Testcel
articles:
  - number: '1'
    text: x is het antwoord van de cel.
    machine_readable:
      open_terms:
        - id: x_volgens_hoofd
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      implements:
        - {law: wet_kring_fictief, article: '1', open_term: x}
      execution:
        input:
          - name: antwoord
            type: boolean
            nullable: true
            source: {reduction: x}
        output:
          - {name: x, type: boolean}
        actions:
          - output: x
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord, value: null}
                  then: $x_volgens_hoofd
              default: $antwoord
"#;
    let reduction = r#"
$id: reductie_kring_fictief
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: x is wat artikel 1 zegt.
    machine_readable:
      execution:
        input:
          - name: weer
            type: boolean
            source: {regulation: wet_kring_fictief, output: uit}
        output:
          - {name: x, type: boolean}
        actions:
          - {output: x, value: $weer}
"#;
    let dir = cell_tempdir();
    common::write_identity(
        dir.path(),
        "name: Testcel\nanswers:\n  x: reductie_kring_fictief\n",
    );
    common::keep_reduction(dir.path(), "reductie_kring_fictief", reduction);
    let mut cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    let mut service = LawExecutionService::new();
    service.load_law(PROCEDURE_LAW).unwrap();
    service.load_law(law).unwrap();
    service.load_law(binding).unwrap();
    service.load_law(reduction).unwrap();
    let on = day("2026-08-01");
    let app = cell
        .record_submission(
            &mut service,
            &[],
            "wet_kring_fictief",
            "1",
            None,
            &object_map(json!({ "dagen": 2 })),
            on,
        )
        .unwrap();
    let e = cell
        .decide(
            &mut service,
            &[],
            "wet_kring_fictief",
            "1",
            &app.id,
            &Map::new(),
            &Map::new(),
            on,
        )
        .unwrap_err();
    assert!(e.to_string().contains("asks its own answer"), "{e}");
    assert_eq!(cell.grams(), [app]);
}
