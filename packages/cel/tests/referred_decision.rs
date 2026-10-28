//! A submission that refers to a decision, and a decision that takes
//! another in turn (RFC-050, slice 4b-2), under toy laws written for the
//! cell: no corpus, no Awb.
//!
//! The cell fills the field of type `decision` from `refers_to` and keeps
//! the decision's id; the deciding article gets its decision value. The
//! deciding article here calls the article of that decision
//! (`source.article`) with its fields and what the objector adds, and gives
//! the outcome as an output of type `decision` when it differs. The cell
//! records that outcome as a decision of its own, in the same append. The
//! cell's reduction reads the chronicle by row id, and its answer, which a
//! policy binds to the term (RFC-048), reaches the called article through
//! the fields of the decision value.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{bare_cell, cell_tempdir, day, object_map};

use regelrecht_cel::{reading, Cell, Error, Gram, Reference};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

/// A procedure for a beschikking, a period that runs from the
/// announcement, and whether an objection came in within it.
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
          - {name: termijn_einde, type: date, required: false}
          - {name: bestreden_besluit, type: decision, required: false}
        output:
          - {name: tijdig, type: boolean}
          - {name: bestreden_grondslag, type: string}
        actions:
          - output: bestreden_grondslag
            value: $bestreden_besluit.basis
          - output: tijdig
            value:
              operation: GREATER_THAN_OR_EQUAL
              subject: {operation: DATE_DIFF, from: $ontvangen_op, to: $termijn_einde, in: days}
              value: 0
"#;

/// Leave for at most ten days, once: not when it was granted already.
const VERLOF: &str = r#"
$id: wet_verlof_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent eenmaal verlof van ten hoogste tien dagen.
    machine_readable:
      open_terms:
        - id: al_verleend
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof_fictief#1
            parameters: [bsn, dagen]
          - type: BESCHIKKING
            outputs: [verleend]
        parameters:
          - {name: bsn, type: string}
          - {name: dagen, type: number}
        output:
          - {name: verleend, type: boolean}
        actions:
          - output: verleend
            value:
              operation: AND
              conditions:
                - {operation: NOT, value: $al_verleend}
                - {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}
"#;

/// The objection names the decision it contests; the decision on it calls
/// that decision's article anew and gives the new decision when the outcome
/// differs.
const HEROVERWEGING: &str = r#"
$id: wet_heroverweging_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Op bezwaar wordt het besluit heroverwogen en zo nodig vervangen.
    machine_readable:
      execution:
        produces:
          - type: BEZWAAR
            requests: wet_heroverweging_fictief#1
            parameters: [bestreden_besluit, nieuwe_gegevens]
          - type: BESCHIKKING
            outputs: [aanleiding, nieuw_besluit]
        parameters:
          - {name: bestreden_besluit, type: decision}
          - {name: nieuwe_gegevens, type: object, nullable: true}
          - {name: tijdig, type: boolean}
        input:
          - name: heroverweging
            type: decision
            nullable: true
            source:
              article: $bestreden_besluit.basis
              parameters_from:
                - $bestreden_besluit.fields
                - $nieuwe_gegevens
        output:
          - {name: aanleiding, type: boolean}
          - {name: nieuw_besluit, type: decision, nullable: true}
        actions:
          - output: aanleiding
            value:
              operation: IF
              cases:
                - when: $tijdig
                  then:
                    operation: NOT
                    value: {operation: EQUALS, subject: $heroverweging.outputs, value: $bestreden_besluit.outputs}
              default: false
          - output: nieuw_besluit
            value:
              operation: IF
              cases:
                - when: $aanleiding
                  then: $heroverweging
              default: null
"#;

/// The policy of the cell's holder that binds the term to the answer of
/// its cell (RFC-048).
const BINDING: &str = r#"
$id: beleid_al_verleend_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid
competent_authority:
  name: Testcel
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/beleid#1
    machine_readable:
      open_terms:
        - id: al_verleend_volgens_hoofd
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      implements:
        - {law: wet_verlof_fictief, article: '1', open_term: al_verleend}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord
            type: boolean
            nullable: true
            source:
              reduction: al_verleend
              parameters: {bsn: $bsn}
        output:
          - {name: al_verleend, type: boolean}
        actions:
          - output: al_verleend
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord, value: null}
                  then: $al_verleend_volgens_hoofd
              default: $antwoord
"#;

/// The cell's reduction: a grant to the same person counts, unless a
/// decision on objection against it found cause. It links the two by row
/// id: `bestreden_besluit` holds the id of the contested decision.
const REDUCTION: &str = r#"
$id: reductie_al_verleend_fictief
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/reductie#1
    machine_readable:
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: besluiten
            type: array
            source: {decisions: 'wet_verlof_fictief#1'}
          - name: heroverwegingen
            type: array
            source: {decisions: 'wet_heroverweging_fictief#1'}
        output:
          - {name: herroepen, type: array}
          - {name: al_verleend, type: boolean}
        actions:
          # The ids of the decisions that a decision on objection found
          # cause against. A FOREACH body does not see the binding of an
          # outer FOREACH, so the link is a list of ids, not a nested loop.
          - output: herroepen
            value:
              operation: FOREACH
              collection: $heroverwegingen
              as: heroverweging
              filter: {operation: EQUALS, subject: $heroverweging.aanleiding, value: true}
              body: $heroverweging.bestreden_besluit
          - output: al_verleend
            value:
              operation: FOREACH
              collection: $besluiten
              as: besluit
              filter:
                operation: AND
                conditions:
                  - {operation: EQUALS, subject: $besluit.verleend, value: true}
                  - {operation: EQUALS, subject: $besluit.bsn, value: $bsn}
                  - {operation: NOT_IN, subject: $besluit.id, value: $herroepen}
              body: true
              combine: OR
