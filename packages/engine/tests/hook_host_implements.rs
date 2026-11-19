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
//! fictional rulework; every application is received on 2026-01-05.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{ArticleResult, EngineError, FilledBy, LawExecutionService, Value};
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
          applies_to: {{legal_character: BESCHIKKING, stage: BESLUIT}}
      open_terms:
        - id: wettelijke_beslistermijn
          type: date
          required: false{delegation}
          default:
            actions:
              - output: wettelijke_beslistermijn
                value: {{operation: DATE_ADD, date: $datum_ontvangst, weeks: 8}}
      execution:
        parameters:
          - {{name: datum_ontvangst, type: date, required: true}}
        output:
          - {{name: uiterste_beslisdag, type: date}}
        actions:
          - {{output: uiterste_beslisdag, value: $wettelijke_beslistermijn}}
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
        produces: {legal_character: BESCHIKKING}
        parameters:
          - {name: subsidiejaar, type: number, required: true}
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: subsidie_verleend, type: boolean}
        actions:
          - {output: subsidie_verleend, value: true}
  - number: '2'
    text: |-
      Onze Minister beslist op een aanvraag tot verlening binnen dertien weken
      na ontvangst, en voor een subsidiejaar vóór 2026 binnen zesentwintig weken.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host: {law: subsidiewet_voorbeeld, article: '1'}
      execution:
        parameters:
          - {name: subsidiejaar, type: number, required: true}
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: wettelijke_beslistermijn, type: date}
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: IF
              cases:
                - when: {operation: LESS_THAN, subject: $subsidiejaar, value: 2026}
                  then: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 26}
              default: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 13}
  - number: '3'
    text: Onze Minister stelt de subsidie vast na ontvangst van het verslag over de projecten.
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING}
        parameters:
          - {name: aantal_projecten, type: number, required: true}
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: subsidie_vastgesteld, type: boolean}
        actions:
          - {output: subsidie_vastgesteld, value: true}
  - number: '4'
    text: Onze Minister beslist op een verslag binnen vier weken per project na ontvangst.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host: {law: subsidiewet_voorbeeld, article: '3'}
      execution:
        parameters:
          - {name: aantal_projecten, type: number, required: true}
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: wettelijke_beslistermijn, type: date}
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: DATE_ADD
              date: $datum_ontvangst
              weeks: {operation: MULTIPLY, values: [4, $aantal_projecten]}
"#;

/// A policy rule of the minister that sets the decision period on the grant
/// of article 1 of the subsidy law, with the subsidy year of that decision:
/// ten weeks, twenty for a subsidy year before 2026. It also fills a term of
/// the same name of another article without a host; that declaration does
/// not decide anything for 4:13.
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
          host: {law: subsidiewet_voorbeeld, article: '1'}
        - {law: algemene_wet_voorbeeld, article: '4:14', open_term: wettelijke_beslistermijn}
      execution:
        parameters:
          - {name: subsidiejaar, type: number, required: true}
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: wettelijke_beslistermijn, type: date}
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: IF
              cases:
                - when: {operation: LESS_THAN, subject: $subsidiejaar, value: 2026}
                  then: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 20}
              default: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 10}
"#;

/// Article `number` filling the decision period of 4:13 with `weeks` weeks
/// after receipt, with one `implements` per entry of `hosts`: the host it
/// names, if any.
fn filling_article(number: &str, hosts: &[Option<(&str, &str)>], weeks: i64) -> String {
    let implements: String = hosts
        .iter()
        .map(|host| {
            let host = host
                .map(|(law, article)| format!(", host: {{law: {law}, article: '{article}'}}"))
                .unwrap_or_default();
            format!(
                "        - {{law: algemene_wet_voorbeeld, article: '4:13', \
                 open_term: wettelijke_beslistermijn{host}}}\n"
            )
        })
        .collect();
    format!(
        r#"
  - number: '{number}'
    text: Op een aanvraag wordt binnen {weeks} weken na ontvangst beslist.
    machine_readable:
      implements:
{implements}      execution:
        parameters:
          - {{name: datum_ontvangst, type: date, required: true}}
        output:
          - {{name: wettelijke_beslistermijn, type: date}}
        actions:
          - output: wettelijke_beslistermijn
            value: {{operation: DATE_ADD, date: $datum_ontvangst, weeks: {weeks}}}"#
    )
}

