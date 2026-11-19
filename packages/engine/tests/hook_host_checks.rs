//! A hook that names its host (`applies_to.host`, schema v0.8.25, RFC-058),
//! and the rulework in `tests/fixtures/hook_host/` that the warnings for a
//! host that cannot be right are pinned on (see `validate_cli.rs`). All of it
//! is fictional; every application is received on 2026-01-05.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{ArticleResult, LawExecutionService, Value};
use std::collections::BTreeMap;

const GENERAL_LAW: &str = include_str!("fixtures/hook_host/algemene_wet_voorbeeld.yaml");
const SUBSIDY_LAW: &str = include_str!("fixtures/hook_host/subsidiewet_voorbeeld.yaml");
const PERMIT_LAW: &str = include_str!("fixtures/hook_host/vergunningwet_voorbeeld.yaml");
const POLICY: &str = include_str!("fixtures/hook_host/beleidsregel_subsidie_voorbeeld.yaml");

fn loaded(laws: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for law in laws {
        service.load_law(law).unwrap();
    }
    service
}

fn run(
    service: &LawExecutionService,
    law: &str,
    output: &str,
    extra: (&str, i64),
) -> ArticleResult {
    let parameters = BTreeMap::from([
        (
            "datum_ontvangst".to_string(),
            Value::String("2026-01-05".to_string()),
        ),
        (extra.0.to_string(), Value::Int(extra.1)),
    ]);
    service
        .evaluate_law_output_with_trace(law, output, parameters, "2026-01-05")
        .unwrap()
}

/// Which hooks of the policy rule fired, as their outputs and trace show.
fn policy_hooks(result: &ArticleResult) -> (bool, bool) {
    let trace = format!("{:?}", result.trace);
    let fired = |article: &str, output: &str| {
        let in_trace = trace.contains(&format!("beleidsregel_subsidie_voorbeeld:{article}"));
        assert_eq!(
            in_trace,
            result.outputs.contains_key(output),
            "a hook leaves a trace node exactly when it fires"
        );
        in_trace
    };
    (
        fired("1", "meldingstermijn_weken"),
        fired("2", "bewaartermijn_jaren"),
    )
}

/// `{law}` fires on every decision of that law, `{law, article}` on that
/// article's alone, and neither on a decision of another law, though all
/// three are a TOEKENNING, nor on an AFWIJZING of the host. The filling of
/// the same policy rule names host article '11', which does not exist, so
/// the grant keeps the default of eight weeks; with article '1' it gets the
/// policy rule's ten.
#[test]
fn a_hook_with_a_host_fires_only_on_the_decisions_it_names() {
    let service = loaded(&[GENERAL_LAW, SUBSIDY_LAW, PERMIT_LAW, POLICY]);

    let grant = run(
        &service,
        "subsidiewet_voorbeeld",
        "subsidie_verleend",
        ("subsidiejaar", 2026),
    );
    assert_eq!(policy_hooks(&grant), (true, false));
    assert_eq!(
        grant.outputs.get("uiterste_beslisdag"),
        Some(&Value::String("2026-03-02".to_string()))
    );

    let settlement = run(
        &service,
        "subsidiewet_voorbeeld",
        "subsidie_vastgesteld",
        ("aantal_projecten", 3),
    );
    assert_eq!(policy_hooks(&settlement), (true, true));

    let permit = run(
        &service,
        "vergunningwet_voorbeeld",
        "vergunning_verleend",
        ("x", 0),
    );
    assert_eq!(policy_hooks(&permit), (false, false));

    // The host admits the refusal, the decision type does not.
    let refusal = run(
        &service,
        "subsidiewet_voorbeeld",
        "subsidie_geweigerd",
        ("subsidiejaar", 2025),
    );
    assert_eq!(policy_hooks(&refusal), (false, false));

    // With the typo fixed, the policy rule's ten weeks apply to the grant.
    let fixed = POLICY.replace("article: '11'", "article: '1'");
    let service = loaded(&[GENERAL_LAW, SUBSIDY_LAW, &fixed]);
    let grant = run(
        &service,
        "subsidiewet_voorbeeld",
        "subsidie_verleend",
        ("subsidiejaar", 2026),
    );
    assert_eq!(
        grant.outputs.get("uiterste_beslisdag"),
        Some(&Value::String("2026-03-16".to_string()))
    );
}

/// A hook on a submission takes no host: `established_by` already narrows it
/// to the submission of one article, and two fields for that could disagree.
#[test]
fn a_hook_on_a_submission_takes_no_host() {
    let hook = POLICY.replacen(
        "legal_character: BESCHIKKING\n            decision_type: TOEKENNING\n",
        "submission: AANVRAAG\n",
        1,
    );
    assert_ne!(hook, POLICY, "the fixture has the hook this test rewrites");
    let error = LawExecutionService::new().load_law(&hook).unwrap_err();
    assert!(
        error.to_string().contains("takes no applies_to.host"),
        "{error}"
    );
}