"#;

const ACTOR: &str = "fictieve_school";
const VERLOF_LAW: &str = "wet_verlof_fictief";
const BEZWAAR_LAW: &str = "wet_heroverweging_fictief";
const BSN: &str = "999990019";

fn service_with(heroverweging: &str) -> LawExecutionService {
    service_of(&[PROCEDURE, VERLOF, heroverweging])
}

fn service() -> LawExecutionService {
    service_with(HEROVERWEGING)
}

/// What the article of the submission `gram` gives on it, with the hooks on
/// it: not in its chronolexogram, but computed again by the cell.
fn given(cell: &Cell, gram: &Gram) -> std::collections::BTreeMap<String, Value> {
    cell.submission_outputs(&mut service(), &[], &gram.id)
        .unwrap()
}

/// A cell that gives the answer `al_verleend` from the reduction, as its
/// catalogue says.
fn cell() -> (Cell, tempfile::TempDir) {
    let dir = cell_tempdir();
    common::write_identity(
        dir.path(),
        "name: Testcel\nanswers:\n  al_verleend: reductie_al_verleend_fictief\n",
    );
    common::keep_reduction(dir.path(), "reductie_al_verleend_fictief", REDUCTION);
    let cell = Cell::in_memory(ACTOR, dir.path()).unwrap();
    (cell, dir)
}

fn apply(cell: &mut Cell, service: &mut LawExecutionService, dagen: i64, on: &str) -> Gram {
    cell.record_submission(
        service,
        &[],
        VERLOF_LAW,
        "1",
        None,
        &object_map(json!({"bsn": BSN, "dagen": dagen})),
        day(on),
    )
    .unwrap()
}

fn decide(
    cell: &mut Cell,
    service: &mut LawExecutionService,
    law: &str,
    submission: &str,
    on: &str,
) -> Result<Vec<Gram>, Error> {
    cell.decide(
        service,
        &[],
        law,
        "1",
        submission,
        &Map::new(),
        &Map::new(),
        day(on),
    )
}

fn object_to(
    cell: &mut Cell,
    service: &mut LawExecutionService,
    contested: &str,
    supplied: Value,
    on: &str,
) -> Result<Gram, Error> {
    cell.record_submission(
        service,
        &[],
        BEZWAAR_LAW,
        "1",
        Some(Reference::Local(contested)),
        &object_map(supplied),
        day(on),
    )
}

/// A grant `granted` and, after it, a refusal `refused` to the same person,
/// refused because of the grant.
struct Granted {
    granted: Gram,
    refused: Gram,
}

fn grant_then_refuse(cell: &mut Cell, service: &mut LawExecutionService) -> Granted {
    let first = apply(cell, service, 2, "2026-03-02");
    let second = apply(cell, service, 3, "2026-03-02");
    let granted = decide(cell, service, VERLOF_LAW, &second.id, "2026-03-03")
        .unwrap()
        .remove(0);
    assert_eq!(granted.outputs["verleend"], json!(true));
    let refused = decide(cell, service, VERLOF_LAW, &first.id, "2026-03-04")
        .unwrap()
        .remove(0);
    assert_eq!(refused.fields["al_verleend"], json!(true));
    assert_eq!(refused.outputs["verleend"], json!(false));
    Granted { granted, refused }
}

#[test]
fn the_decision_field_comes_from_refers_to_and_is_kept_as_its_id() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    assert_eq!(bezwaar.kind, "BEZWAAR");
    assert_eq!(bezwaar.refers_to.as_deref(), Some(granted.id.as_str()));
    assert_eq!(bezwaar.fields["bestreden_besluit"], json!(granted.id));
    // A field the decision holds by name comes from it.
    assert_eq!(
        bezwaar.fields["termijn_einde"],
        granted.outputs["termijn_einde"]
    );
    assert_eq!(bezwaar.fields["nieuwe_gegevens"], Value::Null);
    assert_eq!(given(&cell, &bezwaar)["tijdig"], json!(true));
    // The hook got the decision value, not the id.
    assert_eq!(
        given(&cell, &bezwaar)["bestreden_grondslag"],
        json!("wet_verlof_fictief#1")
    );
}

#[test]
fn what_comes_from_the_decision_the_submitter_does_not_supply() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    for supplied in [
        json!({"bestreden_besluit": granted.id}),
        json!({"termijn_einde": "2026-12-31"}),
    ] {
        let e =
            object_to(&mut cell, &mut service, &granted.id, supplied, "2026-03-10").unwrap_err();
        assert!(
            matches!(&e, Error::Refused(m) if m.contains("comes from the decision it refers to")),
            "{e}"
        );
    }
    // Also without a reference, the decision field is not the submitter's.
    let e = cell
        .record_submission(
            &mut service,
            &[],
            BEZWAAR_LAW,
            "1",
            None,
            &object_map(json!({"bestreden_besluit": granted.id})),
            day("2026-03-10"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("comes from the decision it refers to")),
        "{e}"
    );
    // Without one, the field is missing.
    let unreferred = cell
        .record_submission(
            &mut service,
            &[],
            BEZWAAR_LAW,
            "1",
            None,
            &Map::new(),
            day("2026-03-10"),
        )
        .unwrap();
    assert_eq!(unreferred.missing, ["bestreden_besluit", "termijn_einde"]);
    assert_eq!(unreferred.refers_to, None);
}

