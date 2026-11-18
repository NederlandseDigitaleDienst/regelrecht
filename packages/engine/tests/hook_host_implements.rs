//! Who fills the open term of an article that fires as a hook (RFC-058).
//!
//! A general law hooks onto every decision of a legal character, and leaves a
//! term open for the special law to fill: the decision period of Awb 4:13 is
//! "the period set by statutory provision", with eight weeks as the fallback.
//! Under a hook only the law the decision comes from (the host) fills such a
//! term, and it fills it with the host's own parameters. These tests pin that
//! on fictional rulework: a host that fills the term, a decision of another
//! law that gets the fallback, and the article outside a hook, which keeps
//! the behaviour it had.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{FilledBy, LawExecutionService, Value};
use std::collections::BTreeMap;

/// A general administrative law with a decision period: the period set by
/// statutory provision, else eight weeks from receipt. It hooks onto every
/// BESCHIKKING at the stage BESLUIT.
fn algemene_wet(delegation_type: Option<&str>) -> String {
    let delegation = delegation_type
        .map(|layer| format!("\n          delegation_type: {layer}"))
        .unwrap_or_default();
    format!(
        r#"
$id: algemene_wet_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4:13'
    text: |-
      1. Een beschikking dient te worden gegeven binnen de bij wettelijk
      voorschrift bepaalde termijn of, bij het ontbreken van zulk een termijn,
      binnen een redelijke termijn na ontvangst van de aanvraag.
      2. De redelijke termijn is in ieder geval verstreken wanneer het
      bestuursorgaan binnen acht weken na ontvangst van de aanvraag geen
      beschikking heeft gegeven.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      open_terms:
        - id: wettelijke_beslistermijn
          type: date
          required: false{delegation}
          default:
            actions:
              - output: wettelijke_beslistermijn
                value:
                  operation: DATE_ADD
                  date: $datum_ontvangst
                  weeks: 8
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: uiterste_beslisdag
            type: date
        actions:
          - output: uiterste_beslisdag
            value: $wettelijke_beslistermijn
"#
    )
}

/// A special law that gives a subsidy by BESCHIKKING and sets its own
/// decision period, which depends on the subsidy year: a parameter of the
/// subsidy decision that the general law does not know.
const SUBSIDIEWET: &str = r#"
$id: subsidiewet_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Onze Minister verleent op aanvraag een subsidie voor het subsidiejaar.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: subsidiejaar
            type: number
            required: true
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: subsidie_verleend
            type: boolean
        actions:
          - output: subsidie_verleend
            value: true
  - number: '2'
    text: |-
      Onze Minister beslist op een aanvraag binnen dertien weken na ontvangst,
      en voor een subsidiejaar vóór 2026 binnen zesentwintig weken.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
      execution:
        parameters:
          - name: subsidiejaar
            type: number
            required: true
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: wettelijke_beslistermijn
            type: date
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: IF
              cases:
                - when:
                    operation: LESS_THAN
                    subject: $subsidiejaar
                    value: 2026
                  then:
                    operation: DATE_ADD
                    date: $datum_ontvangst
                    weeks: 26
              default:
                operation: DATE_ADD
                date: $datum_ontvangst
                weeks: 13
"#;

/// Another law that gives a BESCHIKKING and sets no decision period.
const ANDERE_WET: &str = r#"
$id: andere_wet_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het college verleent op aanvraag een vergunning.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: vergunning_verleend
            type: boolean
        actions:
          - output: vergunning_verleend
            value: true
"#;

fn service(delegation_type: Option<&str>) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&algemene_wet(delegation_type)).unwrap();
    service.load_law(SUBSIDIEWET).unwrap();
    service.load_law(ANDERE_WET).unwrap();
    service
}

fn params(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect()
}

fn date(s: &str) -> Value {
    Value::String(s.to_string())
}

fn subsidie_aanvraag(subsidiejaar: i64) -> BTreeMap<String, Value> {
    params(&[
        ("subsidiejaar", Value::Int(subsidiejaar)),
        ("datum_ontvangst", date("2026-01-05")),
    ])
}

fn filled_by_subsidiewet(filled_by: &FilledBy) -> bool {
    matches!(filled_by, FilledBy::Regulation { law_id, .. } if law_id == "subsidiewet_voorbeeld")
}

/// (a) The host fills the term, and with its own parameter: the subsidy year
/// is no parameter of the general law, so without the host's scope the
/// implementation could not run.
#[test]
fn the_host_law_fills_the_term_of_the_hook_with_its_own_parameters() {
    let service = service(None);

    let result = service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            subsidie_aanvraag(2026),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-04-06")),
        "thirteen weeks from receipt, set by the host"
    );
    assert!(
        result
            .open_terms
            .iter()
            .any(|filled| filled.term == "wettelijke_beslistermijn"
                && filled_by_subsidiewet(&filled.filled_by)),
        "the receipt names the host as the one that filled the term, got {:?}",
        result.open_terms
    );

    // The host's parameter decides the period, not a value passed in by name.
    let earlier = service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            subsidie_aanvraag(2025),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        earlier.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-07-06")),
        "twenty-six weeks for a subsidy year before 2026"
    );
}

