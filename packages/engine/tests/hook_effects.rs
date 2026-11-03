//! Overrides on hook outputs, hooks that void, and several kinds per item
//! (schema v0.9.0), on synthetic laws.
//!
//! The corpus shows a member law setting aside a hook of a general law with
//! an override that voids (the Wet op het kindgebonden budget and the Wet
//! kinderopvang on the Awir, in the BDD scenarios next to the Wet
//! kinderopvang). These laws show the rest: an override that applies only in
//! some cases (`when`), an override from a law that another law calls on the
//! way, a hook that takes away what a decision yields, and an item that
//! falls under three general laws at once.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{LawExecutionService, OutputProvenance, Value};
use std::collections::BTreeMap;

const SCHEMA: &str = "https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.9.0/schema/v0.9.0/schema.json";

/// A general law in the shape of the Awb: it defines the besluit, the
/// beschikking and a besluit prepared with a public procedure, hooks a
/// motivering onto every besluit and a ter-inzage-legging onto the public
/// procedure.
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
/// regulations.
const BELASTING: &str = r#"
$id: algemene_belasting_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een belastingwet is een wet betreffende de heffing van rijksbelastingen.
    machine_readable:
      kinds:
        - id: belastingwet
          level: regulation
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

/// Every claim of membership and every conditional override is held against what is
/// defined and against the words of the regulation.
#[test]
fn wrong_memberships_are_reported() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: claims_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
member_of:
  - kind: algemene_test_wet/besluit
    legal_text_excerpt: Het bureau beslist.
  - kind: algemene_belasting_test_wet/belastingwet
articles:
  - number: '1'
    text: Het bureau beslist.
    machine_readable:
      execution:
        produces:
          - type: algemene_test_wet/beschikking
            member_of:
              - kind: algemene_test_wet/bestaat_niet
                legal_text_excerpt: Het bureau beslist.
              - kind: algemene_belasting_test_wet/belastingwet
                legal_text_excerpt: Het bureau beslist.
              - kind: algemene_test_wet/besluit
              - kind: algemene_test_wet/besluit
                legal_text_excerpt: staat hier niet
        output:
          - name: beslist
            type: string
        actions:
          - output: beslist
            value: ja
  - number: '2'
    text: Artikel 2 van de algemene test wet is niet van toepassing.
    machine_readable:
      overrides:
        - law: algemene_test_wet
          article: '2'
          output: motivering_vereist
          voids: true
          when: geval
          legal_text_excerpt: Artikel 2 van de algemene test wet is niet van toepassing.
      execution:
        output:
          - name: geval
            type: string
        actions:
          - output: geval
            value: ja
"#
        ))
        .unwrap();
    let findings = service.resolver().validate_kinds();
    let item = "claims_test_wet article 1";
    for expected in [
        "claims_test_wet: member_of algemene_test_wet/besluit names a kind of items, not of regulations".to_string(),
        "claims_test_wet: member_of algemene_belasting_test_wet/belastingwet quotes no words of claims_test_wet and cites no provision".to_string(),
        format!("{item}: member_of algemene_test_wet/bestaat_niet is defined nowhere"),
        format!("{item}: member_of algemene_belasting_test_wet/belastingwet names a kind of regulations, not of items"),
        format!("{item}: member_of algemene_test_wet/besluit quotes no words of claims_test_wet and cites no provision"),
        format!("{item}: member_of algemene_test_wet/besluit: the excerpt is not in the text of claims_test_wet"),
        "claims_test_wet article 2: overrides algemene_test_wet#2 motivering_vereist when geval, which is not a boolean".to_string(),
    ] {
        assert!(findings.contains(&expected), "missing {expected:?} in {findings:#?}");
    }
}

/// A `when` that yields something other than true, false or nothing is an
/// error, not a silent "the override does not apply".
#[test]
fn a_when_that_is_no_truth_value_is_an_error() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: tekst_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan beslist.
    machine_readable:
      execution:
        produces:
          - type: algemene_test_wet/beschikking
        output:
          - name: beslist
            type: boolean
        actions:
          - output: beslist
            value: true
  - number: '2'
    text: Artikel 2 van de algemene test wet is in bepaalde gevallen niet van toepassing.
    machine_readable:
      overrides:
        - law: algemene_test_wet
          article: '2'
          output: motivering_vereist
          voids: true
          when: geval
          legal_text_excerpt: Artikel 2 van de algemene test wet is in bepaalde gevallen niet van toepassing.
      execution:
        output:
          - name: geval
            type: string
        actions:
          - output: geval
            value: misschien
