//! What a submission gives, computed with the answers of the other cells as
//! they knew them on the day it came in ([`Cell::submission_outputs`]), under a
//! small fictional law. Art. 2 establishes an application with a count the
//! applicant states, and gives the count of a register next to it and whether
//! the two differ. Art. 3 leaves that count to the register keeper, whose
//! cell answers it from its own records through a policy (RFC-048). The
//! application is recorded in the cell of the deciding body, which takes the
//! register's cell along.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{day, object_map};

use regelrecht_cel::Cell;
use regelrecht_engine::LawExecutionService;
use serde_json::{Map, Value, json};

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

/// Art. 1 decides; art. 2 is the application, which gives the register's
/// count and whether the stated count differs from it; art. 3 leaves the
/// register's count to its keeper, and counts none where no one says.
const LAW: &str = r#"
$id: wet_opgave_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het bestuur besluit op de aanvraag.
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
            outputs: [toegekend]
        parameters:
          - {name: nummer, type: string}
        output:
          - {name: toegekend, type: boolean}
        actions:
          - {output: toegekend, value: true}
  - number: '2'
    text: De aanvrager vermeldt het aantal; het register zegt hoeveel er zijn.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_opgave_fictief#1
            parameters: [nummer, opgegeven]
            outputs: [volgens_register, wijkt_af]
        parameters:
          - {name: nummer, type: string}
          - {name: opgegeven, type: number}
        input:
          - name: geregistreerd
            type: number
            source:
              regulation: wet_opgave_fictief
              output: aantal_geregistreerd
              parameters:
                nummer: $nummer
        output:
          - {name: volgens_register, type: number}
          - {name: wijkt_af, type: boolean}
        actions:
          - {output: volgens_register, value: $geregistreerd}
          - output: wijkt_af
            value: {operation: NOT_EQUALS, subject: $opgegeven, value: $geregistreerd}
  - number: '3'
    text: Hoeveel er zijn geregistreerd, zegt de registerhouder.
    machine_readable:
      open_terms:
        - id: aantal_geregistreerd
          type: number
          required: false
          competent_authority:
            name: Registerhouder
          default:
            actions:
              - {output: aantal_geregistreerd, value: 0}
      execution:
        parameters:
          - {name: nummer, type: string}
        output:
          - {name: aantal_geregistreerd, type: number}
        actions:
          - {output: aantal_geregistreerd, value: $aantal_geregistreerd}
"#;

/// The policy that binds the term of art. 3 to the answer of the register's
/// cell (RFC-048), and counts none where that cell is silent.
const BINDING: &str = r#"
$id: beleid_register_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid
competent_authority:
  name: Registerhouder
articles:
  - number: '1'
    text: Het aantal is wat het register zegt.
    machine_readable:
      implements:
        - {law: wet_opgave_fictief, article: '3', open_term: aantal_geregistreerd}
      execution:
        parameters:
          - {name: nummer, type: string}
        input:
          - name: antwoord
            type: number
            nullable: true
            source:
              reduction: aantal_geregistreerd
              parameters:
                nummer: $nummer
        output:
          - {name: aantal_geregistreerd, type: number}
        actions:
          - output: aantal_geregistreerd
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord, value: null}
                  then: 0
              default: $antwoord
"#;

/// The register's own regelwerk: the keeper records a count per number, as
/// an executogram of the fact `registratie` (RFC-051).
const REGISTER_LAW: &str = r#"
$id: regeling_register_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/register
competent_authority:
  name: Registerhouder
articles:
  - number: '1'
    text: De registerhouder legt per nummer het aantal vast.
    machine_readable:
      requires:
        - fact: registratie
          parameters: [nummer, aantal]
      execution:
        parameters:
          - {name: nummer, type: string}
          - {name: aantal, type: number}
"#;

/// The reduction of the register's cell: the highest count recorded for
/// `nummer`, otherwise silent. It reads the executograms directly
/// (`source.facts`), which schema v0.8.19 no longer allows, so it declares
/// v0.8.17.
const REDUCTION: &str = r#"
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.17/schema/v0.8.17/schema.json
$id: reductie_aantal_geregistreerd
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Het laatst vastgelegde aantal voor het nummer.
    machine_readable:
      execution:
        parameters:
          - {name: nummer, type: string}
        input:
          - name: registraties
            type: array
            source:
              facts: regeling_register_fictief#1#registratie
        output:
          - {name: aantal_geregistreerd, type: number, nullable: true}
        actions:
          - output: aantal_geregistreerd
            value:
              operation: FOREACH
              collection: $registraties
              as: registratie
              filter: {operation: EQUALS, subject: $registratie.nummer, value: $nummer}
              body: $registratie.aantal
              combine: MAX
"#;

const NUMMER: &str = "NR-0042";

