//! The decision on an application, in one article (RFC-045).
//!
//! An article that establishes an application and is the decision on it
//! follows a procedure. The hooks on the application fire at the stage named
//! after its kind (AANVRAAG), the hooks on the decision at their own stage,
//! so recording the application and deciding on it are two steps. A norm the
//! law leaves to the deciding authority per case comes from the caller.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{
    ExecutionOutcome, HookPoint, LawExecutionService, SubmissionArticle, Value,
};
use std::collections::BTreeMap;

/// A procedure, a hook on every application and one on the decision.
const GENERAL_LAW: &str = r#"
$id: wet_algemeen
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
    text: De aanvraag bevat de naam van de aanvrager.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {submission: AANVRAAG}
      execution:
        parameters:
          - {name: naam_aanvrager, type: string, required: false}
        output:
          - {name: naam_gegeven, type: string}
        actions:
          - {output: naam_gegeven, value: $naam_aanvrager}
  - number: '2'
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
$id: wet_verlof
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
            requests: wet_verlof#1
            parameters: [dagen]
          - type: BESCHIKKING
            outputs: [verlof_verleend]
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: verlof_verleend, type: boolean}
        actions:
          - output: verlof_verleend
            value:
              operation: AND
              conditions:
                - $grond_aanwezig
                - {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}
"#;

/// A regulation that fills the term generally.
const POLICY: &str = r#"
$id: beleid_verlof
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid
articles:
  - number: '1'
    text: De grond is altijd aanwezig.
    machine_readable:
      implements:
        - {law: wet_verlof, article: '1', open_term: grond_aanwezig}
      execution:
        output:
          - {name: grond_aanwezig, type: boolean}
        actions:
          - {output: grond_aanwezig, value: true}
"#;

/// A regulation that leaves the norm to the head per case, and says so
/// itself: with an open term of its own (RFC-048 §2).
const JUDGING_POLICY: &str = r#"
$id: beleid_verlof_oordeel
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid-oordeel
articles:
  - number: '1'
    text: Over de grond oordeelt het hoofd per aanvraag.
    machine_readable:
      open_terms:
        - id: grond_aanwezig_volgens_hoofd
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      implements:
        - {law: wet_verlof, article: '1', open_term: grond_aanwezig}
      execution:
        output:
          - {name: grond_aanwezig, type: boolean}
        actions:
          - {output: grond_aanwezig, value: $grond_aanwezig_volgens_hoofd}
"#;

const DAY: &str = "2026-10-13";

fn service(laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
}

fn params(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn date(s: &str) -> Value {
    Value::String(s.to_string())
}

fn names(result: &regelrecht_engine::ArticleResult) -> Vec<&str> {
    result.outputs.keys().map(String::as_str).collect()
}

#[test]
fn the_application_stage_fires_only_the_hooks_on_the_application() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let result = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "AANVRAAG",
            None,
            params(&[
                ("dagen", Value::Int(5)),
                ("naam_aanvrager", Value::String("J. Jansen".into())),
                ("aanvraag_datum", date(DAY)),
            ]),
            DAY,
        )
        .unwrap();
    // Only the outputs of the application, none here, and no hook on the
    // decision: the judgement is not asked yet.
    assert_eq!(names(&result), ["naam_gegeven"]);
}

/// A decision item without `outputs` takes every output, and the submission
/// without `outputs` none: recording the application still does not ask the
/// head's judgement.
#[test]
fn a_decision_without_outputs_leaves_the_application_stage_without_the_judgement() {
    let special = SPECIAL_LAW.replace("            outputs: [verlof_verleend]\n", "");
    assert_ne!(special, SPECIAL_LAW);
    let service = service(&[GENERAL_LAW, &special]);
    let result = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "AANVRAAG",
            None,
            params(&[
                ("dagen", Value::Int(5)),
                ("naam_aanvrager", Value::String("J. Jansen".into())),
                ("aanvraag_datum", date(DAY)),
            ]),
            DAY,
        )
        .unwrap();
    assert_eq!(names(&result), ["naam_gegeven"]);
}

