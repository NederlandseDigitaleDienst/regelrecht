//! A filling of a hook's open term whose host cannot be right (RFC-058), on
//! the fictional rulework in `tests/fixtures/hook_host/` that the loader's
//! warnings are pinned on (see `validate_cli.rs` and the load-order test in
//! `resolver.rs`). Every application is received on 2026-01-05.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use regelrecht_engine::{LawExecutionService, Value};
use std::collections::BTreeMap;

const GENERAL_LAW: &str = include_str!("fixtures/hook_host/algemene_wet_voorbeeld.yaml");
const SUBSIDY_LAW: &str = include_str!("fixtures/hook_host/subsidiewet_voorbeeld.yaml");
const POLICY: &str = include_str!("fixtures/hook_host/beleidsregel_subsidie_voorbeeld.yaml");

/// The last day for the decision on a subsidy grant, with `policy` loaded.
fn last_day_of_grant(policy: &str) -> Option<Value> {
    let mut service = LawExecutionService::new();
    for law in [GENERAL_LAW, SUBSIDY_LAW, policy] {
        service.load_law(law).unwrap();
    }
    let parameters = BTreeMap::from([
        (
            "datum_ontvangst".to_string(),
            Value::String("2026-01-05".to_string()),
        ),
        ("subsidiejaar".to_string(), Value::Int(2026)),
    ]);
    service
        .evaluate_law_output(
            "subsidiewet_voorbeeld",
            "subsidie_verleend",
            parameters,
            "2026-01-05",
        )
        .unwrap()
        .outputs
        .get("uiterste_beslisdag")
        .cloned()
}

/// The policy rule names host article '11', which does not exist, so the
/// grant keeps the default of eight weeks: the law loads, and only the
/// warning says so. With article '1' the grant gets the policy rule's ten.
#[test]
fn a_filling_for_a_host_that_does_not_exist_never_applies() {
    let date = |d: &str| Some(Value::String(d.to_string()));
    assert_eq!(last_day_of_grant(POLICY), date("2026-03-02"));
    let fixed = POLICY.replace("article: '11'", "article: '1'");
    assert_eq!(last_day_of_grant(&fixed), date("2026-03-16"));
}

/// A key `implements` or `applies_to` does not know, such as `within`, is
/// refused with its name, where it used to pass without a word.
#[test]
fn an_unknown_key_in_implements_or_applies_to_is_refused() {
    for (fixture, known, unknown) in [
        (
            POLICY,
            "host: {law: subsidiewet_voorbeeld, article: '11'}",
            "within: {law: subsidiewet_voorbeeld}",
        ),
        (
            GENERAL_LAW,
            "applies_to: {legal_character: BESCHIKKING, stage: BESLUIT}",
            "applies_to: {legal_character: BESCHIKKING, within: subsidiewet_voorbeeld}",
        ),
    ] {
        let law = fixture.replacen(known, unknown, 1);
        assert_ne!(law, fixture, "the fixture has `{known}`");
        let error = LawExecutionService::new().load_law(&law).unwrap_err();
        assert!(
            error.to_string().contains("unknown field `within`"),
            "{error}"
        );
    }
}
