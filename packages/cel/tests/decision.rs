//! A cell decides on an application under a small fictional law (RFC-047):
//! a norm the law leaves to the deciding authority per case, a regulation
//! that fills it for some cases and is silent for others, and what a stage
//! requires of the cell.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::NaiveDate;
use regelrecht_cel::{decision, Cell, Error, Source};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

mod common;
use common::cell_tempdir;

/// A procedure for a beschikking, with a stage for the application.
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
        requires: [{name: aanvraag_datum, type: date}]
      - name: BESLUIT
        requires: [{name: besluit_datum, type: date}]
      - name: BEKENDMAKING
articles:
  - number: '1'
    text: Een besluit wordt gemotiveerd.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}
      execution:
        output:
          - {name: motivering_vereist, type: boolean}
        actions:
          - {output: motivering_vereist, value: true}
"#;

/// Article 1 establishes an application and decides on it, two things it
/// produces; the head judges per case whether the ground applies.
const SPECIAL_LAW: &str = r#"
$id: wet_verlof_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent op aanvraag verlof.
    machine_readable:
      open_terms:
        - id: grond_aanwezig
          type: boolean
          decided_per_case_by: het hoofd
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof_fictief#1
            parameters: [dagen]
          - type: BESCHIKKING
            outputs: [verlof_verleend]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: verlof_verleend, type: boolean}
        actions:
          - output: verlof_verleend
            value: $grond_aanwezig
"#;

/// A regulation that grants the ground for a short leave, and says nothing
/// for a longer one.
const POLICY: &str = r#"
$id: beleid_verlof_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid
articles:
  - number: '1'
    text: Bij ten hoogste drie dagen is de grond aanwezig.
    machine_readable:
      implements:
        - {law: wet_verlof_fictief, article: '1', open_term: grond_aanwezig}
      execution:
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: grond_aanwezig, type: boolean, nullable: true}
        actions:
          - output: grond_aanwezig
            value:
              operation: IF
              cases:
                - when: {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 3}
                  then: true
              default: null
"#;

const LAW: &str = "wet_verlof_fictief";
const ON: &str = "2026-10-13";

fn service(laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
}

/// A cell whose directory holds no reductions: a term no regulation fills
/// is then the deciding authority's finding.
fn bare(actor: &str) -> Cell {
    let dir = cell_tempdir();
    Cell::in_memory(actor, dir.path()).unwrap()
}

fn day() -> NaiveDate {
    NaiveDate::parse_from_str(ON, "%Y-%m-%d").unwrap()
}

fn object(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

/// Record an application for `dagen` days and decide on it with `supplied`:
/// the decision, and the findings it used ([`Cell::findings`]).
fn decide(
    service: &mut LawExecutionService,
    dagen: i64,
    supplied: Value,
) -> Result<(regelrecht_cel::Gram, Vec<String>), Error> {
    let mut cell = bare("fictieve_school");
    let app = cell
        .record_submission(
            service,
            &[],
            LAW,
            "1",
            None,
            &object(json!({"dagen": dagen})),
            day(),
        )
        .unwrap();
    cell.decide(
        service,
        &[],
        LAW,
        "1",
        &app.id,
        &Map::new(),
        &object(supplied),
        day(),
    )
    .map(|mut grams| grams.remove(0))
    .map(|gram| {
        let findings = cell.findings(service, &[], &gram.id).unwrap();
        (gram, findings)
    })
}

#[test]
fn a_judgement_on_a_term_a_regulation_also_fills_may_be_supplied() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW, POLICY]);
    let shape = decision::derive(&service, LAW, "1", day()).unwrap();
    let input = shape.input("grond_aanwezig").unwrap();
    assert_eq!(input.source, Source::Caller);
    assert!(input.policy_first);
}

#[test]
fn where_the_regulation_is_silent_the_judgement_decides() {
    let mut service = service(&[GENERAL_LAW, SPECIAL_LAW, POLICY]);
    let (gram, findings) = decide(&mut service, 5, json!({"grond_aanwezig": false})).unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    assert_eq!(gram.fields["grond_aanwezig"], json!(false));
    assert_eq!(findings, ["grond_aanwezig"]);

    // Without the judgement, nothing decides the norm for this case.
    let e = decide(&mut service, 5, json!({})).unwrap_err();
    assert!(
        matches!(&e, Error::Engine(m) if m.to_string().contains("decided per case by het hoofd")),
        "{e}"
    );
}

