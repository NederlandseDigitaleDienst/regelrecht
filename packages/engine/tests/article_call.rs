//! A call to an article (`source.article`, schema v0.8.4, RFC-050): the
//! article comes from the law or from a value, its parameters from
//! `parameters_from` with `parameters` on top, and without `output` the
//! input is a decision value `{basis, fields, findings, outputs}`. Since
//! v0.8.14 a reference may name an article that takes no decision, for one
//! output.
//! A decision value as the target is called anew with its own fields and
//! findings, and the caller's findings win over those. Also the strict
//! `EQUALS` on lists and records that compares two such values.
//!
//! The laws here are toys, written for the engine: no corpus, no Awb.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use regelrecht_engine::{EngineError, FilledBy, LawExecutionService, OpenTermFilled, Value};
use rust_decimal::Decimal;
use serde_json::json;

const DATE: &str = "2025-06-01";

/// The deciding law. Article 1 establishes an application and decides on
/// it; its decision yields two of its four outputs, and `dagen_over`, which
/// it lists under `derived`, is data derived for the decision (RFC-050). Article 3
/// decides nothing.
const VERLOF: &str = r#"
$id: wet_verlof
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent verlof voor ten hoogste tien dagen, om een gewichtige reden.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: wet_verlof#1
            parameters: [dagen, reden_gewichtig]
            outputs: [aanvraag_compleet]
          - type: BESCHIKKING
            outputs: [verlof_verleend, aantal_dagen]
            derived: [dagen_over]
        parameters:
          - name: dagen
            type: number
          - name: reden_gewichtig
            type: boolean
        output:
          - name: aanvraag_compleet
            type: boolean
          - name: verlof_verleend
            type: boolean
          - name: aantal_dagen
            type: number
          - name: dagen_over
            type: number
        actions:
          - output: aanvraag_compleet
            value: true
          - output: dagen_over
            value:
              operation: SUBTRACT
              values: [10, $dagen]
          - output: verlof_verleend
            value:
              operation: AND
              conditions:
                - $reden_gewichtig
                - operation: LESS_THAN_OR_EQUAL
                  subject: $dagen
                  value: 10
          - output: aantal_dagen
            value:
              operation: IF
              cases:
                - when: $verlof_verleend
                  then: $dagen
              default: 0
  - number: '3'
    text: De school houdt een register bij.
    machine_readable:
      execution:
        output:
          - name: register_bijgehouden
            type: boolean
        actions:
          - output: register_bijgehouden
            value: true
"#;

/// A law not yet in force on [`DATE`].
const LATER: &str = r#"
$id: wet_later
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Een besluit dat pas later kan worden genomen.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - name: toegekend
            type: boolean
        actions:
          - output: toegekend
            value: true
"#;

/// The calling law. Article 1 calls the article a decision value names,
/// with that decision's fields and the findings given anew; article 2
/// calls a fixed article for one output; article 3 merges two objects and
/// a parameter on top; article 4 reads the call only when the first
/// condition of an AND holds.
const HEROVERWEGING: &str = r#"
$id: wet_heroverweging
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het besluit wordt heroverwogen.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: decision
          - name: nieuwe_oordelen
            type: object
            nullable: true
        input:
          - name: heroverweging
            type: decision
            nullable: true
            source:
              article: $besluit.basis
              parameters_from:
                - $besluit.fields
                - $nieuwe_oordelen
        output:
          - name: heroverwogen
            type: decision
            nullable: true
          - name: aanleiding
            type: boolean
        actions:
          - output: heroverwogen
            value: $heroverweging
          - output: aanleiding
            value:
              operation: NOT
              value:
                operation: EQUALS
                subject: $heroverweging.outputs
                value: $besluit.outputs
  - number: '2'
    text: Of het verlof wordt verleend.
    machine_readable:
      execution:
        parameters:
          - name: dagen
            type: number
          - name: reden
            type: boolean
        input:
          - name: verleend
            type: boolean
            source:
              article: wet_verlof#1
              output: verlof_verleend
              parameters:
                dagen: $dagen
                reden_gewichtig: $reden
        output:
          - name: verlof_zou_worden_verleend
            type: boolean
        actions:
          - output: verlof_zou_worden_verleend
            value: $verleend
  - number: '3'
    text: Twee oordelen en een parameter erbovenop.
    machine_readable:
      execution:
        parameters:
          - name: eerst
            type: object
          - name: dan
            type: object
          - name: dagen
            type: number
        input:
          - name: samengevoegd
            type: decision
            nullable: true
            source:
              article: wet_verlof#1
              parameters_from: [$eerst, $dan]
              parameters:
                dagen: $dagen
        output:
          - name: samengevoegd_besluit
            type: decision
            nullable: true
        actions:
          - output: samengevoegd_besluit
            value: $samengevoegd
  - number: '4'
    text: Alleen een ontvankelijk verzoek wordt heroverwogen.
    machine_readable:
      execution:
        parameters:
          - name: ontvankelijk
            type: boolean
          - name: besluit
            type: decision
        input:
          - name: opnieuw
            type: decision
            nullable: true
            source:
              article: $besluit.basis
              parameters_from: [$besluit.fields]
        output:
          - name: herroepen
            type: boolean
        actions:
          - output: herroepen
            value:
              operation: AND
              conditions:
                - $ontvankelijk
                - operation: NOT
                  value:
                    operation: EQUALS
                    subject: $opnieuw.outputs
                    value: $besluit.outputs
  - number: '5'
    text: Een uitkomst van een besluit dat de waarde noemt.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: decision
        input:
          - name: register
            type: boolean
            source:
              article: $besluit.basis
              output: register_bijgehouden
        output:
          - name: register_ok
            type: boolean
        actions:
          - output: register_ok
            value: $register
  - number: '6'
    text: Gelijk of niet.
    machine_readable:
      execution:
        parameters:
          - name: links
            type: object
          - name: rechts
            type: object
        output:
          - name: gelijk
            type: boolean
          - name: ongelijk
            type: boolean
        actions:
          - output: gelijk
            value:
              operation: EQUALS
              subject: $links
              value: $rechts
          - output: ongelijk
            value:
              operation: NOT_EQUALS
              subject: $links
              value: $rechts
  - number: '7'
    text: Of het verlof bij heroverweging zou worden verleend.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: decision
        input:
          - name: verleend_opnieuw
            type: boolean
            nullable: true
            source:
              article: $besluit.basis
              output: verlof_verleend
              parameters_from: [$besluit.fields]
        output:
          - name: opnieuw_verleend
            type: boolean
            nullable: true
        actions:
          - output: opnieuw_verleend
            value: $verleend_opnieuw
  - number: '8'
    text: Een besluit dat er misschien niet is.
    machine_readable:
      execution:
        parameters:
          - name: misschien
            type: decision
            nullable: true
        input:
          - name: opnieuw_of_niet
            type: decision
            nullable: true
            source:
              article: $misschien.basis
              parameters_from: [$misschien.fields]
        output:
          - name: geen_besluit
            type: boolean
        actions:
          - output: geen_besluit
            value:
              operation: EQUALS
              subject: $opnieuw_of_niet
              value: null
"#;

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(VERLOF).unwrap();
    service.load_law(LATER).unwrap();
    service.load_law(HEROVERWEGING).unwrap();
    service
}

fn params(value: serde_json::Value) -> BTreeMap<String, Value> {
    match Value::from(value) {
        Value::Object(map) => map,
        other => panic!("not an object: {other:?}"),
    }
}

/// A decision under `wet_verlof#1` as a caller holds it.
fn decision(dagen: i64, reden: bool, verleend: bool) -> serde_json::Value {
    json!({
        "basis": "wet_verlof#1",
        "taken_on": "wet_verlof#1",
        "fields": {"dagen": dagen, "reden_gewichtig": reden},
        "findings": {},
        "outputs": {"verlof_verleend": verleend, "aantal_dagen": if verleend { dagen } else { 0 }},
    })
}

fn evaluate(
    service: &LawExecutionService,
    output: &str,
    parameters: serde_json::Value,
) -> regelrecht_engine::Result<Value> {
    service
        .evaluate_law_output("wet_heroverweging", output, params(parameters), DATE)
        .map(|result| result.outputs[output].clone())
}

fn traced(service: &LawExecutionService, output: &str, parameters: serde_json::Value) -> String {
    let result = service
        .evaluate_law_output_with_trace("wet_heroverweging", output, params(parameters), DATE)
        .unwrap();
    result.trace.as_ref().unwrap().render_box_drawing()
}

