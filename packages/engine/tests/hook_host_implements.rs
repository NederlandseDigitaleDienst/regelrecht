//! Who fills the open term of an article that fires as a hook (RFC-058).
//!
//! A general law hooks onto every decision of a legal character, and leaves a
//! term open for the special law to fill: the decision period of Awb 4:13 is
//! "the period set by statutory provision", with eight weeks as the fallback.
//! A filling can name the article the hook fires on (`host`). It then applies
//! only while the hook fires on that article, reads that article's
//! parameters, may come from any regulation the delegation gate admits, and
//! goes before a filling that names no host. A filling without a host fills
//! the term wherever the article runs, under any hook and outside one, with
//! the parameters of the hook article itself. These tests pin that on
//! fictional rulework.

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

/// A special law with two decisions that both fall under 4:13, each with a
/// decision period of its own that depends on a parameter of that decision
/// alone: the grant on the subsidy year (article 1, filled by article 2), the
/// settlement on the number of projects (article 3, filled by article 4).
/// Each filling names the article whose decision it sets the period of.
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
      Onze Minister beslist op een aanvraag tot verlening binnen dertien weken
      na ontvangst, en voor een subsidiejaar vóór 2026 binnen zesentwintig weken.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidiewet_voorbeeld
            article: '1'
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
  - number: '3'
    text: Onze Minister stelt de subsidie vast na ontvangst van het verslag over de projecten.
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: aantal_projecten
            type: number
            required: true
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: subsidie_vastgesteld
            type: boolean
        actions:
          - output: subsidie_vastgesteld
            value: true
  - number: '4'
    text: Onze Minister beslist op een verslag binnen vier weken per project na ontvangst.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidiewet_voorbeeld
            article: '3'
      execution:
        parameters:
          - name: aantal_projecten
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
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks:
                operation: MULTIPLY
                values:
                  - 4
                  - $aantal_projecten
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

/// A ministerial regulation that fills the decision period without naming a
/// host, with a parameter the hook article itself has.
const GENERAL_FILLING: &str = r#"
$id: algemene_regeling_voorbeeld
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

/// A policy rule of the minister that sets the decision period on the grant
/// of article 1 of the subsidy law, with the subsidy year of that decision:
/// ten weeks, twenty for a subsidy year before 2026.
const GRANT_POLICY: &str = r#"
$id: uitvoeringsbeleid_subsidie_voorbeeld
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Op een aanvraag tot verlening wordt binnen tien weken beslist, voor een subsidiejaar vóór 2026 binnen twintig.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidiewet_voorbeeld
            article: '1'
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
                    weeks: 20
              default:
                operation: DATE_ADD
                date: $datum_ontvangst
                weeks: 10
"#;

/// The subsidy law with its decisions and without its own fillings.
fn subsidy_decisions_only() -> String {
    let end = SUBSIDY_LAW
        .find("  - number: '2'")
        .expect("the subsidy law has an article 2");
    let settlement = SUBSIDY_LAW
        .find("  - number: '3'")
        .expect("the subsidy law has an article 3");
    let filling = SUBSIDY_LAW
        .find("  - number: '4'")
        .expect("the subsidy law has an article 4");
    format!(
        "{}{}",
        &SUBSIDY_LAW[..end],
        &SUBSIDY_LAW[settlement..filling]
    )
}

fn service(delegation_type: Option<&str>, laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&general_law(delegation_type)).unwrap();
    for law in laws {
        service.load_law(law).unwrap();
    }
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

/// Which regulation and article filled the open term of 4:13, by the receipt.
fn filled_by(result: &regelrecht_engine::ArticleResult) -> Option<(String, String)> {
    result
        .open_terms
        .iter()
        .filter(|filled| filled.term == "wettelijke_beslistermijn")
        .find_map(|filled| match &filled.filled_by {
            FilledBy::Regulation { law_id, article } => Some((law_id.clone(), article.clone())),
            _ => None,
        })
}

