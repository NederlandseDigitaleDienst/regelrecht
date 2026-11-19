//! The procedure a decision goes through, stage by stage (RFC-007, RFC-008),
//! on a small fictional general act.
//!
//! The unit tests of `execute_stage` cover the machinery on synthetic laws,
//! and `decision_on_application.rs` the stages of an application. These tests
//! hold what a general act adds to a decision whose law does not name it: the
//! decision enters the act's procedure, each hook fires at its own stage, an
//! override of one hook reaches a hook that reads it, and an open term the
//! act leaves to the law of the application is filled for that application
//! only. The general act is a mini-version after the Awb (articles 3:46,
//! 4:13, 6:7 and 6:8); the values of the real Awb are for the scenarios next
//! to it.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{ExecutionOutcome, LawExecutionService, StageState, Value};
use std::collections::BTreeMap;

/// A fictional general act after the Awb: the procedure of a decision, the
/// duty to give reasons at the decision (3:46), the decision term of an
/// application (4:13), the objection term (6:7) and its first and last day
/// after the decision is made known (6:8).
const GENERAL_ACT: &str = r#"
$id: test_general_administrative_act
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '1994-01-01'
name: Testwet algemene bestuursregels
procedure:
  - id: beschikking
    default: true
    applies_to:
      legal_character: BESCHIKKING
    stages:
      - name: AANVRAAG
        description: The application is filed.
        requires:
          - {name: aanvraag_datum, type: date}
      - name: BEHANDELING
        description: The application is examined.
        requires:
          - {name: beslistermijn_start, type: date}
      - name: BESLUIT
        description: The decision is taken.
        requires:
          - {name: besluit_datum, type: date}
      - name: BEKENDMAKING
        description: The decision is made known.
        requires:
          - {name: bekendmaking_datum, type: date}
      - name: BEZWAAR
        description: The objection term runs.
articles:
  - number: '3:46'
    text: FICTIEF. Een besluit berust op een deugdelijke motivering.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}
      execution:
        output:
          - {name: motivering_vereist, type: boolean}
        actions:
          - {output: motivering_vereist, value: true}
  - number: '4:13'
    text: >-
      FICTIEF. Een beschikking wordt gegeven binnen de bij wettelijk voorschrift
      bepaalde termijn, en anders binnen acht weken na ontvangst van de aanvraag.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {submission: AANVRAAG}
      open_terms:
        - id: wettelijke_beslistermijn
          type: date
          required: false
          default:
            actions:
              - output: wettelijke_beslistermijn
                value: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 8}
      execution:
        produces:
          moment:
            parameter: datum_ontvangst
            legal_basis:
              law: Testwet algemene bestuursregels
              article: '4:13'
              paragraph: '1'
              url: https://example.com/test_general_administrative_act/4:13
        parameters:
          - {name: datum_ontvangst, type: date, required: false}
        output:
          - {name: uiterste_beslisdatum, type: date}
          - {name: redelijke_beslistermijn_einddatum, type: date}
        actions:
          - {output: uiterste_beslisdatum, value: $wettelijke_beslistermijn}
          - output: redelijke_beslistermijn_einddatum
            value: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 8}
  - number: '6:7'
    text: FICTIEF. De termijn voor het indienen van een bezwaarschrift bedraagt zes weken.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}
      execution:
        output:
          - {name: termijn_weken, type: number}
        actions:
          - {output: termijn_weken, value: 6}
  - number: '6:8'
    text: >-
      FICTIEF. De termijn vangt aan met ingang van de dag na die waarop het
      besluit is bekendgemaakt.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BEKENDMAKING}
      execution:
        parameters:
          - {name: bekendmaking_datum, type: date, required: true}
        input:
          - name: termijn_weken
            type: number
            source: {regulation: test_general_administrative_act, output: termijn_weken}
        output:
          - {name: termijn_startdatum, type: date}
          - {name: termijn_einddatum, type: date}
        actions:
          - output: termijn_startdatum
            value: {operation: DATE_ADD, date: $bekendmaking_datum, days: 1}
          - output: termijn_einddatum
            value: {operation: DATE_ADD, date: $bekendmaking_datum, weeks: $termijn_weken}
"#;

/// A fictional law that gives a decision and names no general act.
const DECISION_ACT: &str = r#"
$id: test_decision_act
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2020-01-01'
name: Testwet die een beschikking geeft
articles:
  - number: '1'
    text: FICTIEF. Het bestuursorgaan kent de voorziening toe.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - {name: toegekend, type: boolean}
        actions:
          - {output: toegekend, value: true}
"#;