"#
        ))
        .unwrap();
    let error = service
        .evaluate_law_output("tekst_test_wet", "beslist", BTreeMap::new(), "2025-06-01")
        .unwrap_err();
    assert!(
        error.to_string().contains("instead of true or false"),
        "{error}"
    );
}

/// A hook that takes away what the item yields, in the shape of Vw 2000
/// art. 10: lid 1 is the hook with `voids`, and a regulation that uses the
/// room of lid 2 to deviate voids the hook's `when`.
const KOPPELING: &str = r#"
$id: koppeling_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De vreemdeling die geen rechtmatig verblijf heeft, kan geen aanspraak maken op toekenning van verstrekkingen bij beschikking.
    machine_readable:
      kinds:
        - id: toekenning
          broader: algemene_test_wet/beschikking
      hooks:
        - hook_point: post_actions
          applies_to:
            kind: koppeling_test_wet/toekenning
          voids:
            when: geen_aanspraak
            legal_text_excerpt: kan geen aanspraak maken op toekenning van verstrekkingen bij beschikking.
      execution:
        parameters:
          - name: rechtmatig_verblijf
            type: boolean
            required: false
        output:
          - name: geen_aanspraak
            type: boolean
        actions:
          - output: geen_aanspraak
            value:
              operation: EQUALS
              subject: $rechtmatig_verblijf
              value: false
"#;

fn toekenning(service: &mut LawExecutionService, id: &str, deviates: bool) {
    let overrides = if deviates {
        "\n      overrides:\n        - law: koppeling_test_wet\n          article: '1'\n          output: geen_aanspraak\n          voids: true\n          legal_text_excerpt: Artikel 1 van de koppeling test wet is niet van toepassing."
    } else {
        ""
    };
    let text = if deviates {
        "Het bestuursorgaan kent een studiebijdrage toe. Artikel 1 van de koppeling test wet is niet van toepassing."
    } else {
        "Het bestuursorgaan kent een bijdrage toe."
    };
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: {text}
    machine_readable:{overrides}
      execution:
        produces:
          - type: algemene_test_wet/beschikking
            member_of:
              - kind: koppeling_test_wet/toekenning
                legal_text_excerpt: kent een
        parameters:
          - name: rechtmatig_verblijf
            type: boolean
            required: false
        output:
          - name: bijdrage
            type: number
        actions:
          - output: bijdrage
            value: 100
"#
        ))
        .unwrap();
}

#[test]
fn a_hook_takes_away_what_the_decision_yields_unless_a_law_deviates() {
    let mut service = service();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KOPPELING.trim_start()))
        .unwrap();
    toekenning(&mut service, "bijdrage_test_wet", false);
    toekenning(&mut service, "studie_test_wet", true);
    assert_eq!(service.resolver().validate_kinds(), Vec::<String>::new());
    let run = |law: &str, rechtmatig: bool| {
        let mut params = BTreeMap::new();
        params.insert("rechtmatig_verblijf".to_string(), Value::Bool(rechtmatig));
        service.evaluate_law_output(law, "bijdrage", params, "2025-06-01")
    };

    // With rechtmatig verblijf the decision yields its amount.
    let granted = run("bijdrage_test_wet", true).unwrap();
    assert_eq!(granted.outputs.get("bijdrage"), Some(&Value::Int(100)));

    // Without it the amount does not arise, and asking for it gives the
    // hook's ground.
    let refused = run("bijdrage_test_wet", false).unwrap_err();
    assert!(
        refused.to_string().contains("kan geen aanspraak maken"),
        "{refused}"
    );
    // A law that deviates (Vw 2000 art. 10 lid 2) voids the hook's `when`.
    let deviating = run("studie_test_wet", false).unwrap();
    assert_eq!(deviating.outputs.get("bijdrage"), Some(&Value::Int(100)));
}