fn by(law: &str, article: &str) -> Option<(String, String)> {
    Some((law.to_string(), article.to_string()))
}

fn received() -> BTreeMap<String, Value> {
    params(&[("datum_ontvangst", date("2026-01-05"))])
}

fn grant(service: &LawExecutionService, subsidiejaar: i64) -> regelrecht_engine::ArticleResult {
    service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            subsidy_application(subsidiejaar),
            "2026-01-05",
        )
        .unwrap()
}

fn settlement(service: &LawExecutionService, projects: i64) -> regelrecht_engine::ArticleResult {
    service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_vastgesteld",
            params(&[
                ("aantal_projecten", Value::Int(projects)),
                ("datum_ontvangst", date("2026-01-05")),
            ]),
            "2026-01-05",
        )
        .unwrap()
}

fn permit(service: &LawExecutionService) -> regelrecht_engine::ArticleResult {
    service
        .evaluate_law_output(
            "andere_wet_voorbeeld",
            "vergunning_verleend",
            received(),
            "2026-01-05",
        )
        .unwrap()
}

fn direct(service: &LawExecutionService) -> regelrecht_engine::ArticleResult {
    service
        .evaluate_law_output(
            "algemene_wet_voorbeeld",
            "uiterste_beslisdag",
            subsidy_application(2026),
            "2026-01-05",
        )
        .unwrap()
}

fn last_day(result: &regelrecht_engine::ArticleResult) -> Option<&Value> {
    result.outputs.get("uiterste_beslisdag")
}

/// Two articles of one law fire the same hook, and each has a filling that
/// names it. Each decision gets its own filling, with a parameter only that
/// decision has: the subsidy year of the grant, the number of projects of
/// the settlement.
#[test]
fn each_article_that_fires_the_hook_gets_the_filling_that_names_it() {
    let service = service(None, &[SUBSIDY_LAW]);

    let grant_2026 = grant(&service, 2026);
    assert_eq!(
        last_day(&grant_2026),
        Some(&date("2026-04-06")),
        "thirteen weeks for the grant"
    );
    assert_eq!(filled_by(&grant_2026), by("subsidiewet_voorbeeld", "2"));
    assert_eq!(
        last_day(&grant(&service, 2025)),
        Some(&date("2026-07-06")),
        "twenty-six weeks for a subsidy year before 2026"
    );

    let settled = settlement(&service, 3);
    assert_eq!(
        last_day(&settled),
        Some(&date("2026-03-30")),
        "twelve weeks for three projects"
    );
    assert_eq!(filled_by(&settled), by("subsidiewet_voorbeeld", "4"));
}

/// A filling that names a host applies only while the hook fires on that
/// article. A decision of another law gets the default, and so does the
/// article called directly; the filling leaves no record there.
#[test]
fn a_filling_with_a_host_applies_nowhere_else() {
    let service = service(None, &[SUBSIDY_LAW, OTHER_LAW]);

    for result in [permit(&service), direct(&service)] {
        assert_eq!(
            last_day(&result),
            Some(&date("2026-03-02")),
            "eight weeks, the default of the general law"
        );
        assert_eq!(filled_by(&result), None);
        assert!(result.delegation_refusals.is_empty());
        assert!(result.declarations_not_in_force.is_empty());
    }
}

/// A filling that names no host fills the term for every article the hook
/// fires on, both articles of one law and a decision of another law, and
/// outside a hook too: as an open term is filled anywhere else.
#[test]
fn a_filling_without_a_host_applies_under_every_hook() {
    let decisions = subsidy_decisions_only();
    let service = service(None, &[&decisions, OTHER_LAW, GENERAL_FILLING]);

    for result in [
        grant(&service, 2026),
        settlement(&service, 3),
        permit(&service),
        direct(&service),
    ] {
        assert_eq!(
            last_day(&result),
            Some(&date("2026-08-03")),
            "thirty weeks, from the filling without a host"
        );
        assert_eq!(filled_by(&result), by("algemene_regeling_voorbeeld", "1"));
    }
}