#[test]
fn a_submission_refers_only_to_a_decision_in_the_chronicle() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let e = object_to(
        &mut cell,
        &mut service,
        &application.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("not a decision")),
        "{e}"
    );
    let e = object_to(&mut cell, &mut service, "elders", json!({}), "2026-03-10").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("no chronolexogram 'elders'")),
        "{e}"
    );
    // A decision is fine, also one that is not on an application.
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    let again = object_to(
        &mut cell,
        &mut service,
        &decided[0].id,
        json!({}),
        "2026-03-12",
    );
    assert!(again.is_ok(), "{again:?}");
}

#[test]
fn a_submission_holds_at_most_one_decision() {
    let two = HEROVERWEGING
        .replace(
            "parameters: [bestreden_besluit, nieuwe_gegevens]",
            "parameters: [bestreden_besluit, ander_besluit, nieuwe_gegevens]",
        )
        .replace(
            "          - {name: nieuwe_gegevens, type: object, nullable: true}",
            "          - {name: ander_besluit, type: decision}\n          - {name: nieuwe_gegevens, type: object, nullable: true}",
        );
    assert_ne!(two, HEROVERWEGING);
    let mut service = service_with(&two);
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    let e = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("holds 2 decisions")),
        "{e}"
    );
}

/// The decision on the objection gets the contested decision as a decision
/// value. Without cause there is no new decision, and the reduction reads
/// the chronicle without the contested decision: it does not count against
/// itself.
#[test]
fn without_cause_one_decision_and_the_contested_one_does_not_count_itself() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided.len(), 1);
    let decision = &decided[0];
    assert_eq!(decision.refers_to.as_deref(), Some(bezwaar.id.as_str()));
    assert_eq!(decision.fields["bestreden_besluit"], json!(granted.id));
    assert_eq!(decision.outputs["aanleiding"], json!(false));
    assert_eq!(decision.outputs["nieuw_besluit"], Value::Null);
    assert_eq!(cell.grams().len(), 4);
}

/// Without an output of type `decision` the deciding article replaces no
/// decision, so the decision it refers to counts in the reductions as any
/// other, also where the cell answers in that decision's article called
/// anew: the grant counts against the person, and taken again the article
/// refuses. One reading of the chronicle serves the whole decision (keuze
/// 35 of slice 4b lapses with slice 4c).
#[test]
fn a_decision_that_replaces_nothing_reads_the_decision_it_refers_to() {
    let only_cause = HEROVERWEGING.replace(
        "outputs: [aanleiding, nieuw_besluit]",
        "outputs: [aanleiding]",
    );
    let only_cause = only_cause
        .replace(
            "          - {name: nieuw_besluit, type: decision, nullable: true}\n",
            "",
        )
        .split_once("          - output: nieuw_besluit")
        .unwrap()
        .0
        .to_string();
    assert!(!only_cause.contains("nieuw_besluit"));
    let mut service = service_with(&only_cause);
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    assert_eq!(granted.outputs["verleend"], json!(true));
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided.len(), 1);
    // Its article anew counts the grant, so it refuses: cause.
    assert_eq!(decided[0].outputs["aanleiding"], json!(true));
}

/// Revoke and replace: the objector's new data give another outcome, the
/// decision on the objection gives it as a decision, and the cell records
/// that decision too: under its own article, referring to the objection,
/// with the stages run again so it has its own period.
#[test]
fn with_cause_the_cell_records_the_new_decision_with_its_own_period() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({"nieuwe_gegevens": {"dagen": 20}}),
        "2026-03-10",
    )
    .unwrap();
    let before = cell.grams().len();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided.len(), 2);
    assert_eq!(cell.grams().len(), before + 2);
    let (decision, new) = (&decided[0], &decided[1]);
    assert_eq!(decision.basis, "wet_heroverweging_fictief#1");
    assert_eq!(decision.outputs["aanleiding"], json!(true));
    // The decision holds the new one by its id.
    assert_eq!(decision.outputs["nieuw_besluit"], json!(new.id));
    assert_eq!(new.kind, "BESCHIKKING");
    assert_eq!(new.basis, "wet_verlof_fictief#1");
    assert_eq!(new.refers_to.as_deref(), Some(bezwaar.id.as_str()));
    assert_eq!(new.recorded_on, day("2026-03-11"));
    assert_eq!(new.fields["dagen"], json!(20));
    assert_eq!(new.fields["bsn"], json!(BSN));
    assert_eq!(new.fields["bekendmaking_datum"], json!("2026-03-11"));
    assert_eq!(new.outputs["verleend"], json!(false));
    // Its own period, from its own announcement.
    assert_eq!(new.outputs["termijn_einde"], json!("2026-04-22"));
    assert_ne!(
        new.outputs["termijn_einde"],
        granted.outputs["termijn_einde"]
    );
}