/// The object form of `produces` names one thing, so every stage computes
/// every output, as before the list form existed (schema v0.8.1).
#[test]
fn an_article_that_produces_one_thing_computes_every_output_at_every_stage() {
    let one_thing = SPECIAL_LAW.replace(
        "          - type: AANVRAAG\n            requests: wet_verlof#1\n            parameters: [dagen]\n          - type: BESCHIKKING\n            outputs: [verlof_verleend]\n",
        "          legal_character: BESCHIKKING\n          submission: {kind: AANVRAAG}\n",
    );
    assert_ne!(one_thing, SPECIAL_LAW, "the fixture names the list form");
    let service = service(&[GENERAL_LAW, &one_thing]);
    let result = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "AANVRAAG",
            None,
            params(&[
                ("dagen", Value::Int(5)),
                ("naam_aanvrager", Value::String("J. Jansen".into())),
                ("aanvraag_datum", date(DAY)),
                ("grond_aanwezig", Value::Bool(true)),
            ]),
            DAY,
        )
        .unwrap();
    assert_eq!(names(&result), ["naam_gegeven", "verlof_verleend"]);
}

#[test]
fn the_decision_stage_fires_only_the_hooks_on_the_decision() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let result = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "BESLUIT",
            None,
            params(&[
                ("dagen", Value::Int(5)),
                ("grond_aanwezig", Value::Bool(true)),
                ("besluit_datum", date(DAY)),
            ]),
            DAY,
        )
        .unwrap();
    assert_eq!(names(&result), ["motivering_vereist", "verlof_verleend"]);
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
}

#[test]
fn outside_a_procedure_stage_every_hook_fires_as_before() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let result = service
        .evaluate_law(
            "wet_verlof",
            &["verlof_verleend"],
            params(&[
                ("dagen", Value::Int(11)),
                ("grond_aanwezig", Value::Bool(true)),
                ("naam_aanvrager", Value::String("J. Jansen".into())),
            ]),
            DAY,
        )
        .unwrap();
    assert_eq!(
        names(&result),
        ["motivering_vereist", "naam_gegeven", "verlof_verleend"]
    );
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
}

#[test]
fn the_engine_names_the_stage_of_the_application_and_what_a_stage_involves() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let submission = service.submission("wet_verlof", "1", DAY).unwrap().unwrap();
    assert_eq!(submission.stage.as_deref(), Some("AANVRAAG"));

    let part = |law: &str, article: &str, hook_point| SubmissionArticle {
        law_id: law.into(),
        article_number: article.into(),
        hook_point,
    };
    let stage = service
        .procedure_stage("wet_verlof", "1", "BESLUIT", DAY)
        .unwrap();
    assert_eq!(stage.procedure_id, "beschikking");
    assert_eq!(
        stage
            .requires
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        ["besluit_datum"]
    );
    assert_eq!(
        stage.articles,
        [
            part("wet_verlof", "1", None),
            part("wet_algemeen", "2", Some(HookPoint::PostActions)),
        ]
    );
    let stage = service
        .procedure_stage("wet_verlof", "1", "AANVRAAG", DAY)
        .unwrap();
    assert_eq!(
        stage.articles,
        [
            part("wet_verlof", "1", None),
            part("wet_algemeen", "1", Some(HookPoint::PreActions)),
        ]
    );
}

#[test]
fn a_stage_is_refused_without_what_it_requires_or_when_it_does_not_exist() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let e = service
        .execute_stage_at("wet_verlof", "1", "BESLUIT", None, params(&[]), DAY)
        .unwrap_err();
    assert!(e.to_string().contains("requires besluit_datum"), "{e}");
    let e = service
        .execute_stage_at("wet_verlof", "1", "ZITTING", None, params(&[]), DAY)
        .unwrap_err();
    assert!(e.to_string().contains("'ZITTING' not found"), "{e}");
    let e = service
        .procedure_stage("wet_algemeen", "2", "BESLUIT", DAY)
        .unwrap_err();
    assert!(e.to_string().contains("follows no procedure"), "{e}");
}

#[test]
fn a_procedure_without_the_stage_of_the_application_is_refused() {
    let general = GENERAL_LAW.replace(
        "      - name: AANVRAAG\n        requires: [{name: aanvraag_datum, type: date}]\n",
        "",
    );
    let service = service(&[&general, SPECIAL_LAW]);
    let e = service.submission("wet_verlof", "1", DAY).unwrap_err();
    assert!(e.to_string().contains("has no stage AANVRAAG"), "{e}");
}

#[test]
fn a_norm_decided_per_case_without_a_judgement_is_an_error_that_says_so() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let e = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "BESLUIT",
            None,
            params(&[("dagen", Value::Int(5)), ("besluit_datum", date(DAY))]),
            DAY,
        )
        .unwrap_err();
    assert!(
        e.to_string().contains("decided per case by het hoofd"),
        "{e}"
    );
}