/// A filling that names the article the hook fires on goes before one that
/// names none, also when the one without a host would win on priority alone.
/// Where nothing names the article, the filling without a host applies.
#[test]
fn a_filling_that_names_the_host_goes_before_one_that_names_none() {
    let general_by_law = GENERAL_FILLING
        .replace(
            "regulatory_layer: MINISTERIELE_REGELING",
            "regulatory_layer: WET",
        )
        .replace(
            "publication_date: '2025-01-01'",
            "publication_date: '2025-06-01'",
        );
    let service = service(
        None,
        &[SUBSIDY_LAW, OTHER_LAW, GRANT_POLICY, &general_by_law],
    );

    let granted = grant(&service, 2026);
    assert_eq!(
        last_day(&granted),
        Some(&date("2026-04-06")),
        "thirteen weeks of the subsidy law, not thirty of the general filling"
    );
    assert_eq!(filled_by(&granted), by("subsidiewet_voorbeeld", "2"));

    assert_eq!(
        last_day(&permit(&service)),
        Some(&date("2026-08-03")),
        "thirty weeks for a decision no filling names"
    );
}

/// The fillings with and without a host side by side: each article of the
/// subsidy law gets the filling that names it, and every other firing, and
/// the article called directly, gets the filling without a host.
#[test]
fn fillings_with_and_without_a_host_side_by_side() {
    let service = service(None, &[SUBSIDY_LAW, OTHER_LAW, GENERAL_FILLING]);

    let cases = [
        (
            grant(&service, 2026),
            "2026-04-06",
            by("subsidiewet_voorbeeld", "2"),
        ),
        (
            grant(&service, 2025),
            "2026-07-06",
            by("subsidiewet_voorbeeld", "2"),
        ),
        (
            settlement(&service, 3),
            "2026-03-30",
            by("subsidiewet_voorbeeld", "4"),
        ),
        (
            permit(&service),
            "2026-08-03",
            by("algemene_regeling_voorbeeld", "1"),
        ),
        (
            direct(&service),
            "2026-08-03",
            by("algemene_regeling_voorbeeld", "1"),
        ),
    ];
    for (result, expected, filler) in cases {
        assert_eq!(last_day(&result), Some(&date(expected)));
        assert_eq!(filled_by(&result), filler);
    }
}

/// A filling that names the host and is silent for the case (null) passes
/// the term on to the filling without a host, as a silent filling passes it
/// on to the next candidate anywhere else.
#[test]
fn a_silent_filling_for_the_host_passes_the_term_to_one_without_a_host() {
    let silent = SUBSIDY_LAW.replacen(
        "              default:\n                operation: DATE_ADD\n                date: $datum_ontvangst\n                weeks: 13\n",
        "              default: null\n",
        1,
    );
    // Article 2 is the first to declare this output; it may now be null.
    let silent = silent.replacen(
        "          - name: wettelijke_beslistermijn
            type: date
",
        "          - name: wettelijke_beslistermijn
            type: date
            nullable: true
",
        1,
    );
    assert!(silent.contains("default: null") && silent.contains("nullable: true"));
    let service = service(None, &[&silent, GENERAL_FILLING]);

    let silent_year = grant(&service, 2026);
    assert_eq!(
        last_day(&silent_year),
        Some(&date("2026-08-03")),
        "the subsidy law is silent for 2026, so the filling without a host applies"
    );
    assert_eq!(
        filled_by(&silent_year),
        by("algemene_regeling_voorbeeld", "1")
    );
    assert_eq!(
        last_day(&grant(&service, 2025)),
        Some(&date("2026-07-06")),
        "for 2025 the subsidy law still sets twenty-six weeks"
    );
}

