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

use regelrecht_engine::{EngineError, FilledBy, LawExecutionService, Value};
use std::collections::BTreeMap;

/// A general administrative law with a decision period: the period set by
/// statutory provision, else eight weeks from receipt. It hooks onto every
/// BESCHIKKING at the stage BESLUIT.
fn general_law(delegation_type: Option<&str>) -> String {
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
const SUBSIDY_LAW: &str = r#"
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
const OTHER_LAW: &str = r#"
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
    service.load_law(&general_law(delegation_type)).unwrap();
    service.load_law(SUBSIDY_LAW).unwrap();
    service.load_law(OTHER_LAW).unwrap();
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

fn subsidy_application(subsidiejaar: i64) -> BTreeMap<String, Value> {
    params(&[
        ("subsidiejaar", Value::Int(subsidiejaar)),
        ("datum_ontvangst", date("2026-01-05")),
    ])
}

fn filled_by_subsidy_law(filled_by: &FilledBy) -> bool {
    matches!(filled_by, FilledBy::Regulation { law_id, .. } if law_id == "subsidiewet_voorbeeld")
}

/// The host fills the term, and with its own parameter: the subsidy year
/// is no parameter of the general law, so without the host's scope the
/// implementation could not run.
#[test]
fn the_host_law_fills_the_term_of_the_hook_with_its_own_parameters() {
    let service = service(None);

    let result = service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            subsidy_application(2026),
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
                && filled_by_subsidy_law(&filled.filled_by)),
        "the receipt names the host as the one that filled the term, got {:?}",
        result.open_terms
    );

    // The host's parameter decides the period, not a value passed in by name.
    let earlier = service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            subsidy_application(2025),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        earlier.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-07-06")),
        "twenty-six weeks for a subsidy year before 2026"
    );
}

/// A decision of another law gets the fallback of eight weeks. The
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
            .any(|filled| filled_by_subsidy_law(&filled.filled_by)),
        "the subsidy law does not fill the term of a decision it did not give, got {:?}",
        result.open_terms
    );
    assert!(result.delegation_refusals.is_empty());
    assert!(result.declarations_not_in_force.is_empty());
}

