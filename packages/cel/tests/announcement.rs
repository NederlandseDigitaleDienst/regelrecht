//! A decision made known by a fact the authority records after it, under
//! toy laws written for the cell: no corpus, no Awb.
//!
//! An article at the stage BEKENDMAKING says that a fact holds the day the
//! decision was sent (`requires`, `fact` with `parameters`). The decision does
//! not ask that day, and that stage does not run when it is taken. Once the
//! authority records the fact, referring to the decision, the stage runs on
//! what the chronicle holds ([`Cell::announcement`]): the period that runs
//! from the announcement follows from the fact, and so does whether an
//! objection came in time. Without such a fact the stage runs with the
//! decision, on its day, as before.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{bare_cell, day, object_map, service_of};

use regelrecht_cel::{Cell, Error, Gram, Reference, Source, decision};
use regelrecht_engine::LawExecutionService;
use serde_json::{Map, Value, json};

/// A procedure for a beschikking; a hook at BESLUIT that asks the authority
/// who signed; a period that runs from the announcement; whether an
/// objection came in within it; and the fact that the decision was sent.
const PROCEDURE: &str = r#"
$id: wet_procedure_fictief
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
        requires: [{name: besluit_datum, type: date}]
      - name: BEKENDMAKING
        requires: [{name: bekendmaking_datum, type: date}]
      - name: BEZWAAR
articles:
  - number: '1'
    text: De termijn eindigt zes weken na de bekendmaking.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BEKENDMAKING}
      execution:
        parameters:
          - {name: bekendmaking_datum, type: date}
        output:
          - {name: termijn_einde, type: date}
        actions:
          - output: termijn_einde
            value: {operation: DATE_ADD, date: $bekendmaking_datum, weeks: 6}
  - number: '2'
    text: Een bezwaar is tijdig als het binnen de termijn is ontvangen.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {submission: BEZWAAR}
      execution:
        produces:
          moment:
            parameter: ontvangen_op
            legal_basis: {law: Wet procedure fictief, article: '2', url: 'https://example.org/wet#2'}
        parameters:
          - {name: ontvangen_op, type: date, required: false}
          - {name: termijn_einde, type: date, required: false, nullable: true}
        output:
          - {name: tijdig, type: boolean, nullable: true}
        actions:
          - output: tijdig
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $termijn_einde, value: null}
                  then: null
              default:
                operation: GREATER_THAN_OR_EQUAL
                subject: {operation: DATE_DIFF, from: $ontvangen_op, to: $termijn_einde, in: days}
                value: 0
  - number: '3'
    text: Een besluit wordt bekendgemaakt door toezending aan de aanvrager.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BEKENDMAKING}
      requires:
        - fact: toegezonden
          description: Dat het besluit aan de aanvrager is toegezonden, en op welke dag.
          parameters: [bekendmaking_datum, adres_toezending]
      execution:
        parameters:
          - {name: bekendmaking_datum, type: date, nullable: true}
          - {name: adres_toezending, type: string, nullable: true}
        output:
          - {name: bekendgemaakt, type: boolean}
        actions:
          - output: bekendgemaakt
            value: {operation: NOT, value: {operation: EQUALS, subject: $bekendmaking_datum, value: null}}
  - number: '4'
    text: Het besluit vermeldt wie het ondertekende.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}
      execution:
        parameters:
          - {name: ondertekend_door, type: string}
        output:
          - {name: ondertekening_vermeld, type: boolean}
        actions:
          - output: ondertekening_vermeld
            value: {operation: NOT, value: {operation: EQUALS, subject: $ondertekend_door, value: ''}}
"#;

/// Leave for at most ten days.
const VERLOF: &str = r#"
$id: wet_verlof_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent verlof van ten hoogste tien dagen.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof_fictief#1
            parameters: [dagen]
          - type: BESCHIKKING
            outputs: [verleend]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: verleend, type: boolean}
        actions:
          - output: verleend
            value: {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}
"#;

/// An objection names the decision it contests.
const BEZWAAR: &str = r#"
$id: wet_bezwaar_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Tegen een besluit kan bezwaar worden gemaakt.
    machine_readable:
      execution:
        produces:
          - type: BEZWAAR
            requests: wet_bezwaar_fictief#1
            parameters: [bestreden_besluit]
          - type: BESCHIKKING
            outputs: [behandeld]
        parameters:
          - {name: bestreden_besluit, type: decision}
        output:
          - {name: behandeld, type: boolean}
        actions:
          - {output: behandeld, value: true}
"#;

const VERLOF_LAW: &str = "wet_verlof_fictief";
const SENT: &str = "wet_procedure_fictief#3#toegezonden";

fn service() -> LawExecutionService {
    service_of(&[PROCEDURE, VERLOF, BEZWAAR])
}

