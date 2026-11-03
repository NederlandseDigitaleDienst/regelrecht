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

/// A kind of organizations, in the shape of the Kaderwet zbo: a hook on it
/// fires on what every member organization decides, and an instellingswet
/// excludes it for its own organization in some cases.
const KADER: &str = r#"
$id: kader_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een zelfstandig bestuursorgaan is een bestuursorgaan dat niet ondergeschikt is aan een minister.
    machine_readable:
      kinds:
        - id: zelfstandig_bestuursorgaan
          level: organization
  - number: '2'
    text: Onze Minister kan een besluit van een zelfstandig bestuursorgaan vernietigen.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            kind: kader_test_wet/zelfstandig_bestuursorgaan
      execution:
        output:
          - name: vernietigbaar
            type: boolean
        actions:
          - output: vernietigbaar
            value: true
"#;

/// An instellingswet: it establishes an organization that is a member, and
/// an article in which that organization decides. With `uitgesloten` the
/// exclusion of article 3 holds.
fn instelling(id: &str, organization: &str, member: bool) -> String {
    let member_of = if member {
        "\n          member_of:\n            - kind: kader_test_wet/zelfstandig_bestuursorgaan\n              legal_text_excerpt: is niet ondergeschikt aan een minister"
    } else {
        ""
    };
    format!(
        r#"$schema: {SCHEMA}
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Er is een {organization}. Zij is niet ondergeschikt aan een minister.
    machine_readable:
      organizations:
        - id: orgaan
          name: {organization}{member_of}
  - number: '2'
    text: De {organization} beslist.
    machine_readable:
      competent_authority:
        name: {organization}
      execution:
        produces:
          - type: algemene_test_wet/beschikking
        parameters:
          - name: uitgesloten
            type: boolean
            required: false
        output:
          - name: beslist
            type: boolean
        actions:
          - output: beslist
            value: true
  - number: '3'
    text: Artikel 2 van de kader test wet is in bepaalde gevallen niet van toepassing.
    machine_readable:
      excludes:
        - law: kader_test_wet
          article: '2'
          kind: {id}/orgaan
          when: geval
          legal_text_excerpt: Artikel 2 van de kader test wet is in bepaalde gevallen niet van toepassing.
      execution:
        parameters:
          - name: uitgesloten
            type: boolean
            required: false
        output:
          - name: geval
            type: boolean
        actions:
          - output: geval
            value:
              operation: EQUALS
              subject: $uitgesloten
              value: true
"#
    )
}

#[test]
fn a_hook_on_a_kind_of_organizations_fires_on_what_a_member_decides() {
    let mut service = service();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KADER.trim_start()))
        .unwrap();
    service
        .load_law(&instelling("zbo_test_wet", "Testautoriteit", true))
        .unwrap();
    service
        .load_law(&instelling("dienst_test_wet", "Testdienst", false))
        .unwrap();
    assert_eq!(service.resolver().validate_kinds(), Vec::<String>::new());

    let run = |law: &str, excluded: bool| {
        let mut params = BTreeMap::new();
        params.insert("uitgesloten".to_string(), Value::Bool(excluded));
        service
            .evaluate_law_output(law, "beslist", params, "2025-06-01")
            .unwrap()
    };
    let member = run("zbo_test_wet", false);
    assert_eq!(
        member.outputs.get("vernietigbaar"),
        Some(&Value::Bool(true))
    );
    let excluded = run("zbo_test_wet", true);
    assert_eq!(excluded.outputs.get("vernietigbaar"), None);
    assert!(matches!(
        excluded.output_provenance.get("vernietigbaar"),
        Some(OutputProvenance::Voided { .. })
    ));
    let other = run("dienst_test_wet", false);
    assert_eq!(other.outputs.get("vernietigbaar"), None);
    assert_eq!(other.output_provenance.get("vernietigbaar"), None);
}

/// Every claim of membership and every exclusion is held against what is
/// defined and against the words of the regulation.
#[test]
fn wrong_memberships_are_reported() {
    let mut service = service();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KADER.trim_start()))
        .unwrap();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: claims_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
member_of:
  - kind: kader_test_wet/zelfstandig_bestuursorgaan
    legal_text_excerpt: Het bureau beslist.
  - kind: kader_test_wet/zelfstandig_bestuursorgaan