/// Outside a hook the article keeps the behaviour it had: called
/// directly, every regulation that implements the term is a candidate, and
/// the implementation reads the parameters of the article it fills.
#[test]
fn the_article_outside_a_hook_keeps_the_current_behaviour() {
    let service = service(None);

    let result = service
        .evaluate_law_output(
            "algemene_wet_voorbeeld",
            "uiterste_beslisdag",
            subsidy_application(2026),
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
            subsidy_application(2026),
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
const FOREIGN_REGULATION: &str = r#"
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
    service.load_law(&general_law(None)).unwrap();
    service.load_law(OTHER_LAW).unwrap();
    service.load_law(FOREIGN_REGULATION).unwrap();
    let received = params(&[("datum_ontvangst", date("2026-01-05"))]);

    let decision = service
        .evaluate_law_output(
            "andere_wet_voorbeeld",
            "vergunning_verleend",
            received.clone(),
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
            received,
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
    service.load_law(&general_law(None)).unwrap();
    service.load_law(FOREIGN_REGULATION).unwrap();
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
    service.load_law(&general_law(None)).unwrap();
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

/// The general law with a decision period the authority judges per case
/// where no regulation sets one (`decided_per_case_by`).
fn general_law_judged_per_case() -> String {
    general_law(None).replace(
        "          required: false\n",
        "          required: false\n          decided_per_case_by: bestuursorgaan\n",
    )
}

/// A finding for a term that only a foreign regulation fills applies under a
/// hook: for this decision the term is unfilled, so the authority's judgement
/// for the case decides it.
#[test]
fn a_finding_applies_under_a_hook_where_only_a_foreign_regulation_fills_the_term() {
    let mut service = LawExecutionService::new();
    service.load_law(&general_law_judged_per_case()).unwrap();
    service.load_law(OTHER_LAW).unwrap();
    service.load_law(FOREIGN_REGULATION).unwrap();

    let result = service
        .evaluate_law_with_findings(
            "andere_wet_voorbeeld",
            &["vergunning_verleend"],
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            params(&[("wettelijke_beslistermijn", date("2026-05-01"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-05-01")),
        "the finding decides the term the foreign regulation does not fill here"
    );
}

/// A foreign policy that fills the term and leaves a case to the authority
/// with a term of its own (RFC-048 §2).
const FOREIGN_POLICY: &str = r#"
$id: vreemd_beleid_voorbeeld
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Er wordt binnen dertig weken beslist, bij bijzondere omstandigheden binnen twee.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
      open_terms:
        - id: bijzondere_omstandigheden
          type: boolean
          required: true
          decided_per_case_by: bestuursorgaan
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
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $bijzondere_omstandigheden
                    value: true
                  then:
                    operation: DATE_ADD
                    date: $datum_ontvangst
                    weeks: 2
              default:
                operation: DATE_ADD
                date: $datum_ontvangst
                weeks: 30
"#;

/// Under a hook the foreign policy does not fill the term, so no article
/// asks for a finding on its own term and the engine refuses it. Called
/// directly, the policy fills the term with the finding.
#[test]
fn a_finding_on_a_foreign_policys_own_term_is_refused_under_a_hook() {
    let mut service = LawExecutionService::new();
    service.load_law(&general_law(None)).unwrap();
    service.load_law(OTHER_LAW).unwrap();
    service.load_law(FOREIGN_POLICY).unwrap();
    let received = params(&[("datum_ontvangst", date("2026-01-05"))]);
    let finding = params(&[("bijzondere_omstandigheden", Value::Bool(true))]);

    let direct = service
        .evaluate_law_with_findings(
            "algemene_wet_voorbeeld",
            &["uiterste_beslisdag"],
            received.clone(),
            finding.clone(),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        direct.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-01-19")),
        "called directly, the policy fills the term with the finding"
    );

    let under_hook = service.evaluate_law_with_findings(
        "andere_wet_voorbeeld",
        &["vergunning_verleend"],
        received,
        finding,
        "2026-01-05",
    );
    match under_hook {
        Err(EngineError::FindingsNotAsked { findings }) => {
            assert_eq!(findings, vec!["bijzondere_omstandigheden".to_string()]);
        }
        other => panic!("expected FindingsNotAsked, got {other:?}"),
    }
}

/// A general law whose hook reads the number of weeks from another article
/// of its own, which leaves that number open.
const GENERAL_LAW_WITH_WEEKS: &str = r#"
$id: algemene_wet_weken_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4:12'
    text: De redelijke termijn bedraagt acht weken, tenzij bij regeling anders bepaald.
    machine_readable:
      open_terms:
        - id: redelijke_termijn_weken
          type: number
          required: false
          default:
            actions:
              - output: redelijke_termijn_weken
                value: 8
      execution:
        output:
          - name: redelijke_termijn_weken
            type: number
        actions:
          - output: redelijke_termijn_weken
            value: $redelijke_termijn_weken
  - number: '4:13'
    text: De beschikking wordt gegeven binnen de redelijke termijn na ontvangst.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        input:
          - name: redelijke_termijn_weken
            type: number
            source:
              regulation: algemene_wet_weken_voorbeeld
              output: redelijke_termijn_weken
        output:
          - name: uiterste_beslisdag
            type: date
        actions:
          - output: uiterste_beslisdag
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: $redelijke_termijn_weken
"#;

/// Another article of the hook law, reached while the hook runs, is not the
/// hook: its open term is filled as anywhere else, by any regulation.
#[test]
fn another_article_of_the_hook_law_is_filled_as_outside_a_hook() {
    let weeks = r#"
$id: regeling_weken_voorbeeld
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De redelijke termijn bedraagt tien weken.
    machine_readable:
      implements:
        - law: algemene_wet_weken_voorbeeld
          article: '4:12'
          open_term: redelijke_termijn_weken
      execution:
        output:
          - name: redelijke_termijn_weken
            type: number
        actions:
          - output: redelijke_termijn_weken
            value: 10
"#;
    let mut service = LawExecutionService::new();
    service.load_law(GENERAL_LAW_WITH_WEEKS).unwrap();
    service.load_law(OTHER_LAW).unwrap();
    service.load_law(weeks).unwrap();

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
        Some(&date("2026-03-16")),
        "ten weeks: the regulation fills article 4:12, which is not a hook"
    );
}

/// The host's filling reads a value the host gets from an input that none
/// of its own actions reads. The engine resolves inputs only when something
/// reads them (RFC-043); the filling is such a reader, so the input is
/// resolved before the hook fires.
#[test]
fn the_host_scope_holds_an_input_only_the_filling_reads() {
    let register = r#"
$id: subsidieregister_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het register vermeldt het subsidiejaar.
    machine_readable:
      execution:
        output:
          - name: subsidiejaar
            type: number
        actions:
          - output: subsidiejaar
            value: 2025
"#;
    let host = SUBSIDY_LAW
        .replacen("subsidiewet_voorbeeld", "subsidiewet_register_voorbeeld", 1)
        .replacen(
            "          - name: subsidiejaar\n            type: number\n            required: true\n          - name: datum_ontvangst\n            type: date\n            required: true\n        output:\n          - name: subsidie_verleend",
            "          - name: datum_ontvangst\n            type: date\n            required: true\n        input:\n          - name: subsidiejaar\n            type: number\n            source:\n              regulation: subsidieregister_voorbeeld\n              output: subsidiejaar\n        output:\n          - name: subsidie_verleend",
            1,
        );
    assert!(host.contains("regulation: subsidieregister_voorbeeld"));
    let mut service = LawExecutionService::new();
    service.load_law(&general_law(None)).unwrap();
    service.load_law(register).unwrap();
    service.load_law(&host).unwrap();

    let result = service
        .evaluate_law_output(
            "subsidiewet_register_voorbeeld",
            "subsidie_verleend",
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-07-06")),
        "twenty-six weeks for the subsidy year 2025 from the register"
    );
}

/// The host's filling reads an output of the host article that the caller
/// did not ask for. The article computes it anyway, because the filling of a
/// post hook reads it.
#[test]
fn the_host_scope_holds_an_output_only_the_filling_reads() {
    let host = r#"
$id: subsidiewet_categorie_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Onze Minister verleent op aanvraag een subsidie en deelt de aanvraag in een categorie in.
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
          - name: aantal_weken_voor_categorie
            type: number
        actions:
          - output: subsidie_verleend
            value: true
          - output: aantal_weken_voor_categorie
            value: 5
  - number: '2'
    text: Onze Minister beslist binnen het aantal weken dat bij de categorie hoort.
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
          - name: aantal_weken_voor_categorie
            type: number
            required: true
        output:
          - name: wettelijke_beslistermijn
            type: date
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: $aantal_weken_voor_categorie
"#;
    let mut service = LawExecutionService::new();
    service.load_law(&general_law(None)).unwrap();
    service.load_law(host).unwrap();

    let result = service
        .evaluate_law_output(
            "subsidiewet_categorie_voorbeeld",
            "subsidie_verleend",
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-02-09")),
        "five weeks, from an output the caller did not request"
    );
}

/// A general law with two hooks on every decision: the first leaves the
/// number of weeks open, the second reads that number from the first.
const GENERAL_LAW_TWO_HOOKS: &str = r#"
$id: algemene_wet_twee_haken_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De beslistermijn is de bij wettelijk voorschrift bepaalde termijn, en anders acht weken.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      open_terms:
        - id: wettelijke_beslistermijn_weken
          type: number
          required: false
          default:
            actions:
              - output: wettelijke_beslistermijn_weken
                value: 8
      execution:
        output:
          - name: beslistermijn_weken
            type: number
        actions:
          - output: beslistermijn_weken
            value: $wettelijke_beslistermijn_weken
  - number: '2'
    text: De beslistermijn vangt aan op de dag na ontvangst van de aanvraag.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        input:
          - name: beslistermijn_weken
            type: number
            source:
              regulation: algemene_wet_twee_haken_voorbeeld
              output: beslistermijn_weken
        output:
          - name: uiterste_beslisdag
            type: date
        actions:
          - output: uiterste_beslisdag
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: $beslistermijn_weken
"#;

/// A hook that reads the output of another hook on the same decision reads
/// it as that decision's: the foreign filling of the first hook's term does
/// not reach the second hook either.
#[test]
fn a_hook_reading_another_hook_on_the_same_decision_gets_the_hosts_filling() {
    let weeks = r#"
$id: vreemde_weken_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De beslistermijn bedraagt een week.
    machine_readable:
      implements:
        - law: algemene_wet_twee_haken_voorbeeld
          article: '1'
          open_term: wettelijke_beslistermijn_weken
      execution:
        output:
          - name: wettelijke_beslistermijn_weken
            type: number
        actions:
          - output: wettelijke_beslistermijn_weken
            value: 1
"#;
    let mut service = LawExecutionService::new();
    service.load_law(GENERAL_LAW_TWO_HOOKS).unwrap();
    service.load_law(OTHER_LAW).unwrap();
    service.load_law(weeks).unwrap();

    let result = service
        .evaluate_law_output(
            "andere_wet_voorbeeld",
            "vergunning_verleend",
            params(&[("datum_ontvangst", date("2026-01-05"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        result.outputs.get("beslistermijn_weken"),
        Some(&Value::Int(8))
    );
    assert_eq!(
        result.outputs.get("uiterste_beslisdag"),
        Some(&date("2026-03-02")),
        "eight weeks in the second hook too, not the foreign one week"
    );
}