fn error(service: &LawExecutionService, output: &str, parameters: serde_json::Value) -> String {
    match evaluate(service, output, parameters) {
        Ok(value) => panic!("expected an error, got {value:?}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_call_from_a_value_returns_the_decision_value() {
    let heroverwogen = evaluate(
        &service(),
        "heroverwogen",
        json!({"besluit": decision(5, false, false), "nieuwe_oordelen": {"reden_gewichtig": true}}),
    )
    .unwrap();
    assert_eq!(
        heroverwogen,
        Value::from(json!({
            "basis": "wet_verlof#1",
            "fields": {"dagen": 5, "reden_gewichtig": true},
        "findings": {},
            "outputs": {"verlof_verleend": true, "aantal_dagen": 5},
        })),
        "outputs holds only the outputs of the decision: not aanvraag_compleet, nor the \
         derived dagen_over"
    );
}

#[test]
fn a_call_with_the_same_outcome_gives_no_cause() {
    let service = service();
    let same = evaluate(
        &service,
        "aanleiding",
        json!({"besluit": decision(5, false, false), "nieuwe_oordelen": null}),
    );
    assert_eq!(same.unwrap(), Value::Bool(false));
    let other = evaluate(
        &service,
        "aanleiding",
        json!({"besluit": decision(5, false, false), "nieuwe_oordelen": {"reden_gewichtig": true}}),
    );
    assert_eq!(other.unwrap(), Value::Bool(true));
}

#[test]
fn a_call_from_a_fixed_reference_with_output_returns_that_value() {
    let service = service();
    for (dagen, reden, expected) in [(5, true, true), (12, true, false), (5, false, false)] {
        let value = evaluate(
            &service,
            "verlof_zou_worden_verleend",
            json!({"dagen": dagen, "reden": reden}),
        );
        assert_eq!(value.unwrap(), Value::Bool(expected), "{dagen} {reden}");
    }
}

#[test]
fn a_call_from_a_value_with_output_takes_an_output_of_the_decision() {
    let service = service();
    // Article 3 decides nothing: a reference to it gives the output named.
    let besluit = json!({"basis": "wet_verlof#3", "taken_on": "wet_verlof#3", "fields": {}, "findings": {}, "outputs": {}});
    let value = evaluate(&service, "register_ok", json!({"besluit": besluit}));
    assert_eq!(value.unwrap(), Value::Bool(true));
    // Article 1 decides, but register_bijgehouden is none of its outputs.
    let message = error(
        &service,
        "register_ok",
        json!({"besluit": decision(5, true, true)}),
    );
    assert!(
        message.contains("'register_bijgehouden'") && message.contains("no output of its decision"),
        "{message}"
    );
}

#[test]
fn a_call_from_a_value_with_output_returns_that_value() {
    let service = service();
    for (dagen, reden, expected) in [(5, true, true), (12, true, false)] {
        let value = evaluate(
            &service,
            "opnieuw_verleend",
            json!({"besluit": decision(dagen, reden, !expected)}),
        );
        assert_eq!(value.unwrap(), Value::Bool(expected), "{dagen} {reden}");
    }
}

#[test]
fn a_null_target_is_no_call_and_the_input_is_null() {
    let service = service();
    let value = evaluate(&service, "geen_besluit", json!({"misschien": null}));
    assert_eq!(value.unwrap(), Value::Bool(true));
    let value = evaluate(
        &service,
        "geen_besluit",
        json!({"misschien": decision(5, true, true)}),
    );
    assert_eq!(value.unwrap(), Value::Bool(false));
}

#[test]
fn a_null_target_for_an_input_that_is_never_absent_is_found_at_load() {
    let law = caller(
        "          - name: misschien_opnieuw\n            type: decision\n            source:\n              article: $misschien.basis\n",
    )
    .replace(
        "          - name: tekst\n            type: string\n",
        "          - name: tekst\n            type: string\n          - name: misschien\n            type: decision\n            nullable: true\n",
    );
    let message = load_error(&[&law]);
    assert!(
        message.contains("[N5]") && message.contains("no article is then called"),
        "{message}"
    );
}

#[test]
fn a_required_field_that_is_null_skips_the_call() {
    // `dagen` is required and not nullable in wet_verlof#1: with null for
    // it the article is not run, as for a cross-law call (RFC-036), and the
    // input, nullable because N5 demands it, is null.
    let mut besluit = decision(5, true, true);
    besluit["fields"]["dagen"] = json!(null);
    let parameters = json!({"besluit": besluit, "nieuwe_oordelen": null});
    let value = evaluate(&service(), "heroverwogen", parameters.clone());
    assert_eq!(value.unwrap(), Value::Null);
    let trace = traced(&service(), "heroverwogen", parameters);
    assert!(
        trace.contains("wet_verlof#1 is not executed; input resolves to null"),
        "{trace}"
    );
}

/// N5 for the skip rule on a call to an article (RFC-036): which parameters
/// the article requires is known only at run time, so a nullable reference
/// under `parameters`, to any target, or any `parameters_from` makes the
/// input nullable.
#[test]
fn a_call_that_may_be_skipped_needs_a_nullable_input() {
    let with_nullable = |input: &str| {
        caller(input).replace(
            "          - name: tekst\n            type: string\n",
            "          - name: tekst\n            type: string\n          - name: misschien\n            type: number\n            nullable: true\n",
        )
    };
    for (input, why) in [
        (
            "          - name: x\n            type: decision\n            source:\n              article: wet_verlof#1\n              parameters:\n                dagen: $misschien\n",
            "'x' is null when 'misschien' is null",
        ),
        (
            "          - name: x\n            type: decision\n            source:\n              article: $besluit.basis\n              parameters:\n                dagen: $misschien\n",
            "'x' is null when 'misschien' is null",
        ),
        (
            "          - name: x\n            type: decision\n            source:\n              article: $besluit.basis\n              parameters_from: [$besluit.fields]\n",
            "from parameters_from",
        ),
    ] {
        let message = load_error(&[VERLOF, &with_nullable(input)]);
        assert!(message.contains("[N5]") && message.contains(why), "{why}: {message}");
        assert!(!message.contains("and so is"), "{why}: {message}");
        // Declared nullable, the same call loads.
        let nullable = with_nullable(&input.replacen(
            "            type: decision\n",
            "            type: decision\n            nullable: true\n",
            1,
        ));
        let mut service = LawExecutionService::new();
        service.load_law(VERLOF).unwrap();
        service
            .load_law(&nullable)
            .unwrap_or_else(|e| panic!("{why}: a nullable input loads: {e}"));
    }
    // A path under a nullable root is null with it, and the finding says so.
    let law = caller(
        "          - name: x\n            type: decision\n            source:\n              article: wet_verlof#1\n              parameters:\n                dagen: $misschien.dagen\n",
    )
    .replace(
        "          - name: tekst\n            type: string\n",
        "          - name: tekst\n            type: string\n          - name: misschien\n            type: object\n            nullable: true\n",
    );
    let message = load_error(&[VERLOF, &law]);
    assert!(
        message.contains("[N5]")
            && message
                .contains("'x' is null when 'misschien' is null (and so is 'misschien.dagen')"),
        "{message}"
    );
}

#[test]
fn a_whole_number_against_a_fresh_decimal_in_a_decision_is_an_error() {
    // MULTIPLY(10, 0.5) leaves 5.0, and the decision value holds it as
    // computed. Against a stored 5 the strict EQUALS fails rather than
    // answers: nothing normalizes the numbers.
    let helft = r#"
$id: wet_helft
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De helft van de dagen.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: dagen
            type: number
        output:
          - name: helft
            type: number
        actions:
          - output: helft
            value:
              operation: MULTIPLY
              values: [$dagen, 0.5]
"#;
    let mut service = service();
    service.load_law(helft).unwrap();
    let stored = json!({"basis": "wet_helft#1", "taken_on": "wet_helft#1", "fields": {"dagen": 10}, "findings": {}, "outputs": {"helft": 5}});
    let parameters = json!({"besluit": stored, "nieuwe_oordelen": null});
    let Value::Object(heroverwogen) =
        evaluate(&service, "heroverwogen", parameters.clone()).unwrap()
    else {
        panic!("no decision value")
    };
    assert_eq!(
        heroverwogen["outputs"],
        Value::Object(BTreeMap::from([(
            "helft".to_string(),
            Value::Decimal(Decimal::new(50, 1))
        )])),
        "the decimal as the article computed it"
    );
    let message = error(&service, "aanleiding", parameters);
    assert!(
        message.contains("numbers written alike") && message.contains("5.0 against 5 at .helft"),
        "{message}"
    );
}

#[test]
fn the_hooks_on_the_submission_do_not_fire_at_the_call() {
    // A hook on the application of wet_verlof#1 fires at the stage of the
    // application, not when the decision is taken again: it needs a
    // signature that the decision's fields do not hold.
    let haak = r#"
$id: wet_haak
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De aanvraag wordt ondertekend.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to:
            submission: AANVRAAG
            established_by: wet_verlof#1
      execution:
        parameters:
          - name: handtekening
            type: string
            nullable: true
        output:
          - name: ondertekend
            type: boolean
        actions:
          - output: ondertekend
            value:
              operation: NOT_NULL
              subject: $handtekening
"#;
    let mut service = service();
    service.load_law(haak).unwrap();
    let heroverwogen = evaluate(
        &service,
        "heroverwogen",
        json!({"besluit": decision(5, true, true), "nieuwe_oordelen": null}),
    );
    assert!(heroverwogen.is_ok(), "{heroverwogen:?}");
}

#[test]
fn parameters_from_merges_in_order_and_parameters_go_on_top() {
    let samengevoegd = evaluate(
        &service(),
        "samengevoegd_besluit",
        json!({
            "eerst": {"dagen": 20, "reden_gewichtig": false},
            "dan": {"reden_gewichtig": true},
            "dagen": 3,
        }),
    )
    .unwrap();
    // `dan` wins over `eerst` for reden_gewichtig, `parameters` over both
    // for dagen.
    assert_eq!(
        samengevoegd,
        Value::from(json!({
            "basis": "wet_verlof#1",
            "fields": {"dagen": 3, "reden_gewichtig": true},
        "findings": {},
            "outputs": {"verlof_verleend": true, "aantal_dagen": 3},
        }))
    );
}

#[test]
fn a_name_the_target_does_not_ask_is_dropped_and_traced() {
    let service = service();
    let mut besluit = decision(5, true, true);
    besluit["fields"]["kleur"] = json!("blauw");
    let parameters = json!({"besluit": besluit, "nieuwe_oordelen": null});
    let heroverwogen = evaluate(&service, "heroverwogen", parameters.clone()).unwrap();
    let Value::Object(value) = heroverwogen else {
        panic!("no decision value")
    };
    assert_eq!(
        value["fields"],
        Value::from(json!({"dagen": 5, "reden_gewichtig": true})),
        "fields is what the article asks"
    );
    let trace = traced(&service, "heroverwogen", parameters);
    assert!(
        trace.contains("wet_verlof#1 does not declare kleur, so not passed"),
        "{trace}"
    );
}

#[test]
fn a_target_not_in_force_is_refused() {
    let mut besluit = decision(5, true, true);
    besluit["basis"] = json!("wet_later#1");
    let error = evaluate(
        &service(),
        "heroverwogen",
        json!({"besluit": besluit, "nieuwe_oordelen": null}),
    )
    .expect_err("a target not in force is refused");
    // The typed error survives, so a caller tells it from a law not loaded.
    assert!(
        matches!(
            &error,
            EngineError::LawNotYetInForce { law_id, reference_date }
                if law_id == "wet_later" && reference_date == DATE
        ),
        "{error:?}"
    );
    besluit["basis"] = json!("wet_onbekend#1");
    let error = evaluate(
        &service(),
        "heroverwogen",
        json!({"besluit": besluit, "nieuwe_oordelen": null}),
    )
    .expect_err("a target not loaded is refused");
    assert!(
        matches!(&error, EngineError::LawNotFound(law_id) if law_id == "wet_onbekend"),
        "{error:?}"
    );
}

#[test]
fn a_target_without_a_decision_is_refused() {
    let besluit = json!({"basis": "wet_verlof#3", "taken_on": "wet_verlof#3", "fields": {}, "findings": {}, "outputs": {}});
    let message = error(
        &service(),
        "heroverwogen",
        json!({"besluit": besluit, "nieuwe_oordelen": null}),
    );
    assert!(
        message.contains("calls the decision of wet_verlof#3, which takes no decision"),
        "{message}"
    );
}

/// [`service`] with `besluit` of article 1 an `object`: a parameter of type
/// `decision` is refused before the article runs, so only a value of
/// another type reaches the checks where the value is read.
fn service_with_besluit_as_object() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(VERLOF).unwrap();
    service.load_law(LATER).unwrap();
    service
        .load_law(&HEROVERWEGING.replacen(
            "          - name: besluit\n            type: decision\n",
            "          - name: besluit\n            type: object\n",
            1,
        ))
        .unwrap();
    service
}

#[test]
fn a_target_that_is_no_article_reference_is_refused() {
    let service = service_with_besluit_as_object();
    let mut besluit = decision(5, true, true);
    besluit["basis"] = json!(13);
    let message = error(
        &service,
        "heroverwogen",
        json!({"besluit": besluit, "nieuwe_oordelen": null}),
    );
    assert!(message.contains("an article reference"), "{message}");
    // A string is refused the same way when it names no article.
    besluit["basis"] = json!("wet_verlof");
    let message = error(
        &service,
        "heroverwogen",
        json!({"besluit": besluit, "nieuwe_oordelen": null}),
    );
    assert!(
        message.contains("an article reference or a decision value in $besluit.basis"),
        "{message}"
    );
}

#[test]
fn a_merged_value_that_is_no_object_is_refused() {
    let mut besluit = decision(5, true, true);
    besluit["fields"] = json!(["dagen", 5]);
    let message = error(
        &service(),
        "heroverwogen",
        json!({"besluit": besluit, "nieuwe_oordelen": null}),
    );
    assert!(
        message.contains("an object in $besluit.fields"),
        "{message}"
    );
}

#[test]
fn an_unknown_merged_value_makes_the_input_unknown_without_a_call() {
    // `nieuwe_oordelen` is not passed: an optional parameter nobody passed
    // is unknown (RFC-036), so the call cannot be made yet.
    let mut service = LawExecutionService::new();
    service.load_law(VERLOF).unwrap();
    service
        .load_law(&HEROVERWEGING.replace(
            "          - name: nieuwe_oordelen\n            type: object\n            nullable: true\n",
            "          - name: nieuwe_oordelen\n            type: object\n            nullable: true\n            required: false\n",
        ))
        .unwrap();
    let value = evaluate(
        &service,
        "heroverwogen",
        json!({"besluit": decision(5, true, true)}),
    )
    .unwrap();
    assert!(value.is_unknown(), "{value:?}");
}

#[test]
fn the_call_does_not_run_when_the_and_has_decided() {
    let service = service();
    let besluit = decision(5, false, false);
    let not_admissible = json!({"ontvankelijk": false, "besluit": besluit});
    assert_eq!(
        evaluate(&service, "herroepen", not_admissible.clone()).unwrap(),
        Value::Bool(false)
    );
    // The first line of the trace echoes the parameters, the decision
    // value among them; the steps below it show what ran.
    let steps = |trace: String| trace.lines().skip(1).collect::<Vec<_>>().join("\n");
    let trace = steps(traced(&service, "herroepen", not_admissible));
    assert!(
        !trace.contains("wet_verlof"),
        "the decision must not be called:\n{trace}"
    );

    let admissible = json!({"ontvankelijk": true, "besluit": besluit});
    let trace = steps(traced(&service, "herroepen", admissible));
    assert!(trace.contains("wet_verlof#1"), "{trace}");
}

/// A decision given as a parameter is a complete decision value: without
/// `taken_on` it would read as one taken on no submission. It is refused
/// before anything runs, also where the article would not read it, naming
/// the parameter and the key. So is a key a decision value does not have.
#[test]
fn a_decision_parameter_without_a_key_or_with_another_is_refused() {
    let service = service();
    let cases = [
        ("taken_on", "lacks 'taken_on'"),
        ("findings", "lacks 'findings'"),
        ("outputs", "lacks 'outputs'"),
    ];
    for (key, flaw) in cases {
        let mut besluit = decision(5, false, false);
        besluit.as_object_mut().unwrap().remove(key);
        let not_admissible = json!({"ontvankelijk": false, "besluit": besluit});
        let message = error(&service, "herroepen", not_admissible);
        assert!(
            message.contains(&format!(
                "parameter 'besluit' of wet_heroverweging#4 is no decision value {{basis, \
                 taken_on, fields, findings, outputs}}: it {flaw}"
            )),
            "{message}"
        );
    }
    let mut besluit = decision(5, false, false);
    besluit["genomen_op"] = json!("wet_verlof#1");
    let message = error(&service, "heroverwogen", json!({"besluit": besluit}));
    assert!(
        message.contains("parameter 'besluit' of wet_heroverweging#1 is no decision value"),
        "{message}"
    );
    assert!(
        message.contains("it holds the unknown key 'genomen_op'"),
        "{message}"
    );
    // `basis` and `taken_on` are article references: a `taken_on` of
    // null, a bare article number or a number would compare unequal to the
    // article that decides on objection, and so pass where it should not.
    for (key, value) in [
        ("taken_on", json!(null)),
        ("taken_on", json!("7:11")),
        ("taken_on", json!(5)),
        ("basis", json!("wet_verlof")),
    ] {
        let mut besluit = decision(5, false, false);
        besluit[key] = value;
        let message = error(&service, "heroverwogen", json!({"besluit": besluit}));
        assert!(
            message.contains(&format!(
                "parameter 'besluit' of wet_heroverweging#1 is no decision value {{basis, \
                 taken_on, fields, findings, outputs}}: it holds no article reference in '{key}'"
            )),
            "{message}"
        );
    }
    // A value that is no object at all, such as the id of the article that
    // took the decision, is refused the same way.
    for (value, kind) in [
        (json!("wet_verlof#1"), "string"),
        (json!(5), "integer"),
        (json!(true), "boolean"),
        (json!([]), "array"),
    ] {
        let message = error(&service, "heroverwogen", json!({"besluit": value}));
        assert!(
            message.contains(&format!(
                "parameter 'besluit' of wet_heroverweging#1 is no decision value {{basis, \
                 taken_on, fields, findings, outputs}}: it is of type {kind}"
            )),
            "{message}"
        );
    }
}

#[test]
fn an_article_calling_itself_is_a_cycle() {
    let law = r#"
$id: wet_kring
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een besluit dat zichzelf vraagt.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        input:
          - name: zelf
            type: boolean
            source:
              article: wet_kring#1
              output: uitkomst
        output:
          - name: uitkomst
            type: boolean
        actions:
          - output: uitkomst
            value: $zelf
"#;
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    let message = service
        .evaluate_law_output("wet_kring", "uitkomst", BTreeMap::new(), DATE)
        .unwrap_err()
        .to_string();
    // The depth limit is a CircularReference too ("Circular reference
    // detected: Cross-law resolution depth exceeded"), so "Circular" alone
    // stays green when the cycle detection breaks and the recursion runs on
    // to the limit.
    assert!(
        message.contains("article wet_kring#1 is already being resolved"),
        "{message}"
    );
    assert!(!message.contains("depth exceeded"), "{message}");
}

#[test]
fn a_cycle_through_a_cross_law_call_is_caught_on_the_same_key() {
    // wet_a calls the decision of wet_b, which asks wet_a for the output
    // being computed.
    let a = r#"
$id: wet_a
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: A vraagt het besluit van B.
    machine_readable:
      execution:
        input:
          - name: b
            type: decision
            source:
              article: wet_b#1
        output:
          - name: uitkomst
            type: boolean
        actions:
          - output: uitkomst
            value: $b.outputs.besloten
"#;
    let b = r#"
$id: wet_b
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: B vraagt de uitkomst van A.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        input:
          - name: a
            type: boolean
            source:
              regulation: wet_a
              output: uitkomst
        output:
          - name: besloten
            type: boolean
        actions:
          - output: besloten
            value: $a
"#;
    let mut service = LawExecutionService::new();
    service.load_law(b).unwrap();
    service.load_law(a).unwrap();
    // The loop is closed on the first key it re-enters: from wet_a that is
    // the decision article wet_b#1, from wet_b the output wet_a#uitkomst.
    for (law, output, key) in [
        ("wet_a", "uitkomst", "article wet_b#1"),
        ("wet_b", "besloten", "wet_a#uitkomst"),
    ] {
        let message = service
            .evaluate_law_output(law, output, BTreeMap::new(), DATE)
            .unwrap_err()
            .to_string();
        assert!(
            message.contains(&format!("{key} is already being resolved")),
            "{law}: {message}"
        );
        assert!(!message.contains("depth exceeded"), "{law}: {message}");
    }
}

#[test]
fn a_cycle_through_another_output_of_the_decision_is_caught() {
    // wet_p calls the decision of wet_q#1; to decide, wet_q asks wet_r,
    // which asks wet_q for an output of that decision. Only the key of that
    // output catches it.
    let p = r#"
$id: wet_p
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: P vraagt het besluit van Q.
    machine_readable:
      execution:
        input:
          - name: q
            type: decision
            source:
              article: wet_q#1
        output:
          - name: uitkomst
            type: boolean
        actions:
          - output: uitkomst
            value: $q.outputs.besloten
"#;
    let q = r#"
$id: wet_q
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Q vraagt R.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        input:
          - name: r
            type: boolean
            source:
              regulation: wet_r
              output: van_r
        output:
          - name: besloten
            type: boolean
        actions:
          - output: besloten
            value: $r
"#;
    let r = r#"
$id: wet_r
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: R vraagt het besluit van Q.
    machine_readable:
      execution:
        input:
          - name: terug
            type: boolean
            source:
              regulation: wet_q
              output: besloten
        output:
          - name: van_r
            type: boolean
        actions:
          - output: van_r
            value: $terug
"#;
    let mut service = LawExecutionService::new();
    for law in [r, q, p] {
        service.load_law(law).unwrap();
    }
    let message = service
        .evaluate_law_output("wet_p", "uitkomst", BTreeMap::new(), DATE)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("wet_q#besloten is already being resolved"),
        "{message}"
    );
}