/// (b) A decision of another law gets the fallback of eight weeks. The
/// special law that fills the term for its own decisions does not fill it
/// for this one, and leaves no record here either: it was never offered.
#[test]
fn a_decision_of_another_law_gets_the_default_not_the_foreign_filling() {
    let service = service(None);

    let result = service
        .evaluate_law_output(
            "andere_wet_voorbeeld",
            "vergunning_verleend",
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-03-02")),
        "eight weeks from receipt, the default of the general law"
    );
    assert!(
        !result
            .open_terms
            .iter()
            .any(|filled| filled_by_subsidiewet(&filled.filled_by)),
        "the subsidy law does not fill the term of a decision it did not give, got {:?}",
        result.open_terms
    );
    assert!(result.delegation_refusals.is_empty());
    assert!(result.declarations_not_in_force.is_empty());
}

/// (c) Outside a hook the article keeps the behaviour it had: called
/// directly, every regulation that implements the term is a candidate, and
/// the implementation reads the parameters of the article it fills.
#[test]
fn the_article_outside_a_hook_keeps_the_current_behaviour() {
    let service = service(None);

    let result = service
        .evaluate_law_output(
            "algemene_wet_voorbeeld",
            "uiterste_beslisdag",
            subsidie_aanvraag(2026),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-04-06")),
        "called directly, the implementing regulation fills the term as before"
    );
}

/// Which layer may fill the term stays the delegation gate's question: a
/// host whose layer the term does not admit is refused, in the receipt, and
/// the default applies. Under a hook the gate is asked of the host alone.
#[test]
fn the_delegation_gate_still_decides_which_layer_of_the_host_may_fill() {
    let service = service(Some("MINISTERIELE_REGELING"));

    let result = service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            subsidie_aanvraag(2026),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-03-02")),
        "the refused host leaves the default of eight weeks"
    );
    let refused: Vec<&str> = result
        .delegation_refusals
        .iter()
        .map(|r| r.refused_law.as_str())
        .collect();
    assert_eq!(refused, vec!["subsidiewet_voorbeeld"]);
}

/// A ministerial regulation that fills the decision period for every
/// decision, from outside any host, with a parameter every decision has.
const VREEMDE_REGELING: &str = r#"
$id: vreemde_regeling_voorbeeld
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Op een aanvraag wordt beslist binnen dertig weken na ontvangst.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: wettelijke_beslistermijn
            type: date
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: 30
"#;

/// A filling from outside the host does not count under a hook, also when it
/// could run on the hook's own parameters. Called directly, it does.
#[test]
fn a_foreign_filling_does_not_count_under_a_hook_even_when_it_could_run() {
    let mut service = LawExecutionService::new();
    service.load_law(&algemene_wet(None)).unwrap();
    service.load_law(ANDERE_WET).unwrap();
    service.load_law(VREEMDE_REGELING).unwrap();
    let ontvangst = params(&[("datum_ontvangst", date("2026-01-05"))]);

    let decision = service
        .evaluate_law_output(
            "andere_wet_voorbeeld",
            "vergunning_verleend",
            ontvangst.clone(),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        decision.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-03-02")),
        "eight weeks: the foreign regulation does not fill the term under a hook"
    );

    let direct = service
        .evaluate_law_output(
            "algemene_wet_voorbeeld",
            "uiterste_beslisdag",
            ontvangst,
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        direct.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-08-03")),
        "thirty weeks: outside a hook the foreign regulation fills it as before"
    );
}

/// A host that states its period in terms of the general rule reads the
/// general rule without its own filling. That read is still part of the
/// host's decision, so another regulation does not fill the term there.
#[test]
fn a_host_reading_the_general_rule_gets_the_default_not_a_foreign_filling() {
    let host = r#"
$id: subsidiewet_verlenging_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Onze Minister verleent op aanvraag een subsidie.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: subsidie_verleend
            type: boolean
        actions:
          - output: subsidie_verleend
            value: true
  - number: '2'
    text: De beslistermijn is de redelijke termijn, verlengd met twee weken.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        input:
          - name: algemene_beslisdag
            type: date
            source:
              regulation: algemene_wet_voorbeeld
              output: uiterste_beslisdag
              parameters:
                datum_ontvangst: $datum_ontvangst
        output:
          - name: wettelijke_beslistermijn
            type: date
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: DATE_ADD
              date: $algemene_beslisdag
              weeks: 2
"#;
    let mut service = LawExecutionService::new();
    service.load_law(&algemene_wet(None)).unwrap();
    service.load_law(VREEMDE_REGELING).unwrap();
    service.load_law(host).unwrap();

    let result = service
        .evaluate_law_output(
            "subsidiewet_verlenging_voorbeeld",
            "subsidie_verleend",
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-03-16")),
        "eight weeks of the default plus two of the host, not thirty plus two"
    );
}

/// A host with a territorial scope fills the term of the hook without the
/// hook's parameters naming that territory: the host applies to the case
/// it decides, as the declaring law does.
#[test]
fn a_scoped_host_fills_the_term_without_a_matching_scope_parameter() {
    let host = r#"
$id: subsidieverordening_voorbeeld
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
gemeente_code: GM9999
articles:
  - number: '1'
    text: Het college verleent op aanvraag een subsidie.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: subsidie_verleend
            type: boolean
        actions:
          - output: subsidie_verleend
            value: true
  - number: '2'
    text: Het college beslist binnen vier weken na ontvangst.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: wettelijke_beslistermijn
            type: date
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: 4
"#;
    let mut service = LawExecutionService::new();
    service.load_law(&algemene_wet(None)).unwrap();
    service.load_law(host).unwrap();

    let result = service
        .evaluate_law_output(
            "subsidieverordening_voorbeeld",
            "subsidie_verleend",
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-02-02")),
        "four weeks, set by the municipal host"
    );
}