/// A regulation `id` (`header` holds the lines after `$id`) whose article 1
/// is a [`filling_article`].
fn filling(id: &str, header: &str, hosts: &[Option<(&str, &str)>], weeks: i64) -> String {
    let article = filling_article("1", hosts, weeks);
    format!("\n$id: {id}\n{header}\narticles:{article}\n")
}

/// A law `id` whose article 1 gives a BESCHIKKING on an application, followed
/// by `more` articles.
fn decision_law(id: &str, header: &str, more: &str) -> String {
    format!(
        r#"
$id: {id}
{header}
articles:
  - number: '1'
    text: Op aanvraag wordt een beschikking gegeven.
    machine_readable:
      execution:
        produces: {{legal_character: BESCHIKKING}}
        parameters:
          - {{name: datum_ontvangst, type: date, required: true}}
        output:
          - {{name: verleend, type: boolean}}
        actions:
          - {{output: verleend, value: true}}{more}
"#
    )
}

const LAW: &str = "regulatory_layer: WET\npublication_date: '2025-01-01'";

/// Another law that gives a BESCHIKKING and sets no decision period.
fn other_law() -> String {
    decision_law("andere_wet_voorbeeld", LAW, "")
}

const NATIONAL: &str = "regulatory_layer: MINISTERIELE_REGELING\npublication_date: '2025-01-01'";

/// A ministerial regulation that fills the decision period with thirty weeks
/// without naming a host.
fn general_filling() -> String {
    filling("algemene_regeling_voorbeeld", NATIONAL, &[None], 30)
}

/// The subsidy law with its decisions and without its own fillings.
fn subsidy_decisions_only() -> String {
    let at = |number: &str| {
        SUBSIDY_LAW
            .find(&format!("  - number: '{number}'"))
            .expect("the subsidy law has the article")
    };
    format!(
        "{}{}",
        &SUBSIDY_LAW[..at("2")],
        &SUBSIDY_LAW[at("3")..at("4")]
    )
}