// --- Type check and load check -------------------------------------------

fn load_error(laws: &[&str]) -> String {
    let mut service = LawExecutionService::new();
    for (index, law) in laws.iter().enumerate() {
        match service.load_law(law) {
            Ok(_) if index + 1 < laws.len() => {}
            Ok(id) => panic!("{id} loaded"),
            Err(e) => return e.to_string(),
        }
    }
    unreachable!()
}

fn caller(input: &str) -> String {
    format!(
        r#"
$id: wet_aanroeper
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een aanroep.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: decision
          - name: getal
            type: number
          - name: tekst
            type: string
        input:
{input}
        output:
          - name: klaar
            type: boolean
        actions:
          - output: klaar
            value: true
"#
    )
}

#[test]
fn a_call_without_output_is_of_type_decision() {
    let law = caller(
        "          - name: heroverweging\n            type: object\n            source:\n              article: $besluit.basis\n",
    );
    let message = load_error(&[&law]);
    assert!(
        message.contains("[T5]") && message.contains("declare it type decision"),
        "{message}"
    );
}

#[test]
fn a_value_target_is_only_known_to_yield_a_decision_value() {
    // Nothing is claimed about the fields of a decision value: the call
    // and `$x.outputs.y` load without findings.
    let mut service = LawExecutionService::new();
    service.load_law(HEROVERWEGING).unwrap();
}