articles:
  - number: '1'
    text: Er is een Testbureau. Het bureau beslist.
    machine_readable:
      organizations:
        - id: bureau
          name: Testbureau
          member_of:
            - kind: kader_test_wet/bestaat_niet
              legal_text_excerpt: Het bureau beslist.
            - kind: algemene_test_wet/besluit
              legal_text_excerpt: Het bureau beslist.
            - kind: kader_test_wet/zelfstandig_bestuursorgaan
            - kind: kader_test_wet/zelfstandig_bestuursorgaan
              legal_text_excerpt: staat hier niet
        - id: tweede_bureau
          name: Testbureau
      competent_authority:
        name: Testbureau
      execution:
        produces:
          - type: algemene_test_wet/beschikking
            member_of:
              - kind: algemene_test_wet/bestaat_niet
                legal_text_excerpt: Het bureau beslist.
              - kind: kader_test_wet/zelfstandig_bestuursorgaan
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
    text: Artikel 2 van de kader test wet is niet van toepassing.
    machine_readable:
      excludes:
        - law: kader_test_wet
          article: '2'
          kind: kader_test_wet/bestaat_niet
          when: geval
          legal_text_excerpt: Artikel 2 van de kader test wet is niet van toepassing.
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
    let org = "claims_test_wet article 1: organization bureau";
    for expected in [
        format!("{item}: member_of algemene_test_wet/bestaat_niet is defined nowhere"),
        format!("{item}: member_of kader_test_wet/zelfstandig_bestuursorgaan names a kind of organizations, not of items"),
        format!("{item}: member_of algemene_test_wet/besluit quotes no words of claims_test_wet and cites no provision"),
        format!("{item}: member_of algemene_test_wet/besluit: the excerpt is not in the text of claims_test_wet"),
        format!("{org}: member_of kader_test_wet/bestaat_niet is defined nowhere"),
        format!("{org}: member_of algemene_test_wet/besluit names a kind of items, not of organizations"),
        format!("{org}: member_of kader_test_wet/zelfstandig_bestuursorgaan quotes no words of claims_test_wet and cites no provision"),
        format!("{org}: member_of kader_test_wet/zelfstandig_bestuursorgaan: the excerpt is not in the text of claims_test_wet"),
        format!("{org}: another organization is named Testbureau"),
        "claims_test_wet: member_of kader_test_wet/zelfstandig_bestuursorgaan names a kind of organizations, not of regulations".to_string(),
        "claims_test_wet: member_of kader_test_wet/zelfstandig_bestuursorgaan quotes no words of claims_test_wet and cites no provision".to_string(),
        "claims_test_wet article 2: excludes kader_test_wet#2 for kind kader_test_wet/bestaat_niet, which is defined nowhere".to_string(),
        "claims_test_wet article 2: excludes kader_test_wet#2 when geval, which is not a boolean".to_string(),
    ] {
        assert!(findings.contains(&expected), "missing {expected:?} in {findings:#?}");
    }
}

/// A `when` that yields something other than true, false or nothing is an
/// error, not a silent "no exclusion".
#[test]
fn a_when_that_is_no_truth_value_is_an_error() {
    let mut service = service();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KADER.trim_start()))
        .unwrap();
    let law = instelling("tekst_test_wet", "Tekstautoriteit", true).replace(
        "            value:\n              operation: EQUALS\n              subject: $uitgesloten\n              value: true",
        "            value: misschien",
    )
    .replace(
        "          - name: geval\n            type: boolean",
        "          - name: geval\n            type: string",
    );
    service.load_law(&law).unwrap();
    let error = service
        .evaluate_law_output("tekst_test_wet", "beslist", BTreeMap::new(), "2025-06-01")
        .unwrap_err();
    assert!(
        error.to_string().contains("instead of true or false"),
        "{error}"
    );
}

/// Loading a regulation again without its exclusion takes the exclusion
/// away: the index does not keep what the newest version no longer says.
#[test]
fn a_reloaded_regulation_without_its_exclusion_excludes_nothing() {
    let mut service = service();
    let strafvordering = |claim: bool| {
        let member_of = if claim {
            "\n            member_of:\n              - kind: algemene_test_wet/opsporing_en_vervolging\n                legal_text_excerpt: de vervolging"
        } else {
            ""
        };
        format!("          - type: algemene_test_wet/beschikking{member_of}")
    };
    member(&mut service, "herlaad_test_wet", "", &strafvordering(true));
    assert_eq!(
        outputs(&service, "herlaad_test_wet")
            .outputs
            .get("motivering_vereist"),
        None
    );
    // The general law again, with article 5 but without its exclusion.
    let start = ALGEMEEN.find("      excludes:").unwrap();
    let without = format!("{}\n", &ALGEMEEN[..start].trim_end());
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", without.trim_start()))
        .unwrap();
    assert_eq!(
        outputs(&service, "herlaad_test_wet")
            .outputs
            .get("motivering_vereist"),
        Some(&Value::Bool(true))
    );
}