#[test]
fn a_regulation_that_fills_the_norm_takes_no_judgement() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW, POLICY]);
    let result = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[("dagen", Value::Int(5)), ("besluit_datum", date(DAY))]),
            DAY,
        )
        .unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
    let traced = service
        .evaluate_law_output_with_trace(
            "wet_verlof",
            "verlof_verleend",
            params(&[("dagen", Value::Int(5))]),
            DAY,
        )
        .unwrap();
    let trace = traced.trace.as_ref().unwrap().render_box_drawing();
    assert!(
        trace.contains("implemented by beleid_verlof article 1"),
        "{trace}"
    );

    // A judgement of the head on the norm the regulation fills is asked by
    // none: the regulation's value is the norm's (RFC-048 §2).
    let e = service
        .execute_stage_at(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[
                ("dagen", Value::Int(5)),
                ("grond_aanwezig", Value::Bool(false)),
                ("besluit_datum", date(DAY)),
            ]),
            DAY,
        )
        .unwrap_err();
    assert!(e.to_string().contains("is asked by none"), "{e}");
}

#[test]
fn a_regulation_that_leaves_the_norm_to_the_head_says_so_itself() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW, JUDGING_POLICY]);
    let run = |judgement: Option<bool>| {
        let findings = judgement
            .map(|j| params(&[("grond_aanwezig_volgens_hoofd", Value::Bool(j))]))
            .unwrap_or_default();
        service.execute_stage_in(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[("dagen", Value::Int(5)), ("besluit_datum", date(DAY))]),
            regelrecht_engine::Call::with_findings(findings),
            DAY,
        )
    };
    assert_eq!(
        run(Some(false)).unwrap().outputs["verlof_verleend"],
        Value::Bool(false)
    );
    assert_eq!(
        run(Some(true)).unwrap().outputs["verlof_verleend"],
        Value::Bool(true)
    );
    let e = run(None).unwrap_err();
    assert!(
        e.to_string().contains("decided per case by het hoofd"),
        "{e}"
    );
}

#[test]
fn walking_the_procedure_asks_the_judgement_only_at_the_decision() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    // At AANVRAAG the application runs with the hooks on it: no judgement,
    // and no output of the decision.
    let outcome = service
        .execute_stage(
            "wet_verlof",
            "verlof_verleend",
            None,
            params(&[
                ("dagen", Value::Int(5)),
                ("naam_aanvrager", Value::String("J. Jansen".into())),
                ("aanvraag_datum", date(DAY)),
            ]),
            DAY,
        )
        .unwrap();
    let ExecutionOutcome::Yielded {
        state,
        outputs,
        pending_inputs,
    } = outcome
    else {
        panic!("the walker should wait for the decision date");
    };
    assert_eq!(state.current_stage, "BESLUIT");
    assert_eq!(pending_inputs, ["besluit_datum"]);
    assert_eq!(
        outputs.keys().map(String::as_str).collect::<Vec<_>>(),
        ["naam_gegeven"]
    );

    // At BESLUIT the head's judgement decides, and BEKENDMAKING follows.
    let outcome = service
        .execute_stage(
            "wet_verlof",
            "verlof_verleend",
            Some(state),
            params(&[
                ("grond_aanwezig", Value::Bool(true)),
                ("besluit_datum", date(DAY)),
            ]),
            DAY,
        )
        .unwrap();
    let ExecutionOutcome::Complete(result) = outcome else {
        panic!("the walker should complete after BEKENDMAKING");
    };
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
    assert_eq!(result.outputs["motivering_vereist"], Value::Bool(true));
    assert_eq!(
        result.outputs["naam_gegeven"],
        Value::String("J. Jansen".into())
    );
}

/// A stage takes the head's finding apart from the parameters (RFC-045 §5),
/// and refuses one no article of the stage asks.
#[test]
fn a_stage_takes_a_finding_apart_from_the_parameters() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW]);
    let run = |ids: &[&str]| {
        let findings: Vec<(&str, Value)> = ids.iter().map(|id| (*id, Value::Bool(true))).collect();
        service.execute_stage_at_with_findings(
            "wet_verlof",
            "1",
            "BESLUIT",
            None,
            params(&[("dagen", Value::Int(5)), ("besluit_datum", date(DAY))]),
            params(&findings),
            DAY,
        )
    };
    let result = run(&["grond_aanwezig"]).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
    let e = run(&["grond_aanwezig", "grond_aanwezg"]).unwrap_err();
    assert!(e.to_string().contains("'grond_aanwezg'"), "{e}");
}