fn loaded(delegation_type: Option<&str>, laws: &[&str]) -> LawExecutionService {
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

fn received() -> BTreeMap<String, Value> {
    params(&[("datum_ontvangst", date("2026-01-05"))])
}

fn subsidy_application(subsidiejaar: i64) -> BTreeMap<String, Value> {
    let mut application = received();
    application.insert("subsidiejaar".to_string(), Value::Int(subsidiejaar));
    application
}

fn run(
    service: &LawExecutionService,
    law: &str,
    output: &str,
    parameters: BTreeMap<String, Value>,
) -> ArticleResult {
    service
        .evaluate_law_output(law, output, parameters, "2026-01-05")
        .unwrap()
}

fn grant(service: &LawExecutionService, subsidiejaar: i64) -> ArticleResult {
    let application = subsidy_application(subsidiejaar);
    run(
        service,
        "subsidiewet_voorbeeld",
        "subsidie_verleend",
        application,
    )
}

fn settlement(service: &LawExecutionService, projects: i64) -> ArticleResult {
    let mut report = received();
    report.insert("aantal_projecten".to_string(), Value::Int(projects));
    run(
        service,
        "subsidiewet_voorbeeld",
        "subsidie_vastgesteld",
        report,
    )
}

fn permit(service: &LawExecutionService) -> ArticleResult {
    run(service, "andere_wet_voorbeeld", "verleend", received())
}

/// 4:13 called directly, outside any hook.
fn direct(service: &LawExecutionService) -> ArticleResult {
    let parameters = subsidy_application(2026);
    run(
        service,
        "algemene_wet_voorbeeld",
        "uiterste_beslisdag",
        parameters,
    )
}

fn last_day(result: &ArticleResult) -> Option<&Value> {
    result.outputs.get("uiterste_beslisdag")
}

/// Which regulation and article filled the open term of 4:13, by the receipt.
fn filled_by(result: &ArticleResult) -> Option<(String, String)> {
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

/// Two articles of one law fire the same hook, and each has a filling that
/// names it. Each decision gets its own filling, with a parameter only that
/// decision has: the subsidy year of the grant, the number of projects of
/// the settlement.
#[test]
fn each_article_that_fires_the_hook_gets_the_filling_that_names_it() {
    let service = loaded(None, &[SUBSIDY_LAW]);

    let grant_2026 = grant(&service, 2026);
    assert_eq!(last_day(&grant_2026), Some(&date("2026-04-06")), "13 weeks");
    assert_eq!(filled_by(&grant_2026), by("subsidiewet_voorbeeld", "2"));
    let grant_2025 = grant(&service, 2025);
    assert_eq!(last_day(&grant_2025), Some(&date("2026-07-06")), "26 weeks");

    let settled = settlement(&service, 3);
    assert_eq!(last_day(&settled), Some(&date("2026-03-30")), "12 weeks");
    assert_eq!(filled_by(&settled), by("subsidiewet_voorbeeld", "4"));
}

/// A filling that names a host applies only while the hook fires on that
/// article. A decision of another law gets the default, and so does the
/// article called directly; the fillings leave no record there. The policy
/// rule's declaration for another article (4:14) without a host does not
/// make it a filling of 4:13 without a host.
#[test]
fn a_filling_with_a_host_applies_nowhere_else() {
    let service = loaded(None, &[SUBSIDY_LAW, &other_law(), GRANT_POLICY]);

    for result in [permit(&service), direct(&service)] {
        assert_eq!(last_day(&result), Some(&date("2026-03-02")), "the default");
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
    let service = loaded(None, &[&decisions, &other_law(), &general_filling()]);

    for result in [
        grant(&service, 2026),
        settlement(&service, 3),
        permit(&service),
        direct(&service),
    ] {
        assert_eq!(last_day(&result), Some(&date("2026-08-03")), "30 weeks");
        assert_eq!(filled_by(&result), by("algemene_regeling_voorbeeld", "1"));
    }
}

/// A filling that names the article the hook fires on goes before one that
/// names none, also when the one without a host would win on priority alone
/// (a law, and newer). Two fillings that name the host are ranked as any two
/// candidates are: the law goes before the policy rule. Where nothing names
/// the article, and outside a hook, the filling without a host applies.
#[test]
fn a_filling_that_names_the_host_goes_before_one_that_names_none() {
    let newer_law = filling(
        "algemene_wet_termijn_voorbeeld",
        "regulatory_layer: WET\npublication_date: '2025-06-01'",
        &[None],
        30,
    );
    let service = loaded(None, &[SUBSIDY_LAW, &other_law(), GRANT_POLICY, &newer_law]);

    let granted = grant(&service, 2026);
    assert_eq!(last_day(&granted), Some(&date("2026-04-06")), "13 weeks");
    assert_eq!(filled_by(&granted), by("subsidiewet_voorbeeld", "2"));
    assert_eq!(last_day(&grant(&service, 2025)), Some(&date("2026-07-06")));
    let settled = settlement(&service, 3);
    assert_eq!(last_day(&settled), Some(&date("2026-03-30")), "12 weeks");
    assert_eq!(filled_by(&settled), by("subsidiewet_voorbeeld", "4"));
    for result in [permit(&service), direct(&service)] {
        assert_eq!(last_day(&result), Some(&date("2026-08-03")), "30 weeks");
        assert_eq!(
            filled_by(&result),
            by("algemene_wet_termijn_voorbeeld", "1")
        );
    }
}

/// Which layer may fill the term stays the delegation gate's question. A
/// policy rule that names the host fills it where the term demands no layer,
/// and is refused, in the receipt, where it demands a law; the default then
/// applies. The gate refuses a law of the wrong layer that names the host
/// in the same way.
#[test]
fn the_delegation_gate_refuses_a_policy_rule_that_names_the_host() {
    let decisions = subsidy_decisions_only();
    let refused = |result: &ArticleResult| -> Vec<(String, String)> {
        result
            .delegation_refusals
            .iter()
            .map(|r| (r.refused_law.clone(), r.refused_layer.clone()))
            .collect()
    };

    let open = grant(&loaded(None, &[&decisions, GRANT_POLICY]), 2026);
    assert_eq!(last_day(&open), Some(&date("2026-03-16")), "10 weeks");
    assert_eq!(
        filled_by(&open),
        by("uitvoeringsbeleid_subsidie_voorbeeld", "1")
    );

    let by_law = grant(&loaded(Some("WET"), &[&decisions, GRANT_POLICY]), 2026);
    assert_eq!(last_day(&by_law), Some(&date("2026-03-02")), "the default");
    assert_eq!(
        refused(&by_law),
        vec![(
            "uitvoeringsbeleid_subsidie_voorbeeld".to_string(),
            "BELEIDSREGEL".to_string()
        )]
    );

    let wrong_layer = grant(&loaded(Some("MINISTERIELE_REGELING"), &[SUBSIDY_LAW]), 2026);
    assert_eq!(last_day(&wrong_layer), Some(&date("2026-03-02")));
    assert_eq!(
        refused(&wrong_layer),
        vec![("subsidiewet_voorbeeld".to_string(), "WET".to_string())]
    );
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
            "          host: {law: subsidiewet_voorbeeld, article: '1'}\n",
            "",
            1,
        )
        .replace(
            "subsidiewet_voorbeeld",
            "subsidiewet_zonder_herkomst_voorbeeld",
        );
    assert_eq!(without_host.matches("host:").count(), 1);
    let service = loaded(None, &[&without_host]);

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
          applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}
      open_terms:
        - id: behandeltermijn_weken
          type: number
          required: false
          default:
            actions:
              - {output: behandeltermijn_weken, value: 8}
      execution:
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: uiterste_behandeldag, type: date}
        actions:
          - output: uiterste_behandeldag
            value: {operation: DATE_ADD, date: $datum_ontvangst, weeks: $behandeltermijn_weken}
  - number: 4a
    text: De behandeltermijn bedraagt zes weken.
    machine_readable:
      implements:
        - {law: behandelbeleid_voorbeeld, article: '4', open_term: behandeltermijn_weken}
      execution:
        output:
          - {name: behandeltermijn_weken, type: number}
        actions:
          - {output: behandeltermijn_weken, value: 6}
"#;
    let general = filling(
        "behandelbeleid_awb_voorbeeld",
        "regulatory_layer: BELEIDSREGEL\npublication_date: '2025-01-01'",
        &[None],
        5,
    );
    let service = loaded(None, &[&other_law(), policy, &general]);

    let result = permit(&service);
    assert_eq!(
        result.outputs.get("uiterste_behandeldag"),
        Some(&date("2026-02-16")),
        "six weeks, from article 4a of the policy rule itself"
    );
    assert_eq!(last_day(&result), Some(&date("2026-02-09")), "5 weeks");
    assert_eq!(filled_by(&result), by("behandelbeleid_awb_voorbeeld", "1"));
}