/// A hook that voids is held to its words and to a truth value.
#[test]
fn a_wrong_void_on_a_hook_is_reported() {
    let mut service = service();
    let wrong = KOPPELING
        .replace("koppeling_test_wet", "foute_koppeling_test_wet")
        .replace(
            "legal_text_excerpt: kan geen aanspraak maken op toekenning van verstrekkingen bij beschikking.",
            "legal_text_excerpt: staat er niet",
        )
        .replace("when: geen_aanspraak", "when: bestaat_niet");
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", wrong.trim_start()))
        .unwrap();
    let findings = service.resolver().validate_kinds();
    let at = "foute_koppeling_test_wet article 1: hook voids when bestaat_niet";
    assert!(
        findings.contains(&format!(
            "{at}: the excerpt is not in the text of the article"
        )),
        "{findings:?}"
    );
    assert!(
        findings.contains(&format!("{at}, which the article does not output")),
        "{findings:?}"
    );

    // At run time a `when` that is no truth value is an error.
    let mut service = self::service();
    let text = KOPPELING
        .replace("          - name: geen_aanspraak\n            type: boolean", "          - name: geen_aanspraak\n            type: string")
        .replace(
            "            value:\n              operation: EQUALS\n              subject: $rechtmatig_verblijf\n              value: false",
            "            value: misschien",
        );
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", text.trim_start()))
        .unwrap();
    toekenning(&mut service, "bijdrage_test_wet", false);
    let error = service
        .evaluate_law_output(
            "bijdrage_test_wet",
            "bijdrage",
            BTreeMap::new(),
            "2025-06-01",
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("instead of true or false"),
        "{error}"
    );
}

/// What a void takes away is what the decision yields, and only that: the
/// decision remains, with what other hooks attach to it, and an undecided
/// case leaves the decision whole.
#[test]
fn a_void_takes_only_what_the_decision_yields() {
    let mut service = service();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KOPPELING.trim_start()))
        .unwrap();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: gedeeltelijke_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan ontvangt de aanvraag en kent een bijdrage toe.
    machine_readable:
      execution:
        produces:
          - type: algemene_test_wet/beschikking
            outputs: [bijdrage]
            member_of:
              - kind: koppeling_test_wet/toekenning
                legal_text_excerpt: kent een bijdrage toe
        parameters:
          - name: rechtmatig_verblijf
            type: boolean
            required: false
        output:
          - name: bijdrage
            type: number
          - name: ontvangen
            type: boolean
        actions:
          - output: bijdrage
            value: 100
          - output: ontvangen
            value: true
"#
        ))
        .unwrap();
    let run = |params: BTreeMap<String, Value>| {
        service
            .evaluate_law_output("gedeeltelijke_test_wet", "ontvangen", params, "2025-06-01")
            .unwrap()
    };
    let mut params = BTreeMap::new();
    params.insert("rechtmatig_verblijf".to_string(), Value::Bool(false));
    let voided = run(params);
    assert_eq!(voided.outputs.get("ontvangen"), Some(&Value::Bool(true)));
    assert_eq!(voided.outputs.get("bijdrage"), None);
    match voided.output_provenance.get("bijdrage") {
        Some(OutputProvenance::Voided {
            law_id, grounds, ..
        }) => {
            assert_eq!(law_id, "koppeling_test_wet");
            assert!(grounds.as_deref().unwrap().contains("geen aanspraak"));
        }
        other => panic!("expected the ground of the void, got {other:?}"),
    }
    // The general law's hook on every besluit still fires.
    assert_eq!(
        voided.outputs.get("motivering_vereist"),
        Some(&Value::Bool(true))
    );

    // Nothing known about the verblijf: the decision yields what it yields.
    let undecided = service
        .evaluate_law(
            "gedeeltelijke_test_wet",
            &["ontvangen", "bijdrage"],
            BTreeMap::new(),
            "2025-06-01",
        )
        .unwrap();
    assert_eq!(undecided.outputs.get("bijdrage"), Some(&Value::Int(100)));
}

/// A law that reads an output of another law, in the shape of a law that
/// reads the kindgebonden budget: it calls `bijdrage_test_wet` on the way.
const LEZER: &str = r#"
$id: lezer_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het inkomen omvat de bijdrage.
    machine_readable:
      execution:
        input:
          - name: bijdrage
            type: number
            source:
              regulation: bijdrage_test_wet
              output: bijdrage
        output:
          - name: inkomen
            type: number
        actions:
          - output: inkomen
            value: $bijdrage
"#;

/// A hook fires on the item of a law that another law calls on the way,
/// not only on the law the execution starts with: Awir 4 applies wherever
/// an inkomensafhankelijke regeling is applied.
#[test]
fn a_hook_fires_on_an_item_computed_for_another_law() {
    let mut service = service();
    toekenning(&mut service, "bijdrage_test_wet", false);
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", LEZER.trim_start()))
        .unwrap();
    let result = service
        .evaluate_law_output_with_trace("lezer_test_wet", "inkomen", BTreeMap::new(), "2025-06-01")
        .unwrap();
    assert_eq!(result.outputs.get("inkomen"), Some(&Value::Int(100)));
    let trace = result.trace.unwrap().render_box_drawing();
    assert!(
        trace.contains("algemene_test_wet:2"),
        "the hook on every besluit did not fire on the nested beschikking:\n{trace}"
    );
}

