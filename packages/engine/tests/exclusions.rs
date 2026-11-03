//! Exclusions and several kinds per item (schema v0.9.0), on synthetic laws.
//!
//! The corpus shows the exclusion by a member law (the Wet op het
//! kindgebonden budget and the Wet kinderopvang on the Awir, in the BDD
//! scenarios next to the Wet kinderopvang). These laws show the two other
//! forms with the same construct, and an item that falls under three general
//! laws at once:
//!
//! - a general law limiting its own reach, as Awb 1:6 does: an exclusion with
//!   a `kind` that some items claim;
//! - a general law switching off another for its members, as AWR 1 lid 3
//!   does for parts of the Awb: an exclusion with a `kind` of regulations;
//! - a vergunning that is a beschikking, is prepared with a public procedure
//!   and falls under an integrity screening: one `type` and two `member_of`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{LawExecutionService, OutputProvenance, Value};
use std::collections::BTreeMap;

const SCHEMA: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.9.0/schema/v0.9.0/schema.json";

/// A general law in the shape of the Awb: it defines the besluit, the
/// beschikking and a besluit prepared with a public procedure, hooks a
/// motivering onto every besluit and a ter-inzage-legging onto the public
/// procedure, and in article 5 excludes its own motivering for acts of
/// prosecution, in the shape of Awb 1:6.
const ALGEMEEN: &str = r#"
$id: algemene_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een besluit is een beslissing. Een beschikking is een besluit dat niet van algemene strekking is.
    machine_readable:
      kinds:
        - id: besluit
        - id: beschikking
          broader: besluit
  - number: '2'
    text: Een besluit berust op een deugdelijke motivering.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            kind: algemene_test_wet/besluit
      execution:
        output:
          - name: motivering_vereist
            type: boolean
        actions:
          - output: motivering_vereist
            value: true
  - number: '3'
    text: Deze afdeling is van toepassing op de voorbereiding van besluiten indien dat bij wettelijk voorschrift is bepaald.
    machine_readable:
      kinds:
        - id: openbaar_voorbereid_besluit
          broader: besluit
  - number: '4'
    text: Het ontwerp van het besluit wordt ter inzage gelegd.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            kind: algemene_test_wet/openbaar_voorbereid_besluit
      execution:
        output:
          - name: ter_inzage_gelegd
            type: boolean
        actions:
          - output: ter_inzage_gelegd
            value: true
  - number: '5'
    text: Artikel 2 is niet van toepassing op de opsporing en vervolging van strafbare feiten.
    machine_readable:
      kinds:
        - id: opsporing_en_vervolging
      excludes:
        - law: algemene_test_wet
          article: '2'
          kind: algemene_test_wet/opsporing_en_vervolging
          legal_text_excerpt: Artikel 2 is niet van toepassing op de opsporing en vervolging van strafbare feiten.
"#;

/// A second general law, in the shape of the Wet Bibob: it hooks an
/// integrity screening onto the beschikkingen of its own kind.
const INTEGRITEIT: &str = r#"
$id: integriteitstoets_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Deze wet is van toepassing op beschikkingen die een vergunning inhouden.
    machine_readable:
      kinds:
        - id: vergunning
          broader: algemene_test_wet/beschikking
  - number: '2'
    text: Het bestuursorgaan kan een vergunning weigeren bij ernstig gevaar voor misbruik.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            kind: integriteitstoets_test_wet/vergunning
      execution:
        output:
          - name: integriteit_getoetst
            type: boolean
        actions:
          - output: integriteit_getoetst
            value: true
"#;

