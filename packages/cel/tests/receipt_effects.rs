//! A fact that revokes the record it refers to (schema v0.8.15,
//! `requires[].effects` with `revokes: refers_to`). The receipt of a copy
//! refers to the decision the copy concerns (RFC-053), and that is the
//! record it revokes, whatever the copy holds besides; another fact with
//! effects may refer to an earlier record through
//! `Cell::record_executogram_referring`. The fictional law and reduction are
//! those of the cell's BDD suite (`bdd/fixtures`).

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use regelrecht_cel::reading::{chronicle_records, Reader};
use regelrecht_cel::{Afschrift, Cell, Disk, Gram, Received, EXECUTOGRAM};
use regelrecht_engine::{
    Authority, Call, LawExecutionService, PathNode, ProjectionEvent, Value as EngineValue,
};
use serde_json::{json, Value};

mod common;
use common::{answered, day, object_map, service_of};

const CELL: &str = "test_cel";
const COURT: &str = "test_rechtbank";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures")
}

fn service() -> LawExecutionService {
    let read = |path: &str| std::fs::read_to_string(fixtures().join(path)).unwrap();
    service_of(&[
        &read("regulation/test_chronicle_effects/2025-01-01.yaml"),
        &read("cells/test_cel/reductions/test_reductie_geldige_kaart/2025-01-01.yaml"),
    ])
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
    let map = |v: Value| -> BTreeMap<String, Value> { object_map(v).into_iter().collect() };
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
/// the cell. The ruling holds a field `bestreden_besluit` that names
/// `names` (normally `about` itself), and annuls the decision when
/// `vernietigd`.
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

fn restored(grams: Vec<Gram>) -> Cell {
    Cell::restore(&Disk, CELL, fixtures().join("cells").join(CELL), grams).unwrap()
}

/// The cell with `grams`, after it recorded the receipt of `copy`, which
/// concerns its decision `about`.
fn received(grams: Vec<Gram>, copy: &Afschrift, about: &str) -> (Cell, Gram) {
    let mut cell = restored(grams);
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

/// Whether `burger_<x>` holds a valid card on 2025-10-01, and the events.
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
            "2025-10-01",
            false,
        )
        .map_err(|e| e.to_string())?;
    let holds = outcome.result.outputs["heeft_geldige_kaart"] == EngineValue::Bool(true);
    Ok((holds, outcome.events))
}