#[test]
fn where_the_regulation_fills_the_term_it_sets_the_judgement_aside() {
    let mut service = service(&[GENERAL_LAW, SPECIAL_LAW, POLICY]);
    let (gram, findings) = decide(&mut service, 2, json!({"grond_aanwezig": false})).unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    assert_eq!(gram.fields["grond_aanwezig"], json!(true));
    // The regulation decided it, not the head.
    assert!(findings.is_empty(), "{:?}", findings);

    // The judgement may be left out.
    let (gram, findings) = decide(&mut service, 2, json!({})).unwrap();
    assert_eq!(gram.fields["grond_aanwezig"], json!(true));
    assert!(findings.is_empty(), "{:?}", findings);
}

/// Where the regulation gives what the head found, the regulation still
/// decided it: the engine says who filled the term, the cell does not
/// read it from the value.
#[test]
fn a_regulation_that_gives_the_judgement_s_value_still_sets_it_aside() {
    let mut service = service(&[GENERAL_LAW, SPECIAL_LAW, POLICY]);
    let (gram, findings) = decide(&mut service, 2, json!({"grond_aanwezig": true})).unwrap();
    assert_eq!(gram.fields["grond_aanwezig"], json!(true));
    assert!(findings.is_empty(), "{:?}", findings);
}

#[test]
fn a_stage_of_the_application_that_requires_more_than_a_date_is_a_setup_error() {
    let general = GENERAL_LAW.replace(
        "requires: [{name: aanvraag_datum, type: date}]",
        "requires: [{name: kanaal, type: string}]",
    );
    let mut service = service(&[&general, SPECIAL_LAW]);
    let mut cell = bare("fictieve_school");
    let e = cell
        .record_submission(
            &mut service,
            &[],
            LAW,
            "1",
            None,
            &object(json!({"dagen": 2})),
            day(),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("requires 'kanaal'") && m.contains("only a date")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

/// The application holds the parameters its item lists, and the hooks on
/// it; a parameter of the article it does not list is no field of it, and
/// the decision asks it of the deciding authority (RFC-047).
#[test]
fn the_application_holds_what_its_item_lists() {
    let special = SPECIAL_LAW.replace(
        "          - {name: dagen, type: number}\n        output:",
        "          - {name: dagen, type: number}\n          - {name: reden, type: string}\n        output:",
    );
    assert_ne!(special, SPECIAL_LAW);
    let service = service(&[GENERAL_LAW, &special]);
    let application = regelrecht_cel::shape::derive(&service, LAW, "1", day()).unwrap();
    let names: Vec<&str> = application.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["dagen"]);
    assert_eq!(
        application.requests.as_deref(),
        Some("wet_verlof_fictief#1")
    );
    let shape = decision::derive(&service, LAW, "1", day()).unwrap();
    assert_eq!(shape.input("dagen").unwrap().source, Source::Application);
    assert_eq!(shape.input("reden").unwrap().source, Source::Caller);
}

/// Article 1 decides; article 2 establishes the application and asks for
/// the decision of article 1 with `requests`.
const SPLIT_LAW: &str = r#"
$id: wet_verlof_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent verlof.
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
            outputs: [verlof_verleend]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: verlof_verleend, type: boolean}
        actions:
          - output: verlof_verleend
            value: {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 3}
  - number: '2'
    text: Verlof wordt aangevraagd bij het hoofd.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof_fictief#1
            parameters: [dagen]
            outputs: [aangevraagd]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: aangevraagd, type: boolean}
        actions:
          - {output: aangevraagd, value: true}
"#;