/// A fictional law that gives a decision and sets an objection term of its
/// own, four weeks, over the one of the general act (lex specialis).
const SHORT_TERM_ACT: &str = r#"
$id: test_decision_act_short_term
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2020-01-01'
name: Testwet met een eigen bezwaartermijn
articles:
  - number: '1'
    text: FICTIEF. Het bestuursorgaan kent de voorziening toe.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - {name: toegekend, type: boolean}
        actions:
          - {output: toegekend, value: true}
  - number: '2'
    text: >-
      FICTIEF. In afwijking van artikel 6:7 van de algemene wet bedraagt de
      termijn voor het indienen van een bezwaarschrift vier weken.
    machine_readable:
      overrides:
        - {law: test_general_administrative_act, article: '6:7', output: termijn_weken}
      execution:
        output:
          - {name: termijn_weken, type: number}
        actions:
          - {output: termijn_weken, value: 4}
"#;

/// A fictional law whose application falls under 4:13 and that sets its own
/// decision term, depending on the subsidy year: a datum the general act
/// does not know.
const SUBSIDY_ACT: &str = r#"
$id: test_subsidiewet_met_beslistermijn
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2020-01-01'
name: Testwet met een eigen beslistermijn
articles:
  - number: '1'
    text: FICTIEF. Een subsidie wordt op aanvraag verleend.
    machine_readable:
      execution:
        produces:
          submission: {kind: AANVRAAG}
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
          - {name: subsidiejaar, type: number, required: true}
        output:
          - {name: aangevraagd, type: boolean}
        actions:
          - {output: aangevraagd, value: true}
  - number: '2'
    text: >-
      FICTIEF. Op de aanvraag wordt beslist binnen dertien weken na ontvangst,
      voor een subsidiejaar vóór 2026 binnen zesentwintig weken.
    machine_readable:
      implements:
        - law: test_general_administrative_act
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host: {law: test_subsidiewet_met_beslistermijn, article: '1'}
      execution:
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
          - {name: subsidiejaar, type: number, required: true}
        output:
          - {name: wettelijke_beslistermijn, type: date}
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: IF
              cases:
                - when: {operation: LESS_THAN, subject: $subsidiejaar, value: 2026}
                  then: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 26}
              default: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 13}
"#;

/// A fictional law whose application falls under 4:13 without a decision
/// term of its own.
const PERMIT_ACT: &str = r#"
$id: test_aanvraag_zonder_beslistermijn
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2020-01-01'
name: Testwet zonder eigen beslistermijn
articles:
  - number: '1'
    text: FICTIEF. Een vergunning wordt op aanvraag verleend.
    machine_readable:
      execution:
        produces:
          submission: {kind: AANVRAAG}
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: aangevraagd, type: boolean}
        actions:
          - {output: aangevraagd, value: true}
"#;

/// The general act and the given laws.
fn service_with(laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(GENERAL_ACT).unwrap();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
}

fn date(s: &str) -> Value {
    Value::String(s.to_string())
}

/// The data of every stage up to the objection term: applied on 5 January,
/// examined from the 6th, decided and made known on 12 March 2026.
fn every_stage() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("aanvraag_datum".to_string(), date("2026-01-05")),
        ("beslistermijn_start".to_string(), date("2026-01-06")),
        ("besluit_datum".to_string(), date("2026-03-12")),
        ("bekendmaking_datum".to_string(), date("2026-03-12")),
    ])
}

/// Run the procedure until it completes or cannot go on, each time supplying
/// what it asks for from `supply`. The outputs, the state it stopped in, and
/// what it waits for.
fn run_lifecycle(
    service: &LawExecutionService,
    law_id: &str,
    output: &str,
    supply: &BTreeMap<String, Value>,
    calculation_date: &str,
) -> (BTreeMap<String, Value>, Option<StageState>, Vec<String>) {
    let mut state: Option<StageState> = None;
    let mut given: BTreeMap<String, Value> = BTreeMap::new();

    // Well above the number of stages: every round supplies at least one
    // missing datum, so this terminates.
    for _ in 0..16 {
        let outcome = service
            .execute_stage(
                law_id,
                output,
                state.clone(),
                given.clone(),
                calculation_date,
            )
            .expect("the procedure should run");

        match outcome {
            ExecutionOutcome::Complete(result) => {
                return (result.outputs.clone(), state, Vec::new());
            }
            ExecutionOutcome::Yielded {
                state: next,
                outputs,
                pending_inputs,
            } => {
                // Supply only what the caller has; what it lacks is what the
                // procedure waits for.
                let mut supplied_any = false;
                for name in &pending_inputs {
                    if let Some(v) = supply.get(name) {
                        given.insert(name.clone(), v.clone());
                        supplied_any = true;
                    }
                }
                if !supplied_any {
                    return (outputs, Some(next), pending_inputs);
                }
                state = Some(next);
            }
        }
    }
    panic!("the procedure did not come to an end");
}

