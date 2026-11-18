//! One binding that takes the answers of two cells (RFC-048 §4), under a
//! small fictional law. Art. 2 of the law sets a neighbourhood subsidy at
//! € 10 per member, with at most € 1500 per association in all. It leaves the
//! number of members to the association register and what the association
//! received before to the subsidy office, each with its own
//! `competent_authority`. One article of the office's policy fills both
//! terms. The engine routes each input of that article by the terms it
//! feeds: the member count to the register's cell, the earlier grants to the
//! office's own cell. The decision combines the two.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{day, object_map};

use regelrecht_cel::{Cell, Gram};
use regelrecht_engine::{FilledBy, LawExecutionService};
use serde_json::{Map, json};

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

/// Art. 1 decides on an application; art. 2 sets the amount from two terms,
/// each for another authority to say.
const LAW: &str = r#"
$id: wet_buurtsubsidie_fictief
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het subsidiebureau verstrekt op aanvraag van een vereniging een buurtsubsidie.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_buurtsubsidie_fictief#1
            parameters: [vereniging]
          - type: BESCHIKKING
            outputs: [subsidie]
        parameters:
          - {name: vereniging, type: string}
        input:
          - name: bedrag
            type: number
            source:
              regulation: wet_buurtsubsidie_fictief
              output: subsidiebedrag
              parameters:
                vereniging: $vereniging
        output:
          - {name: subsidie, type: number}
        actions:
          - output: subsidie
            value: $bedrag
  - number: '2'
    text: >-
      De subsidie bedraagt € 10 per lid, en een vereniging ontvangt in totaal
      ten hoogste € 1500. Het aantal leden zegt het verenigingsregister; wat de
      vereniging eerder ontving, zegt het subsidiebureau.
    machine_readable:
      open_terms:
        - id: aantal_leden
          type: number
          required: true
          competent_authority:
            name: verenigingsregister
            type: CATEGORY
        - id: eerder_ontvangen
          type: number
          required: true
          competent_authority:
            name: subsidiebureau
            type: CATEGORY
      execution:
        parameters:
          - {name: vereniging, type: string}
        output:
          - {name: subsidiebedrag, type: number}
        actions:
          - output: subsidiebedrag
            value:
              operation: MIN
              values:
                - {operation: MULTIPLY, values: [$aantal_leden, 10]}
                - {operation: SUBTRACT, values: [1500, $eerder_ontvangen]}
"#;

/// The office's policy: one article fills both terms of art. 2, each from
/// the answer of the cell the term names. Where a cell says nothing, the
/// policy counts zero.
const POLICY: &str = r#"
$id: beleid_buurtsubsidie_wilgendaal_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid-buurtsubsidie
competent_authority:
  name: Subsidiebureau Wilgendaal
articles:
  - number: '1'
    text: >-
      Het subsidiebureau neemt het aantal leden over uit het
      verenigingsregister en telt wat het de vereniging eerder verstrekte.
      Zegt een van beide niets, dan telt het als nul.
    machine_readable:
      implements:
        - {law: wet_buurtsubsidie_fictief, article: '2', open_term: aantal_leden}
        - {law: wet_buurtsubsidie_fictief, article: '2', open_term: eerder_ontvangen}
      execution:
        parameters:
          - {name: vereniging, type: string}
        input:
          - name: leden_volgens_register
            type: number
            nullable: true
            source:
              reduction: aantal_leden
              parameters: {vereniging: $vereniging}
          - name: eerder_verstrekt
            type: number
            nullable: true
            source:
              reduction: eerder_ontvangen
              parameters: {vereniging: $vereniging}
        output:
          - {name: aantal_leden, type: number}
          - {name: eerder_ontvangen, type: number}
        actions:
          - output: aantal_leden
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $leden_volgens_register, value: null}
                  then: 0
              default: $leden_volgens_register
          - output: eerder_ontvangen
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $eerder_verstrekt, value: null}
                  then: 0
              default: $eerder_verstrekt
"#;

/// The register's own regelwerk: it records the members of an association.
const REGISTER: &str = r#"
$id: regeling_verenigingsregister_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/verenigingsregister
competent_authority:
  name: Verenigingsregister Oostmeer
