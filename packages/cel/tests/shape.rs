//! The shape of an application on small fictional laws: what the cell does
//! when two articles that take part ask the same datum, a moment that is no
//! date, a table the application holds, and which values fit a field.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::NaiveDate;
use regelrecht_cel::{Error, shape};
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
        columns: None,
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
        columns: None,
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

/// An application that holds a table (schema v0.8.20): per row a name, a
/// date, an amount, a number and a yes or no, as `columns` declares them.
/// `hook_columns` are the columns a hook on every application gives the same
/// datum, or `None` when no hook asks it.
fn table(hook_columns: Option<&str>) -> Result<shape::Shape, Error> {
    const COLUMNS: &str = "[{name: naam, type: string}, {name: sinds, type: date}, \
                           {name: bijdrage, type: amount}, {name: aantal, type: number}, \
                           {name: actief, type: boolean}]";
    let special = format!(
        r#"
$id: wet_tabel
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: De vereniging vraagt inschrijving aan en noemt haar leden.
    machine_readable:
      execution:
        produces:
          submission: {{kind: AANVRAAG}}
        parameters:
          - name: leden
            type: array
            columns: {COLUMNS}
        output:
          - {{name: aangevraagd, type: boolean}}
        actions:
          - {{output: aangevraagd, value: true}}
"#
    );
    let mut service = LawExecutionService::new();
    service.load_law(&special).unwrap();
    if let Some(columns) = hook_columns {
        service
            .load_law(&format!(
                r#"
$id: wet_algemeen
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '9'
    text: De aanvraag noemt de leden.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {{submission: AANVRAAG}}
      execution:
        parameters:
          - name: leden
            type: array
            columns: {columns}
        output:
          - {{name: leden_gevraagd, type: boolean}}
        actions:
          - {{output: leden_gevraagd, value: true}}
"#
            ))
            .unwrap();
    }
    let day = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    shape::derive(&service, "wet_tabel", "1", day)
}

#[test]
fn a_table_field_carries_its_columns() {
    let shape = table(None).unwrap();
    let field = shape.field("leden").unwrap();
    assert_eq!(field.type_, ParameterType::Array);
    let columns: Vec<(&str, ParameterType)> = field
        .columns
        .as_deref()
        .unwrap()
        .iter()
        .map(|c| (c.name.as_str(), c.column_type))
        .collect();
    assert_eq!(
        columns,
        [
            ("naam", ParameterType::String),
            ("sinds", ParameterType::Date),
            ("bijdrage", ParameterType::Amount),
            ("aantal", ParameterType::Number),
            ("actief", ParameterType::Boolean),
        ]
    );
    // The form reads them under `columns`; a field without them has none.
    let json = serde_json::to_value(field).unwrap();
    assert_eq!(json["columns"][1], json!({"name": "sinds", "type": "date"}));
    let plain = derive(&general("string", true)).unwrap();
    let json = serde_json::to_value(plain.field("naam_aanvrager").unwrap()).unwrap();
    assert!(json.get("columns").is_none(), "{json}");
}

#[test]
fn a_table_two_articles_ask_with_the_same_columns_is_one_field() {
    let same = "[{name: naam, type: string}, {name: sinds, type: date}, \
                {name: bijdrage, type: amount}, {name: aantal, type: number}, \
                {name: actief, type: boolean}]";
    let shape = table(Some(same)).unwrap();
    assert_eq!(
        shape.field("leden").unwrap().declared_by,
        vec!["wet_tabel#1", "wet_algemeen#9"]
    );
}

#[test]
fn a_table_two_articles_describe_differently_is_still_one_field() {
    let described = "[{name: naam, type: string, description: De naam van het lid.}, \
                     {name: sinds, type: date}, {name: bijdrage, type: amount}, \
                     {name: aantal, type: number}, {name: actief, type: boolean}]";
    let shape = table(Some(described)).unwrap();
    assert_eq!(shape.field("leden").unwrap().declared_by.len(), 2);
}

#[test]
fn a_table_two_articles_ask_with_other_columns_is_an_error() {
    let e = table(Some("[{name: naam, type: string}]")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("with other columns")),
        "{e}"
    );
}

#[test]
fn a_table_whose_columns_differ_in_one_name_or_one_type_is_another_table() {
    // As many columns each time, so only the name or the type tells them apart.
    for columns in [
        "[{name: naam, type: string}, {name: sinds, type: string}, \
          {name: bijdrage, type: amount}, {name: aantal, type: number}, \
          {name: actief, type: boolean}]",
        "[{name: naam, type: string}, {name: datum, type: date}, \
          {name: bijdrage, type: amount}, {name: aantal, type: number}, \
          {name: actief, type: boolean}]",
    ] {
        let e = table(Some(columns)).unwrap_err();
        assert!(
            matches!(&e, Error::Setup(m) if m.contains("with other columns")),
            "{columns}: {e}"
        );
    }
}

#[test]
fn a_table_one_article_asks_with_columns_and_another_without_is_an_error() {
    // A bare array says nothing of its rows; one chronolexogram cannot hold
    // a table that one article reads by its columns and another does not.
    let e = table(Some("null")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("with other columns")),
        "{e}"
    );
}

#[test]
fn a_table_fits_only_rows_of_its_columns() {
    let shape = table(None).unwrap();
    let field = shape.field("leden").unwrap();
    let row = json!({
        "naam": "Lid Een",
        "sinds": "2026-02-01",
        "bijdrage": 1250,
        "aantal": 2.5,
        "actief": true,
    });
    assert!(shape::fits(field, &json!([])));
    assert!(shape::fits(field, &json!([row])));
    // Every column may hold null, and a row lacking a column holds null
    // there (schema v0.8.20).
    assert!(shape::fits(field, &json!([{"naam": null, "aantal": null}])));
    assert!(shape::fits(field, &json!([{}])));
    // A decimal as the chronicle holds it fits a number column.
    assert!(shape::fits(
        field,
        &json!([{"aantal": {"__decimal": "2.5"}}])
    ));
    for odd in [
        json!(["Lid Een"]),
        json!([{"onbekend": 1}]),
        json!([{"naam": 3}]),
        json!([{"sinds": "1-2-2026"}]),
        json!([{"bijdrage": 12.5}]),
        json!([{"aantal": "2"}]),
        json!([{"actief": "ja"}]),
        json!([row, {"naam": []}]),
        json!({"naam": "Lid Een"}),
        json!("Lid Een"),
        json!(null),
    ] {
        assert!(!shape::fits(field, &odd), "{odd}");
    }
}

#[test]
fn a_nullable_table_may_be_absent() {
    let shape = table(None).unwrap();
    let mut field = shape.field("leden").unwrap().clone();
    field.nullable = true;
    assert!(shape::fits(&field, &json!(null)));
    assert!(!shape::fits(&field, &json!("Lid Een")));
}