/// A municipal policy rule that fills the term both for the grant and in
/// general. Under the hook on the grant the host's scope names no
/// municipality, so the filling for the host does not apply; the same
/// article still fills the term in general, checked against the hook
/// article's parameters, which do.
#[test]
fn an_article_that_fills_for_the_host_and_in_general_falls_back_to_the_general_filling() {
    let policy = filling(
        "gemeentelijk_beleid_voorbeeld",
        "regulatory_layer: BELEIDSREGEL\npublication_date: '2025-01-01'\ngemeente_code: GM9999",
        &[Some(("subsidiewet_voorbeeld", "1")), None],
        3,
    );
    let general = general_law(None).replacen(
        "          - {name: datum_ontvangst, type: date, required: true}\n",
        "          - {name: datum_ontvangst, type: date, required: true}\n          \
         - {name: gemeente_code, type: string, required: false}\n",
        1,
    );
    assert!(general.contains("name: gemeente_code"));
    let mut service = LawExecutionService::new();
    service.load_law(&general).unwrap();
    service.load_law(&subsidy_decisions_only()).unwrap();
    service.load_law(&policy).unwrap();

    // The grant hands its hooks no gemeente_code; the caller does give it,
    // and the hook article declares it.
    let mut application = subsidy_application(2026);
    application.insert("gemeente_code".to_string(), Value::String("GM9999".into()));
    let granted = run(
        &service,
        "subsidiewet_voorbeeld",
        "subsidie_verleend",
        application,
    );
    assert_eq!(last_day(&granted), Some(&date("2026-01-26")), "3 weeks");
}