articles:
  - number: '1'
    text: Het verenigingsregister schrijft een vereniging in met haar aantal leden.
    machine_readable:
      requires:
        - fact: inschrijving
          parameters: [vereniging, leden]
      execution:
        parameters:
          - {name: vereniging, type: string}
          - {name: leden, type: number}
"#;

/// The register's reduction: the members of the association as its
/// registration records them, silent for one it does not know. It reads the
/// recorded facts directly (`source.facts`), which schema v0.8.19 no longer
/// allows, so it declares v0.8.17.
const MEMBERS: &str = r#"
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.17/schema/v0.8.17/schema.json
$id: reductie_aantal_leden
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie-aantal-leden
articles:
  - number: '1'
    text: Het aantal leden uit de inschrijving van de vereniging.
    machine_readable:
      execution:
        parameters:
          - {name: vereniging, type: string}
        input:
          - name: inschrijvingen
            type: array
            source:
              facts: regeling_verenigingsregister_fictief#1#inschrijving
        output:
          - {name: aantal_leden, type: number, nullable: true}
        actions:
          - output: aantal_leden
            value:
              operation: FOREACH
              collection: $inschrijvingen
              as: inschrijving
              filter: {operation: EQUALS, subject: $inschrijving.vereniging, value: $vereniging}
              body: $inschrijving.leden
              combine: ONLY
"#;

/// The office's reduction: what it granted the association in its earlier
/// decisions. It reads its decisions directly (`source.decisions`), which
/// schema v0.8.17 no longer allows, so it declares v0.8.16.
const GRANTED: &str = r#"
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.16/schema/v0.8.16/schema.json
$id: reductie_eerder_ontvangen
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie-eerder-ontvangen
articles:
  - number: '1'
    text: Wat het subsidiebureau de vereniging in eerdere besluiten verstrekte.
    machine_readable:
      execution:
        parameters:
          - {name: vereniging, type: string}
        input:
          - name: besluiten
            type: array
            source:
              decisions: wet_buurtsubsidie_fictief#1
        output:
          - {name: eerder_ontvangen, type: number}
        actions:
          - output: eerder_ontvangen
            value:
              operation: FOREACH
              collection: $besluiten
              as: besluit
              filter: {operation: EQUALS, subject: $besluit.vereniging, value: $vereniging}
              body: $besluit.subsidie
              combine: ADD
"#;

/// The register's catalogue (`cel.yaml`).
const REGISTER_CELL: &str = r#"
name: Verenigingsregister Oostmeer
category: verenigingsregister
answers:
  aantal_leden: reductie_aantal_leden
"#;

/// The office's catalogue (`cel.yaml`).
const OFFICE_CELL: &str = r#"
name: Subsidiebureau Wilgendaal
category: subsidiebureau
answers:
  eerder_ontvangen: reductie_eerder_ontvangen
"#;

const LAW_ID: &str = "wet_buurtsubsidie_fictief";
const ASSOCIATION: &str = "Buurtvereniging De Linde";

fn service() -> LawExecutionService {
    common::service_of(&[GENERAL_LAW, LAW, POLICY, REGISTER, MEMBERS, GRANTED])
}

/// The cell of `actor` with the catalogue `identity`, keeping `reduction`.
fn cell(actor: &str, identity: &str, reduction: (&str, &str)) -> Cell {
    let dir = tempfile::tempdir().unwrap();
    common::keep_reduction(dir.path(), reduction.0, reduction.1);
    common::write_identity(dir.path(), identity.trim_start());
    Cell::in_memory(actor, dir.path()).unwrap()
}

fn register() -> Cell {
    cell(
        "verenigingsregister",
        REGISTER_CELL,
        ("reductie_aantal_leden", MEMBERS),
    )
}

fn office() -> Cell {
    cell(
        "subsidiebureau",
        OFFICE_CELL,
        ("reductie_eerder_ontvangen", GRANTED),
    )
}

/// The register records the association with `leden` members on `on`.
fn register_members(service: &mut LawExecutionService, register: &mut Cell, leden: i64, on: &str) {
    register
        .record_executogram(
            service,
            "regeling_verenigingsregister_fictief#1#inschrijving",
            &object_map(json!({"vereniging": ASSOCIATION, "leden": leden})),
            day(on),
        )
        .unwrap();
}