#[test]
fn parameters_from_takes_objects_and_the_target_value_is_a_string() {
    let law = caller(
        "          - name: heroverweging\n            type: decision\n            source:\n              article: $getal\n              parameters_from: [$tekst]\n",
    );
    let message = load_error(&[&law]);
    assert!(
        message.contains("merges $tekst") && message.contains("declared string"),
        "{message}"
    );
    assert!(
        message.contains("named by $getal, which is declared number"),
        "{message}"
    );
    assert!(
        !message.contains("the parameters of a decision"),
        "{message}"
    );

    // A decision is no object to merge: its parameters are under `fields`.
    let law = caller(
        "          - name: heroverweging\n            type: decision\n            source:\n              article: $besluit.basis\n              parameters_from: [$besluit]\n",
    );
    let message = load_error(&[&law]);
    assert!(
        message.contains("(the parameters of a decision are $besluit.fields)"),
        "{message}"
    );
}

#[test]
fn an_output_that_may_be_null_needs_a_nullable_input() {
    let target = r#"
$id: wet_misschien
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een besluit met een uitkomst die er niet altijd is.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        output:
          - name: bedrag
            type: number
            nullable: true
        actions:
          - output: bedrag
            value: null
"#;
    let input = "          - name: x\n            type: number\n            source:\n              article: wet_misschien#1\n              output: bedrag\n";
    let message = load_error(&[target, &caller(input)]);
    assert!(
        message.contains("[N5]") && message.contains("which may be null"),
        "{message}"
    );
    let mut service = LawExecutionService::new();
    service.load_law(target).unwrap();
    service
        .load_law(&caller(&input.replace(
            "            type: number\n",
            "            type: number\n            nullable: true\n",
        )))
        .expect("a nullable input loads");
}

#[test]
fn a_fixed_target_is_checked_against_the_loaded_law() {
    let no_decision = caller(
        "          - name: register\n            type: decision\n            source:\n              article: wet_verlof#3\n",
    );
    let message = load_error(&[VERLOF, &no_decision]);
    assert!(message.contains("which takes no decision"), "{message}");

    let not_of_the_decision = caller(
        "          - name: compleet\n            type: boolean\n            source:\n              article: wet_verlof#1\n              output: aanvraag_compleet\n",
    );
    let message = load_error(&[VERLOF, &not_of_the_decision]);
    assert!(message.contains("no output of its decision"), "{message}");

    // The other order: the consumer is checked again when its target loads.
    let message = load_error(&[&not_of_the_decision, VERLOF]);
    assert!(message.contains("no output of its decision"), "{message}");
}

#[test]
fn a_fixed_target_in_the_same_law_must_exist() {
    let law = caller(
        "          - name: elders\n            type: decision\n            source:\n              article: wet_aanroeper#9\n",
    );
    let message = load_error(&[&law]);
    assert!(message.contains("this law has no article 9"), "{message}");
}

#[test]
fn an_article_call_stands_apart_from_regulation_and_parameters_from_needs_it() {
    let beside = caller(
        "          - name: x\n            type: decision\n            source:\n              article: wet_verlof#1\n              regulation: wet_verlof\n",
    );
    let message = load_error(&[&beside]);
    assert!(
        message.contains("which no regulation can stand beside"),
        "{message}"
    );

    let alone = caller(
        "          - name: x\n            type: object\n            source:\n              parameters_from: [$besluit]\n",
    );
    let message = load_error(&[&alone]);
    assert!(
        message.contains("parameters_from without an article"),
        "{message}"
    );

    let not_a_target = caller(
        "          - name: x\n            type: decision\n            source:\n              article: wet verlof\n",
    );
    let message = load_error(&[&not_a_target]);
    assert!(message.contains("nor a $-reference"), "{message}");
}

// --- EQUALS on lists and records -----------------------------------------

