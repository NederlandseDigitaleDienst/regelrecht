//! Consulting an output before anything is recorded ([`Cell::consult`]),
//! under a small fictional regulation: which parameters the article gets,
//! the day a date parameter left out takes, and what the cell refuses.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{bare_cell, day, object_map, service_of};

use regelrecht_engine::Value;
use serde_json::{Map, json};

/// Art. 1 reads its source on one day and echoes it, with the number it is
/// asked about. Art. 2 reads a period, with two dates and an optional third.
const LAW: &str = r#"
$id: regeling_peildatum_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Het register wordt gelezen op de peildatum.
    machine_readable:
      execution:
        parameters:
          - {name: peildatum, type: date}
          - {name: nummer, type: string, required: false, nullable: true}
        output:
          - {name: gelezen_op, type: date}
          - {name: gelezen_nummer, type: string, nullable: true}
        actions:
          - output: gelezen_op
            value: $peildatum
          - output: gelezen_nummer
            value: $nummer
  - number: '2'
    text: Het register wordt gelezen over een tijdvak.
    machine_readable:
      execution:
        parameters:
          - {name: van, type: date}
          - {name: tot, type: date}
          - {name: herzien_op, type: date, required: false, nullable: true}
        output:
          - {name: tijdvak_begint, type: date}
        actions:
          - output: tijdvak_begint
            value: $van
"#;

fn output(source: &str, given: &Map<String, serde_json::Value>, on: &str) -> Value {
    let service = service_of(&[LAW]);
    let cell = bare_cell("register");
    let (_, name) = source.rsplit_once('#').unwrap();
    let result = cell.consult(&service, &[], source, given, day(on)).unwrap();
    result.outputs[name].clone()
}

#[test]
fn the_one_date_left_out_is_the_day_of_asking() {
    let gelezen = output(
        "regeling_peildatum_fictief#1#gelezen_op",
        &Map::new(),
        "2026-05-04",
    );
    assert_eq!(gelezen, Value::from(json!("2026-05-04")));
}

#[test]
fn a_given_value_reaches_the_article_and_a_given_date_is_kept() {
    let given = object_map(json!({ "peildatum": "2026-02-01", "nummer": "123", "ander": 1 }));
    let op = output(
        "regeling_peildatum_fictief#1#gelezen_op",
        &given,
        "2026-05-04",
    );
    assert_eq!(op, Value::from(json!("2026-02-01")));
    let nummer = output(
        "regeling_peildatum_fictief#1#gelezen_nummer",
        &given,
        "2026-05-04",
    );
    assert_eq!(nummer, Value::from(json!("123")));
}

#[test]
fn a_parameter_of_another_type_left_out_stays_out() {
    let nummer = output(
        "regeling_peildatum_fictief#1#gelezen_nummer",
        &Map::new(),
        "2026-05-04",
    );
    // Not passed, so unknown to the article; not the day of asking.
    assert!(matches!(nummer, Value::Unknown(_)), "{nummer:?}");
}

#[test]
fn two_dates_left_out_are_refused() {
    let service = service_of(&[LAW]);
    let e = bare_cell("register")
        .consult(
            &service,
            &[],
            "regeling_peildatum_fictief#2#tijdvak_begint",
            &Map::new(),
            day("2026-05-04"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("takes the dates van, tot"), "{e}");
}

#[test]
fn an_output_the_article_does_not_have_is_refused() {
    let service = service_of(&[LAW]);
    let cell = bare_cell("register");
    for (source, why) in [
        (
            "regeling_peildatum_fictief#1#tijdvak_begint",
            "has no output 'tijdvak_begint'",
        ),
        (
            "regeling_peildatum_fictief#3#gelezen_op",
            "has no article 3 in force",
        ),
        (
            "regeling_peildatum_fictief#1",
            "is no <regulation>#<article>#<fact>",
        ),
    ] {
        let e = cell
            .consult(&service, &[], source, &Map::new(), day("2026-05-04"))
            .unwrap_err();
        assert!(e.to_string().contains(why), "{source}: {e}");
    }
}

#[test]
fn an_optional_date_left_out_does_not_count() {
    let given = object_map(json!({ "van": "2026-01-01" }));
    let begint = output(
        "regeling_peildatum_fictief#2#tijdvak_begint",
        &given,
        "2026-05-04",
    );
    assert_eq!(begint, Value::from(json!("2026-01-01")));
}

#[test]
fn an_output_the_engine_takes_from_another_article_is_refused() {
    // Two articles that give the same output: the engine runs one of them
    // for it, and the cell does not pass that off as the other.
    let twice = r#"
$id: regeling_dubbel_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: Eerste.
    machine_readable:
      execution:
        output:
          - {name: uitkomst, type: number}
        actions:
          - {output: uitkomst, value: 1}
  - number: '2'
    text: Tweede.
    machine_readable:
      execution:
        output:
          - {name: uitkomst, type: number}
        actions:
          - {output: uitkomst, value: 2}
"#;
    let service = service_of(&[twice]);
    let cell = bare_cell("register");
    let results: Vec<_> = ["1", "2"]
        .iter()
        .map(|article| {
            cell.consult(
                &service,
                &[],
                &format!("regeling_dubbel_fictief#{article}#uitkomst"),
                &Map::new(),
                day("2026-05-04"),
            )
        })
        .collect();
    let refused: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert_eq!(refused.len(), 1, "{results:?}");
    assert!(
        refused[0]
            .to_string()
            .contains("gives 'uitkomst' from article")
    );
}