/// A regulation that fills the norm for a short leave and leaves a longer
/// one to an authority it names by its own term (`competent_authority`),
/// held by the head of a school it names (root `competent_authority`).
const SHORT_LEAVE_POLICY: &str = r#"
$id: beleid_verlof_kort
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid-kort
competent_authority:
  name: Het hoofd van school Voorbeeld
articles:
  - number: '1'
    text: Bij ten hoogste drie dagen is de grond aanwezig.
    machine_readable:
      open_terms:
        - id: grond_aanwezig_volgens_hoofd
          type: boolean
          decided_per_case_by: het hoofd
        - id: grond_aanwezig_volgens_inspectie
          type: boolean
          required: false
          decided_per_case_by: de inspectie
          competent_authority:
            name: inspectie
            type: CATEGORY
      implements:
        - {law: wet_verlof, article: '1', open_term: grond_aanwezig}
      execution:
        parameters:
          - {name: dagen, type: number}
        output:
          - {name: grond_aanwezig, type: boolean}
        actions:
          - output: grond_aanwezig
            value:
              operation: IF
              cases:
                - when: {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 3}
                  then: true
              default: $grond_aanwezig_volgens_hoofd
"#;

/// Who answers a term (RFC-002): the authority the term names, else the
/// one its article names, else the holder of its rulework; none of them
/// named, whoever executes the article. `decided_per_case_by` only marks
/// the term as judged per case: its words name no one to the engine.
#[test]
fn a_term_is_answered_by_its_own_authority_else_the_executing_one() {
    use regelrecht_engine::Authority;
    let service = service(&[GENERAL_LAW, SPECIAL_LAW, SHORT_LEAVE_POLICY]);
    let terms = service
        .decision_terms("wet_verlof", "1", DAY, &Default::default())
        .unwrap();
    let term = |id: &str| terms.iter().find(|t| t.id == id).unwrap();
    let head = Authority {
        name: "Het hoofd van school Voorbeeld".into(),
        category: Some("het hoofd".into()),
    };
    let inspection = Authority {
        name: "Inspectie Voorbeeld".into(),
        category: Some("inspectie".into()),
    };
    // The law names no one: whoever executes it answers.
    assert!(term("grond_aanwezig").answered_by.is_empty());
    assert!(!term("grond_aanwezig").asks_finding());
    // The policy's own term: its holder.
    let own = term("grond_aanwezig_volgens_hoofd");
    assert_eq!(own.declared_by(), "beleid_verlof_kort#1");
    assert_eq!(own.answered_by.len(), 1);
    assert!(own.is_finding(&head));
    assert!(!own.is_finding(&inspection));
    // A term that names its own authority: that one, not the holder.
    let other = term("grond_aanwezig_volgens_inspectie");
    assert!(other.is_finding(&inspection));
    assert!(!other.is_finding(&head));
    assert!(other.asks_finding());
}

/// A finding on a policy's own term counts as asked when the case does not
/// reach it, but its type is checked all the same (RFC-048 §2).
#[test]
fn a_finding_on_a_policys_own_term_is_checked_where_the_case_does_not_reach_it() {
    let service = service(&[GENERAL_LAW, SPECIAL_LAW, SHORT_LEAVE_POLICY]);
    let run = |dagen: i64, finding: Value| {
        service.execute_stage_at_with_findings(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[("dagen", Value::Int(dagen)), ("besluit_datum", date(DAY))]),
            params(&[("grond_aanwezig_volgens_hoofd", finding)]),
            DAY,
        )
    };
    // Two days: the policy fills the term without the finding.
    let result = run(2, Value::Bool(false)).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
    // A finding of the wrong type fails, reached or not.
    let e = run(2, Value::String("ja".into())).unwrap_err();
    assert!(
        e.to_string().contains("grond_aanwezig_volgens_hoofd"),
        "{e}"
    );
    let e = run(5, Value::String("ja".into())).unwrap_err();
    assert!(
        e.to_string().contains("grond_aanwezig_volgens_hoofd"),
        "{e}"
    );
    // Five days: it reads the finding.
    let result = run(5, Value::Bool(false)).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
}

