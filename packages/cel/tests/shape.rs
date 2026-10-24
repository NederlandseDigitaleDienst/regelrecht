//! The shape of an application on small fictional laws: what the cell does
//! when two articles that take part ask the same datum, a moment that is no
//! date, and which values fit a field.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::NaiveDate;
use regelrecht_cel::{shape, Error};
use regelrecht_engine::{LawExecutionService, ParameterType};
use serde_json::json;

const SPECIAL: &str = r#"
$id: wet_verlof
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: De ouder kan verlof aanvragen en noemt daarbij zijn naam.
    machine_readable:
      execution:
        produces:
          submission: {kind: AANVRAAG}
        parameters:
          - name: naam_aanvrager
            type: string
            nullable: true
            required: false
        output:
          - {name: verlof_aangevraagd, type: boolean}
        actions:
          - {output: verlof_aangevraagd, value: true}
"#;

/// A hook on every application that asks the name too, as `type_`, and
/// absent or not as `nullable` says.
fn general(type_: &str, nullable: bool) -> String {
    format!(
        r#"
$id: wet_algemeen
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '9'
    text: De aanvraag bevat de naam van de aanvrager.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {{submission: AANVRAAG}}
      execution:
        parameters:
          - name: naam_aanvrager
            type: {type_}
            nullable: {nullable}
            required: false
        output:
          - {{name: naam_gevraagd, type: boolean}}
        actions:
          - {{output: naam_gevraagd, value: true}}
"#
    )
}

/// A hook on every application that names the receipt as the moment, held
/// in a parameter of type `type_`.
fn receipt(type_: &str) -> String {
    format!(
        r#"
$id: wet_ontvangst
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '2'
    text: De termijn loopt vanaf de ontvangst van de aanvraag.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {{submission: AANVRAAG}}
      execution:
        produces:
          moment:
            parameter: datum_ontvangst
            legal_basis: {{law: Wet ontvangst, article: '2'}}
        parameters:
          - name: datum_ontvangst
            type: {type_}
            required: false
        output:
          - {{name: ontvangst_gezien, type: boolean}}
        actions:
          - {{output: ontvangst_gezien, value: true}}
"#
    )
}

fn derive(general: &str) -> Result<shape::Shape, Error> {
    let mut service = LawExecutionService::new();
    service.load_law(SPECIAL).unwrap();
    service.load_law(general).unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    shape::derive(&service, "wet_verlof", "1", day)
}

#[test]
fn a_datum_two_articles_ask_alike_is_one_field_resting_on_both() {
    let shape = derive(&general("string", true)).unwrap();
    let field = shape.field("naam_aanvrager").unwrap();
    assert_eq!(
        field.declared_by,
        vec!["wet_verlof#1", "wet_algemeen#9"],
        "{field:?}"
    );
    assert!(field.nullable);
    assert_eq!(shape.fields.len(), 1);
}

#[test]
fn a_datum_can_be_absent_only_if_every_article_that_asks_it_says_so() {
    let shape = derive(&general("string", false)).unwrap();
    assert!(!shape.field("naam_aanvrager").unwrap().nullable);
}

#[test]
fn a_datum_two_articles_ask_differently_is_an_error() {
    let e = derive(&general("number", true)).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("with another type")),
        "{e}"
    );
}

#[test]
fn the_moment_is_no_field_of_the_application() {
    let shape = derive(&receipt("date")).unwrap();
    assert_eq!(
        shape.moment.as_ref().unwrap().parameter.as_deref(),
        Some("datum_ontvangst")
    );
    assert!(shape.field("datum_ontvangst").is_none());
}

#[test]
fn a_moment_that_is_no_date_is_an_error() {
    let e = derive(&receipt("string")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("not a Date")),
        "{e}"
    );
}

#[test]
fn a_date_fits_only_as_yyyy_mm_dd() {
    let field = shape::Field {
        name: "eerste_verlofdag".into(),
        type_: ParameterType::Date,
        nullable: false,
        declared_by: vec!["wet_verlof#1".into()],
    };
    assert!(shape::fits(&field, &json!("2027-02-01")));
    for odd in [
        "2027-2-1",
        "+2027-02-01",
        "2027-02-01 ",
        "2027-02-30",
        "1-2-2027",
    ] {
        assert!(!shape::fits(&field, &json!(odd)), "{odd}");
    }
}

#[test]
fn a_decision_fits_only_as_a_decision_value() {
    let field = shape::Field {
        name: "bestreden_besluit".into(),
        type_: ParameterType::Decision,
        nullable: false,
        declared_by: vec!["wet_heroverweging#1".into()],
    };
    let value = json!({
        "basis": "wet_verlof#1",
        "fields": {},
        "findings": {"reden": true},
        "outputs": {"verleend": true},
    });
    assert!(shape::fits(&field, &value));
    // As the cell gives one from its chronicle: with the article of the
    // submission it was taken on.
    let taken = json!({
        "basis": "wet_verlof#1",
        "taken_on": "wet_verlof#2",
        "fields": {},
        "findings": {},
        "outputs": {},
    });
    assert!(shape::fits(&field, &taken));
    for odd in [
        json!("019a-besluit"),
        json!({"basis": "wet_verlof", "fields": {}, "findings": {}, "outputs": {}}),
        json!({"basis": "wet_verlof#1", "fields": [], "findings": {}, "outputs": {}}),
        json!({"basis": "wet_verlof#1", "fields": {}, "findings": [], "outputs": {}}),
        json!({"basis": "wet_verlof#1", "fields": {}, "findings": {}}),
        json!({"basis": "wet_verlof#1", "fields": {}, "outputs": {}}),
        json!({"basis": "wet_verlof#1", "fields": {}, "findings": {}, "outputs": {}, "extra": 1}),
        json!({"basis": "wet_verlof#1", "taken_on": "wet_verlof", "fields": {}, "findings": {}, "outputs": {}}),
        json!({"basis": "wet_verlof#1", "taken_on": null, "fields": {}, "findings": {}, "outputs": {}}),
        json!({"basis": "wet_verlof#1", "taken_on": "wet_verlof#2", "fields": {}, "findings": {}, "outputs": {}, "extra": 1}),
        json!(null),
    ] {
        assert!(!shape::fits(&field, &odd), "{odd}");
    }
}