#[test]
fn a_decision_is_taken_on_the_application_that_requests_it_from_another_article() {
    let mut service = service(&[GENERAL_LAW, SPLIT_LAW]);
    let shape = decision::derive(&service, LAW, "1", day()).unwrap();
    assert_eq!(shape.application, "wet_verlof_fictief#2");
    let mut cell = bare("fictieve_school");
    let app = cell
        .record_submission(
            &mut service,
            &[],
            LAW,
            "2",
            None,
            &object(json!({"dagen": 2})),
            day(),
        )
        .unwrap();
    let gram = cell
        .decide(
            &mut service,
            &[],
            LAW,
            "1",
            &app.id,
            &Map::new(),
            &Map::new(),
            day(),
        )
        .unwrap()
        .remove(0);
    assert_eq!(gram.refers_to.as_deref(), Some(app.id.as_str()));
    assert_eq!(gram.basis, "wet_verlof_fictief#1");
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn an_application_for_a_decision_of_a_law_that_is_not_loaded_is_refused_when_recorded() {
    let split = SPLIT_LAW.replace(
        "requests: wet_verlof_fictief#1",
        "requests: wet_elders_fictief#4",
    );
    let mut service = service(&[GENERAL_LAW, &split]);
    let mut cell = bare("fictieve_school");
    let e = cell
        .record_submission(
            &mut service,
            &[],
            LAW,
            "2",
            None,
            &object(json!({"dagen": 2})),
            day(),
        )
        .unwrap_err();
    assert!(
        e.to_string()
            .contains("requests the decision of wet_elders_fictief#4"),
        "{e}"
    );
    // And no application asks for the decision of article 1.
    let e = decision::derive(&service, LAW, "1", day()).unwrap_err();
    assert!(e.to_string().contains("no application asks"), "{e}");
}

#[test]
fn an_application_whose_decision_cannot_be_derived_is_still_recorded() {
    // A second article asks for the same decision, so the decision of
    // article 1 cannot be derived; the application is still recorded.
    let second = r#"
  - number: '3'
    text: Verlof wordt ook aangevraagd bij de directie.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof_fictief#1
            parameters: [dagen]
        parameters:
          - {name: dagen, type: number}
"#;
    let split = format!("{SPLIT_LAW}{}", second.trim_start_matches('\n'));
    let mut service = service(&[GENERAL_LAW, &split]);
    let e = decision::derive(&service, LAW, "1", day()).unwrap_err();
    assert!(e.to_string().contains("several applications"), "{e}");
    let mut cell = bare("fictieve_school");
    let app = cell
        .record_submission(
            &mut service,
            &[],
            LAW,
            "2",
            None,
            &object(json!({"dagen": 2})),
            day(),
        )
        .unwrap();
    assert_eq!(app.kind, "AANVRAAG");
    assert_eq!(cell.grams().len(), 1);
}

/// Data derived for the decision (an output the decision item lists under
/// `derived`, RFC-050) is computed at every stage of the decision, so a hook
/// that reads it as a parameter gets it from the execution: it is no input of
/// the decision.
#[test]
fn a_hook_that_reads_derived_data_asks_nothing_for_it() {
    let special = SPECIAL_LAW
        .replace(
            "            outputs: [verlof_verleend]\n",
            "            outputs: [verlof_verleend]\n            derived: [dagen_gevraagd]\n",
        )
        .replace(
            "          - {name: verlof_verleend, type: boolean}\n        actions:",
            "          - {name: verlof_verleend, type: boolean}\n          - {name: dagen_gevraagd, type: number}\n        actions:\n          - {output: dagen_gevraagd, value: $dagen}",
        );
    assert_ne!(special, SPECIAL_LAW);
    let general = format!(
        "{GENERAL_LAW}{}",
        r#"  - number: '2'
    text: Bij het besluit wordt het aantal dagen vermeld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BEKENDMAKING}
      execution:
        parameters:
          - {name: dagen_gevraagd, type: number}
        output:
          - {name: dagen_vermeld, type: number}
        actions:
          - {output: dagen_vermeld, value: $dagen_gevraagd}
"#
    );
    let mut service = service(&[&general, &special, POLICY]);
    let shape = decision::derive(&service, LAW, "1", day()).unwrap();
    assert!(
        shape.input("dagen_gevraagd").is_none(),
        "{:?}",
        shape.inputs
    );
    assert_eq!(shape.outputs, ["verlof_verleend"]);
    let (gram, _) = decide(&mut service, 2, json!({})).unwrap();
    assert_eq!(gram.outputs["dagen_vermeld"], json!(2));
    assert!(!gram.fields.contains_key("dagen_gevraagd"));
}

/// A judgement that may be absent and that the head leaves out is null in
/// the chronolexogram, but no finding: the head found nothing.
#[test]
fn a_judgement_left_out_is_no_finding() {
    let special = SPECIAL_LAW
        .replace(
            "          decided_per_case_by: het hoofd\n",
            "          required: false\n          decided_per_case_by: het hoofd\n",
        )
        .replace(
            "          - {name: verlof_verleend, type: boolean}",
            "          - {name: verlof_verleend, type: boolean, nullable: true}",
        );
    assert_ne!(special, SPECIAL_LAW);
    let mut service = service(&[GENERAL_LAW, &special]);
    let (gram, findings) = decide(&mut service, 5, json!({})).unwrap();
    assert_eq!(gram.fields["grond_aanwezig"], Value::Null);
    assert!(findings.is_empty(), "{:?}", findings);
}