/// A regulation scoped by `gemeente_code` fills a term only for a scope
/// with that code, in what a decision needs as in an execution.
#[test]
fn the_terms_of_a_decision_follow_the_scope() {
    let scoped = POLICY.replace(
        "$id: beleid_verlof\n",
        "$id: beleid_verlof_gemeente\ngemeente_code: GM0001\n",
    );
    let service = service(&[GENERAL_LAW, SPECIAL_LAW, &scoped]);
    let filled = |scope: &std::collections::HashMap<String, Value>| {
        service
            .decision_terms("wet_verlof", "1", DAY, scope)
            .unwrap()
            .into_iter()
            .find(|t| t.id == "grond_aanwezig")
            .unwrap()
            .filled_by
    };
    assert!(filled(&Default::default()).is_empty());
    let scope = [("gemeente_code".to_string(), Value::String("GM0001".into()))]
        .into_iter()
        .collect();
    assert_eq!(filled(&scope), ["beleid_verlof_gemeente#1"]);
}

/// A law with a default on a term it leaves to the head per case, filled by
/// a regulation that is silent for a long leave: the default applies, as a
/// silent regulation leaves it (RFC-036), and a finding on the term is asked
/// by none (RFC-048 §2).
#[test]
fn a_silent_regulation_leaves_a_term_decided_per_case_to_its_default() {
    let law = SPECIAL_LAW.replace(
        "          decided_per_case_by: het hoofd\n",
        "          decided_per_case_by: het hoofd\n          default:\n            actions:\n              - {output: grond_aanwezig, value: false}\n",
    );
    assert_ne!(law, SPECIAL_LAW);
    let silent = r#"
$id: beleid_verlof_stil
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid-stil
articles:
  - number: '1'
    text: Bij ten hoogste drie dagen is de grond aanwezig.
    machine_readable:
      implements:
        - {law: wet_verlof, article: '1', open_term: grond_aanwezig}
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
"#;
    let service = service(&[GENERAL_LAW, &law, silent]);
    let run = |dagen: i64, findings: BTreeMap<String, Value>| {
        service.execute_stage_at_with_findings(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[("dagen", Value::Int(dagen)), ("besluit_datum", date(DAY))]),
            findings,
            DAY,
        )
    };
    assert_eq!(
        run(2, BTreeMap::new()).unwrap().outputs["verlof_verleend"],
        Value::Bool(true)
    );
    assert_eq!(
        run(5, BTreeMap::new()).unwrap().outputs["verlof_verleend"],
        Value::Bool(false)
    );
    let e = run(5, params(&[("grond_aanwezig", Value::Bool(true))])).unwrap_err();
    assert!(e.to_string().contains("asked by none"), "{e}");
}

/// The law reads the term only for a leave of at most ten days, so for a
/// longer one the policy never runs. A finding on the policy's own term is
/// then not reached, yet counts as asked, and its type is checked.
#[test]
fn a_finding_on_a_policys_own_term_counts_where_the_policy_does_not_run() {
    let law = SPECIAL_LAW.replace(
        "                - $grond_aanwezig\n                - {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}\n",
        "                - {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}\n                - $grond_aanwezig\n",
    );
    assert_ne!(law, SPECIAL_LAW);
    let service = service(&[GENERAL_LAW, &law, SHORT_LEAVE_POLICY]);
    let run = |finding: Value| {
        service.execute_stage_at_with_findings(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[("dagen", Value::Int(20)), ("besluit_datum", date(DAY))]),
            params(&[("grond_aanwezig_volgens_hoofd", finding)]),
            DAY,
        )
    };
    let result = run(Value::Bool(true)).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
    let e = run(Value::String("ja".into())).unwrap_err();
    assert!(
        e.to_string().contains("grond_aanwezig_volgens_hoofd"),
        "{e}"
    );
}

/// A finding on a term a regulation fills fails the execution also where
/// the case never reads the term (RFC-048 §2).
#[test]
fn a_finding_on_a_filled_term_fails_where_the_case_does_not_read_it() {
    let law = SPECIAL_LAW.replace(
        "                - $grond_aanwezig\n                - {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}\n",
        "                - {operation: LESS_THAN_OR_EQUAL, subject: $dagen, value: 10}\n                - $grond_aanwezig\n",
    );
    assert_ne!(law, SPECIAL_LAW);
    let service = service(&[GENERAL_LAW, &law, POLICY]);
    let e = service
        .execute_stage_at_with_findings(
            "wet_verlof",
            "1",
            "BESLUIT",
            Some(&["verlof_verleend"]),
            params(&[("dagen", Value::Int(20)), ("besluit_datum", date(DAY))]),
            params(&[("grond_aanwezig", Value::Bool(true))]),
            DAY,
        )
        .unwrap_err();
    assert!(e.to_string().contains("'grond_aanwezig'"), "{e}");
}