/// Decision 7, option a: the reduction's answer reaches the called article
/// through the fields of the decision value, answered again on the day of
/// the decision on the objection. The refusal was right when it was taken,
/// as the grant counted; by now that grant is revoked, so the heroverweging
/// grants.
#[test]
fn the_answer_of_a_reduction_reaches_the_called_article_on_the_day_of_the_decision() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let Granted { granted, refused } = grant_then_refuse(&mut cell, &mut service);
    // The grant is revoked on objection; in its place, a refusal.
    let against_grant = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({"nieuwe_gegevens": {"dagen": 20}}),
        "2026-03-10",
    )
    .unwrap();
    let revoked = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &against_grant.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(revoked.len(), 2);

    let against_refusal = object_to(
        &mut cell,
        &mut service,
        &refused.id,
        json!({}),
        "2026-03-12",
    )
    .unwrap();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &against_refusal.id,
        "2026-03-13",
    )
    .unwrap();
    assert_eq!(decided.len(), 2, "{decided:#?}");
    let new = &decided[1];
    assert_eq!(new.fields["al_verleend"], json!(false));
    assert_eq!(new.fields["dagen"], json!(2));
    assert_eq!(new.outputs["verleend"], json!(true));
    // The refusal itself keeps what it was taken on.
    assert_eq!(refused.fields["al_verleend"], json!(true));

    // The rows link a decision on objection to the decision it contests by
    // id, and hold the new decision by id too.
    let rows = serde_json::to_value(
        reading::rows(
            &service,
            cell.grams(),
            "wet_heroverweging_fictief#1",
            "BESCHIKKING",
        )
        .unwrap(),
    )
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], json!(revoked[0].id));
    assert_eq!(rows[0]["bestreden_besluit"], json!(granted.id));
    assert_eq!(rows[0]["nieuw_besluit"], json!(revoked[1].id));
    assert_eq!(rows[1]["bestreden_besluit"], json!(refused.id));

    // A later application counts the new grant.
    let later = apply(&mut cell, &mut service, 1, "2026-03-20");
    let refused_again = decide(&mut cell, &mut service, VERLOF_LAW, &later.id, "2026-03-20")
        .unwrap()
        .remove(0);
    assert_eq!(refused_again.fields["al_verleend"], json!(true));
}

/// The cell takes a decision value as the decision of its article only if
/// that article, run on the value's fields, decides what the value says.
/// Otherwise nothing lands in the chronicle.
#[test]
fn a_decision_value_its_article_does_not_give_is_refused_and_nothing_is_appended() {
    // The deciding article gives the contested decision back as the new one;
    // since it was taken, the law allows one day of leave at most.
    let stale = HEROVERWEGING.replace("then: $heroverweging", "then: $bestreden_besluit");
    assert_ne!(stale, HEROVERWEGING);
    let stricter = VERLOF
        .replace("valid_from: '2026-01-01'", "valid_from: '2026-03-05'")
        .replace("value: 10", "value: 1");
    let mut service = service_with(&stale);
    service.load_law(&stricter).unwrap();
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let before = cell.grams().len();
    let e = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("taken again on these fields")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), before);
    assert!(service.data_registry().list_sources().is_empty());

    // With the call's own outcome, the new decision is the refusal.
    let mut fresh = service_with(HEROVERWEGING);
    fresh.load_law(&stricter).unwrap();
    let decided = decide(
        &mut cell,
        &mut fresh,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided[1].outputs["verleend"], json!(false));
}

/// [`VERLOF`] with a finding per case: the head judges whether the
/// reason is weighty (`decided_per_case_by`).
fn verlof_with_finding() -> String {
    VERLOF
        .replace(
            "    machine_readable:\n      open_terms:\n",
            "    machine_readable:\n      open_terms:\n        - id: reden_gewichtig\n          type: boolean\n          required: true\n          decided_per_case_by: het hoofd\n",
        )
        .replace(
            "                - {operation: NOT, value: $al_verleend}\n",
            "                - {operation: NOT, value: $al_verleend}\n                - $reden_gewichtig\n",
        )
}

/// [`HEROVERWEGING`] that calls the contested decision anew, with its own
/// fields and findings, and gives the parts of the decision value it got.
fn heroverweging_with_findings() -> String {
    HEROVERWEGING
        .replace(
            "              article: $bestreden_besluit.basis\n              parameters_from:\n                - $bestreden_besluit.fields\n                - $nieuwe_gegevens\n",
            "              article: $bestreden_besluit\n",
        )
        .replace(
            "            outputs: [aanleiding, nieuw_besluit]",
            "            outputs: [aanleiding, nieuw_besluit, gegevens, oordelen]",
        )
        .replace(
            "          - {name: nieuw_besluit, type: decision, nullable: true}\n",
            "          - {name: nieuw_besluit, type: decision, nullable: true}\n          - {name: gegevens, type: object}\n          - {name: oordelen, type: object}\n",
        )
        .replace(
            "        actions:\n          - output: aanleiding",
            "        actions:\n          - {output: gegevens, value: $bestreden_besluit.fields}\n          - {output: oordelen, value: $bestreden_besluit.findings}\n          - output: aanleiding",
        )
}

