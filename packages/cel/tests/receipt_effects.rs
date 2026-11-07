//! The receipt of a copy whose fact revokes the decision the ruling is
//! about (schema v0.8.15, `requires[].effects`). `Cell::record_receipt`
//! takes every field of the fact from the copy by name, also the id of the
//! record the effect revokes: the cell fills nothing, so what a receipt
//! revokes follows from the law and the recorded data. The fictional law and
//! reduction are those of the cell's BDD suite (`bdd/fixtures`).

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regelrecht_cel::reading::chronicle_records;
use regelrecht_cel::{Afschrift, Cell, Disk, Gram, Received, EXECUTOGRAM};
use regelrecht_engine::{LawExecutionService, ProjectionEvent, Value as EngineValue};
use serde_json::{json, Value};

mod common;
use common::day;

const CELL: &str = "test_cel";
const COURT: &str = "test_rechtbank";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures")
}

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let law = fixtures().join("regulation/test_chronicle_effects/2025-01-01.yaml");
    service
        .load_law(&std::fs::read_to_string(law).unwrap())
        .unwrap();
    let reduction =
        fixtures().join("cells/test_cel/reductions/test_reductie_geldige_kaart/2025-01-01.yaml");
    service
        .load_law(&std::fs::read_to_string(reduction).unwrap())
        .unwrap();
    service
}

fn gram(
    actor: &str,
    id: &str,
    kind: &str,
    basis: &str,
    refers_to: Option<&str>,
    fields: Value,
    outputs: Value,
) -> Gram {
    let map = |v: Value| -> BTreeMap<String, Value> {
        v.as_object().unwrap().clone().into_iter().collect()
    };
    Gram {
        id: id.to_string(),
        kind: kind.to_string(),
        basis: basis.to_string(),
        fact: None,
        refers_to: refers_to.map(str::to_string),
        received: None,
        after: None,
        recording_actor: actor.to_string(),
        recorded_on: day("2025-03-01"),
        moment: None,
        fields: map(fields),
        missing: Vec::new(),
        outputs: map(outputs),
    }
}

/// The applications and grants of `applicants`, as the cell recorded them:
/// `aanvraag_<x>` and `verlening_<x>` per applicant `burger_<x>`.
fn grants(applicants: &[&str]) -> Vec<Gram> {
    applicants
        .iter()
        .flat_map(|x| {
            let fields = json!({"ernstig_beperkt": true, "aanvrager": format!("burger_{x}")});
            [
                gram(
                    CELL,
                    &format!("aanvraag_{x}"),
                    "AANVRAAG",
                    "test_chronicle_effects#1",
                    None,
                    fields.clone(),
                    json!({}),
                ),
                gram(
                    CELL,
                    &format!("verlening_{x}"),
                    "BESCHIKKING",
                    "test_chronicle_effects#1",
                    Some(&format!("aanvraag_{x}")),
                    fields,
                    json!({"kaart_verleend": true}),
                ),
            ]
        })
        .collect()
}

/// A copy of the court's ruling on an appeal against `about`, a decision of
/// the cell. The ruling names the decision it is about by the id `names`
/// (normally `about` itself) and annuls it when `vernietigd`.
fn copy(about: &str, names: &str, vernietigd: bool) -> Afschrift {
    let ruling = gram(
        COURT,
        "uitspraak",
        "UITSPRAAK",
        "test_chronicle_effects#10",
        Some("beroep"),
        json!({"bestreden_besluit": names, "beroep_gegrond": vernietigd}),
        json!({"vernietigd": vernietigd}),
    );
    Afschrift {
        from: COURT.to_string(),
        value: json!({
            "basis": ruling.basis,
            "taken_on": "test_chronicle_effects#10",
            "fields": ruling.fields,
            "findings": {},
            "outputs": ruling.outputs,
        }),
        gram: ruling,
        about: Some(Received {
            from: CELL.to_string(),
            id: about.to_string(),
        }),
    }
}

/// The cell with `grams`, after it recorded the receipt of `copy`, which
/// concerns its decision `about`.
fn received(grams: Vec<Gram>, copy: &Afschrift, about: &str) -> (Cell, Gram) {
    let mut cell = Cell::restore(&Disk, CELL, fixtures().join("cells").join(CELL), grams).unwrap();
    let receipt = cell
        .record_receipt(
            &service(),
            "test_chronicle_effects#11#afschrift_uitspraak_ontvangen",
            copy,
            about,
            day("2025-05-10"),
        )
        .unwrap();
    (cell, receipt)
}

/// Whether `burger_<x>` holds a valid card on 2025-06-01, and the events.
fn holds_card(cell: &Cell, x: &str) -> Result<(bool, Vec<ProjectionEvent>), String> {
    let records = chronicle_records(cell.grams()).unwrap();
    let outcome = service()
        .evaluate_projection(
            "test_reductie_geldige_kaart",
            &["heeft_geldige_kaart"],
            &records,
            BTreeMap::from([(
                "aanvrager".to_string(),
                EngineValue::String(format!("burger_{x}")),
            )]),
            "2025-06-01",
            false,
        )
        .map_err(|e| e.to_string())?;
    let holds = outcome.result.outputs["heeft_geldige_kaart"] == EngineValue::Bool(true);
    Ok((holds, outcome.events))
}

#[test]
fn the_receipt_takes_the_id_it_revokes_from_the_copy() {
    for vernietigd in [true, false] {
        let (cell, receipt) = received(
            grants(&["x"]),
            &copy("verlening_x", "verlening_x", vernietigd),
            "verlening_x",
        );
        assert_eq!(receipt.kind, EXECUTOGRAM);
        assert_eq!(receipt.refers_to.as_deref(), Some("verlening_x"));
        // Every field comes from the copy; the cell adds none.
        assert_eq!(receipt.fields["bestreden_besluit"], json!("verlening_x"));
        assert_eq!(receipt.fields["vernietigd"], json!(vernietigd));
        assert_eq!(receipt.fields.len(), 2);
        // A ruling that annuls the grant takes the card; one that does not,
        // leaves it.
        assert_eq!(holds_card(&cell, "x").unwrap().0, !vernietigd);
    }
}

#[test]
fn a_receipt_that_names_an_unknown_record_is_refused_when_played() {
    // The cell records what the copy says; the projection refuses an effect
    // on a record that is no earlier record of the chronicle, as for a
    // decision.
    let (cell, _) = received(
        grants(&["x"]),
        &copy("verlening_x", "verlening_elders", true),
        "verlening_x",
    );
    let err = holds_card(&cell, "x").expect_err("no such record");
    assert!(
        err.contains("has an effect on 'verlening_elders', which is no record recorded before it"),
        "{err}"
    );
}

#[test]
fn a_receipt_that_names_a_record_of_another_key_revokes_it_for_that_key() {
    // The receipt concerns X's grant, but the copy names Y's. The law says
    // the receipt revokes what it names, so Y's grant no longer counts. For
    // X the receipt belongs (it refers to X's grant), but its effect is on a
    // record outside X's records, so X's grant counts.
    let (cell, _) = received(
        grants(&["x", "y"]),
        &copy("verlening_x", "verlening_y", true),
        "verlening_x",
    );
    let (x, _) = holds_card(&cell, "x").unwrap();
    assert!(x, "X's grant still counts");
    let (y, events) = holds_card(&cell, "y").unwrap();
    assert!(!y, "Y's grant is revoked");
    assert!(
        events.iter().any(|e| matches!(
            e,
            ProjectionEvent::Skipped { record, .. } if record == "verlening_y"
        )),
        "{events:?}"
    );
}