/// A third general law in the shape of the AWR: it defines a kind of
/// regulations and switches article 2 of the general law off for them, as
/// AWR 1 lid 3 switches off titel 5.2 Awb for the heffing of rijksbelastingen.
const BELASTING: &str = r#"
$id: algemene_belasting_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een belastingwet is een wet betreffende de heffing van rijksbelastingen. Met betrekking tot de heffing van rijksbelastingen blijft artikel 2 van de algemene test wet buiten toepassing.
    machine_readable:
      kinds:
        - id: belastingwet
          level: regulation
      excludes:
        - law: algemene_test_wet
          article: '2'
          kind: algemene_belasting_test_wet/belastingwet
          legal_text_excerpt: Met betrekking tot de heffing van rijksbelastingen blijft artikel 2 van de algemene test wet buiten toepassing.
"#;

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in [ALGEMEEN, INTEGRITEIT, BELASTING] {
        service
            .load_law(&format!("$schema: {SCHEMA}\n{}", law.trim_start()))
            .unwrap();
    }
    service
}

/// A law with one article that produces `item`, under `law_top` (such as a
/// `member_of`).
fn member(service: &mut LawExecutionService, id: &str, law_top: &str, item: &str) {
    let yaml = format!(
        r#"$schema: {SCHEMA}
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
{law_top}
articles:
  - number: '1'
    text: Het bestuursorgaan beslist op de heffing, de vergunning of de vervolging die openbaar wordt voorbereid.
    machine_readable:
      execution:
        produces:
{item}
        output:
          - name: beslist
            type: boolean
        actions:
          - output: beslist
            value: true
"#
    );
    service.load_law(&yaml).unwrap();
    assert_eq!(service.resolver().validate_kinds(), Vec::<String>::new());
}

fn outputs(service: &LawExecutionService, law: &str) -> regelrecht_engine::ArticleResult {
    service
        .evaluate_law_output(law, "beslist", BTreeMap::new(), "2025-06-01")
        .unwrap()
}

#[test]
fn an_item_falls_under_three_general_laws_at_once() {
    let mut service = service();
    member(
        &mut service,
        "vergunningen_test_wet",
        "",
        r#"          - type: algemene_test_wet/beschikking
            member_of:
              - kind: algemene_test_wet/openbaar_voorbereid_besluit
                legal_text_excerpt: openbaar wordt voorbereid
              - kind: integriteitstoets_test_wet/vergunning
                legal_text_excerpt: de vergunning"#,
    );
    let result = outputs(&service, "vergunningen_test_wet");
    for name in [
        "motivering_vereist",
        "ter_inzage_gelegd",
        "integriteit_getoetst",
    ] {
        assert_eq!(result.outputs.get(name), Some(&Value::Bool(true)), "{name}");
        assert!(
            matches!(
                result.output_provenance.get(name),
                Some(OutputProvenance::Reactive { .. })
            ),
            "{name}: {:?}",
            result.output_provenance.get(name)
        );
    }
}

#[test]
fn without_a_claim_only_the_hooks_on_its_type_fire() {
    let mut service = service();
    member(
        &mut service,
        "gewone_test_wet",
        "",
        "          - type: algemene_test_wet/beschikking",
    );
    let result = outputs(&service, "gewone_test_wet");
    assert_eq!(
        result.outputs.get("motivering_vereist"),
        Some(&Value::Bool(true))
    );
    assert_eq!(result.outputs.get("ter_inzage_gelegd"), None);
    assert_eq!(result.outputs.get("integriteit_getoetst"), None);
}

/// Awb 1:6: the general law limits its own reach for items of a kind it
/// defines. The excluded hook does not run, and the ground is recorded.
#[test]
fn a_general_law_excludes_its_own_hook_for_a_kind() {
    let mut service = service();
    member(
        &mut service,
        "strafvordering_test_wet",
        "",
        r#"          - type: algemene_test_wet/beschikking
            member_of:
              - kind: algemene_test_wet/opsporing_en_vervolging
                legal_text_excerpt: de vervolging"#,
    );
    let result = outputs(&service, "strafvordering_test_wet");
    assert_eq!(result.outputs.get("motivering_vereist"), None);
    match result.output_provenance.get("motivering_vereist") {
        Some(OutputProvenance::Voided {
            law_id,
            article,
            grounds,
        }) => {
            assert_eq!(law_id, "algemene_test_wet");
            assert_eq!(article, "5");
            assert!(grounds.as_deref().unwrap().contains("opsporing"));
        }
        other => panic!("expected the ground of the exclusion, got {other:?}"),
    }
}

