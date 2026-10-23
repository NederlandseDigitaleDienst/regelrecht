//! `produces` in either form (schema v0.8.1, RFC-045): one object, or a
//! list of items that each say with `type` what they are, and own their
//! parameters and outputs. Readers go through the article's helpers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_law_model::ArticleBasedLaw;

fn law(produces: &str) -> Result<ArticleBasedLaw, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(&format!(
        r#"
$id: wet_verlof
regulatory_layer: WET
publication_date: '2026-10-14'
articles:
  - number: '1'
    text: Het hoofd verleent op aanvraag verlof.
    machine_readable:
      execution:
        produces:
{produces}
        parameters:
          - {{name: dagen, type: number}}
          - {{name: reden, type: string}}
        output:
          - {{name: verlof_verleend, type: boolean}}
          - {{name: ontvangen, type: boolean}}
"#
    ))
}

const LIST: &str = "          - type: AANVRAAG
            requests: wet_verlof#1
            parameters: [dagen]
          - type: BESCHIKKING
            decision_type: TOEKENNING
            outputs: [verlof_verleend]";

#[test]
fn a_list_item_says_what_it_is_with_type_and_owns_its_parameters_and_outputs() {
    let law = law(LIST).unwrap();
    let article = &law.articles[0];
    assert!(article.produces_list());
    let submission = article.get_submission().unwrap();
    assert_eq!(submission.submission_kind(), Some("AANVRAAG"));
    assert_eq!(submission.legal_character(), None);
    assert_eq!(submission.requests(), Some("wet_verlof#1"));
    assert_eq!(article.parameters_of(submission), ["dagen"]);
    // The submission lists no outputs, so it yields none.
    assert!(article.outputs_of(submission).is_empty());
    let decision = article.get_decision().unwrap();
    assert_eq!(decision.legal_character(), Some("BESCHIKKING"));
    assert_eq!(decision.decision_type(), Some("TOEKENNING"));
    assert_eq!(article.outputs_of(decision), ["verlof_verleend"]);
    assert!(article.parameters_of(decision).is_empty());
}

#[test]
fn an_objection_is_a_submission_and_the_decision_on_it_the_decision() {
    // As Awb 7:11 writes it (RFC-045 §7): the objection, and the decision on it.
    let law = law("          - type: BEZWAAR
            requests: wet_verlof#1
          - type: BESCHIKKING
            outputs: [verlof_verleend]")
    .unwrap();
    let article = &law.articles[0];
    let submission = article.get_submission().unwrap();
    assert_eq!(submission.submission_kind(), Some("BEZWAAR"));
    assert_eq!(submission.legal_character(), None);
    let decision = article.get_decision().unwrap();
    assert_eq!(decision.legal_character(), Some("BESCHIKKING"));
    assert_eq!(decision.submission_kind(), None);
    assert_eq!(article.outputs_of(decision), ["verlof_verleend"]);
}

#[test]
fn an_appeal_is_a_submission_and_the_judgment_on_it_the_decision() {
    // As Awb 8:70 writes it (schema v0.8.10): the appeal, and the judgment.
    let law = law("          - type: BEROEP
            requests: wet_verlof#1
          - type: UITSPRAAK
            outputs: [verlof_verleend]")
    .unwrap();
    let article = &law.articles[0];
    let submission = article.get_submission().unwrap();
    assert_eq!(submission.submission_kind(), Some("BEROEP"));
    assert_eq!(submission.legal_character(), None);
    let decision = article.get_decision().unwrap();
    assert_eq!(decision.legal_character(), Some("UITSPRAAK"));
    assert_eq!(decision.submission_kind(), None);
}

#[test]
fn the_decision_without_outputs_yields_every_output_no_item_lists() {
    let law = law("          - type: AANVRAAG
            requests: wet_verlof#1
            parameters: [dagen]
          - type: BESCHIKKING")
    .unwrap();
    let article = &law.articles[0];
    let submission = article.get_submission().unwrap();
    assert!(article.outputs_of(submission).is_empty());
    let decision = article.get_decision().unwrap();
    assert_eq!(
        article.outputs_of(decision),
        ["verlof_verleend", "ontvangen"]
    );
}

#[test]
fn the_object_form_is_one_item_with_every_parameter_and_output() {
    let law = law("          legal_character: BESCHIKKING\n          submission: {kind: AANVRAAG}")
        .unwrap();
    let article = &law.articles[0];
    assert!(!article.produces_list());
    let item = article.get_submission().unwrap();
    assert_eq!(article.get_decision(), Some(item));
    assert_eq!(article.parameters_of(item), ["dagen", "reden"]);
    assert_eq!(article.outputs_of(item), ["verlof_verleend", "ontvangen"]);
    assert_eq!(item.requests(), None);
}

#[test]
fn each_form_serializes_back_as_written() {
    for produces in [
        LIST,
        "          legal_character: BESCHIKKING\n          submission: {kind: AANVRAAG}",
    ] {
        let law = law(produces).unwrap();
        let again: ArticleBasedLaw =
            serde_yaml_ng::from_str(&serde_yaml_ng::to_string(&law).unwrap()).unwrap();
        assert_eq!(again, law);
    }
}

#[test]
fn a_list_item_refuses_the_fields_of_the_object_form_and_a_requests_that_names_no_article() {
    for (produces, why) in [
        ("          - submission: {kind: AANVRAAG}", "submission"),
        (
            "          - legal_character: BESCHIKKING",
            "legal_character",
        ),
        (
            "          - type: AANVRAAG\n            requests: BESCHIKKING",
            "requests",
        ),
    ] {
        assert!(
            law(produces).is_err(),
            "{why} must not parse in a list item"
        );
    }
}

/// Each form reports its own error, not that the value matches neither form.
#[test]
fn an_error_inside_either_form_is_reported_as_written() {
    for (produces, expected) in [
        (
            "          legal_character: BESCHIKKING\n          submission: {kind: AANVRAAG, soort: x}",
            "unknown field `soort`",
        ),
        (
            "          - type: AANVRAAG\n            soort: x",
            "unknown field `soort`",
        ),
    ] {
        let message = law(produces).unwrap_err().to_string();
        assert!(message.contains(expected), "got: {message}");
        assert!(!message.contains("did not match any variant"), "got: {message}");
    }
}