fn compare(links: Value, rechts: Value) -> regelrecht_engine::Result<(Value, Value)> {
    let service = service();
    let parameters = BTreeMap::from([("links".to_string(), links), ("rechts".to_string(), rechts)]);
    let result = service.evaluate_law(
        "wet_heroverweging",
        &["gelijk", "ongelijk"],
        parameters,
        DATE,
    )?;
    Ok((
        result.outputs["gelijk"].clone(),
        result.outputs["ongelijk"].clone(),
    ))
}

#[test]
fn equals_on_records_is_structural() {
    let record = json!({"verleend": true, "dagen": 5, "data": ["2025-06-02", "2025-06-03"]});
    let (equal, unequal) =
        compare(Value::from(record.clone()), Value::from(record.clone())).unwrap();
    assert_eq!((equal, unequal), (Value::Bool(true), Value::Bool(false)));

    let mut other = record.clone();
    other["data"][1] = json!("2025-06-04");
    let (equal, unequal) = compare(Value::from(record.clone()), Value::from(other)).unwrap();
    assert_eq!((equal, unequal), (Value::Bool(false), Value::Bool(true)));

    let mut fewer = record.clone();
    fewer.as_object_mut().unwrap().remove("dagen");
    let (equal, _) = compare(Value::from(record), Value::from(fewer)).unwrap();
    assert_eq!(equal, Value::Bool(false));
}

#[test]
fn equals_fails_on_an_integer_against_a_decimal_inside_a_record() {
    let with = |dagen: Value| {
        Value::Object(BTreeMap::from([
            ("verleend".to_string(), Value::Bool(true)),
            (
                "data".to_string(),
                Value::Array(vec![Value::Object(BTreeMap::from([(
                    "dagen".to_string(),
                    dagen,
                )]))]),
            ),
        ]))
    };
    let twelve = Value::Int(12);
    let twelve_point_zero = Value::Decimal(Decimal::new(120, 1));
    let message = compare(with(twelve.clone()), with(twelve_point_zero.clone()))
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("numbers written alike") && message.contains(".data[0].dagen"),
        "{message}"
    );
    // Another number is no question of how it is written: just unequal.
    let (equal, _) = compare(
        with(twelve.clone()),
        with(Value::Decimal(Decimal::new(125, 1))),
    )
    .unwrap();
    assert_eq!(equal, Value::Bool(false));
    // Alike on both sides compares as before.
    let (equal, _) = compare(with(twelve_point_zero.clone()), with(twelve_point_zero)).unwrap();
    assert_eq!(equal, Value::Bool(true));
}

#[test]
fn equals_on_bare_numbers_keeps_its_coercion() {
    let law = r#"
$id: wet_getallen
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Twaalf is twaalf.
    machine_readable:
      execution:
        parameters:
          - name: a
            type: number
          - name: b
            type: number
        output:
          - name: gelijk
            type: boolean
        actions:
          - output: gelijk
            value:
              operation: EQUALS
              subject: $a
              value: $b
"#;
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    let parameters = BTreeMap::from([
        ("a".to_string(), Value::Int(12)),
        ("b".to_string(), Value::Decimal(Decimal::new(120, 1))),
    ]);
    let result = service
        .evaluate_law_output("wet_getallen", "gelijk", parameters, DATE)
        .unwrap();
    assert_eq!(result.outputs["gelijk"], Value::Bool(true));
}

// --- A judgement per case passed to the article --------------------------

/// The head judges per case whether the ground is present (an open term with
/// `decided_per_case_by`, RFC-047), as for Leerplichtwet 13a. The caller
/// merges the judgement into the parameters of the call.
const HOOFD: &str = r#"
$id: wet_hoofd
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent verlof als de grond naar zijn oordeel aanwezig is.
    machine_readable:
      open_terms:
        - id: grond_aanwezig
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: dagen
            type: number
        output:
          - name: verleend
            type: boolean
        actions:
          - output: verleend
            value:
              operation: AND
              conditions:
                - $grond_aanwezig
                - operation: LESS_THAN_OR_EQUAL
                  subject: $dagen
                  value: 10
  - number: '2'
    text: Het besluit opnieuw, met de oordelen van nu.
    machine_readable:
      execution:
        parameters:
          - name: dagen
            type: number
          - name: oordelen
            type: object
        input:
          - name: opnieuw
            type: decision
            nullable: true
            source:
              article: wet_hoofd#1
              parameters_from: [$oordelen]
              parameters:
                dagen: $dagen
        output:
          - name: opnieuw_besloten
            type: decision
            nullable: true
        actions:
          - output: opnieuw_besloten
            value: $opnieuw
"#;

/// Policy that fills the head's term for every case.
const HOOFD_BELEID: &str = r#"
$id: beleid_hoofd
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.invalid/beleid-hoofd
articles:
  - number: '1'
    text: De grond is nooit aanwezig.
    machine_readable:
      implements:
        - {law: wet_hoofd, article: '1', open_term: grond_aanwezig}
      execution:
        output:
          - {name: grond_aanwezig, type: boolean}
        actions:
          - {output: grond_aanwezig, value: false}
"#;

fn hoofd(laws: &[&str], oordelen: serde_json::Value) -> (Value, String) {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    let parameters = params(json!({"dagen": 5, "oordelen": oordelen}));
    let result = service
        .evaluate_law_output_with_trace("wet_hoofd", "opnieuw_besloten", parameters, DATE)
        .unwrap();
    let trace = result.trace.as_ref().unwrap().render_box_drawing();
    (result.outputs["opnieuw_besloten"].clone(), trace)
}

#[test]
fn a_judgement_per_case_reaches_the_article_and_the_rest_is_dropped() {
    for judgement in [true, false] {
        let (value, trace) = hoofd(
            &[HOOFD],
            json!({"grond_aanwezig": judgement, "onbekend": 1}),
        );
        assert_eq!(
            value,
            Value::from(json!({
                "basis": "wet_hoofd#1",
                "fields": {"dagen": 5},
                "findings": {"grond_aanwezig": judgement},
                "outputs": {"verleend": judgement},
            })),
            "{trace}"
        );
        assert!(
            trace.contains("decided in this case by het hoofd"),
            "{trace}"
        );
        assert!(
            trace.contains(
                "wet_hoofd#1 leaves grond_aanwezig to an authority per case, so passed as its \
                 finding"
            ),
            "{trace}"
        );
        assert!(
            trace.contains("does not declare onbekend, so not passed"),
            "{trace}"
        );
    }
}

#[test]
fn a_judgement_passed_on_a_term_an_implementation_fills_is_not_passed() {
    // The implementation's value is the term's; no judgement steps in
    // (RFC-048 §2). One the call passes is not passed on, and the trace
    // says why.
    let (value, trace) = hoofd(&[HOOFD, HOOFD_BELEID], json!({"grond_aanwezig": true}));
    assert_eq!(
        value.as_object().unwrap()["outputs"],
        Value::from(json!({"verleend": false}))
    );
    assert!(
        trace.contains("a regulation in force fills grond_aanwezig, so the finding is not passed"),
        "{trace}"
    );
}

#[test]
fn without_a_judgement_the_required_term_is_refused() {
    let mut service = LawExecutionService::new();
    service.load_law(HOOFD).unwrap();
    let error = service
        .evaluate_law_output(
            "wet_hoofd",
            "opnieuw_besloten",
            params(json!({"dagen": 5, "oordelen": {}})),
            DATE,
        )
        .expect_err("no judgement, no decision");
    assert!(
        error
            .to_string()
            .contains("no judgement was given for this case"),
        "{error}"
    );
}

#[test]
fn a_term_not_decided_per_case_is_no_judgement_and_is_dropped() {
    // `max_dagen` is an open term the article fills by default, not one an
    // authority judges per case: a value for it passed to the call is no
    // judgement, and the default stands.
    let law = HOOFD.replace(
        "          decided_per_case_by: het hoofd\n",
        "          decided_per_case_by: het hoofd\n        - id: max_dagen\n          type: number\n          required: false\n          default:\n            actions:\n              - output: max_dagen\n                value: 10\n",
    )
    .replace(
        "                  subject: $dagen\n                  value: 10\n",
        "                  subject: $dagen\n                  value: $max_dagen\n",
    );
    let (value, trace) = hoofd(&[&law], json!({"grond_aanwezig": true, "max_dagen": 3}));
    assert_eq!(
        value,
        Value::from(json!({
            "basis": "wet_hoofd#1",
            "fields": {"dagen": 5},
            "findings": {"grond_aanwezig": true},
            "outputs": {"verleend": true},
        })),
        "{trace}"
    );
    assert!(
        trace.contains("does not declare max_dagen, so not passed"),
        "{trace}"
    );
}

/// A chain of `n` articles, each calling the next one's decision.
fn chain(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let input = if i + 1 < n {
                format!(
                    "        input:\n          - name: verder\n            type: boolean\n            source:\n              article: wet_schakel_{}#1\n              output: besloten\n",
                    i + 1
                )
            } else {
                String::new()
            };
            let value = if i + 1 < n { "$verder" } else { "true" };
            format!(
                r#"
$id: wet_schakel_{i}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Schakel {i}.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
{input}        output:
          - name: besloten
            type: boolean
        actions:
          - output: besloten
            value: {value}
"#
            )
        })
        .collect()
}