/// A municipal policy rule that fills the term both for the grant and in
/// general. Under the hook on the grant the host's scope names no
/// municipality, so the filling for the host does not apply; the same
/// article still fills the term in general, checked against the hook
/// article's parameters, which do.
#[test]
fn an_article_that_fills_for_the_host_and_in_general_falls_back_to_the_general_filling() {
    let policy = r#"
$id: gemeentelijk_beleid_voorbeeld
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
gemeente_code: GM9999
articles:
  - number: '1'
    text: Het college beslist binnen drie weken na ontvangst.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidiewet_voorbeeld
            article: '1'
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
              weeks: 3
"#;
    let general = general_law(None).replacen(
        "          - name: datum_ontvangst\n            type: date\n            required: true\n        output:",
        "          - name: datum_ontvangst\n            type: date\n            required: true\n          - name: gemeente_code\n            type: string\n            required: false\n        output:",
        1,
    );
    assert!(general.contains("name: gemeente_code"));
    let decisions = subsidy_decisions_only();
    let mut service = LawExecutionService::new();
    service.load_law(&general).unwrap();
    service.load_law(&decisions).unwrap();
    service.load_law(policy).unwrap();

    // The grant hands its hooks no gemeente_code; the caller does give it,
    // and the hook article declares it.
    let mut application = subsidy_application(2026);
    application.insert("gemeente_code".to_string(), Value::String("GM9999".into()));
    let granted = service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            application,
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        last_day(&granted),
        Some(&date("2026-01-26")),
        "three weeks, from the general filling of the same article"
    );
}

/// The group of a filling is read from the version in force on the date,
/// not from the newest version the index is built from. An older version
/// fills the term without a host; the newest names the grant as its host.
#[test]
fn the_host_is_read_from_the_version_in_force() {
    let newest = r#"
$id: termijnregeling_voorbeeld
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2027-01-01'
valid_from: '2027-01-01'
articles:
  - number: '1'
    text: Op een aanvraag tot verlening wordt binnen dertig weken beslist.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidiewet_voorbeeld
            article: '1'
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
    let older = newest
        .replace(
            "publication_date: '2027-01-01'",
            "publication_date: '2025-01-01'",
        )
        .replace("valid_from: '2027-01-01'", "valid_from: '2025-01-01'")
        .replace(
            "          host:\n            law: subsidiewet_voorbeeld\n            article: '1'\n",
            "",
        );
    let decisions = subsidy_decisions_only();
    let service = service(None, &[&decisions, OTHER_LAW, &older, newest]);

    let result = permit(&service);
    assert_eq!(
        last_day(&result),
        Some(&date("2026-08-03")),
        "in 2026 the regulation fills the term without a host, also for a permit"
    );
    assert_eq!(filled_by(&result), by("termijnregeling_voorbeeld", "1"));
}

/// A municipal policy rule of the host's own municipality that names the
/// host fills the term, although the host hands its hooks no
/// `gemeente_code`: a regulation of the host's territory applies to the case
/// the host decides, as the host itself does. A policy rule of another
/// municipality does not.
#[test]
fn a_policy_of_the_hosts_territory_fills_without_a_matching_scope_parameter() {
    let host = r#"
$id: subsidieverordening_gemeente_voorbeeld
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
"#;
    let policy = |code: &str| {
        format!(
            r#"
$id: beleid_{code}_voorbeeld
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
gemeente_code: {code}
articles:
  - number: '1'
    text: Het college beslist binnen zes weken na ontvangst.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidieverordening_gemeente_voorbeeld
            article: '1'
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
              weeks: 6
"#
        )
    };
    let run = |code: &str| {
        let service = service(None, &[host, &policy(code)]);
        service
            .evaluate_law_output(
                "subsidieverordening_gemeente_voorbeeld",
                "subsidie_verleend",
                received(),
                "2026-01-05",
            )
            .unwrap()
    };

    assert_eq!(
        last_day(&run("GM9999")),
        Some(&date("2026-02-16")),
        "six weeks, from the policy rule of the host's municipality"
    );
    assert_eq!(
        last_day(&run("GM0001")),
        Some(&date("2026-03-02")),
        "eight weeks: the policy rule of another municipality does not apply"
    );
}

