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

use regelrecht_cel::reading::{Reader, chronicle_records};
use regelrecht_cel::{Afschrift, Cell, Disk, EXECUTOGRAM, Gram, Received};
use regelrecht_engine::{
    Authority, Call, LawExecutionService, PathNode, ProjectionEvent, Value as EngineValue,
};
use serde_json::{Value, json};

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
        &read("cells/test_cel/reductions/test_reductie_kaart_per_besluit/2025-01-01.yaml"),
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

/// Whether `burger_<x>` holds a valid card on 2025-10-01, and the events
/// of the aggregates the read model read.
fn holds_card(cell: &Cell, x: &str) -> Result<(bool, Vec<ProjectionEvent>), String> {
    let records = chronicle_records(cell.grams()).unwrap();
    let service = service();
    let aanvrager = BTreeMap::from([(
        "aanvrager".to_string(),
        EngineValue::String(format!("burger_{x}")),
    )]);
    let outcome = service
        .evaluate_projection(
            "test_reductie_geldige_kaart",
            &["heeft_geldige_kaart"],
            &records,
            aanvrager.clone(),
            "2025-10-01",
            false,
        )
        .map_err(|e| e.to_string())?;
    let holds = outcome.result.outputs["heeft_geldige_kaart"] == EngineValue::Bool(true);
    let events = service
        .aggregates(
            "test_reductie_kaart_per_besluit",
            &records,
            &aanvrager,
            "2025-10-01",
        )
        .map_err(|e| e.to_string())?
        .into_iter()
        .flat_map(|a| a.events)
        .collect();
    Ok((holds, events))
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

/// A withdrawal ex officio (fictional article 4) is taken on no
/// submission and refers to the grant it withdraws, which it revokes. The
/// cell refuses one about a record it does not hold, and one whose effect
/// names a grant it is not about.
#[test]
fn a_decision_ex_officio_refers_to_the_record_it_is_about() {
    let mut cell = restored(grants(&["x", "y"]));
    // The Awb gives a decision its stages (procedure).
    let mut service = service();
    common::load(
        &mut service,
        ["regulation/nl/wet/algemene_wet_bestuursrecht"],
    );
    let mut withdraw = |cell: &mut Cell, about: &str, withdrawn: &str| {
        cell.decide_ex_officio(
            &mut service,
            &[],
            "test_chronicle_effects",
            "4",
            Some(about),
            &object_map(json!({"ingetrokken_besluit": withdrawn})),
            &object_map(json!({})),
            day("2025-04-01"),
        )
    };
    let err = withdraw(&mut cell, "elders", "elders").expect_err("no such record");
    assert!(
        err.to_string()
            .contains("refers to 'elders', which is no chronolexogram in the chronicle"),
        "{err}"
    );
    let err = withdraw(&mut cell, "verlening_x", "verlening_y").expect_err("not about y's grant");
    assert!(
        err.to_string().contains(
            "revokes 'verlening_y' (ingetrokken_besluit), which its refers_to does not reach"
        ),
        "{err}"
    );
    let taken = withdraw(&mut cell, "verlening_x", "verlening_x").unwrap();
    let [intrekking] = &taken[..] else {
        panic!("one decision: {taken:?}");
    };
    assert_eq!(intrekking.kind, "BESCHIKKING");
    assert_eq!(intrekking.basis, "test_chronicle_effects#4");
    assert_eq!(intrekking.refers_to.as_deref(), Some("verlening_x"));
    assert_eq!(intrekking.after, None);
    assert_eq!(intrekking.outputs["ingetrokken"], json!(true));
    assert!(!holds_card(&cell, "x").unwrap().0);
    assert!(holds_card(&cell, "y").unwrap().0);
    // The chronicle opens again with it.
    restored(cell.grams().to_vec());
    // It is taken on no submission, so the cell does not replay it.
    let err = cell
        .explain(&mut service, &[], &intrekking.id)
        .expect_err("not replayed");
    assert!(
        err.to_string()
            .contains("was taken ex officio, about 'verlening_x', which is no submission"),
        "{err}"
    );
    // A decision is taken on an application: a grant under the same
    // article is not one.
    let err = cell
        .decide(
            &mut service,
            &[],
            "test_chronicle_effects",
            "1",
            "verlening_y",
            &object_map(json!({})),
            &object_map(json!({})),
            day("2025-04-02"),
        )
        .expect_err("no application");
    assert!(
        err.to_string().contains(
            "chronolexogram 'verlening_y' is a BESCHIKKING under test_chronicle_effects#1, not an \
             AANVRAAG under test_chronicle_effects#1"
        ),
        "{err}"
    );
}

#[test]
fn a_fact_refers_only_to_a_record_of_the_chronicle() {
    let mut cell = restored(grants(&["x"]));
    // Article 5 requires the fact of handing a card in, without effects. It
    // may still be about the grant: refers_to names the record a record is
    // about (schema v0.8.17), whether or not the law gives it an effect.
    let fact = cell
        .record_executogram_referring(
            &service(),
            "test_chronicle_effects#5#kaart_ingeleverd",
            &object_map(json!({"ingeleverd_op": "2025-04-01", "aanvrager": "burger_x"})),
            Some("verlening_x"),
            day("2025-04-01"),
        )
        .unwrap();
    assert_eq!(fact.refers_to.as_deref(), Some("verlening_x"));
    // Up to v0.8.16 only a fact with effects refers to a record.
    let read = |path: &str| std::fs::read_to_string(fixtures().join(path)).unwrap();
    let older = service_of(&[
        &read("regulation/test_chronicle_effects/2025-01-01.yaml").replace(
            "schema-v0.8.17/schema/v0.8.17",
            "schema-v0.8.16/schema/v0.8.16",
        ),
    ]);
    let err = cell
        .record_executogram_referring(
            &older,
            "test_chronicle_effects#5#kaart_ingeleverd",
            &object_map(json!({"ingeleverd_op": "2025-04-01", "aanvrager": "burger_x"})),
            Some("verlening_x"),
            day("2025-04-01"),
        )
        .expect_err("no effects under v0.8.16");
    assert!(
        err.to_string()
            .contains("declares no effects; up to schema v0.8.16 only a fact with effects"),
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
    // answer: the run of the read model, with the aggregate it read and
    // the law the step called under the record that counted.
    let read = |path: &str| std::fs::read_to_string(fixtures().join(path)).unwrap();
    let service = service_of(&[
        &read("regulation/test_chronicle_effects/2025-01-01.yaml"),
        &read("regulation/test_beleid_geldige_kaart/2025-01-01.yaml"),
        &read("cells/test_cel/reductions/test_reductie_geldige_kaart/2025-01-01.yaml"),
        &read("cells/test_cel/reductions/test_reductie_kaart_per_besluit/2025-01-01.yaml"),
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
    // Under the answer: the read model's own run, and in it, where it
    // reads the aggregates, the one aggregate of the applicant's grant.
    let [run] = &answer.children[..] else {
        panic!("the reduction's run: {}", root.render_box_drawing());
    };
    let projection = find(run, "PROJECTION")
        .unwrap_or_else(|| panic!("a PROJECTION node: {}", run.render_box_drawing()));
    assert_eq!(
        projection.message.as_deref(),
        Some("the aggregate of verlening_x")
    );
    let names = |n: &PathNode| {
        n.children
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(projection), ["COUNTS", "FOLD"]);
    assert_eq!(names(&projection.children[0]), ["COUNT"]);
    let fold = &projection.children[1];
    assert_eq!(names(fold), ["APPLY"]);
    let grant = &fold.children[0];
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
    // Every node keeps an id of its own in the one tree, and the aggregate
    // sits under the answer.
    let mut ids = Vec::new();
    collect_ids(root, &mut ids);
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "{ids:?}");
    let answer_id = answer.node_id.as_deref().unwrap();
    assert!(
        projection
            .node_id
            .as_deref()
            .unwrap()
            .starts_with(&format!("{answer_id}."))
    );
}

/// The first node named `name` under `node`, depth first.
fn find<'n>(node: &'n PathNode, name: &str) -> Option<&'n PathNode> {
    node.children
        .iter()
        .find_map(|c| (c.name == name).then_some(c).or_else(|| find(c, name)))
}

/// The `node_id` of `node` and of every node under it.
fn collect_ids(node: &PathNode, ids: &mut Vec<String>) {
    ids.extend(node.node_id.clone());
    for child in &node.children {
        collect_ids(child, ids);
    }
}