/// An override on the output of a hook receives the parameters it declares
/// itself, when it applies: the hook article asks for none of them.
#[test]
fn an_override_on_a_hook_output_receives_its_own_parameters() {
    let mut service = service();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KADER.trim_start()))
        .unwrap();
    let law = instelling("afwijkende_test_wet", "Afwijkende autoriteit", true).replace(
        "  - number: '3'",
        r#"  - number: '4'
    text: In afwijking van artikel 2 van de kader test wet kan de minister alleen een algemeen besluit vernietigen.
    machine_readable:
      overrides:
        - law: kader_test_wet
          article: '2'
          output: vernietigbaar
      execution:
        parameters:
          - name: algemeen
            type: boolean
            required: false
        output:
          - name: vernietigbaar
            type: boolean
        actions:
          - output: vernietigbaar
            value:
              operation: EQUALS
              subject: $algemeen
              value: true
  - number: '3'"#,
    );
    service.load_law(&law).unwrap();
    let run = |algemeen: bool| {
        let mut params = BTreeMap::new();
        params.insert("algemeen".to_string(), Value::Bool(algemeen));
        service
            .evaluate_law_output("afwijkende_test_wet", "beslist", params, "2025-06-01")
            .unwrap()
    };
    assert_eq!(
        run(true).outputs.get("vernietigbaar"),
        Some(&Value::Bool(true))
    );
    let narrowed = run(false);
    assert_eq!(
        narrowed.outputs.get("vernietigbaar"),
        Some(&Value::Bool(false))
    );
    assert!(matches!(
        narrowed.output_provenance.get("vernietigbaar"),
        Some(OutputProvenance::Override { .. })
    ));
}

/// A hook may name one organization rather than a kind of them, and an
/// organization that is unloaded no longer draws its hooks.
#[test]
fn a_hook_on_one_organization_fires_until_it_is_unloaded() {
    let mut service = service();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: toezicht_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een besluit van de Testautoriteit wordt gemeld.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            kind: zbo_test_wet/orgaan
      execution:
        output:
          - name: gemeld
            type: boolean
        actions:
          - output: gemeld
            value: true
"#
        ))
        .unwrap();
    service
        .load_law(&format!("$schema: {SCHEMA}\n{}", KADER.trim_start()))
        .unwrap();
    service
        .load_law(&instelling("zbo_test_wet", "Testautoriteit", true))
        .unwrap();
    service
        .load_law(&instelling("dienst_test_wet", "Testdienst", false))
        .unwrap();
    assert_eq!(service.resolver().validate_kinds(), Vec::<String>::new());
    let gemeld = |service: &LawExecutionService, law: &str| {
        service
            .evaluate_law_output(law, "beslist", BTreeMap::new(), "2025-06-01")
            .unwrap()
            .outputs
            .get("gemeld")
            .cloned()
    };
    assert_eq!(gemeld(&service, "zbo_test_wet"), Some(Value::Bool(true)));
    assert_eq!(gemeld(&service, "dienst_test_wet"), None);
    // Unloading the regulation that establishes an organization takes the
    // organization away; its name in a competent_authority of another
    // regulation is then only a name.
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: orgaan_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Er is een Losse autoriteit. Zij is niet ondergeschikt aan een minister.
    machine_readable:
      organizations:
        - id: orgaan
          name: Losse autoriteit
          member_of:
            - kind: kader_test_wet/zelfstandig_bestuursorgaan
              legal_text_excerpt: Zij is niet ondergeschikt aan een minister.
"#
        ))
        .unwrap();
    service
        .load_law(&format!(
            r#"$schema: {SCHEMA}
$id: losse_besluit_test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De Losse autoriteit beslist.
    machine_readable:
      competent_authority:
        name: Losse autoriteit
      execution:
        produces:
          - type: algemene_test_wet/beschikking
        output:
          - name: beslist
            type: boolean
        actions:
          - output: beslist
            value: true
"#
        ))
        .unwrap();
    let vernietigbaar = |service: &LawExecutionService| {
        service
            .evaluate_law_output(
                "losse_besluit_test_wet",
                "beslist",
                BTreeMap::new(),
                "2025-06-01",
            )
            .unwrap()
            .outputs
            .get("vernietigbaar")
            .cloned()
    };
    assert_eq!(vernietigbaar(&service), Some(Value::Bool(true)));
    assert!(service.unload_law("orgaan_test_wet"));
    assert_eq!(vernietigbaar(&service), None);
}
