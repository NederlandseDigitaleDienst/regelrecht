//! Then steps: output assertions.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use crate::world::NappWorld;
use cucumber::then;
use regelrecht_engine::Value;

fn assert_success(world: &NappWorld) {
    assert!(
        world.is_success(),
        "Expected successful execution, got error: {:?}",
        world.error_message()
    );
}

fn assert_bool_output(world: &NappWorld, name: &str, expected: bool) {
    assert_success(world);
    let actual = world.get_output(name);
    assert!(
        matches!(actual, Some(Value::Bool(b)) if *b == expected),
        "Expected output '{name}' to be {expected}, got {actual:?}"
    );
}

fn assert_amount_output(world: &NappWorld, name: &str, expected: i64) {
    assert_success(world);
    let actual = world.get_output(name);
    match actual {
        Some(Value::Int(n)) => assert_eq!(
            *n, expected,
            "Expected output '{name}' to be {expected} eurocent, got {n}"
        ),
        Some(Value::Decimal(d)) => {
            use regelrecht_engine::Rounding;
            assert_eq!(
                d.round_to(0, Rounding::HalfAwayFromZero)
                    .and_then(|rounded| rounded.to_i64())
                    .unwrap_or(0),
                expected,
                "Expected output '{name}' to be {expected} eurocent, got {d}"
            )
        }
        other => panic!("Expected numeric output '{name}', got {other:?}"),
    }
}

#[then("the subsidie is toegekend")]
fn subsidie_toegekend(world: &mut NappWorld) {
    assert_bool_output(world, "subsidie_toegekend", true);
}

#[then("the subsidie is afgewezen")]
fn subsidie_afgewezen(world: &mut NappWorld) {
    assert_bool_output(world, "subsidie_toegekend", false);
}

#[then(regex = r#"^the subsidiebedrag is "(-?\d+)" eurocent$"#)]
fn check_subsidiebedrag(world: &mut NappWorld, expected: String) {
    assert_amount_output(world, "subsidiebedrag", expected.parse().unwrap());
}

#[then(regex = r#"^a betaalopdracht of "(-?\d+)" eurocent is required$"#)]
fn check_betaalopdracht(world: &mut NappWorld, expected: String) {
    assert_bool_output(world, "betaalopdracht_vereist", true);
    assert_amount_output(world, "betaalopdracht_bedrag", expected.parse().unwrap());
}

#[then("no betaalopdracht is required")]
fn no_betaalopdracht(world: &mut NappWorld) {
    assert_bool_output(world, "betaalopdracht_vereist", false);
}

#[then(regex = r#"^the bezwaartermijn is "(\d+)" weken$"#)]
fn check_bezwaartermijn(world: &mut NappWorld, expected: String) {
    assert_amount_output(world, "bezwaartermijn_weken", expected.parse().unwrap());
}

#[then("motivering is vereist")]
fn check_motivering(world: &mut NappWorld) {
    assert_bool_output(world, "motivering_vereist", true);
}

#[then(regex = r#"^the verlengde einddatum is "([^"]+)"$"#)]
fn check_verlengde_einddatum(world: &mut NappWorld, expected: String) {
    assert_success(world);
    let actual = world.get_output("verlengde_einddatum");
    assert!(
        actual.and_then(Value::as_str) == Some(expected.as_str()),
        "Expected verlengde_einddatum {expected}, got {actual:?}"
    );
}

#[then(regex = r#"^the output "([^"]+)" is "(-?\d+)" eurocent$"#)]
fn check_amount_output(world: &mut NappWorld, name: String, expected: String) {
    assert_amount_output(world, &name, expected.parse().unwrap());
}

fn assert_date_output(world: &NappWorld, name: &str, expected: &str) {
    assert_success(world);
    let actual = world.get_output(name);
    assert!(
        actual.and_then(Value::as_str) == Some(expected),
        "Expected output '{name}' to be {expected}, got {actual:?}"
    );
}

#[then(regex = r#"^the aanvraagtermijn ends on "([^"]+)"$"#)]
fn check_aanvraagtermijn(world: &mut NappWorld, expected: String) {
    assert_date_output(world, "aanvraagtermijn_einddatum", &expected);
}

#[then(regex = r#"^the beslistermijn ends on "([^"]+)"$"#)]
fn check_beslistermijn_einddatum(world: &mut NappWorld, expected: String) {
    assert_date_output(world, "beslistermijn_einddatum", &expected);
}

#[then(regex = r#"^the voorschotpercentage is "(\d+)"$"#)]
fn check_voorschotpercentage(world: &mut NappWorld, expected: String) {
    assert_amount_output(world, "voorschotpercentage", expected.parse().unwrap());
}

/// Generieke boolean-assertie op een wet-output.
#[then(regex = r#"^the output "([^"]+)" is (true|false)$"#)]
fn check_bool_output(world: &mut NappWorld, name: String, expected: String) {
    assert_bool_output(world, &name, expected == "true");
}