fn run_chain(n: usize) -> regelrecht_engine::Result<Value> {
    let mut service = LawExecutionService::new();
    for law in chain(n).iter().rev() {
        service.load_law(law).unwrap();
    }
    service
        .evaluate_law_output("wet_schakel_0", "besloten", BTreeMap::new(), DATE)
        .map(|result| result.outputs["besloten"].clone())
}

#[test]
fn a_chain_of_calls_may_reach_the_depth_limit_but_not_pass_it() {
    // Each call enters one level; the top article is level zero.
    let deepest = regelrecht_engine::MAX_CROSS_LAW_DEPTH + 1;
    assert_eq!(run_chain(deepest).unwrap(), Value::Bool(true));
    let message = run_chain(deepest + 1).unwrap_err().to_string();
    assert!(message.contains("depth exceeded"), "{message}");
}

// --- A decision called anew, and the caller's findings ---------------------

/// [`HOOFD`] with an article 3 that calls a decision value anew: no
/// `parameters_from`, the value's own fields and findings go along.
fn hoofd_anew() -> String {
    format!(
        "{HOOFD}{}",
        r#"  - number: '3'
    text: Het besluit opnieuw.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: decision
        input:
          - name: opnieuw
            type: decision
            nullable: true
            source:
              article: $besluit
        output:
          - name: heroverwogen
            type: decision
            nullable: true
          - name: aanleiding
            type: boolean
        actions:
          - output: heroverwogen
            value: $opnieuw
          - output: aanleiding
            value:
              operation: NOT
              value:
                operation: EQUALS
                subject: $opnieuw.outputs
                value: $besluit.outputs
  - number: '4'
    text: Iets anders.
    machine_readable:
      execution:
        output:
          - name: iets_anders
            type: boolean
        actions:
          - output: iets_anders
            value: true
"#
    )
}

fn hoofd_besluit(grond: bool) -> serde_json::Value {
    json!({
        "basis": "wet_hoofd#1",
        "taken_on": "wet_hoofd#1",
        "fields": {"dagen": 5},
        "findings": {"grond_aanwezig": grond},
        "outputs": {"verleend": grond},
    })
}

/// [`hoofd_besluit`] as a call anew gives it: built by the engine, so with
/// no `taken_on`.
fn hoofd_anew_besluit(grond: bool) -> serde_json::Value {
    let mut value = hoofd_besluit(grond);
    value.as_object_mut().unwrap().remove("taken_on");
    value
}

fn anew_service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&hoofd_anew()).unwrap();
    service
}

/// Article 3 of [`hoofd_anew`] on `besluit`, with `findings` from the
/// caller, for `output`.
fn anew(
    service: &LawExecutionService,
    besluit: serde_json::Value,
    findings: serde_json::Value,
    output: &str,
) -> regelrecht_engine::Result<Value> {
    service
        .evaluate_law_with_findings(
            "wet_hoofd",
            &[output],
            params(json!({ "besluit": besluit })),
            params(findings),
            DATE,
        )
        .map(|result| result.outputs[output].clone())
}

#[test]
fn a_decision_value_is_called_anew_with_its_own_fields_and_findings() {
    let service = anew_service();
    for grond in [true, false] {
        let value = anew(&service, hoofd_besluit(grond), json!({}), "heroverwogen").unwrap();
        assert_eq!(value, Value::from(hoofd_anew_besluit(grond)), "{grond}");
        let cause = anew(&service, hoofd_besluit(grond), json!({}), "aanleiding").unwrap();
        assert_eq!(cause, Value::Bool(false), "{grond}");
    }
}

#[test]
fn a_finding_of_the_caller_wins_over_that_of_the_decision_value() {
    let service = anew_service();
    let value = anew(
        &service,
        hoofd_besluit(false),
        json!({"grond_aanwezig": true}),
        "heroverwogen",
    )
    .unwrap();
    assert_eq!(value, Value::from(hoofd_anew_besluit(true)));
    let cause = anew(
        &service,
        hoofd_besluit(false),
        json!({"grond_aanwezig": true}),
        "aanleiding",
    );
    assert_eq!(cause.unwrap(), Value::Bool(true));
    let trace = service
        .evaluate_law_with_trace_and_findings(
            "wet_hoofd",
            &["heroverwogen"],
            params(json!({"besluit": hoofd_besluit(false)})),
            params(json!({"grond_aanwezig": true})),
            DATE,
        )
        .unwrap()
        .trace
        .unwrap()
        .render_box_drawing();
    assert!(
        trace.contains("the finding given for this execution on grond_aanwezig wins"),
        "{trace}"
    );
}

#[test]
fn a_finding_of_the_caller_on_a_term_an_implementation_fills_is_asked_by_none() {
    let mut service = anew_service();
    service.load_law(HOOFD_BELEID).unwrap();
    let e = anew(
        &service,
        hoofd_besluit(true),
        json!({"grond_aanwezig": true}),
        "heroverwogen",
    )
    .unwrap_err();
    assert!(e.to_string().contains("is asked by none"), "{e}");
}

#[test]
fn a_finding_no_article_asks_is_an_error_that_names_it() {
    let service = anew_service();
    let message = anew(
        &service,
        hoofd_besluit(false),
        json!({"grond_aanwezg": true}),
        "heroverwogen",
    )
    .unwrap_err()
    .to_string();
    assert!(
        message.contains("leaves 'grond_aanwezg' to an authority per case"),
        "{message}"
    );
    // The trace path fails the same way.
    let message = service
        .evaluate_law_with_trace_and_findings(
            "wet_hoofd",
            &["heroverwogen"],
            params(json!({"besluit": hoofd_besluit(false)})),
            params(json!({"grond_aanwezg": true})),
            DATE,
        )
        .unwrap_err()
        .to_string();
    assert!(message.contains("'grond_aanwezg'"), "{message}");
    // A finding for an article the execution does not reach is asked by
    // none either.
    let message = service
        .evaluate_law_with_findings(
            "wet_hoofd",
            &["iets_anders"],
            BTreeMap::new(),
            params(json!({"grond_aanwezig": true})),
            DATE,
        )
        .unwrap_err()
        .to_string();
    assert!(message.contains("'grond_aanwezig'"), "{message}");
}

#[test]
fn a_finding_of_another_type_is_an_error_that_names_it() {
    let service = anew_service();
    for finding in [json!("ja"), json!(1), json!(null)] {
        let message = anew(
            &service,
            hoofd_besluit(false),
            json!({ "grond_aanwezig": finding }),
            "heroverwogen",
        )
        .unwrap_err()
        .to_string();
        assert!(
            message.contains("a boolean for the finding 'grond_aanwezig' on wet_hoofd#1"),
            "{finding}: {message}"
        );
    }
}

/// A finding fits its term by kind: a number for a number or an amount, a
/// string for a string or a date, a list, an object for an object or a
/// decision, and null only for a term that is not required.
#[test]
fn a_finding_fits_the_type_of_its_term() {
    for (ty, required, fits, misfits) in [
        ("number", true, json!(1.5), json!("1")),
        ("amount", true, json!(100), json!(1.5)),
        ("string", true, json!("x"), json!(1)),
        ("date", true, json!("2025-01-01"), json!("1 januari 2025")),
        ("array", true, json!([1]), json!({})),
        ("object", true, json!({}), json!([])),
        ("boolean", false, json!(null), json!(0)),
    ] {
        let law = format!(
            r#"
$id: wet_soort
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het hoofd stelt iets vast.
    machine_readable:
      open_terms:
        - id: vastgesteld
          type: {ty}
          required: {required}
          decided_per_case_by: het hoofd
      execution:
        output:
          - name: gezien
            type: boolean
        actions:
          - output: gezien
            value: true
"#
        );
        let mut service = LawExecutionService::new();
        service.load_law(&law).unwrap();
        let run = |finding: &serde_json::Value| {
            service.evaluate_law_with_findings(
                "wet_soort",
                &["gezien"],
                BTreeMap::new(),
                params(json!({ "vastgesteld": finding })),
                DATE,
            )
        };
        run(&fits).unwrap_or_else(|e| panic!("{ty} {fits}: {e}"));
        let message = run(&misfits).unwrap_err().to_string();
        assert!(
            message.contains(&format!("a {ty} for the finding")),
            "{ty}: {message}"
        );
    }
}

#[test]
fn a_finding_at_the_top_is_not_also_a_parameter() {
    let mut service = LawExecutionService::new();
    service.load_law(HOOFD).unwrap();
    let run = |parameters: serde_json::Value| {
        service.evaluate_law_with_findings(
            "wet_hoofd",
            &["verleend"],
            params(parameters),
            params(json!({"grond_aanwezig": true})),
            DATE,
        )
    };
    // At the top a finding reaches the article's own term.
    let result = run(json!({"dagen": 5})).unwrap();
    assert_eq!(result.outputs["verleend"], Value::Bool(true));
    let message = run(json!({"dagen": 5, "grond_aanwezig": false}))
        .unwrap_err()
        .to_string();
    assert!(
        message.contains(
            "'grond_aanwezig' of wet_hoofd#1 is given both as a parameter and as a finding"
        ),
        "{message}"
    );
}