/// The decision value keeps the findings apart from the fields (RFC-050):
/// which of the gram's fields are findings follows from the open terms of
/// the deciding article. A finding not given anew stays that of the
/// decision, and one given anew, apart from the parameters, reaches the
/// decision called anew and is recorded with the decision on the
/// objection. One that no article asks is refused.
#[test]
fn the_decision_value_holds_the_findings_apart() {
    let mut service = common::service_of(&[
        PROCEDURE,
        &verlof_with_finding(),
        &heroverweging_with_findings(),
    ]);
    let mut cell = bare_cell(ACTOR);
    let applied = apply(&mut cell, &mut service, 2, "2026-03-02");
    let refused = cell
        .decide(
            &mut service,
            &[],
            VERLOF_LAW,
            "1",
            &applied.id,
            &Map::new(),
            &object_map(json!({"al_verleend": false, "reden_gewichtig": false})),
            day("2026-03-03"),
        )
        .unwrap()
        .remove(0);
    assert_eq!(refused.fields["reden_gewichtig"], json!(false));
    assert_eq!(refused.outputs["verleend"], json!(false));

    for (anew, cause, objected, decided_on) in [
        (json!({}), false, "2026-03-10", "2026-03-11"),
        (
            json!({"reden_gewichtig": true}),
            true,
            "2026-03-12",
            "2026-03-13",
        ),
    ] {
        let bezwaar = object_to(&mut cell, &mut service, &refused.id, json!({}), objected).unwrap();
        let decided = cell
            .decide(
                &mut service,
                &[],
                BEZWAAR_LAW,
                "1",
                &bezwaar.id,
                &Map::new(),
                &object_map(anew.clone()),
                day(decided_on),
            )
            .unwrap();
        let decision = &decided[0];
        assert_eq!(
            decision.outputs["oordelen"],
            json!({"al_verleend": false, "reden_gewichtig": false}),
            "{anew}"
        );
        assert!(
            decision.outputs["gegevens"]
                .get("reden_gewichtig")
                .is_none(),
            "{}",
            decision.outputs["gegevens"]
        );
        assert_eq!(decision.outputs["aanleiding"], json!(cause), "{anew}");
        if cause {
            assert_eq!(decision.fields["reden_gewichtig"], json!(true));
            // The new decision is taken with the finding given anew.
            assert_eq!(decided[1].fields["reden_gewichtig"], json!(true));
            assert_eq!(decided[1].outputs["verleend"], json!(true));
            break;
        }
    }

    // A finding no article asks is refused, and nothing is appended.
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &refused.id,
        json!({}),
        "2026-03-14",
    )
    .unwrap();
    let before = cell.grams().len();
    let e = cell
        .decide(
            &mut service,
            &[],
            BEZWAAR_LAW,
            "1",
            &bezwaar.id,
            &Map::new(),
            &object_map(json!({"reden_gewichtg": true})),
            day("2026-03-15"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("'reden_gewichtg'"), "{e}");
    assert_eq!(cell.grams().len(), before);
    // A finding is given among the findings, not supplied; and what the
    // deciding authority supplies is no finding.
    let applied = apply(&mut cell, &mut service, 3, "2026-03-16");
    for (supplied, findings, says) in [
        (
            json!({"reden_gewichtig": true}),
            json!({}),
            "'reden_gewichtig' of the decision of wet_verlof_fictief#1 is a finding of the \
             deciding authority; it is given among the findings",
        ),
        (
            json!({}),
            json!({"bsn": BSN, "reden_gewichtig": true}),
            "'bsn' of the decision of wet_verlof_fictief#1 is no finding",
        ),
        (
            json!({}),
            json!({"dagen": 4, "reden_gewichtig": true}),
            "'dagen' of the decision of wet_verlof_fictief#1 is no finding",
        ),
    ] {
        let e = cell
            .decide(
                &mut service,
                &[],
                VERLOF_LAW,
                "1",
                &applied.id,
                &object_map(supplied),
                &object_map(findings),
                day("2026-03-16"),
            )
            .unwrap_err();
        assert!(e.to_string().contains(says), "{says}: {e}");
    }
}

/// A regulation that fills the head's judgement on whether the reason is
/// weighty, from `valid_from`: with `value`, or, for `null`, with what the
/// head judges per case, an open term of its own (RFC-048 §2).
fn reden_policy(valid_from: &str, value: &str) -> String {
    let (own, value) = if value == "null" {
        (
            "      open_terms:\n        - id: reden_gewichtig_volgens_hoofd\n          type: boolean\n          required: true\n          decided_per_case_by: het hoofd\n",
            "$reden_gewichtig_volgens_hoofd",
        )
    } else {
        ("", value)
    };
    format!(
        r#"
$id: beleid_reden_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '{valid_from}'
url: https://example.org/beleid
articles:
  - number: '1'
    text: Fictief.
    machine_readable:
{own}      implements:
        - {{law: wet_verlof_fictief, article: '1', open_term: reden_gewichtig}}
      execution:
        output:
          - {{name: reden_gewichtig, type: boolean}}
        actions:
          - {{output: reden_gewichtig, value: {value}}}
"#
    )
}

/// `laws`, with the binding and the reduction of the cell.
fn service_of(laws: &[&str]) -> LawExecutionService {
    common::service_of(&[laws, &[BINDING, REDUCTION]].concat())
}

/// Record an application for two days on 2026-03-02 and decide on it on
/// 2026-03-03 with `findings`; the cell has no reductions, so the head
/// finds that no leave was granted before.
fn decided_with(cell: &mut Cell, service: &mut LawExecutionService, findings: Value) -> Gram {
    let applied = apply(cell, service, 2, "2026-03-02");
    let mut findings = object_map(findings);
    findings.insert("al_verleend_volgens_hoofd".into(), json!(false));
    cell.decide(
        service,
        &[],
        VERLOF_LAW,
        "1",
        &applied.id,
        &Map::new(),
        &findings,
        day("2026-03-03"),
    )
    .unwrap()
    .remove(0)
}

/// Decide on an objection with the head's `findings`.
fn decide_objection(
    cell: &mut Cell,
    service: &mut LawExecutionService,
    bezwaar: &str,
    findings: Value,
    on: &str,
) -> Result<Vec<Gram>, Error> {
    cell.decide(
        service,
        &[],
        BEZWAAR_LAW,
        "1",
        bezwaar,
        &Map::new(),
        &object_map(findings),
        day(on),
    )
}