/// The group of a filling is read from the version in force on the date when
/// that version declares the filling, not from the newest version the index
/// is built from. The newest version here names the grant as its host.
/// - An older version that fills without a host fills the term for a permit.
/// - An older version that declares nothing stays a candidate through the
///   index, in the group of the newest version: it fills the grant only.
/// - An older version that fills only for another host (the settlement) is
///   no candidate for the grant, and the receipt says why.
#[test]
fn the_host_is_read_from_the_version_in_force() {
    let header = |year: &str| {
        format!(
            "regulatory_layer: MINISTERIELE_REGELING\npublication_date: '{year}-01-01'\nvalid_from: '{year}-01-01'"
        )
    };
    let grant_host = Some(("subsidiewet_voorbeeld", "1"));
    let newest = filling(
        "termijnregeling_voorbeeld",
        &header("2027"),
        &[grant_host],
        30,
    );
    let older = |hosts: &[Option<(&str, &str)>]| {
        filling("termijnregeling_voorbeeld", &header("2025"), hosts, 30)
    };
    let decisions = subsidy_decisions_only();
    let with_older = |older: &str| loaded(None, &[&decisions, &other_law(), older, &newest]);

    let general = with_older(&older(&[None]));
    let result = permit(&general);
    assert_eq!(last_day(&result), Some(&date("2026-08-03")), "30 weeks");
    assert_eq!(filled_by(&result), by("termijnregeling_voorbeeld", "1"));

    let silent = older(&[None]).replace(
        "      implements:\n        - {law: algemene_wet_voorbeeld, article: '4:13', \
         open_term: wettelijke_beslistermijn}\n",
        "",
    );
    assert!(!silent.contains("implements"));
    let index = with_older(&silent);
    assert_eq!(last_day(&permit(&index)), Some(&date("2026-03-02")));
    assert_eq!(last_day(&grant(&index, 2026)), Some(&date("2026-08-03")));

    let other_host = with_older(&older(&[Some(("subsidiewet_voorbeeld", "3"))]));
    let result = grant(&other_host, 2026);
    assert_eq!(last_day(&result), Some(&date("2026-03-02")), "the default");
    assert!(
        result.declarations_not_in_force.iter().any(|note| {
            note.law_id == "termijnregeling_voorbeeld"
                && note.reason.contains("only for another host")
        }),
        "the receipt says why, got {:?}",
        result.declarations_not_in_force
    );
}

/// A municipal host. With `own_filling` its article 2 names article 1 as its
/// host and sets four weeks.
fn municipal_host(own_filling: bool) -> String {
    let own = if own_filling {
        filling_article("2", &[Some(("subsidieverordening_voorbeeld", "1"))], 4)
    } else {
        String::new()
    };
    let header = "regulatory_layer: GEMEENTELIJKE_VERORDENING\npublication_date: '2025-01-01'\ngemeente_code: GM9999";
    decision_law("subsidieverordening_voorbeeld", header, &own)
}