/// AWR 1 lid 3: one general law switches a hook of another off for the
/// regulations of its kind, and only for those.
#[test]
fn a_general_law_excludes_a_hook_of_another_for_its_members() {
    let mut service = service();
    member(
        &mut service,
        "heffings_test_wet",
        "member_of:\n  - kind: algemene_belasting_test_wet/belastingwet\n    legal_text_excerpt: de heffing",
        "          - type: algemene_test_wet/beschikking",
    );
    member(
        &mut service,
        "andere_test_wet",
        "",
        "          - type: algemene_test_wet/beschikking",
    );
    let excluded = outputs(&service, "heffings_test_wet");
    assert_eq!(excluded.outputs.get("motivering_vereist"), None);
    assert!(matches!(
        excluded.output_provenance.get("motivering_vereist"),
        Some(OutputProvenance::Voided { law_id, .. }) if law_id == "algemene_belasting_test_wet"
    ));
    let applied = outputs(&service, "andere_test_wet");
    assert_eq!(
        applied.outputs.get("motivering_vereist"),
        Some(&Value::Bool(true))
    );
}

/// An exclusion whose words are not in the article, or whose target is no
/// hook, is reported.
#[test]
fn a_wrong_exclusion_is_reported() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: foute_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Artikel 1 van de algemene test wet is niet van toepassing.
    machine_readable:
      excludes:
        - law: algemene_test_wet
          article: '1'
          when: bestaat_niet
          legal_text_excerpt: Artikel 3 is niet van toepassing.
"#
        ))
        .unwrap();
    let findings = service.resolver().validate_kinds();
    let at = "foute_test_wet article 1: excludes algemene_test_wet#1";
    assert!(
        findings.contains(&format!(
            "{at}: the excerpt is not in the text of the article"
        )),
        "{findings:?}"
    );
    assert!(
        findings.contains(&format!("{at}, which declares no hooks")),
        "{findings:?}"
    );
    assert!(
        findings.contains(&format!(
            "{at} when bestaat_niet, which the article does not output"
        )),
        "{findings:?}"
    );
}

/// An exclusion from a regulation that is not in force yet does not apply:
/// the general rule does, and the hook fires. Once it is in force, it
/// applies.
#[test]
fn an_exclusion_applies_only_while_in_force() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: latere_test_wet
regulatory_layer: WET
publication_date: '2030-01-01'
valid_from: '2030-01-01'
articles:
  - number: '1'
    text: Artikel 2 van de algemene test wet is niet van toepassing op een vergunning.
    machine_readable:
      excludes:
        - law: algemene_test_wet
          article: '2'
          kind: integriteitstoets_test_wet/vergunning
          legal_text_excerpt: Artikel 2 van de algemene test wet is niet van toepassing op een vergunning.
"#
        ))
        .unwrap();
    member(
        &mut service,
        "vergunningen_test_wet",
        "",
        "          - type: integriteitstoets_test_wet/vergunning",
    );
    let before = outputs(&service, "vergunningen_test_wet");
    assert_eq!(
        before.outputs.get("motivering_vereist"),
        Some(&Value::Bool(true))
    );
    let after = service
        .evaluate_law_output(
            "vergunningen_test_wet",
            "beslist",
            BTreeMap::new(),
            "2031-01-01",
        )
        .unwrap();
    assert_eq!(after.outputs.get("motivering_vereist"), None);
    assert!(matches!(
        after.output_provenance.get("motivering_vereist"),
        Some(OutputProvenance::Voided { law_id, .. }) if law_id == "latere_test_wet"
    ));
}