fn skipped(events: &[ProjectionEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            ProjectionEvent::Skipped { record, .. } => Some(record.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_receipt_revokes_the_decision_it_refers_to() {
    for vernietigd in [true, false] {
        let (cell, receipt) = received(
            grants(&["x"]),
            &copy("verlening_x", "verlening_x", vernietigd),
            "verlening_x",
        );
        assert_eq!(receipt.kind, EXECUTOGRAM);
        assert_eq!(receipt.refers_to.as_deref(), Some("verlening_x"));
        // The fact holds what the copy gives, and nothing that names a
        // record: which record it is about is its `refers_to` alone.
        assert_eq!(receipt.fields.len(), 1);
        assert_eq!(receipt.fields["vernietigd"], json!(vernietigd));
        let (holds, events) = holds_card(&cell, "x").unwrap();
        assert_eq!(holds, !vernietigd);
        let expected: &[&str] = if vernietigd { &["verlening_x"] } else { &[] };
        assert_eq!(skipped(&events), expected);
    }
}

#[test]
fn the_revoked_record_is_always_the_one_the_receipt_refers_to() {
    // The copy concerns X's grant, while the ruling names Y's grant in a
    // field of its own. The receipt refers to X's grant and revokes that
    // one; Y's grant still counts.
    let (cell, _) = received(
        grants(&["x", "y"]),
        &copy("verlening_x", "verlening_y", true),
        "verlening_x",
    );
    let (x, events) = holds_card(&cell, "x").unwrap();
    assert!(!x, "X's grant is revoked");
    assert_eq!(skipped(&events), ["verlening_x"]);
    let (y, events) = holds_card(&cell, "y").unwrap();
    assert!(y, "Y's grant still counts");
    assert!(skipped(&events).is_empty(), "{events:?}");
}

#[test]
fn a_fact_with_effects_may_refer_to_an_earlier_record() {
    // The ruling annulled on further appeal (article 12): the fact refers
    // to the receipt and revokes it, so the grant counts again.
    let (mut cell, receipt) = received(
        grants(&["x"]),
        &copy("verlening_x", "verlening_x", true),
        "verlening_x",
    );
    let fact = cell
        .record_executogram_referring(
            &service(),
            "test_chronicle_effects#12#uitspraak_vernietigd_in_hoger_beroep",
            &object_map(json!({"uitspraak_in_hoger_beroep_op": "2025-09-01"})),
            Some(&receipt.id),
            day("2025-09-01"),
        )
        .unwrap();
    assert_eq!(fact.refers_to.as_deref(), Some(receipt.id.as_str()));
    let (holds, events) = holds_card(&cell, "x").unwrap();
    assert!(holds);
    assert_eq!(skipped(&events), [receipt.id.as_str()]);
    // The fact still reads back as a chronicle of the cell.
    restored(cell.grams().to_vec());
}

#[test]
fn only_a_fact_with_effects_refers_and_only_to_a_record_of_the_chronicle() {
    let mut cell = restored(grants(&["x"]));
    // Article 5 requires the fact of handing a card in, without effects.
    let err = cell
        .record_executogram_referring(
            &service(),
            "test_chronicle_effects#5#kaart_ingeleverd",
            &object_map(json!({"ingeleverd_op": "2025-04-01", "aanvrager": "burger_x"})),
            Some("verlening_x"),
            day("2025-04-01"),
        )
        .expect_err("a fact without effects refers to nothing");
    assert!(
        err.to_string().contains(
            "the fact test_chronicle_effects#5#kaart_ingeleverd refers to 'verlening_x', but \
             declares no effects"
        ),
        "{err}"
    );
    let err = cell
        .record_executogram_referring(
            &service(),
            "test_chronicle_effects#12#uitspraak_vernietigd_in_hoger_beroep",
            &object_map(json!({"uitspraak_in_hoger_beroep_op": "2025-04-01"})),
            Some("elders"),
            day("2025-04-01"),
        )
        .expect_err("no such record");
    assert!(
        err.to_string()
            .contains("refers to 'elders', which is no chronolexogram in the chronicle"),
        "{err}"
    );
    // Opening refuses a fact that refers to a record that is not before
    // it, also when other records are.
    let mut tampered = cell.grams().to_vec();
    let mut fact = tampered[0].clone();
    fact.id = "feit".to_string();
    fact.kind = EXECUTOGRAM.to_string();
    fact.fact = Some("uitspraak_vernietigd_in_hoger_beroep".to_string());
    fact.basis = "test_chronicle_effects#12".to_string();
    fact.refers_to = Some("verlening_x".to_string());
    fact.fields = BTreeMap::from([(
        "uitspraak_in_hoger_beroep_op".to_string(),
        json!("2025-03-01"),
    )]);
    fact.outputs = BTreeMap::new();
    // After the application, before the grant it refers to.
    tampered.insert(1, fact);
    let err = Cell::restore(&Disk, CELL, fixtures().join("cells").join(CELL), tampered)
        .expect_err("refers to no earlier record");
    assert!(
        err.to_string()
            .contains("refers to 'verlening_x', which is not a chronolexogram before it"),
        "{err}"
    );
}

#[test]
fn the_trace_of_a_decision_holds_the_projection_under_the_answer() {
    // The policy binds the term of article 9 to the cell's answer. Traced,
    // the engine nests how the cell came to it under the node of the
    // answer: the run of the reduction, with the projection it played and
    // the law the step called under the record that counted.
    let read = |path: &str| std::fs::read_to_string(fixtures().join(path)).unwrap();
    let service = service_of(&[
        &read("regulation/test_chronicle_effects/2025-01-01.yaml"),
        &read("regulation/test_beleid_geldige_kaart/2025-01-01.yaml"),
        &read("cells/test_cel/reductions/test_reductie_geldige_kaart/2025-01-01.yaml"),
    ]);
    let authority = Authority {
        name: "het college van de testcel".to_string(),
        category: None,
    };
    let answers = BTreeMap::from([(
        "heeft_geldige_kaart".to_string(),
        "test_reductie_geldige_kaart".to_string(),
    )]);
    let reductions = BTreeSet::from(["test_reductie_geldige_kaart".to_string()]);
    let reader = Reader::new(
        CELL,
        &authority,
        &answers,
        &reductions,
        grants(&["x"]),
        None,
    );
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&reader),
        chronicle: Some(&reader),
        sphere: Vec::new(),
    };
    let result = service
        .evaluate_law_with_trace_in(
            "test_chronicle_effects",
            &["mag_parkeren"],
            BTreeMap::from([(
                "aanvrager".to_string(),
                EngineValue::String("burger_x".to_string()),
            )]),
            call,
            "2025-03-01",
        )
        .unwrap();
    assert_eq!(result.outputs["mag_parkeren"], EngineValue::Bool(true));
    let root = result.trace.as_ref().unwrap();
    let [answer] = answered(root)[..] else {
        panic!("one answer: {}", root.render_box_drawing());
    };
    assert_eq!(answer.name, "heeft_geldige_kaart");
    // Under the answer: the reduction's own run, and the projection first.
    let [run] = &answer.children[..] else {
        panic!("the reduction's run: {}", root.render_box_drawing());
    };
    let projection = &run.children[0];
    assert_eq!(projection.name, "PROJECTION");
    let names = |n: &PathNode| {
        n.children
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(projection), ["COUNTS", "FOLD"]);
    assert_eq!(names(&projection.children[0]), ["COUNT", "COUNT"]);
    let fold = &projection.children[1];
    assert_eq!(names(fold), ["APPLY", "APPLY"]);
    let grant = &fold.children[1];
    assert!(
        grant
            .message
            .as_deref()
            .unwrap_or("")
            .starts_with("verlening_x: test_chronicle_effects#7"),
        "{grant:?}"
    );
    assert!(
        !grant.children.is_empty(),
        "the law the step called: {grant:?}"
    );
    // Every node keeps an id of its own in the one tree.
    let mut ids = Vec::new();
    collect_ids(root, &mut ids);
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "{ids:?}");
    // Under the answer, the run is its first child and the projection the
    // first child of the run.
    let answer_id = answer.node_id.as_deref().unwrap();
    assert_eq!(
        projection.node_id.as_deref(),
        Some(format!("{answer_id}.0.0").as_str())
    );
}

/// The `node_id` of `node` and of every node under it.
fn collect_ids(node: &PathNode, ids: &mut Vec<String>) {
    ids.extend(node.node_id.clone());
    for child in &node.children {
        collect_ids(child, ids);
    }
}