/// A term a regulation filled for the decision is the regulation's, not a
/// finding of the head: called anew when the regulation is silent, the
/// decision has no finding on it to pass, and the head gives one anew.
#[test]
fn a_term_a_regulation_filled_is_no_finding_of_the_decision() {
    let mut service = service_of(&[
        PROCEDURE,
        &verlof_with_finding(),
        &heroverweging_with_findings(),
        &reden_policy("2026-01-01", "true"),
        &reden_policy("2026-03-05", "null"),
    ]);
    let mut cell = bare_cell(ACTOR);
    let granted = decided_with(&mut cell, &mut service, json!({}));
    assert_eq!(granted.fields["reden_gewichtig"], json!(true));
    // Only the head's finding that no leave was granted before.
    assert_eq!(
        cell.findings(&mut service, &[], &granted.id).unwrap(),
        ["al_verleend_volgens_hoofd"]
    );

    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let before = cell.grams().len();
    let e = decide_objection(
        &mut cell,
        &mut service,
        &bezwaar.id,
        json!({}),
        "2026-03-11",
    )
    .unwrap_err();
    assert!(e.to_string().contains("het hoofd"), "{e}");
    assert_eq!(cell.grams().len(), before);

    let decided = decide_objection(
        &mut cell,
        &mut service,
        &bezwaar.id,
        json!({"reden_gewichtig_volgens_hoofd": false}),
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(
        decided[0].outputs["oordelen"],
        json!({"al_verleend_volgens_hoofd": false})
    );
    assert_eq!(decided.len(), 2);
    let new = &decided[1];
    assert_eq!(new.fields["reden_gewichtig"], json!(false));
    assert_eq!(
        cell.findings(&mut service, &[], &new.id).unwrap(),
        ["al_verleend_volgens_hoofd", "reden_gewichtig_volgens_hoofd"]
    );
    assert_eq!(new.outputs["verleend"], json!(false));
}

/// A regulation in force on the day of the call anew sets the decision's
/// own finding aside, as it would a finding given anew; the new decision
/// records the regulation's value, and no finding.
#[test]
fn a_regulation_in_force_at_the_call_anew_sets_the_old_finding_aside() {
    let mut service = service_of(&[
        PROCEDURE,
        &verlof_with_finding(),
        &heroverweging_with_findings(),
        &reden_policy("2026-03-05", "true"),
    ]);
    let mut cell = bare_cell(ACTOR);
    let refused = decided_with(&mut cell, &mut service, json!({"reden_gewichtig": false}));
    assert_eq!(refused.outputs["verleend"], json!(false));
    assert_eq!(
        cell.findings(&mut service, &[], &refused.id).unwrap(),
        ["al_verleend_volgens_hoofd", "reden_gewichtig"]
    );

    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &refused.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide_objection(
        &mut cell,
        &mut service,
        &bezwaar.id,
        json!({}),
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(
        decided[0].outputs["oordelen"],
        json!({"al_verleend_volgens_hoofd": false, "reden_gewichtig": false})
    );
    assert_eq!(decided.len(), 2);
    let new = &decided[1];
    assert_eq!(new.outputs["verleend"], json!(true));
    assert_eq!(new.fields["reden_gewichtig"], json!(true));
    // The regulation set the old finding aside; the other stays.
    assert_eq!(
        cell.findings(&mut service, &[], &new.id).unwrap(),
        ["al_verleend_volgens_hoofd"]
    );
}

/// The head's finding reaches every article in the decision that leaves
/// the term to it, the deciding article and the article called anew alike,
/// also where both name the term the same.
#[test]
fn a_finding_on_a_term_both_articles_name_reaches_the_call_anew() {
    let heroverweging = heroverweging_with_findings().replace(
        "    machine_readable:\n      execution:\n        produces:\n          - type: BEZWAAR",
        "    machine_readable:\n      open_terms:\n        - id: reden_gewichtig\n          type: boolean\n          decided_per_case_by: het hoofd\n      execution:\n        produces:\n          - type: BEZWAAR",
    );
    assert_ne!(heroverweging, heroverweging_with_findings());
    let mut service = service_of(&[PROCEDURE, &verlof_with_finding(), &heroverweging]);
    let mut cell = bare_cell(ACTOR);
    let refused = decided_with(&mut cell, &mut service, json!({"reden_gewichtig": false}));
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &refused.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide_objection(
        &mut cell,
        &mut service,
        &bezwaar.id,
        json!({"reden_gewichtig": true}),
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided[0].fields["reden_gewichtig"], json!(true));
    assert_eq!(
        cell.findings(&mut service, &[], &decided[0].id).unwrap(),
        ["reden_gewichtig"]
    );
    assert_eq!(decided.len(), 2, "{decided:#?}");
    assert_eq!(decided[1].fields["reden_gewichtig"], json!(true));
    assert_eq!(decided[1].outputs["verleend"], json!(true));
}

/// The value the deciding article gets holds the dates the stages require
/// of the cell as the day of the decision, as the new decision does: an
/// output that depends on one is the same in the call anew and in the
/// decision the cell records.
#[test]
fn a_decision_called_anew_has_the_stage_dates_of_the_day() {
    let dated = VERLOF
        .replace("outputs: [verleend]", "outputs: [verleend, besloten_op]")
        .replace(
            "          - {name: dagen, type: number}\n",
            "          - {name: dagen, type: number}\n          - {name: besluit_datum, type: date}\n",
        )
        .replace(
            "          - {name: verleend, type: boolean}\n        actions:\n",
            "          - {name: verleend, type: boolean}\n          - {name: besloten_op, type: date}\n        actions:\n          - {output: besloten_op, value: $besluit_datum}\n",
        );
    assert_ne!(dated, VERLOF);
    let mut service = service_of(&[PROCEDURE, &dated, HEROVERWEGING]);
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    assert_eq!(granted.outputs["besloten_op"], json!("2026-03-03"));
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({"nieuwe_gegevens": {"dagen": 20}}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided.len(), 2);
    assert_eq!(decided[1].outputs["besloten_op"], json!("2026-03-11"));
}

/// The head gives a finding for a call anew once the call happens: given
/// while the decision does not call anew (the objection is late), no
/// article asks it, and the decision is refused rather than the finding
/// passed over.
#[test]
fn a_finding_for_a_call_anew_that_does_not_happen_is_refused() {
    let mut service = service_of(&[
        PROCEDURE,
        &verlof_with_finding(),
        &heroverweging_with_findings(),
    ]);
    let mut cell = bare_cell(ACTOR);
    let refused = decided_with(&mut cell, &mut service, json!({"reden_gewichtig": false}));
    // Six weeks after its announcement on 2026-03-03 the period is over.
    let late = object_to(
        &mut cell,
        &mut service,
        &refused.id,
        json!({}),
        "2026-05-01",
    )
    .unwrap();
    assert_eq!(
        cell.submission_outputs(&mut service, &[], &late.id)
            .unwrap()["tijdig"],
        json!(false)
    );
    let before = cell.grams().len();
    let e = decide_objection(
        &mut cell,
        &mut service,
        &late.id,
        json!({"reden_gewichtig": true}),
        "2026-05-02",
    )
    .unwrap_err();
    assert!(
        e.to_string().contains("'reden_gewichtig'") && e.to_string().contains("asked by none"),
        "{e}"
    );
    assert_eq!(cell.grams().len(), before);
    let decided =
        decide_objection(&mut cell, &mut service, &late.id, json!({}), "2026-05-02").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs["aanleiding"], json!(false));
}

/// A field the cell takes by name from the decision referred to must fit
/// the field of the submission: the two articles may mean another thing by
/// the name.
#[test]
fn a_value_taken_by_name_from_the_decision_must_fit_the_field() {
    let typed = HEROVERWEGING
        .replace(
            "parameters: [bestreden_besluit, nieuwe_gegevens]",
            "parameters: [bestreden_besluit, nieuwe_gegevens, dagen]",
        )
        .replace(
            "          - {name: tijdig, type: boolean}\n",
            "          - {name: tijdig, type: boolean}\n          - {name: dagen, type: string}\n",
        );
    assert_ne!(typed, HEROVERWEGING);
    let mut service = service_with(&typed);
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    let e = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("'dagen'")
            && m.contains("wet_heroverweging_fictief#1")
            && m.contains("wet_verlof_fictief#1")),
        "{e}"
    );
}