/// A law that gives a BESCHIKKING enters the procedure of the general act,
/// although it names that act nowhere (RFC-007). Without the date of the
/// application it does not get past the first stage, and says what it needs
/// instead of computing on.
#[test]
fn a_decision_enters_the_procedure_of_a_general_act_it_does_not_name() {
    let service = service_with(&[DECISION_ACT]);
    let (_outputs, state, pending) = run_lifecycle(
        &service,
        "test_decision_act",
        "toegekend",
        &BTreeMap::new(),
        "2026-03-12",
    );

    let state = state.expect("the procedure should wait for a datum");
    assert_eq!(state.procedure_id, "beschikking");
    assert_eq!(state.current_stage, "AANVRAAG");
    assert_eq!(pending, vec!["aanvraag_datum".to_string()]);
}

/// The hooks of the general act fire at their own stage: 3:46 and 6:7 at the
/// decision, 6:8 only once the decision is made known. Without stages all
/// three would fire at once, and 6:8 would count from a date that does not
/// exist yet (RFC-008). The term comes out as a date, not as "six weeks".
#[test]
fn each_hook_fires_at_its_stage_and_the_term_ends_on_a_date() {
    let service = service_with(&[DECISION_ACT]);
    let (outputs, _state, pending) = run_lifecycle(
        &service,
        "test_decision_act",
        "toegekend",
        &every_stage(),
        "2026-03-12",
    );

    assert!(
        pending.is_empty(),
        "the procedure kept waiting for {pending:?}"
    );
    assert_eq!(
        outputs.get("termijn_weken"),
        Some(&Value::Int(6)),
        "outputs: {outputs:?}"
    );
    assert_eq!(
        outputs.get("motivering_vereist"),
        Some(&Value::Bool(true)),
        "outputs: {outputs:?}"
    );
    // The day after it is made known, and six weeks after that day (the
    // first day counts).
    assert_eq!(
        outputs.get("termijn_startdatum"),
        Some(&date("2026-03-13")),
        "outputs: {outputs:?}"
    );
    assert_eq!(
        outputs.get("termijn_einddatum"),
        Some(&date("2026-04-23")),
        "outputs: {outputs:?}"
    );
}

/// An override of the hook 6:7 reaches the date 6:8 computes from it,
/// without 6:8 knowing the overriding law (RFC-007, "the override works
/// through the reference chain").
#[test]
fn an_override_of_a_hook_reaches_the_hook_that_reads_it() {
    let service = service_with(&[SHORT_TERM_ACT]);
    let (outputs, _state, pending) = run_lifecycle(
        &service,
        "test_decision_act_short_term",
        "toegekend",
        &every_stage(),
        "2026-03-12",
    );

    assert!(
        pending.is_empty(),
        "the procedure kept waiting for {pending:?}"
    );
    assert_eq!(
        outputs.get("termijn_weken"),
        Some(&Value::Int(4)),
        "outputs: {outputs:?}"
    );
    // Four weeks after it is made known, not six.
    assert_eq!(
        outputs.get("termijn_einddatum"),
        Some(&date("2026-04-09")),
        "outputs: {outputs:?}"
    );
}

/// 4:13 lid 1 leaves the decision term to the statutory provision; the law of
/// the application fills it for its own application (`host`), with a datum of
/// its own (RFC-058). An application under another law keeps the eight weeks
/// of lid 2, although the subsidy law is loaded too.
#[test]
fn an_open_term_filled_for_its_host_applies_only_to_that_application() {
    let service = service_with(&[SUBSIDY_ACT, PERMIT_ACT]);

    let subsidy = BTreeMap::from([
        ("datum_ontvangst".to_string(), date("2026-01-05")),
        ("subsidiejaar".to_string(), Value::Int(2025)),
    ]);
    let with = service
        .evaluate_law_output(
            "test_subsidiewet_met_beslistermijn",
            "aangevraagd",
            subsidy,
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        with.outputs.get("uiterste_beslisdatum"),
        Some(&date("2026-07-06")),
        "twenty-six weeks, set by the law of the application"
    );
    assert_eq!(
        with.outputs.get("redelijke_beslistermijn_einddatum"),
        Some(&date("2026-03-02")),
        "the bound of lid 2 stays eight weeks after receipt"
    );

    let permit = BTreeMap::from([("datum_ontvangst".to_string(), date("2026-01-05"))]);
    let without = service
        .evaluate_law_output(
            "test_aanvraag_zonder_beslistermijn",
            "aangevraagd",
            permit,
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        without.outputs.get("uiterste_beslisdatum"),
        Some(&date("2026-03-02")),
        "eight weeks: the subsidy law does not fill the term of this application"
    );
}
