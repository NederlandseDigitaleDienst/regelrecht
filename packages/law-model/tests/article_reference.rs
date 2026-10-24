//! `established_by` names an article (`articleReference`, RFC-045):
//! `<regulation>#<article>` without a paragraph.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_law_model::{is_article_reference, is_variable_reference, ArticleBasedLaw};

fn law_established_by(reference: &str) -> Result<ArticleBasedLaw, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(&format!(
        r#"
$id: test_established_by
regulatory_layer: WET
publication_date: '2026-10-13'
articles:
  - number: '1'
    text: Een hook op de aanvraag van één artikel.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {{submission: AANVRAAG, established_by: '{reference}'}}
"#
    ))
}

#[test]
fn an_article_reference_names_an_article_without_a_paragraph() {
    for ok in [
        "leerplichtwet_1969#13a",
        "algemene_wet_bestuursrecht#4:13",
        "kieswet#G 1",
        "1e_wet#2.3",
    ] {
        assert!(is_article_reference(ok), "{ok}");
    }
    for refused in [
        "algemene_wet_bestuursrecht#4:13 lid 1",
        "kieswet#G 1 lid 2",
        "kieswet#G 1 ",
        "Kieswet#G 1",
        "kieswet",
        "#13a",
    ] {
        assert!(!is_article_reference(refused), "{refused}");
    }
}

#[test]
fn established_by_reads_an_article_with_a_space_and_refuses_a_paragraph() {
    let law = law_established_by("kieswet#G 1").expect("an article with a space parses");
    let hooks = law.articles[0].get_hooks().expect("hooks");
    assert_eq!(
        hooks[0].applies_to.established_by.as_deref(),
        Some("kieswet#G 1")
    );
    let e = law_established_by("algemene_wet_bestuursrecht#4:13 lid 1")
        .unwrap_err()
        .to_string();
    assert!(e.contains("without a paragraph"), "{e}");
}

fn law_with_moment(legal_basis: &str) -> Result<ArticleBasedLaw, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(&format!(
        r#"
$id: test_moment
regulatory_layer: WET
publication_date: '2026-10-13'
articles:
  - number: '1'
    text: De ontvangst telt.
    machine_readable:
      execution:
        produces:
          moment:
            parameter: datum_ontvangst
            legal_basis: {legal_basis}
"#
    ))
}

/// `produces.moment.legal_basis` cites a provision the way every other
/// `legal_basis` on a field or an action does; there is no string form.
#[test]
fn the_moment_cites_its_provision_as_a_legal_basis_object() {
    let law = law_with_moment("{law: Algemene wet bestuursrecht, article: '4:13', paragraph: '1'}")
        .expect("a legalBasis object parses");
    let moment = law.articles[0]
        .get_produced()
        .first()
        .and_then(|p| p.moment())
        .expect("moment");
    assert_eq!(moment.legal_basis.article.as_deref(), Some("4:13"));
    assert_eq!(moment.legal_basis.paragraph.as_deref(), Some("1"));
    assert!(law_with_moment("'algemene_wet_bestuursrecht#4:13 lid 1'").is_err());
}

/// `source.article` takes an article reference or a variable reference
/// (`variableReference`, schema v0.8.4), `parameters_from` only the latter.
#[test]
fn a_variable_reference_is_a_dollar_and_a_dotted_name() {
    for ok in ["$besluit", "$besluit.basis", "$_x.y_2", "$a.b.c"] {
        assert!(is_variable_reference(ok), "{ok}");
    }
    for bad in [
        "$", "besluit", "$1x", "$a..b", "$a.", "$a b", "$a.1", "$a-b",
    ] {
        assert!(!is_variable_reference(bad), "{bad}");
    }
}