/// A finding in the decision value the article no longer leaves to an
/// authority is no finding: it is dropped, and the trace names it. A value
/// that is no decision value is refused.
#[test]
fn what_the_value_holds_that_the_article_does_not_ask_is_dropped() {
    let service = anew_service();
    let mut besluit = hoofd_besluit(true);
    besluit["findings"]["vervallen"] = json!(true);
    let trace = service
        .evaluate_law_with_trace(
            "wet_hoofd",
            &["heroverwogen"],
            params(json!({ "besluit": besluit })),
            DATE,
        )
        .unwrap()
        .trace
        .unwrap()
        .render_box_drawing();
    assert!(
        trace.contains("wet_hoofd#1 does not declare vervallen, so not passed"),
        "{trace}"
    );
    // A value with no article reference is refused before the article
    // runs; one whose parts are no objects where the value is read.
    for (odd, flaw) in [
        (
            json!({"basis": "wet_hoofd", "taken_on": "wet_hoofd", "fields": {}, "findings": {}, "outputs": {}}),
            "it holds no article reference in 'basis'",
        ),
        (
            json!({"basis": "wet_hoofd#1", "taken_on": "wet_hoofd#1", "fields": [], "findings": {}, "outputs": {}}),
            "a decision value {basis, fields, findings, outputs} in $besluit",
        ),
    ] {
        let message = anew(&service, odd.clone(), json!({}), "heroverwogen")
            .unwrap_err()
            .to_string();
        assert!(message.contains(flaw), "{odd}: {message}");
    }
}

/// A law that passes its `object` parameter `waarde` on as `besluit` of
/// article 3 of [`hoofd_anew`]. Across laws a parameter is what the calling
/// article passes on, so only the call anew checks the value.
const DOORGEEF: &str = r#"
$id: wet_doorgeef
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het besluit wordt doorgegeven.
    machine_readable:
      execution:
        parameters:
          - name: waarde
            type: object
        input:
          - name: heroverwogen
            type: decision
            nullable: true
            source:
              regulation: wet_hoofd
              output: heroverwogen
              parameters:
                besluit: $waarde
        output:
          - name: doorgegeven
            type: decision
            nullable: true
        actions:
          - output: doorgegeven
            value: $heroverwogen
"#;

#[test]
fn a_decision_value_passed_on_without_an_article_reference_is_refused_at_the_call() {
    let mut service = anew_service();
    service.load_law(DOORGEEF).unwrap();
    let odd = json!({"basis": "wet_hoofd", "taken_on": "wet_hoofd#1", "fields": {}, "findings": {}, "outputs": {}});
    let message = service
        .evaluate_law_output(
            "wet_doorgeef",
            "doorgegeven",
            params(json!({ "waarde": odd })),
            DATE,
        )
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("a decision value {basis, fields, findings, outputs} in $besluit"),
        "{message}"
    );
    // A value that is no object is refused at the call too, not before.
    let message = service
        .evaluate_law_output(
            "wet_doorgeef",
            "doorgegeven",
            params(json!({ "waarde": 5 })),
            DATE,
        )
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("expected an article reference or a decision value in $besluit"),
        "{message}"
    );
}

/// A decision value passed on across laws may lack `taken_on`, as one built
/// through `source.article` does. One it holds is an article reference:
/// `null` would compare unequal to every article, and so pass for a
/// decision not taken on objection.
#[test]
fn a_decision_value_passed_on_with_a_taken_on_that_is_no_article_reference_is_refused() {
    let mut service = anew_service();
    service.load_law(DOORGEEF).unwrap();
    let doorgegeven = |besluit: serde_json::Value| {
        service.evaluate_law_output(
            "wet_doorgeef",
            "doorgegeven",
            params(json!({ "waarde": besluit })),
            DATE,
        )
    };
    let mut besluit = hoofd_besluit(true);
    besluit["taken_on"] = json!(null);
    let message = doorgegeven(besluit).unwrap_err().to_string();
    assert!(
        message.contains(
            "parameter 'besluit' of wet_hoofd#3 is no decision value {basis, taken_on, \
             fields, findings, outputs}: it holds no article reference in 'taken_on'"
        ),
        "{message}"
    );
    // Without `taken_on` it is called anew.
    assert!(doorgegeven(hoofd_anew_besluit(true)).is_ok());
}

#[test]
fn a_decision_called_anew_needs_a_nullable_input() {
    let law = caller(
        "          - name: x\n            type: decision\n            source:\n              article: $besluit\n",
    );
    let message = load_error(&[&law]);
    assert!(
        message.contains("[N5]") && message.contains("calls the decision $besluit anew"),
        "{message}"
    );
    // A decision value is a target the type check takes, a number is not.
    let law = caller(
        "          - name: x\n            type: decision\n            source:\n              article: $getal\n",
    );
    let message = load_error(&[&law]);
    assert!(
        message.contains("or called anew from a decision value"),
        "{message}"
    );
}

/// A finding of the caller reaches every article in the execution that
/// leaves its term to an authority per case, also one a `regulation`
/// reference calls.
#[test]
fn a_finding_reaches_an_article_a_regulation_reference_calls() {
    let caller = r#"
$id: wet_vraagt
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Of het hoofd verlof zou verlenen.
    machine_readable:
      execution:
        parameters:
          - name: dagen
            type: number
        input:
          - name: verleend
            type: boolean
            source:
              regulation: wet_hoofd
              output: verleend
              parameters:
                dagen: $dagen
        output:
          - name: zou_verlenen
            type: boolean
        actions:
          - output: zou_verlenen
            value: $verleend
"#;
    let mut service = LawExecutionService::new();
    service.load_law(HOOFD).unwrap();
    service.load_law(caller).unwrap();
    for grond in [true, false] {
        let result = service
            .evaluate_law_with_findings(
                "wet_vraagt",
                &["zou_verlenen"],
                params(json!({"dagen": 5})),
                params(json!({ "grond_aanwezig": grond })),
                DATE,
            )
            .unwrap();
        assert_eq!(result.outputs["zou_verlenen"], Value::Bool(grond));
    }
}

// --- Who filled an open term -------------------------------------------------

/// What article 1 of [`HOOFD`] decides with `dagen` 5 and `findings`, with
/// `laws` loaded beside it: the open terms the execution reports filled.
fn filled(laws: &[&str], findings: serde_json::Value) -> Vec<OpenTermFilled> {
    let mut service = LawExecutionService::new();
    service.load_law(HOOFD).unwrap();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
        .evaluate_law_with_findings(
            "wet_hoofd",
            &["verleend"],
            params(json!({"dagen": 5})),
            params(findings),
            DATE,
        )
        .unwrap()
        .open_terms
}

fn grond_aanwezig(filled_by: FilledBy) -> OpenTermFilled {
    OpenTermFilled {
        law_id: "wet_hoofd".to_string(),
        article: "1".to_string(),
        term: "grond_aanwezig".to_string(),
        filled_by,
    }
}

#[test]
fn the_execution_reports_a_term_a_finding_filled() {
    let found = filled(&[], json!({"grond_aanwezig": true}));
    assert_eq!(found, vec![grond_aanwezig(FilledBy::Finding)]);
}

/// A regulation that fills the term: the report names the regulation.
#[test]
fn the_execution_reports_a_term_a_regulation_filled() {
    let gelijk = HOOFD_BELEID.replace("value: false", "value: true");
    assert_ne!(gelijk, HOOFD_BELEID);
    for beleid in [HOOFD_BELEID, gelijk.as_str()] {
        let found = filled(&[beleid], json!({}));
        assert_eq!(
            found,
            vec![grond_aanwezig(FilledBy::Regulation {
                law_id: "beleid_hoofd".to_string(),
                article: "1".to_string(),
            })]
        );
    }
}

/// The report keeps the declaring article and the implementing one apart,
/// on the result and on the receipt.
#[test]
fn a_term_a_regulation_filled_serializes_both_articles() {
    let mut service = LawExecutionService::new();
    service.load_law(HOOFD).unwrap();
    service.load_law(HOOFD_BELEID).unwrap();
    let parameters = params(json!({"dagen": 5}));
    let result = service
        .evaluate_law_with_findings(
            "wet_hoofd",
            &["verleend"],
            parameters.clone(),
            BTreeMap::new(),
            DATE,
        )
        .unwrap();
    let expected = json!([{
        "law_id": "wet_hoofd",
        "article": "1",
        "term": "grond_aanwezig",
        "filled_by": {"regulation": {"law_id": "beleid_hoofd", "article": "1"}},
    }]);
    assert_eq!(serde_json::to_value(&result.open_terms).unwrap(), expected);
    let receipt = service.build_receipt_with_outputs(&result, &parameters, DATE, &[]);
    assert_eq!(
        serde_json::to_value(&receipt).unwrap()["results"]["open_terms"],
        expected
    );
}