/// A municipal host, and a policy rule of its own municipality that names
/// it, fill the term although the host hands its hooks no `gemeente_code`: a
/// regulation of the host's territory applies to the case the host decides,
/// as the host itself does. A policy rule of another municipality does not.
#[test]
fn a_regulation_of_the_hosts_territory_fills_without_a_matching_scope_parameter() {
    let policy = |code: &str| {
        filling(
            &format!("beleid_{code}_voorbeeld"),
            &format!(
                "regulatory_layer: BELEIDSREGEL\npublication_date: '2025-01-01'\ngemeente_code: {code}"
            ),
            &[Some(("subsidieverordening_voorbeeld", "1"))],
            6,
        )
    };
    let municipal = |laws: &[&str]| {
        let service = loaded(None, laws);
        let result = run(
            &service,
            "subsidieverordening_voorbeeld",
            "verleend",
            received(),
        );
        last_day(&result).cloned()
    };

    assert_eq!(
        municipal(&[&municipal_host(true)]),
        Some(date("2026-02-02")),
        "4 weeks"
    );
    let own_policy = policy("GM9999");
    assert_eq!(
        municipal(&[&municipal_host(false), &own_policy]),
        Some(date("2026-02-16"))
    );
    let foreign_policy = policy("GM0001");
    assert_eq!(
        municipal(&[&municipal_host(false), &foreign_policy]),
        Some(date("2026-03-02"))
    );
}

/// A filling for the host that is silent for the case (null) passes the
/// term on to the filling without a host, as it would pass it on to the
/// default. For another subsidy year it still sets the period.
#[test]
fn a_filling_for_the_host_that_gives_no_value_passes_the_term_on() {
    let silent = SUBSIDY_LAW
        .replacen(
            "              default: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 13}\n",
            "              default: null\n",
            1,
        )
        .replacen(
            "          - {name: wettelijke_beslistermijn, type: date}\n",
            "          - {name: wettelijke_beslistermijn, type: date, nullable: true}\n",
            1,
        );
    assert!(silent.contains("default: null") && silent.contains("nullable: true"));
    let service = loaded(None, &[&silent, &general_filling()]);
    let silent_year = grant(&service, 2026);
    assert_eq!(
        last_day(&silent_year),
        Some(&date("2026-08-03")),
        "30 weeks"
    );
    assert_eq!(
        filled_by(&silent_year),
        by("algemene_regeling_voorbeeld", "1")
    );
    assert_eq!(last_day(&grant(&service, 2025)), Some(&date("2026-07-06")));
}

/// Findings on terms the authority judges per case (`decided_per_case_by`).
/// - A finding for 4:13 applies under the hook on a permit: the only filling
///   names the grant, so for the permit the term is unfilled.
/// - A policy rule that names the grant leaves a case to the authority with
///   a term of its own. Under the hook on the grant it takes the finding on
///   that term; under the hook on the permit it fills nothing, so no article
///   asks for that finding and the engine refuses it.
#[test]
fn findings_under_a_hook_follow_the_filling_that_applies() {
    let judged = general_law(None).replace(
        "          required: false\n",
        "          required: false\n          decided_per_case_by: bestuursorgaan\n",
    );
    assert!(judged.contains("decided_per_case_by"));
    let mut service = LawExecutionService::new();
    service.load_law(&judged).unwrap();
    service.load_law(SUBSIDY_LAW).unwrap();
    service.load_law(&other_law()).unwrap();
    let period = params(&[("wettelijke_beslistermijn", date("2026-05-01"))]);
    let result = service
        .evaluate_law_with_findings(
            "andere_wet_voorbeeld",
            &["verleend"],
            received(),
            period,
            "2026-01-05",
        )
        .unwrap();
    assert_eq!(last_day(&result), Some(&date("2026-05-01")), "the finding");

    let policy = r#"
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
          host: {law: subsidiewet_voorbeeld, article: '1'}
      open_terms:
        - {id: bijzondere_omstandigheden, type: boolean, required: true, decided_per_case_by: bestuursorgaan}
      execution:
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
        output:
          - {name: wettelijke_beslistermijn, type: date}
        actions:
          - output: wettelijke_beslistermijn
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $bijzondere_omstandigheden, value: true}
                  then: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 2}
              default: {operation: DATE_ADD, date: $datum_ontvangst, weeks: 30}
