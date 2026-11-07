//! The receipt of a copy whose fact revokes the decision the copy concerns
//! (schema v0.8.15, `requires[].effects`). `Cell::record_receipt` fills a
//! field of type `decision` that an effect revokes with the id of that
//! decision; the other fields come from the copy. The fictional law is the
//! one of the cell's BDD suite (`bdd/fixtures`), articles 1, 10 and 11.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regelrecht_cel::{Afschrift, Cell, Disk, Gram, Received, EXECUTOGRAM};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Value};

mod common;
use common::day;

const CELL: &str = "test_cel";
const COURT: &str = "test_rechtbank";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures")
}

/// A fictional law whose receipt fact revokes two decisions at once, which
/// the cell cannot fill: a copy concerns one decision.
const TWO_DECISIONS: &str = r#"
$id: test_twee_besluiten
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: FICTIEF. Het college legt de ontvangst van een afschrift vast.
    machine_readable:
      requires:
        - fact: ontvangen
          parameters: [eerste, tweede]
          effects:
            - revokes: eerste
            - revokes: tweede
      execution:
        parameters:
          - {name: eerste, type: decision}
          - {name: tweede, type: decision}
"#;

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let law = fixtures().join("regulation/test_chronicle_effects/2025-01-01.yaml");
    service
        .load_law(&std::fs::read_to_string(law).unwrap())
        .unwrap();
    service.load_law(TWO_DECISIONS).unwrap();
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

/// The cell with a grant, and a copy of the court's ruling that annuls it
/// (`vernietigd`), concerning that grant. The ruling also holds a field
/// `vernietigd_besluit` of its own, which the receipt must not take.
fn case(vernietigd: bool) -> (Cell, Afschrift) {
    let grams = vec![
        gram(
            CELL,
            "aanvraag",
            "AANVRAAG",
            "test_chronicle_effects#1",
            None,
            json!({"ernstig_beperkt": true, "aanvrager": "burger_x"}),
            json!({}),
        ),
        gram(
            CELL,
            "verlening",
            "BESCHIKKING",
            "test_chronicle_effects#1",
            Some("aanvraag"),
            json!({"ernstig_beperkt": true, "aanvrager": "burger_x"}),
            json!({"kaart_verleend": true}),
        ),
    ];
    let cell = Cell::restore(&Disk, CELL, fixtures().join("cells").join(CELL), grams).unwrap();
    let ruling = gram(
        COURT,
        "uitspraak",
        "UITSPRAAK",
        "test_chronicle_effects#10",
        Some("beroep"),
        json!({"bestreden_besluit": "verlening", "beroep_gegrond": vernietigd, "vernietigd_besluit": "iets anders"}),
        json!({"vernietigd": vernietigd}),
    );
    let copy = Afschrift {
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
            id: "verlening".to_string(),
        }),
    };
    (cell, copy)
}

#[test]
fn the_receipt_holds_the_decision_the_copy_concerns_and_what_the_copy_gives() {
    for vernietigd in [true, false] {
        let (mut cell, copy) = case(vernietigd);
        let receipt = cell
            .record_receipt(
                &service(),
                "test_chronicle_effects#11#afschrift_uitspraak_ontvangen",
                &copy,
                "verlening",
                day("2025-05-10"),
            )
            .unwrap();
        assert_eq!(receipt.kind, EXECUTOGRAM);
        assert_eq!(receipt.refers_to.as_deref(), Some("verlening"));
        // The cell fills the field the effect revokes, not the copy.
        assert_eq!(receipt.fields["vernietigd_besluit"], json!("verlening"));
        assert_eq!(receipt.fields["vernietigd"], json!(vernietigd));
        assert_eq!(receipt.fields.len(), 2);
    }
}

#[test]
fn a_receipt_cannot_revoke_two_decisions() {
    let (mut cell, copy) = case(true);
    let err = cell
        .record_receipt(
            &service(),
            "test_twee_besluiten#1#ontvangen",
            &copy,
            "verlening",
            day("2025-05-10"),
        )
        .expect_err("two decisions to fill");
    assert!(
        err.to_string().contains(
            "the receipt of test_twee_besluiten#1#ontvangen revokes 2 decisions (eerste, \
             tweede); the cell fills one, with the decision the copy concerns"
        ),
        "{err}"
    );
}