#[test]
fn a_regulation_not_in_force_leaves_the_finding_to_fill_the_term() {
    let later = HOOFD_BELEID.replace("valid_from: '2025-01-01'", "valid_from: '2026-01-01'");
    let found = filled(&[&later], json!({"grond_aanwezig": true}));
    assert_eq!(found, vec![grond_aanwezig(FilledBy::Finding)]);
}

/// A term of an article the executed one calls is reported with the
/// article that declares it: here the decision called anew, whose finding
/// comes with the decision value.
#[test]
fn the_execution_reports_a_term_of_an_article_it_calls() {
    let service = anew_service();
    let result = service
        .evaluate_law_with_findings(
            "wet_hoofd",
            &["heroverwogen"],
            params(json!({"besluit": hoofd_besluit(true)})),
            BTreeMap::new(),
            DATE,
        )
        .unwrap();
    assert_eq!(result.open_terms, vec![grond_aanwezig(FilledBy::Finding)]);
    assert_eq!(
        serde_json::to_value(&result.open_terms[0]).unwrap(),
        json!({
            "law_id": "wet_hoofd",
            "article": "1",
            "term": "grond_aanwezig",
            "filled_by": "finding",
        })
    );
}

// --- A decision called anew, with a policy that judges per case ------------

/// The head grants leave of at most ten days where the ground applies; the
/// law reads the ground only for such a leave. Article 3 calls a decision
/// value anew.
const KORT: &str = r#"
$id: wet_kort
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het hoofd verleent verlof van ten hoogste tien dagen als de grond aanwezig is.
    machine_readable:
      open_terms:
        - id: grond_aanwezig
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: dagen
            type: number
        output:
          - name: verleend
            type: boolean
        actions:
          - output: verleend
            value:
              operation: AND
              conditions:
                - operation: LESS_THAN_OR_EQUAL
                  subject: $dagen
                  value: 10
                - $grond_aanwezig
  - number: '3'
    text: Het besluit opnieuw.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: decision
        input:
          - name: opnieuw
            type: decision
            nullable: true
            source:
              article: $besluit
        output:
          - name: heroverwogen
            type: decision
            nullable: true
        actions:
          - output: heroverwogen
            value: $opnieuw
"#;

/// A policy that fills the ground and leaves it to the head with an open
/// term of its own (RFC-048 §2).
const KORT_BELEID: &str = r#"
$id: beleid_kort
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.invalid/beleid-kort
articles:
  - number: '1'
    text: Over de grond oordeelt het hoofd.
    machine_readable:
      open_terms:
        - id: grond_volgens_hoofd
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      implements:
        - {law: wet_kort, article: '1', open_term: grond_aanwezig}
      execution:
        output:
          - {name: grond_aanwezig, type: boolean}
        actions:
          - {output: grond_aanwezig, value: $grond_volgens_hoofd}
"#;

fn kort_besluit(dagen: i64, grond: bool) -> serde_json::Value {
    json!({
        "basis": "wet_kort#1",
        "taken_on": "wet_kort#1",
        "fields": {"dagen": dagen},
        "findings": {"grond_volgens_hoofd": grond},
        "outputs": {"verleend": dagen <= 10 && grond},
    })
}

/// Article 3 of [`KORT`] on `besluit`, with the caller's `findings`: the
/// decision value it gives back, and the trace.
fn kort_anew(besluit: serde_json::Value, findings: serde_json::Value) -> (Value, String) {
    let result = kort_anew_result(besluit, findings).unwrap();
    let trace = result.trace.as_ref().unwrap().render_box_drawing();
    (result.outputs["heroverwogen"].clone(), trace)
}

fn kort_anew_result(
    besluit: serde_json::Value,
    findings: serde_json::Value,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let mut service = LawExecutionService::new();
    service.load_law(KORT).unwrap();
    service.load_law(KORT_BELEID).unwrap();
    service.evaluate_law_with_trace_and_findings(
        "wet_kort",
        &["heroverwogen"],
        params(json!({ "besluit": besluit })),
        params(findings),
        DATE,
    )
}

fn findings_of(value: &Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap()["findings"].clone()
}

fn verleend(value: &Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap()["outputs"]["verleend"].clone()
}

#[test]
fn a_decision_value_keeps_the_finding_on_its_policys_own_term() {
    let (value, trace) = kort_anew(kort_besluit(5, true), json!({}));
    assert_eq!(verleend(&value), json!(true));
    assert_eq!(findings_of(&value), json!({"grond_volgens_hoofd": true}));
    assert!(
        trace.contains("the finding on grond_volgens_hoofd applies to an article the call runs"),
        "{trace}"
    );
}

#[test]
fn a_finding_of_the_caller_on_a_term_the_policy_fills_fails_a_call_anew() {
    // Twenty days: the law does not read the ground, yet a finding on it
    // is applied by no article, since the policy fills it (RFC-048 §2).
    let e = kort_anew_result(kort_besluit(20, true), json!({"grond_aanwezig": true})).unwrap_err();
    assert!(e.to_string().contains("'grond_aanwezig'"), "{e}");
}

#[test]
fn a_finding_of_the_caller_on_the_policys_own_term_wins_and_is_kept() {
    let (value, _) = kort_anew(kort_besluit(5, true), json!({"grond_volgens_hoofd": false}));
    assert_eq!(verleend(&value), json!(false));
    assert_eq!(findings_of(&value), json!({"grond_volgens_hoofd": false}));
}

#[test]
fn a_finding_on_a_policys_own_term_the_call_does_not_reach_is_dropped_quietly() {
    // Twenty days: the law does not read the ground, so the policy does not
    // run. Its finding is not part of the new decision value, and is not
    // traced as one the article does not declare.
    let (value, trace) = kort_anew(kort_besluit(20, true), json!({}));
    assert_eq!(verleend(&value), json!(false));
    assert_eq!(findings_of(&value), json!({}));
    assert!(
        !trace.contains("does not declare grond_volgens_hoofd"),
        "{trace}"
    );
    assert!(
        !trace.contains("applies to an article the call runs"),
        "{trace}"
    );
}

/// A law that gives an amount but decides nothing, with an input it reads
/// from another law, and the law that reads it through a reference. The
/// source of `inkomen` has no value of its own here.
const REGELING: &str = r#"
$id: wet_bedrag
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De aanspraak is tien procent van het inkomen.
    machine_readable:
      execution:
        input:
          - name: inkomen
            type: number
            source:
              regulation: wet_register
              output: geregistreerd_inkomen
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value:
              operation: DIVIDE
              values: [$inkomen, 10]
"#;

const REGISTER: &str = r#"
$id: wet_register
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het register houdt het inkomen bij.
    machine_readable:
      execution:
        output:
          - name: geregistreerd_inkomen
            type: number
        actions:
          - output: geregistreerd_inkomen
            value: 1000
"#;

const AANVRAAG: &str = r#"
$id: wet_aanvraag
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De tegemoetkoming is het bedrag uit het genoemde artikel.
    machine_readable:
      execution:
        parameters:
          - name: gevraagd
            type: string
        input:
          - name: gelezen
            type: number
            source:
              article: $gevraagd
              output: bedrag
        output:
          - name: toegekend
            type: number
        actions:
          - output: toegekend
            value: $gelezen
"#;

fn verwijzing() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in [REGELING, REGISTER, AANVRAAG] {
        service.load_law(law).unwrap();
    }
    service
}

#[test]
fn a_reference_reads_a_named_output_of_an_article_that_takes_no_decision() {
    let result = verwijzing()
        .evaluate_law_output(
            "wet_aanvraag",
            "toegekend",
            params(json!({"gevraagd": "wet_bedrag#1"})),
            DATE,
        )
        .unwrap();
    assert_eq!(
        result.outputs["toegekend"],
        Value::Decimal(Decimal::from(100))
    );
}

#[test]
fn an_output_the_article_without_a_decision_lacks_is_refused() {
    let e = verwijzing()
        .evaluate_law_output(
            "wet_aanvraag",
            "toegekend",
            params(json!({"gevraagd": "wet_register#1"})),
            DATE,
        )
        .unwrap_err();
    assert!(e.to_string().contains("has no such output"), "{e}");
}

#[test]
fn a_finding_under_the_name_of_an_input_of_the_called_article_wins_over_its_source() {
    let result = verwijzing()
        .evaluate_law_with_findings(
            "wet_aanvraag",
            &["toegekend"],
            params(json!({"gevraagd": "wet_bedrag#1"})),
            params(json!({"inkomen": 500})),
            DATE,
        )
        .unwrap();
    assert_eq!(
        result.outputs["toegekend"],
        Value::Decimal(Decimal::from(50))
    );
}

#[test]
fn a_finding_no_called_article_reads_is_still_not_asked() {
    let e = verwijzing()
        .evaluate_law_with_findings(
            "wet_aanvraag",
            &["toegekend"],
            params(json!({"gevraagd": "wet_bedrag#1"})),
            params(json!({"iets_anders": 500})),
            DATE,
        )
        .unwrap_err();
    assert!(matches!(e, EngineError::FindingsNotAsked { .. }), "{e}");
}