/// A version in force that declares nothing for the term stays a candidate
/// through the index, in the group of the newest version, as before RFC-058.
/// A version in force that fills the term only for another host is no
/// candidate here, and the receipt says why.
#[test]
fn a_version_in_force_without_the_declaration_follows_the_index() {
    let newest = r#"
$id: termijnregeling_index_voorbeeld
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2027-01-01'
valid_from: '2027-01-01'
articles:
  - number: '1'
    text: Op een aanvraag tot verlening wordt binnen dertig weken beslist.
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
    let dated = |implements: &str| {
        newest
            .replace("publication_date: '2027-01-01'", "publication_date: '2025-01-01'")
            .replace("valid_from: '2027-01-01'", "valid_from: '2025-01-01'")
            .replace(
                "      implements:\n        - law: algemene_wet_voorbeeld\n          article: '4:13'\n          open_term: wettelijke_beslistermijn\n",
                implements,
            )
    };
    let decisions = subsidy_decisions_only();

    let silent_older = dated("");
    let first = service(None, &[&decisions, OTHER_LAW, &silent_older, newest]);
    assert_eq!(
        filled_by(&permit(&first)),
        by("termijnregeling_index_voorbeeld", "1"),
        "the older version declares nothing and fills the term through the index"
    );

    let other_host_older = dated(
        "      implements:\n        - law: algemene_wet_voorbeeld\n          article: '4:13'\n          open_term: wettelijke_beslistermijn\n          host:\n            law: subsidiewet_voorbeeld\n            article: '3'\n",
    );
    let second = service(None, &[&decisions, OTHER_LAW, &other_host_older, newest]);
    let result = permit(&second);
    assert_eq!(last_day(&result), Some(&date("2026-03-02")));
    assert!(
        result
            .declarations_not_in_force
            .iter()
            .any(|note| note.law_id == "termijnregeling_index_voorbeeld"
                && note.reason.contains("only for another host")),
        "the receipt says the version in force fills it for another host only, got {:?}",
        result.declarations_not_in_force
    );
}

/// Two fillings that both name the host are ranked as any two candidates
/// are: lex superior, so the law goes before the policy rule. Without the
/// law's own filling, the policy rule fills the term with the subsidy year.
#[test]
fn two_fillings_that_name_the_host_are_ranked_by_priority() {
    let both = service(None, &[SUBSIDY_LAW, GRANT_POLICY]);
    let granted = grant(&both, 2026);
    assert_eq!(filled_by(&granted), by("subsidiewet_voorbeeld", "2"));
    assert_eq!(last_day(&granted), Some(&date("2026-04-06")));

    let decisions = subsidy_decisions_only();
    let policy_only = service(None, &[&decisions, GRANT_POLICY]);
    let granted = grant(&policy_only, 2026);
    assert_eq!(
        filled_by(&granted),
        by("uitvoeringsbeleid_subsidie_voorbeeld", "1")
    );
    assert_eq!(
        last_day(&granted),
        Some(&date("2026-03-16")),
        "ten weeks, set by the policy rule"
    );
}

/// A policy rule that names the host is still asked the delegation gate's
/// question: where the term demands a law, it is refused, in the receipt,
/// and the default applies.
#[test]
fn the_delegation_gate_refuses_a_policy_rule_that_names_the_host() {
    let decisions = subsidy_decisions_only();
    let service = service(Some("WET"), &[&decisions, GRANT_POLICY]);

    let granted = grant(&service, 2026);
    assert_eq!(
        last_day(&granted),
        Some(&date("2026-03-02")),
        "the refused policy rule leaves the default of eight weeks"
    );
    let refused: Vec<(&str, &str)> = granted
        .delegation_refusals
        .iter()
        .map(|r| (r.refused_law.as_str(), r.refused_layer.as_str()))
        .collect();
    assert_eq!(
        refused,
        vec![("uitvoeringsbeleid_subsidie_voorbeeld", "BELEIDSREGEL")]
    );
}