"#;
    let decisions = subsidy_decisions_only();
    let service = loaded(None, &[&decisions, &other_law(), policy]);
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
    assert_eq!(last_day(&granted), Some(&date("2026-01-19")), "2 weeks");

    let other = service.evaluate_law_with_findings(
        "andere_wet_voorbeeld",
        &["verleend"],
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

/// The host's filling reads an output of the host that the caller did not
/// ask for. The engine computes outputs only when something reads them
/// (RFC-043); the filling that names the host is such a reader, so the
/// output is in the host's scope when the hook fires.
#[test]
fn the_host_scope_holds_an_output_only_the_filling_reads() {
    let filling = r#"
  - number: '2'
    text: Onze Minister beslist binnen de weken van de categorie.
    machine_readable:
      implements:
        - law: algemene_wet_voorbeeld
          article: '4:13'
          open_term: wettelijke_beslistermijn
          host: {law: subsidiewet_categorie_voorbeeld, article: '1'}
      execution:
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
          - {name: weken_voor_categorie, type: number, required: true}
        output:
          - {name: wettelijke_beslistermijn, type: date}
        actions:
          - output: wettelijke_beslistermijn
            value: {operation: DATE_ADD, date: $datum_ontvangst, weeks: $weken_voor_categorie}"#;
    let host = decision_law("subsidiewet_categorie_voorbeeld", LAW, filling).replacen(
        "          - {output: verleend, value: true}",
        "          - {output: verleend, value: true}\n          \
         - {output: weken_voor_categorie, value: 5}",
        1,
    );
    let host = host.replacen(
        "          - {name: verleend, type: boolean}",
        "          - {name: verleend, type: boolean}\n          \
         - {name: weken_voor_categorie, type: number}",
        1,
    );
    let service = loaded(None, &[&host]);
    let result = run(
        &service,
        "subsidiewet_categorie_voorbeeld",
        "verleend",
        received(),
    );
    assert_eq!(last_day(&result), Some(&date("2026-02-09")), "5 weeks");
}

/// A second general law with a hook on every decision that reads the last
/// day 4:13 gives, and reminds the authority a week before.
const REMINDER_LAW: &str = r#"
$id: herinneringswet_voorbeeld
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan wordt een week voor de uiterste beslisdag herinnerd.
    machine_readable:
      hooks:
        - {hook_point: post_actions, applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}}
      execution:
        parameters:
          - {name: datum_ontvangst, type: date, required: true}
        input:
          - name: beslisdag
            type: date
            source:
              regulation: algemene_wet_voorbeeld
              output: uiterste_beslisdag
              parameters: {datum_ontvangst: $datum_ontvangst}
        output:
          - {name: herinneringsdag, type: date}
        actions:
          - output: herinneringsdag
            value: {operation: DATE_ADD, date: $beslisdag, weeks: -1}
"#;

/// A hook that reads the output of another hook on the same decision reads
/// it as that decision's: the filling that names the grant reaches the
/// reminder through 4:13, and a permit keeps the default in both.
#[test]
fn a_hook_reading_another_hook_on_the_same_decision_gets_the_hosts_filling() {
    let service = loaded(None, &[SUBSIDY_LAW, &other_law(), REMINDER_LAW]);
    let reminder = |result: &ArticleResult| result.outputs.get("herinneringsdag").cloned();

    assert_eq!(
        reminder(&grant(&service, 2026)),
        Some(date("2026-03-30")),
        "13 - 1 weeks"
    );
    assert_eq!(
        reminder(&permit(&service)),
        Some(date("2026-02-23")),
        "8 - 1 weeks"
    );
}

/// A host-named filling in a file on a schema version before v0.8.24 is
/// refused when it loads: that version does not know `host`, and a reader of
/// it would take the filling to apply to every decision.
#[test]
fn a_host_before_schema_v0_8_24_is_refused() {
    let old = format!(
        "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.22/schema/v0.8.22/schema.json\n{SUBSIDY_LAW}"
    );
    match LawExecutionService::new().load_law(&old) {
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
        "{name: datum_ontvangst, type: date, required: true}",
        "{name: datum_ontvangst, type: date, required: false}",
        1,
    );
    let service = service_with(&optional_receipt);
    let result = run(
        &service,
        "algemene_wet_voorbeeld",
        "uiterste_beslisdag",
        BTreeMap::new(),
    );
    assert!(
        !matches!(last_day(&result), Some(Value::String(_))),
        "no day of receipt, so no last day either, got {:?}",
        last_day(&result)
    );
}

fn service_with(law: &str) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(law).unwrap();
    service
}
