//! A cell answers from a reduction that plays its chronicle forward
//! (`Cell::answer`), on the fixtures of the cell's BDD suite: the fictional
//! law test_chronicle_effects and the reductions of the cell test_cel.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::{Cell, Disk, Gram};
use regelrecht_engine::{LawExecutionService, Value};
use serde_json::json;
use walkdir::WalkDir;

const CELL: &str = "test_cel";

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures")
}

/// Every regelwerk of the fixtures: the regulation and the reductions.
fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    for entry in WalkDir::new(fixtures()).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let rel = path.strip_prefix(fixtures()).unwrap();
        let regelwerk = rel.starts_with("regulation")
            || rel.components().any(|c| c.as_os_str() == "reductions");
        if regelwerk && path.extension().is_some_and(|x| x == "yaml") {
            service
                .load_law(&std::fs::read_to_string(path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
    }
    service
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn gram(
    id: &str,
    kind: &str,
    refers_to: Option<&str>,
    fields: serde_json::Value,
    outputs: serde_json::Value,
) -> Gram {
    Gram {
        id: id.to_string(),
        kind: kind.to_string(),
        basis: "test_chronicle_effects#1".to_string(),
        fact: None,
        refers_to: refers_to.map(str::to_string),
        received: None,
        after: None,
        recording_actor: CELL.to_string(),
        recorded_on: day("2025-03-01"),
        moment: None,
        fields: serde_json::from_value(fields).unwrap(),
        missing: Vec::new(),
        outputs: serde_json::from_value(outputs).unwrap(),
    }
}

#[test]
fn the_cell_answers_from_the_records_it_knew() {
    let service = service();
    let grams = vec![
        gram(
            "aanvraag",
            "AANVRAAG",
            None,
            json!({"ernstig_beperkt": true, "aanvrager": "burger_x"}),
            json!({}),
        ),
        gram(
            "verlening",
            "BESCHIKKING",
            Some("aanvraag"),
            json!({"ernstig_beperkt": true, "aanvrager": "burger_x"}),
            json!({"kaart_verleend": true}),
        ),
    ];
    let cell = Cell::restore(&Disk, CELL, fixtures().join("cells").join(CELL), grams).unwrap();
    let parameters = BTreeMap::from([(
        "aanvrager".to_string(),
        Value::String("burger_x".to_string()),
    )]);
    let answer = |known_on: &str| {
        cell.answer(
            &service,
            "heeft_geldige_kaart",
            &parameters,
            day("2025-03-02"),
            day(known_on),
        )
        .unwrap()
        .expect("the cell gives the answer")
        .value
    };
    assert_eq!(answer("2025-03-02"), Value::Bool(true));
    // Before the grant was recorded, the cell knew of no card.
    assert_eq!(answer("2025-02-28"), Value::Bool(false));
    // An answer the catalogue does not give: the cell is silent.
    assert!(cell
        .answer(
            &service,
            "onbekend",
            &parameters,
            day("2025-03-02"),
            day("2025-03-02")
        )
        .unwrap()
        .is_none());
}