/// The association applies at the office on `on`, and the office decides,
/// with the cells of `sphere` along: the decision.
fn apply(
    service: &mut LawExecutionService,
    office: &mut Cell,
    sphere: &[&Cell],
    on: &str,
) -> regelrecht_cel::Result<Gram> {
    let applied = office.record_submission(
        service,
        sphere,
        LAW_ID,
        "1",
        None,
        &object_map(json!({"vereniging": ASSOCIATION})),
        day(on),
    )?;
    let mut taken = office.decide(
        service,
        sphere,
        LAW_ID,
        "1",
        &applied.id,
        &Map::new(),
        &Map::new(),
        day(on),
    )?;
    Ok(taken.remove(0))
}

fn subsidie(gram: &Gram) -> serde_json::Value {
    gram.outputs["subsidie"].clone()
}

/// The register knows 120 members; the office granted € 1200 in March.
fn after_a_first_grant() -> (LawExecutionService, Cell, Cell) {
    let mut service = service();
    let mut register = register();
    let mut office = office();
    register_members(&mut service, &mut register, 120, "2026-02-02");
    let first = apply(&mut service, &mut office, &[&register], "2026-03-02").unwrap();
    assert_eq!(subsidie(&first), json!(1200));
    (service, register, office)
}

#[test]
fn one_binding_takes_each_answer_from_the_cell_of_its_own_term() {
    let (mut service, register, mut office) = after_a_first_grant();
    // A second application: 120 members give € 1200, but of the € 1500 only
    // € 300 is left. That needs the register's count and the office's own
    // earlier grant, from one binding article.
    let second = apply(&mut service, &mut office, &[&register], "2026-09-01").unwrap();
    assert_eq!(subsidie(&second), json!(300));

    let explained = office
        .explain(&mut service, &[&register], &second.id)
        .unwrap();
    for (term, answerer, reduction) in [
        (
            "aantal_leden",
            "verenigingsregister",
            "reductie_aantal_leden",
        ),
        (
            "eerder_ontvangen",
            "subsidiebureau",
            "reductie_eerder_ontvangen",
        ),
    ] {
        let filled = explained
            .open_terms
            .iter()
            .find(|f| f.term == term)
            .unwrap_or_else(|| panic!("'{term}' is filled: {:?}", explained.open_terms));
        let FilledBy::Answer {
            law_id, answers, ..
        } = &filled.filled_by
        else {
            panic!("'{term}' is filled by an answer: {:?}", filled.filled_by)
        };
        assert_eq!(law_id, "beleid_buurtsubsidie_wilgendaal_fictief");
        let [provenance] = answers.as_slice() else {
            panic!("'{term}' has one answer: {answers:?}")
        };
        assert_eq!(
            (provenance.answerer.as_str(), provenance.law_id.as_str()),
            (answerer, reduction),
            "{term}"
        );
    }
}

#[test]
fn without_the_register_the_missing_answer_is_reported_with_its_term() {
    let (mut service, _, mut office) = after_a_first_grant();
    // The office decides without the register along. The policy counts zero
    // members, as it says; the report of the term names the answer it
    // missed, so the zero is not mistaken for the register's count.
    let second = apply(&mut service, &mut office, &[], "2026-09-01").unwrap();
    assert_eq!(subsidie(&second), json!(0));
    let explained = office.explain(&mut service, &[], &second.id).unwrap();
    let filled = explained
        .open_terms
        .iter()
        .find(|f| f.term == "aantal_leden")
        .unwrap();
    let FilledBy::Answer {
        answers,
        unanswered,
        ..
    } = &filled.filled_by
    else {
        panic!("filled by an answer: {:?}", filled.filled_by)
    };
    assert!(answers.is_empty(), "{answers:?}");
    let [missing] = unanswered.as_slice() else {
        panic!("one answer is missing: {unanswered:?}")
    };
    assert_eq!(missing.answer, "aantal_leden");
    assert_eq!(missing.input, "leden_volgens_register");
    assert!(
        missing
            .message
            .contains("No cell of this execution gives the answer 'aantal_leden'"),
        "{}",
        missing.message
    );
}