/// The procedure without the fact: the cell announces on the day of the
/// decision.
fn service_without_fact() -> LawExecutionService {
    let start = PROCEDURE.find("  - number: '3'").unwrap();
    let end = PROCEDURE.find("  - number: '4'").unwrap();
    let procedure = format!("{}{}", &PROCEDURE[..start], &PROCEDURE[end..]);
    service_of(&[&procedure, VERLOF, BEZWAAR])
}

fn signed() -> Map<String, Value> {
    object_map(json!({"ondertekend_door": "het hoofd"}))
}

/// Apply for five days and decide on it on `on`.
fn decide(cell: &mut Cell, service: &mut LawExecutionService, on: &str) -> Gram {
    let application = cell
        .record_submission(
            service,
            &[],
            VERLOF_LAW,
            "1",
            None,
            &object_map(json!({"dagen": 5})),
            day(on),
        )
        .unwrap();
    cell.decide(
        service,
        &[],
        VERLOF_LAW,
        "1",
        &application.id,
        &signed(),
        &Map::new(),
        day(on),
    )
    .unwrap()
    .remove(0)
}

fn send(cell: &mut Cell, service: &LawExecutionService, decision: &str, on: &str) -> Gram {
    cell.record_executogram_referring(
        service,
        SENT,
        &object_map(json!({"bekendmaking_datum": on, "adres_toezending": "Dorpsstraat 1"})),
        Some(decision),
        day(on),
    )
    .unwrap()
}

fn object(cell: &mut Cell, service: &mut LawExecutionService, decision: &str, on: &str) -> Gram {
    cell.record_submission(
        service,
        &[],
        "wet_bezwaar_fictief",
        "1",
        Some(Reference::Local(decision)),
        &Map::new(),
        day(on),
    )
    .unwrap()
}

#[test]
fn a_parameter_a_fact_holds_is_no_input_the_authority_gives() {
    let shape = decision::derive(&service(), VERLOF_LAW, "1", day("2026-10-01")).unwrap();
    let source = |name: &str| shape.input(name).map(|i| i.source.clone());
    let fact = Some(Source::Fact {
        fact: SENT.to_string(),
    });
    // The date the stage requires, and what else the fact holds.
    assert_eq!(source("bekendmaking_datum"), fact);
    assert_eq!(source("adres_toezending"), fact);
    // A parameter of a hook at BESLUIT stays the authority's.
    assert_eq!(source("ondertekend_door"), Some(Source::Supplied));
    assert_eq!(source("besluit_datum"), Some(Source::Cell));
    assert_eq!(shape.announcement().unwrap().stage, "BEKENDMAKING");
    let taken: Vec<_> = shape.taken_stages().iter().map(|s| &s.stage).collect();
    assert_eq!(taken, ["BESLUIT"]);
    assert_eq!(shape.announcement_facts(), [SENT]);
}

#[test]
fn the_decision_is_taken_without_its_announcement() {
    let mut service = service();
    let mut cell = bare_cell("fictieve_school");
    let decision = decide(&mut cell, &mut service, "2026-10-01");
    assert_eq!(decision.outputs["verleend"], json!(true));
    assert_eq!(decision.outputs["ondertekening_vermeld"], json!(true));
    // The announcement has not run: no period, no day it was made known.
    assert!(!decision.outputs.contains_key("termijn_einde"));
    assert!(!decision.fields.contains_key("bekendmaking_datum"));
    let announcement = cell
        .announcement(&mut service, &[], &decision.id, day("2026-10-01"))
        .unwrap()
        .unwrap();
    assert!(!announcement.made_known);
    assert!(announcement.outputs.is_empty());
}

#[test]
fn the_authority_cannot_supply_what_the_fact_holds() {
    let mut service = service();
    let mut cell = bare_cell("fictieve_school");
    let application = cell
        .record_submission(
            &mut service,
            &[],
            VERLOF_LAW,
            "1",
            None,
            &object_map(json!({"dagen": 5})),
            day("2026-10-01"),
        )
        .unwrap();
    let mut supplied = signed();
    supplied.insert("bekendmaking_datum".into(), json!("2026-10-01"));
    let e = cell
        .decide(
            &mut service,
            &[],
            VERLOF_LAW,
            "1",
            &application.id,
            &supplied,
            &Map::new(),
            day("2026-10-01"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("comes from the fact") && m.contains(SENT)),
        "{e}"
    );
}

#[test]
fn the_period_runs_from_the_day_the_fact_records() {
    let mut service = service();
    let mut cell = bare_cell("fictieve_school");
    let decision = decide(&mut cell, &mut service, "2026-10-01");
    let sent = send(&mut cell, &service, &decision.id, "2026-10-05");
    let announcement = |cell: &Cell, service: &mut LawExecutionService, on: &str| {
        cell.announcement(service, &[], &decision.id, day(on))
            .unwrap()
            .unwrap()
    };
    // Before the day it was recorded, the chronicle does not hold it.
    assert!(!announcement(&cell, &mut service, "2026-10-04").made_known);
    let known = announcement(&cell, &mut service, "2026-10-05");
    assert!(known.made_known);
    assert_eq!(known.read, [sent.id]);
    assert_eq!(known.fields["bekendmaking_datum"], json!("2026-10-05"));
    assert_eq!(known.outputs["termijn_einde"], json!("2026-11-16"));
    assert_eq!(known.outputs["bekendgemaakt"], json!(true));
    // Sent again, later: the latest record counts.
    send(&mut cell, &service, &decision.id, "2026-10-12");
    let again = announcement(&cell, &mut service, "2026-10-12");
    assert_eq!(again.outputs["termijn_einde"], json!("2026-11-23"));
    // Nothing of it is recorded with the decision.
    assert!(!cell.grams()[1].outputs.contains_key("termijn_einde"));
}