fn service() -> LawExecutionService {
    common::service_of(&[GENERAL_LAW, LAW, BINDING, REGISTER_LAW, REDUCTION])
}

/// The register keeper's cell, which answers `aantal_geregistreerd`.
fn register() -> Cell {
    let dir = tempfile::tempdir().unwrap();
    common::keep_reduction(dir.path(), "reductie_aantal_geregistreerd", REDUCTION);
    common::write_identity(
        dir.path(),
        "name: Registerhouder\nanswers:\n  aantal_geregistreerd: reductie_aantal_geregistreerd\n",
    );
    Cell::in_memory("registerhouder", dir.path()).unwrap()
}

/// The deciding body's cell, which answers nothing.
fn body() -> Cell {
    let dir = tempfile::tempdir().unwrap();
    common::write_identity(dir.path(), "name: Bestuur\n");
    Cell::in_memory("bestuur", dir.path()).unwrap()
}

/// Record `aantal` for [`NUMMER`] in the register on `on`.
fn register_count(service: &mut LawExecutionService, register: &mut Cell, aantal: i64, on: &str) {
    register
        .record_executogram(
            service,
            "regeling_register_fictief#1#registratie",
            &object_map(json!({"nummer": NUMMER, "aantal": aantal})),
            day(on),
        )
        .unwrap();
}

/// Record an application stating `opgegeven` in `body` on `on`, with
/// `register` along, and return what it gives.
fn apply(
    service: &mut LawExecutionService,
    body: &mut Cell,
    register: &Cell,
    opgegeven: i64,
    on: &str,
) -> (String, Map<String, Value>) {
    let gram = body
        .record_submission(
            service,
            &[register],
            "wet_opgave_fictief",
            "2",
            None,
            &object_map(json!({"nummer": NUMMER, "opgegeven": opgegeven})),
            day(on),
        )
        .unwrap();
    let outputs = body
        .submission_outputs(service, &[register], &gram.id)
        .unwrap();
    (gram.id, outputs.into_iter().collect())
}

#[test]
fn a_submission_gives_the_answer_another_cell_knew_on_the_day_it_came_in() {
    let mut service = service();
    let mut register = register();
    let mut body = body();
    register_count(&mut service, &mut register, 7, "2026-03-01");

    let (_, outputs) = apply(&mut service, &mut body, &register, 7, "2026-03-10");

    // The register's answer, not the 0 of the term left unanswered.
    assert_eq!(outputs["volgens_register"], json!(7));
    assert_eq!(outputs["wijkt_af"], json!(false));
}

#[test]
fn a_later_record_in_the_other_cell_does_not_change_what_the_submission_gives() {
    let mut service = service();
    let mut register = register();
    let mut body = body();
    register_count(&mut service, &mut register, 7, "2026-03-01");
    let (id, _) = apply(&mut service, &mut body, &register, 7, "2026-03-10");

    register_count(&mut service, &mut register, 9, "2026-03-20");

    let outputs = body
        .submission_outputs(&mut service, &[&register], &id)
        .unwrap();
    assert_eq!(outputs["volgens_register"], json!(7));
    assert_eq!(outputs["wijkt_af"], json!(false));
}

/// The register keeper takes the application itself: its own cell answers,
/// from its chronicle as it stood when the application came in. A count it
/// records after the application, on the same day, does not count.
#[test]
fn the_cell_that_records_the_submission_answers_from_its_chronicle_up_to_it() {
    let mut service = service();
    let mut register = register();
    register_count(&mut service, &mut register, 7, "2026-03-10");
    let gram = register
        .record_submission(
            &mut service,
            &[],
            "wet_opgave_fictief",
            "2",
            None,
            &object_map(json!({"nummer": NUMMER, "opgegeven": 7})),
            day("2026-03-10"),
        )
        .unwrap();
    register_count(&mut service, &mut register, 9, "2026-03-10");

    let outputs = register
        .submission_outputs(&mut service, &[], &gram.id)
        .unwrap();
    assert_eq!(outputs["volgens_register"], json!(7));
}

/// Without the register's cell along, the term keeps what the policy gives
/// where no cell answers: the outcome the application had before cells were
/// asked.
#[test]
fn a_submission_with_no_cell_along_falls_back_on_the_term_left_unanswered() {
    let mut service = service();
    let mut body = body();
    let gram = body
        .record_submission(
            &mut service,
            &[],
            "wet_opgave_fictief",
            "2",
            None,
            &object_map(json!({"nummer": NUMMER, "opgegeven": 7})),
            day("2026-03-10"),
        )
        .unwrap();
    let outputs = body
        .submission_outputs(&mut service, &[], &gram.id)
        .unwrap();
    assert_eq!(outputs["volgens_register"], json!(0));
    assert_eq!(outputs["wijkt_af"], json!(true));
}