/// The gate holds for a law that names the host as well: a layer the term
/// does not admit is refused, and the default applies.
#[test]
fn the_delegation_gate_refuses_a_host_of_the_wrong_layer() {
    let service = service(Some("MINISTERIELE_REGELING"), &[SUBSIDY_LAW]);

    let granted = grant(&service, 2026);
    assert_eq!(last_day(&granted), Some(&date("2026-03-02")));
    let refused: Vec<&str> = granted
        .delegation_refusals
        .iter()
        .map(|r| r.refused_law.as_str())
        .collect();
    assert_eq!(refused, vec!["subsidiewet_voorbeeld"]);
}

/// A filling without a host reads the parameters of the hook article only.
/// One that asks for a parameter of the decision (the subsidy year) does not
/// get it from the decision: the execution fails with `VariableNotFound`,
/// the error an implementation gets for a required parameter that is not in
/// the scope it runs in, as before RFC-058.
#[test]
fn a_filling_without_a_host_does_not_get_a_parameter_of_the_host() {
    let without_host = SUBSIDY_LAW
        .replacen(
            "          host:\n            law: subsidiewet_voorbeeld\n            article: '1'\n",
            "",
            1,
        )
        .replace(
            "$id: subsidiewet_voorbeeld",
            "$id: subsidiewet_zonder_herkomst_voorbeeld",
        )
        .replace(
            "law: subsidiewet_voorbeeld",
            "law: subsidiewet_zonder_herkomst_voorbeeld",
        );
    assert_eq!(without_host.matches("host:").count(), 1);
    let service = service(None, &[&without_host]);

    let result = service.evaluate_law_output(
        "subsidiewet_zonder_herkomst_voorbeeld",
        "subsidie_verleend",
        subsidy_application(2026),
        "2026-01-05",
    );
    match result {
        Err(EngineError::VariableNotFound(name)) => assert_eq!(name, "subsidiejaar"),
        other => panic!("expected the missing parameter subsidiejaar, got {other:?}"),
    }
}

/// Regression for #2020: a regulation with an article that fires as a hook,
/// and another article of the same regulation that fills the hook's open
/// term without naming a host. That filling counts under every hook, as it
/// did before #2020, which let only the law of the decision fill the term.
/// So does a policy rule that fills the term of a hook article of a general
/// law without a host.
#[test]
fn a_filling_of_the_hooks_own_regulation_without_a_host_counts() {
    let policy = r#"
$id: behandelbeleid_voorbeeld
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: Op elke beschikking wordt binnen de behandeltermijn beslist.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      open_terms:
        - id: behandeltermijn_weken
          type: number
          required: false
          default:
            actions:
              - output: behandeltermijn_weken
                value: 8
      execution:
        parameters:
          - name: datum_ontvangst
            type: date
            required: true
        output:
          - name: uiterste_behandeldag
            type: date
        actions:
          - output: uiterste_behandeldag
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: $behandeltermijn_weken
  - number: 4a
    text: De behandeltermijn bedraagt zes weken.
    machine_readable:
      implements:
        - law: behandelbeleid_voorbeeld
          article: '4'
          open_term: behandeltermijn_weken
      execution:
        output:
          - name: behandeltermijn_weken
            type: number
        actions:
          - output: behandeltermijn_weken
            value: 6
  - number: '5'
    text: Op een aanvraag wordt binnen vijf weken na ontvangst beslist.
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
              weeks: 5
"#;
    let service = service(None, &[OTHER_LAW, policy]);

    let result = permit(&service);
    assert_eq!(
        result.outputs.get("uiterste_behandeldag"),
        Some(&date("2026-02-16")),
        "six weeks, from article 4a of the policy rule itself"
    );
    assert_eq!(
        last_day(&result),
        Some(&date("2026-02-09")),
        "five weeks, from the policy rule on the general law's hook"
    );
    assert_eq!(filled_by(&result), by("behandelbeleid_voorbeeld", "5"));
}