#[test]
fn an_objection_takes_the_period_as_the_chronicle_holds_it() {
    let mut service = service();
    let mut cell = bare_cell("fictieve_school");
    let decision = decide(&mut cell, &mut service, "2026-10-01");
    // Before the decision is sent there is no period: whether the objection
    // came in time is not known.
    let early = object(&mut cell, &mut service, &decision.id, "2026-10-03");
    assert_eq!(early.fields["termijn_einde"], Value::Null);
    let given = cell
        .submission_outputs(&mut service, &[], &early.id)
        .unwrap();
    assert_eq!(given["tijdig"], Value::Null);

    send(&mut cell, &service, &decision.id, "2026-10-05");
    let late = object(&mut cell, &mut service, &decision.id, "2026-11-20");
    assert_eq!(late.fields["termijn_einde"], json!("2026-11-16"));
    let given = cell
        .submission_outputs(&mut service, &[], &late.id)
        .unwrap();
    assert_eq!(given["tijdig"], json!(false));
}

#[test]
fn without_the_fact_the_decision_is_announced_on_its_day() {
    let mut service = service_without_fact();
    let shape = decision::derive(&service, VERLOF_LAW, "1", day("2026-10-01")).unwrap();
    assert_eq!(
        shape.input("bekendmaking_datum").map(|i| &i.source),
        Some(&Source::Cell)
    );
    assert!(shape.announcement().is_none());
    let mut cell = bare_cell("fictieve_school");
    let decision = decide(&mut cell, &mut service, "2026-10-01");
    assert_eq!(decision.outputs["termijn_einde"], json!("2026-11-12"));
    assert!(
        cell.announcement(&mut service, &[], &decision.id, day("2026-10-01"))
            .unwrap()
            .is_none()
    );
    let objection = object(&mut cell, &mut service, &decision.id, "2026-10-20");
    assert_eq!(objection.fields["termijn_einde"], json!("2026-11-12"));
}

/// The procedure with the fact of article 3 holding only the address: the
/// day of the announcement stays the cell's, the day of the decision.
fn service_with_address_fact() -> LawExecutionService {
    let procedure = PROCEDURE.replace(
        "parameters: [bekendmaking_datum, adres_toezending]",
        "parameters: [adres_toezending]",
    );
    assert_ne!(procedure, PROCEDURE);
    service_of(&[&procedure, VERLOF, BEZWAAR])
}

#[test]
fn a_date_the_cell_fills_does_not_wait_for_a_fact() {
    let mut service = service_with_address_fact();
    let shape = decision::derive(&service, VERLOF_LAW, "1", day("2026-10-01")).unwrap();
    assert_eq!(
        shape.input("bekendmaking_datum").map(|i| &i.source),
        Some(&Source::Cell)
    );
    assert!(shape.announcement().is_some());
    let mut cell = bare_cell("fictieve_school");
    let decision = decide(&mut cell, &mut service, "2026-10-01");
    assert_eq!(decision.fields["bekendmaking_datum"], json!("2026-10-01"));
    // No fact recorded, yet the stage runs: the date it requires is there.
    let announcement = cell
        .announcement(&mut service, &[], &decision.id, day("2026-10-01"))
        .unwrap()
        .unwrap();
    assert!(announcement.made_known);
    assert_eq!(announcement.outputs["termijn_einde"], json!("2026-11-12"));
}

#[test]
fn a_fact_cannot_hold_what_the_decision_is_taken_with() {
    // Article 4, a hook at BESLUIT, asks who signed; a fact at the
    // announcement claims it too.
    let procedure = PROCEDURE.replace(
        "parameters: [bekendmaking_datum, adres_toezending]",
        "parameters: [bekendmaking_datum, adres_toezending, ondertekend_door]",
    );
    let procedure = procedure.replace(
        "          - {name: adres_toezending, type: string, nullable: true}\n",
        "          - {name: adres_toezending, type: string, nullable: true}\n          - {name: ondertekend_door, type: string, nullable: true}\n",
    );
    let service = service_of(&[&procedure, VERLOF, BEZWAAR]);
    let e = decision::derive(&service, VERLOF_LAW, "1", day("2026-10-01")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("'ondertekend_door'") && m.contains("BESLUIT")),
        "{e}"
    );
}