/// A decision taken under a law with a decision of its own: its decision
/// is a decision of the leave law, which the cell records with it.
const NEST: &str = r#"
$id: wet_nest_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Fictief.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_nest_fictief#1
            parameters: [bsn, dagen]
          - type: BESCHIKKING
            outputs: [binnen]
        parameters:
          - {name: bsn, type: string}
          - {name: dagen, type: number}
        input:
          - name: verlof
            type: decision
            source:
              article: wet_verlof_fictief#1
              parameters: {bsn: $bsn, dagen: $dagen}
        output:
          - {name: binnen, type: decision}
        actions:
          - {output: binnen, value: $verlof}
"#;

/// The cell records a decision taken in turn, one level: a new decision
/// that would take a decision in turn itself is refused, and nothing is
/// appended.
#[test]
fn a_new_decision_that_takes_a_decision_in_turn_is_refused() {
    let mut service = service_of(&[PROCEDURE, VERLOF, HEROVERWEGING, NEST]);
    // The reduction fills `al_verleend` at every depth.
    let (mut cell, _dir) = cell();
    let applied = cell
        .record_submission(
            &mut service,
            &[],
            "wet_nest_fictief",
            "1",
            None,
            &object_map(json!({"bsn": BSN, "dagen": 2})),
            day("2026-03-02"),
        )
        .unwrap();
    let nested = cell
        .decide(
            &mut service,
            &[],
            "wet_nest_fictief",
            "1",
            &applied.id,
            &Map::new(),
            &Map::new(),
            day("2026-03-03"),
        )
        .unwrap();
    assert_eq!(nested.len(), 2);
    assert_eq!(nested[1].basis, "wet_verlof_fictief#1");
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &nested[0].id,
        json!({"nieuwe_gegevens": {"dagen": 20}}),
        "2026-03-10",
    )
    .unwrap();
    let before = cell.grams().len();
    let e = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("takes a decision in turn ('binnen')")
            && m.contains("records one level")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), before);
}

/// An output of type `decision` that holds no decision value is a defect
/// in the law, not a decision the cell records.
#[test]
fn an_output_of_type_decision_that_is_no_decision_value_is_refused() {
    let broken = HEROVERWEGING.replace("then: $heroverweging", "then: $tijdig");
    assert_ne!(broken, HEROVERWEGING);
    let mut service = service_with(&broken);
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({"nieuwe_gegevens": {"dagen": 20}}),
        "2026-03-10",
    )
    .unwrap();
    let before = cell.grams().len();
    let e = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("'nieuw_besluit'") && m.contains("no decision value")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), before);
}

/// A decision the deciding article asks besides the one the submission
/// refers to is no value the authority supplies: a decision is one in the
/// chronicle.
#[test]
fn the_deciding_authority_supplies_no_decision() {
    let other = HEROVERWEGING.replace(
        "          - {name: tijdig, type: boolean}\n",
        "          - {name: tijdig, type: boolean}\n          - {name: ander_besluit, type: decision, required: false}\n",
    );
    assert_ne!(other, HEROVERWEGING);
    let mut service = service_with(&other);
    let (mut cell, _dir) = cell();
    let Granted { granted, refused } = grant_then_refuse(&mut cell, &mut service);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let e = cell
        .decide(
            &mut service,
            &[],
            BEZWAAR_LAW,
            "1",
            &bezwaar.id,
            &object_map(json!({"ander_besluit": refused.id})),
            &Map::new(),
            day("2026-03-11"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("'ander_besluit'") && m.contains("is a decision")),
        "{e}"
    );
}