/// A host that states its period in terms of the general rule reads the
/// general rule without its own filling: held back, it passes the term on
/// to the filling without a host, as it would to the default.
#[test]
fn a_host_reading_the_general_rule_gets_the_filling_without_a_host() {
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
    text: De beslistermijn is de algemene termijn, verlengd met twee weken.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host:
            law: subsidiewet_verlenging_voorbeeld
            article: '1'
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
    let without_general = service(None, &[host]);
    let with_general = service(None, &[host, GENERAL_FILLING]);
    let run = |service: &LawExecutionService| {
        service
            .evaluate_law_output(
                "subsidiewet_verlenging_voorbeeld",
                "subsidie_verleend",
                received(),
                "2026-01-05",
            )
            .unwrap()
    };

    assert_eq!(
        last_day(&run(&without_general)),
        Some(&date("2026-03-16")),
        "eight weeks of the default plus two of the host"
    );
    assert_eq!(
        last_day(&run(&with_general)),
        Some(&date("2026-08-17")),
        "thirty weeks of the filling without a host plus two of the host"
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
          host:
            law: subsidieverordening_voorbeeld
            article: '1'
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
    let service = service(None, &[host]);

    let result = service
        .evaluate_law_output(
            "subsidieverordening_voorbeeld",
            "subsidie_verleend",
            received(),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        last_day(&result),
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

/// A finding for a term that only a filling for another host fills applies
/// under a hook: for this decision the term is unfilled, so the authority's
/// judgement for the case decides it.
#[test]
fn a_finding_applies_where_only_a_filling_for_another_host_fills_the_term() {
    let mut service = LawExecutionService::new();
    service.load_law(&general_law_judged_per_case()).unwrap();
    service.load_law(SUBSIDY_LAW).unwrap();
    service.load_law(OTHER_LAW).unwrap();

    let result = service
        .evaluate_law_with_findings(
            "andere_wet_voorbeeld",
            &["vergunning_verleend"],
            received(),
            params(&[("wettelijke_beslistermijn", date("2026-05-01"))]),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        last_day(&result),
        Some(&date("2026-05-01")),
        "the finding decides the term no filling fills for this decision"
    );
}

/// A policy rule that names the grant as its host and leaves a case to the
/// authority with a term of its own (RFC-048 §2).
const GRANT_POLICY_JUDGED_PER_CASE: &str = r#"
$id: bijzonder_beleid_voorbeeld
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
          host:
            law: subsidiewet_voorbeeld
            article: '1'
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

/// Under the hook on the grant the policy rule fills the term and takes the
/// finding on its own term. Under the hook on another decision it fills
/// nothing, so no article asks for that finding and the engine refuses it.
#[test]
fn a_finding_on_the_own_term_of_a_policy_counts_only_under_its_host() {
    let decisions = subsidy_decisions_only();
    let service = service(None, &[&decisions, OTHER_LAW, GRANT_POLICY_JUDGED_PER_CASE]);
    let finding = params(&[("bijzondere_omstandigheden", Value::Bool(true))]);

    let granted = service
        .evaluate_law_with_findings(
            "subsidiewet_voorbeeld",
            &["subsidie_verleend"],
            subsidy_application(2026),
            finding.clone(),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        last_day(&granted),
        Some(&date("2026-01-19")),
        "under its host the policy rule fills the term with the finding"
    );

    let other = service.evaluate_law_with_findings(
        "andere_wet_voorbeeld",
        &["vergunning_verleend"],
        received(),
        finding,
        "2026-01-05",
    );
    match other {
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
/// reads them (RFC-043); the filling that names the host is such a reader,
/// so the input is resolved before the hook fires.
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
        .replace("subsidiewet_voorbeeld", "subsidiewet_register_voorbeeld")
        .replacen(
            "          - name: subsidiejaar\n            type: number\n            required: true\n          - name: datum_ontvangst\n            type: date\n            required: true\n        output:\n          - name: subsidie_verleend",
            "          - name: datum_ontvangst\n            type: date\n            required: true\n        input:\n          - name: subsidiejaar\n            type: number\n            source:\n              regulation: subsidieregister_voorbeeld\n              output: subsidiejaar\n        output:\n          - name: subsidie_verleend",
            1,
        );
    assert!(host.contains("regulation: subsidieregister_voorbeeld"));
    let service = service(None, &[register, &host]);

    let result = service
        .evaluate_law_output(
            "subsidiewet_register_voorbeeld",
            "subsidie_verleend",
            received(),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        last_day(&result),
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
          host:
            law: subsidiewet_categorie_voorbeeld
            article: '1'
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
    let service = service(None, &[host]);

    let result = service
        .evaluate_law_output(
            "subsidiewet_categorie_voorbeeld",
            "subsidie_verleend",
            received(),
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(
        last_day(&result),
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
/// it as that decision's: the filling that names the decision reaches the
/// second hook through the first, and a decision it does not name keeps the
/// default in both.
#[test]
fn a_hook_reading_another_hook_on_the_same_decision_gets_the_hosts_filling() {
    let weeks = r#"
$id: weken_voor_vergunning_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Op een aanvraag om een vergunning wordt binnen een week beslist.
    machine_readable:
      implements:
        - law: algemene_wet_twee_haken_voorbeeld
          article: '1'
          open_term: wettelijke_beslistermijn_weken
          host:
            law: andere_wet_voorbeeld
            article: '1'
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
    service.load_law(&subsidy_decisions_only()).unwrap();
    service.load_law(weeks).unwrap();

    let named = permit(&service);
    assert_eq!(
        named.outputs.get("beslistermijn_weken"),
        Some(&Value::Int(1))
    );
    assert_eq!(
        last_day(&named),
        Some(&date("2026-01-12")),
        "one week in the second hook too, through the first"
    );

    let other = settlement(&service, 3);
    assert_eq!(
        other.outputs.get("beslistermijn_weken"),
        Some(&Value::Int(8))
    );
    assert_eq!(last_day(&other), Some(&date("2026-03-02")));
}

/// A host-named filling in a file on a schema version before v0.8.24 is
/// refused when it loads: that version does not know `host`, and a reader of
/// it would take the filling to apply to every decision.
#[test]
fn a_host_before_schema_v0_8_24_is_refused() {
    let old = format!(
        "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json\n{SUBSIDY_LAW}"
    );
    let mut service = LawExecutionService::new();
    match service.load_law(&old) {
        Err(error) => assert!(
            error.to_string().contains("schema v0.8.24 introduced"),
            "got {error}"
        ),
        Ok(_) => panic!("a host under v0.8.22 should be refused"),
    }
}

/// A default reads the parameters of the article its term belongs to as that
/// article does: an optional one the caller left out is unknown, not a
/// missing variable (RFC-036). The decision period of an application that
/// was recorded without its day of receipt is then unknown too.
#[test]
fn a_default_reads_an_optional_parameter_left_out_as_unknown() {
    let optional_receipt = general_law(None).replacen(
        "          - name: datum_ontvangst\n            type: date\n            required: true",
        "          - name: datum_ontvangst\n            type: date\n            required: false",
        1,
    );
    assert!(optional_receipt.contains("required: false\n        output:"));
    let mut service = LawExecutionService::new();
    service.load_law(&optional_receipt).unwrap();

    let result = service
        .evaluate_law_output(
            "algemene_wet_voorbeeld",
            "uiterste_beslisdag",
            BTreeMap::new(),
            "2026-01-05",
        )
        .unwrap();
    assert!(
        !matches!(
            result.outputs.get("uiterste_beslisdag"),
            Some(Value::String(_))
        ),
        "no day of receipt, so no last day either, got {:?}",
        result.outputs.get("uiterste_beslisdag")
    );
}