/// An override from a law applies while that law executes anywhere in the
/// chain, not only when it started the execution: the member voids the
/// hook's output on its own item, also when another law calls it on the
/// way (the Wkb read by another law). Until v0.9.0 only the law that started
/// the execution counted (RFC-007), and this override was lost.
#[test]
fn an_override_applies_where_its_law_executes_in_the_chain() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: bijdrage_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan kent een bijdrage toe.
    machine_readable:
      execution:
        produces:
          - type: algemene_test_wet/beschikking
        output:
          - name: bijdrage
            type: number
        actions:
          - output: bijdrage
            value: 100
  - number: '2'
    text: Artikel 2 van de algemene test wet is niet van toepassing.
    machine_readable:
      overrides:
        - law: algemene_test_wet
          article: '2'
          output: motivering_vereist
          voids: true
          legal_text_excerpt: Artikel 2 van de algemene test wet is niet van toepassing.
"#
        ))
        .unwrap();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", LEZER.trim_start()))
        .unwrap();
    // Started by the member itself.
    let direct = service
        .evaluate_law_output(
            "bijdrage_test_wet",
            "bijdrage",
            BTreeMap::new(),
            "2025-06-01",
        )
        .unwrap();
    assert_eq!(direct.outputs.get("motivering_vereist"), None);
    assert!(matches!(
        direct.output_provenance.get("motivering_vereist"),
        Some(OutputProvenance::Voided { .. })
    ));
    // Called by another law on the way: the hook fires on the member's
    // item, and the member's override still applies there.
    let nested = service
        .evaluate_law_output_with_trace("lezer_test_wet", "inkomen", BTreeMap::new(), "2025-06-01")
        .unwrap();
    assert_eq!(nested.outputs.get("inkomen"), Some(&Value::Int(100)));
    let trace = nested.trace.unwrap().render_box_drawing();
    assert!(
        trace.contains("bijdrage_test_wet:2 voids algemene_test_wet:2.motivering_vereist"),
        "the member's override did not apply in the nested call:\n{trace}"
    );
}

/// An override with `when` applies only where its condition holds: the Wet
/// kinderopvang sets Awir 5 aside only for changes in the costs per child.
#[test]
fn an_override_with_when_applies_only_where_it_holds() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: voorwaarde_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan beslist.
    machine_readable:
      execution:
        produces:
          - type: algemene_test_wet/beschikking
        output:
          - name: beslist
            type: boolean
        actions:
          - output: beslist
            value: true
  - number: '2'
    text: Artikel 2 van de algemene test wet is niet van toepassing op een spoedeisend besluit.
    machine_readable:
      overrides:
        - law: algemene_test_wet
          article: '2'
          output: motivering_vereist
          voids: true
          when: spoedeisend_besluit
          legal_text_excerpt: Artikel 2 van de algemene test wet is niet van toepassing op een spoedeisend besluit.
      execution:
        parameters:
          - name: spoed
            type: boolean
            required: false
        output:
          - name: spoedeisend_besluit
            type: boolean
        actions:
          - output: spoedeisend_besluit
            value:
              operation: EQUALS
              subject: $spoed
              value: true
"#
        ))
        .unwrap();
    assert_eq!(service.resolver().validate_kinds(), Vec::<String>::new());
    let run = |spoed: Option<bool>| {
        let mut params = BTreeMap::new();
        if let Some(spoed) = spoed {
            params.insert("spoed".to_string(), Value::Bool(spoed));
        }
        service
            .evaluate_law_output("voorwaarde_test_wet", "beslist", params, "2025-06-01")
            .unwrap()
    };
    // The hook article declares no `spoed`; the override gets it anyway.
    let urgent = run(Some(true));
    assert_eq!(urgent.outputs.get("motivering_vereist"), None);
    assert!(matches!(
        urgent.output_provenance.get("motivering_vereist"),
        Some(OutputProvenance::Voided { .. })
    ));
    for spoed in [Some(false), None] {
        assert_eq!(
            run(spoed).outputs.get("motivering_vereist"),
            Some(&Value::Bool(true)),
            "spoed: {spoed:?}"
        );
    }
}