/// A decision is read in the law it was taken under: when that law has no
/// version in force on the day it was recorded, the cell cannot say what
/// the decision holds.
#[test]
fn a_decision_whose_law_has_no_version_on_its_day_is_a_setup_error() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let Granted { granted, .. } = grant_then_refuse(&mut cell, &mut service);
    let later = VERLOF.replace("valid_from: '2026-01-01'", "valid_from: '2026-03-05'");
    let mut other = service_of(&[PROCEDURE, &later, HEROVERWEGING]);
    let e = object_to(&mut cell, &mut other, &granted.id, json!({}), "2026-03-10").unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("no version in force on 2026-03-03")),
        "{e}"
    );
}

/// In a chronicle kept in a file, a refused decision writes nothing: the
/// file holds what it held, and opened again the chronicle is the same.
#[test]
fn a_refused_decision_writes_nothing_to_the_file() {
    let stale = HEROVERWEGING.replace("then: $heroverweging", "then: $bestreden_besluit");
    let stricter = VERLOF
        .replace("valid_from: '2026-01-01'", "valid_from: '2026-03-05'")
        .replace("value: 10", "value: 1");
    let mut service = service_with(&stale);
    service.load_law(&stricter).unwrap();
    let (_, dir) = cell();
    let path = dir.path().join("kroniek.jsonl");
    let mut cell = Cell::open(ACTOR, &path, dir.path()).unwrap();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written.lines().count(), 3);
    let e = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("taken again on these fields")),
        "{e}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), written);
    assert_eq!(
        Cell::open(ACTOR, &path, dir.path()).unwrap().grams(),
        cell.grams()
    );
}

/// Without a field of type `decision`, a submission that refers to a
/// decision still takes from it what it holds by name, and keeps the
/// reference.
#[test]
fn a_reference_without_a_decision_field_fills_what_the_decision_holds() {
    let mut service = service();
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    let again = cell
        .record_submission(
            &mut service,
            &[],
            VERLOF_LAW,
            "1",
            Some(Reference::Local(&granted.id)),
            &Map::new(),
            day("2026-03-10"),
        )
        .unwrap();
    assert_eq!(again.refers_to.as_deref(), Some(granted.id.as_str()));
    assert_eq!(again.fields["bsn"], json!(BSN));
    assert_eq!(again.fields["dagen"], json!(2));
    assert!(again.missing.is_empty());
    // What it takes from the decision, the submitter does not supply.
    let e = cell
        .record_submission(
            &mut service,
            &[],
            VERLOF_LAW,
            "1",
            Some(Reference::Local(&granted.id)),
            &object_map(json!({"dagen": 3})),
            day("2026-03-10"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("comes from the decision it refers to")),
        "{e}"
    );
}

/// A reduction may fill a term the law leaves to nobody in particular (no
/// `decided_per_case_by`): then the term is the policy's, as for any
/// regulation, and no finding. The replay derives what the decision asks
/// with the terms the cell answers, as the decision did, so it does not pass the
/// recorded value as a finding, and an objection against the decision is
/// decided as any.
#[test]
fn a_term_only_a_reduction_fills_is_no_finding_and_replays() {
    let policy_only = VERLOF.replace("          decided_per_case_by: het hoofd\n", "");
    assert_ne!(policy_only, VERLOF);
    // A required term without a default or a finding: the binding takes an
    // answer that is never absent, so without the cell the decision fails.
    let never_absent = BINDING.replace("            nullable: true\n", "").replace(
        "          - output: al_verleend\n            value:\n              operation: IF\n              cases:\n                - when: {operation: EQUALS, subject: $antwoord, value: null}\n                  then: $al_verleend_volgens_hoofd\n              default: $antwoord\n",
        "          - {output: al_verleend, value: $antwoord}\n",
    );
    assert!(!never_absent.contains("nullable"));
    let mut service = common::service_of(&[
        PROCEDURE,
        &policy_only,
        HEROVERWEGING,
        &never_absent,
        REDUCTION,
    ]);
    let (mut cell, _dir) = cell();
    let application = apply(&mut cell, &mut service, 2, "2026-03-02");
    let granted = decide(
        &mut cell,
        &mut service,
        VERLOF_LAW,
        &application.id,
        "2026-03-03",
    )
    .unwrap()
    .remove(0);
    assert_eq!(granted.fields["al_verleend"], json!(false));
    assert!(cell
        .findings(&mut service, &[], &granted.id)
        .unwrap()
        .is_empty());
    let result = cell.explain(&mut service, &[], &granted.id).unwrap();
    assert!(result.open_terms.iter().any(|f| f.term == "al_verleend"
        && matches!(&f.filled_by, regelrecht_engine::FilledBy::Answer { answers, .. }
                if answers[0].law_id == "reductie_al_verleend_fictief")));
    let bezwaar = object_to(
        &mut cell,
        &mut service,
        &granted.id,
        json!({}),
        "2026-03-10",
    )
    .unwrap();
    let decided = decide(
        &mut cell,
        &mut service,
        BEZWAAR_LAW,
        &bezwaar.id,
        "2026-03-11",
    )
    .unwrap();
    assert_eq!(decided[0].outputs["aanleiding"], json!(false));
}
