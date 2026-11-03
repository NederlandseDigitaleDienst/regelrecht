//! An input a competent authority answers from its own records (RFC-056),
//! on the demo rulework of the Wet op de accijns.
//!
//! Art. 39 lets a place serve as an accijnsgoederenplaats only "indien
//! daartoe een vergunning is verstrekt door de inspecteur". Its input
//! `heeft_agp_vergunning_register` names the inspecteur as competent
//! (`source.competent_authority`). A fictional inspecteur keeps a cell
//! (`tests/fixtures/corpus/cells/inspecteur_wilgendaal`): it grants the
//! permit on an application (art. 43, with what art. 42 says it holds) and revokes it at the holder's
//! request (art. 48, onder f), and its reduction implements the input. With
//! the cell in the call, the engine asks it; without it, the value is
//! supplied by hand, as the demo's scenario does with a data table. Both at
//! once is an error.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{day, object_map};
use regelrecht_cel::{Cell, Error, Gram, Reference};
use regelrecht_engine::{
    ArticleResult, Call, InputResolvedBy, LawExecutionService, OpenTermAnswerer, Value,
};
use serde_json::{json, Map};

const LAW: &str = "wet_op_de_accijns/accijnsplicht_alcohol";
const INPUT: &str = "heeft_agp_vergunning_register";
const KVK: &str = "85234567";
const ACTOR: &str = "inspecteur_wilgendaal";
const REDUCTION: &str = "reductie_agp_vergunningen";

/// The demo rulework of the Wet op de accijns, the Awb for the procedure of
/// a beschikking, and the inspecteur's reduction.
fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    common::load(
        &mut service,
        [
            format!("demo/regulation/nl/{LAW}"),
            "regulation/nl/wet/algemene_wet_bestuursrecht".to_string(),
            common::reduction_dir(REDUCTION),
        ],
    );
    service
}

fn inspecteur() -> Cell {
    Cell::in_memory(
        ACTOR,
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/corpus/cells/{ACTOR}")),
    )
    .unwrap()
}

/// The inspecteur grants the permit on an application received on `on`.
fn grant(service: &mut LawExecutionService, cell: &mut Cell, on: &str) -> Gram {
    let applied = cell
        .record_submission(
            service,
            &[],
            LAW,
            "43",
            None,
            &object_map(json!({ "kvk_nummer": KVK })),
            day(on),
        )
        .unwrap();
    let mut taken = cell
        .decide(
            service,
            &[],
            LAW,
            "43",
            &applied.id,
            &Map::new(),
            &object_map(json!({ "gegronde_redenen_om_niet_voldoen_aan_te_nemen": false })),
            day(on),
        )
        .unwrap();
    let decision = taken.remove(0);
    assert_eq!(decision.outputs["agp_vergunning_verleend"], json!(true));
    decision
}

/// The holder asks on `on` to revoke the permit of `granted`, and the
/// inspecteur does (art. 48, onder f).
fn revoke(service: &mut LawExecutionService, cell: &mut Cell, granted: &Gram, on: &str) {
    let applied = cell
        .record_submission(
            service,
            &[],
            LAW,
            "48",
            Some(Reference::Local(&granted.id)),
            &Map::new(),
            day(on),
        )
        .unwrap();
    let taken = cell
        .decide(
            service,
            &[],
            LAW,
            "48",
            &applied.id,
            &Map::new(),
            &object_map(json!({ "intrekking_op_verzoek_toegewezen": true })),
            day(on),
        )
        .unwrap();
    assert_eq!(taken[0].outputs["agp_vergunning_ingetrokken"], json!(true));
}

fn parameters() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("kvk_nummer".to_string(), Value::String(KVK.to_string())),
        (
            "activiteit".to_string(),
            Value::String("productie".to_string()),
        ),
    ])
}

/// Art. 39 and art. 5, which reads it, on `on`, with `cells` in the call.
fn evaluate(
    service: &LawExecutionService,
    cells: Vec<&dyn OpenTermAnswerer>,
    on: &str,
) -> regelrecht_engine::Result<ArticleResult> {
    let call = Call {
        sphere: cells,
        ..Call::default()
    };
    service.evaluate_law_in(
        LAW,
        &["heeft_agp_vergunning", "voldoet_aan_vergunningsplicht"],
        parameters(),
        call,
        on,
    )
}

fn permitted(result: &ArticleResult) -> (bool, bool) {
    let flag = |name: &str| match result.outputs.get(name) {
        Some(Value::Bool(b)) => *b,
        other => panic!("{name}: {other:?}"),
    };
    (
        flag("heeft_agp_vergunning"),
        flag("voldoet_aan_vergunningsplicht"),
    )
}

/// How the input of art. 39 got its value.
fn resolved_by(result: &ArticleResult) -> &InputResolvedBy {
    let [entry] = result
        .competent_inputs
        .iter()
        .filter(|c| c.input == INPUT)
        .collect::<Vec<_>>()[..]
    else {
        panic!("one entry for {INPUT}: {:?}", result.competent_inputs)
    };
    assert_eq!((entry.law_id.as_str(), entry.article.as_str()), (LAW, "39"));
    &entry.resolved_by
}

#[test]
fn the_inspecteur_answers_from_his_chronicle_before_during_and_after_the_permit() {
    let mut service = service();
    let mut cell = inspecteur();
    let granted = grant(&mut service, &mut cell, "2024-02-01");
    revoke(&mut service, &mut cell, &granted, "2024-09-02");
    let answerer = cell.answerer();

    // Before the grant, while it holds, and after the revocation: the cell
    // reads its chronicle as it stood on the day asked about.
    for (on, expected) in [
        ("2024-01-15", (false, false)),
        ("2024-02-01", (true, true)),
        ("2024-06-01", (true, true)),
        ("2024-09-02", (false, false)),
        ("2024-10-01", (false, false)),
    ] {
        let result = evaluate(&service, vec![&answerer], on).unwrap();
        assert_eq!(permitted(&result), expected, "on {on}");
        let InputResolvedBy::Answered(provenance) = resolved_by(&result) else {
            panic!("answered by the cell on {on}: {:?}", resolved_by(&result))
        };
        assert_eq!(provenance.answerer, ACTOR);
        assert_eq!(provenance.answer, INPUT);
        assert_eq!(provenance.law_id, REDUCTION);
        assert_eq!(provenance.article, "1");
    }
    // The chronicle on 1 June holds the application and the grant.
    let result = evaluate(&service, vec![&answerer], "2024-06-01").unwrap();
    let InputResolvedBy::Answered(provenance) = resolved_by(&result) else {
        unreachable!()
    };
    assert_eq!(provenance.records_read, 2);
    assert_eq!(provenance.last_record.as_deref(), Some(granted.id.as_str()));
}

#[test]
fn the_trace_names_the_cell_that_answered() {
    let mut service = service();
    let mut cell = inspecteur();
    grant(&mut service, &mut cell, "2024-02-01");
    let answerer = cell.answerer();
    let call = Call {
        sphere: vec![&answerer],
        ..Call::default()
    };
    let result = service
        .evaluate_law_with_trace_in(
            LAW,
            &["heeft_agp_vergunning"],
            parameters(),
            call,
            "2024-06-01",
        )
        .unwrap();
    let trace = result.trace.as_ref().unwrap();
    let answered = common::answered(trace);
    let [node] = answered.as_slice() else {
        panic!("one answered node: {answered:?}")
    };
    assert_eq!(node.name, INPUT);
    assert_eq!(node.answer.as_ref().unwrap().answerer, ACTOR);
}

#[test]
fn without_the_cell_a_supplied_value_counts() {
    // The scenario of the demo: the register of the Douane, as a data table.
    let mut service = service();
    let record = BTreeMap::from([
        ("kvk_nummer".to_string(), Value::String(KVK.to_string())),
        (INPUT.to_string(), Value::Bool(true)),
    ]);
    service
        .register_dict_source_for_law(LAW, "DOUANE", "kvk_nummer", vec![record], 10)
        .unwrap();
    let result = evaluate(&service, Vec::new(), "2024-06-01").unwrap();
    assert_eq!(permitted(&result), (true, true));
    assert_eq!(
        resolved_by(&result),
        &InputResolvedBy::Supplied {
            provider: Some("DOUANE".to_string())
        }
    );
}

#[test]
fn a_value_passed_as_a_parameter_counts_too() {
    let service = service();
    let mut parameters = parameters();
    parameters.insert(INPUT.to_string(), Value::Bool(false));
    let result = service
        .evaluate_law_in(
            LAW,
            &["heeft_agp_vergunning"],
            parameters,
            Call::default(),
            "2024-06-01",
        )
        .unwrap();
    assert_eq!(result.outputs["heeft_agp_vergunning"], Value::Bool(false));
    assert_eq!(
        resolved_by(&result),
        &InputResolvedBy::Supplied { provider: None }
    );
}

#[test]
fn supplied_and_answered_is_an_error() {
    let mut service = service();
    let mut cell = inspecteur();
    grant(&mut service, &mut cell, "2024-02-01");
    let record = BTreeMap::from([
        ("kvk_nummer".to_string(), Value::String(KVK.to_string())),
        (INPUT.to_string(), Value::Bool(true)),
    ]);
    service
        .register_dict_source_for_law(LAW, "DOUANE", "kvk_nummer", vec![record], 10)
        .unwrap();
    let answerer = cell.answerer();
    let e = evaluate(&service, vec![&answerer], "2024-06-01").unwrap_err();
    assert!(
        e.to_string().contains(
            "is supplied by DOUANE and answered by the cell inspecteur_wilgendaal; the engine \
             does not choose between two sources for one value"
        ),
        "{e}"
    );
}

#[test]
fn passed_and_answered_is_an_error() {
    let mut service = service();
    let mut cell = inspecteur();
    grant(&mut service, &mut cell, "2024-02-01");
    let answerer = cell.answerer();
    let mut parameters = parameters();
    parameters.insert(INPUT.to_string(), Value::Bool(false));
    let call = Call {
        sphere: vec![&answerer],
        ..Call::default()
    };
    let e = service
        .evaluate_law_in(
            LAW,
            &["heeft_agp_vergunning"],
            parameters,
            call,
            "2024-06-01",
        )
        .unwrap_err();
    assert!(
        e.to_string()
            .contains("is passed by the caller and answered by the cell inspecteur_wilgendaal"),
        "{e}"
    );
}

#[test]
fn neither_supplied_nor_answered_leaves_the_input_without_a_value() {
    // As a `source: {}` input without data: unknown, and art. 39 with it.
    let service = service();
    let result = evaluate(&service, Vec::new(), "2024-06-01").unwrap();
    assert!(
        result.outputs["heeft_agp_vergunning"].is_unknown(),
        "{:?}",
        result.outputs
    );
    assert_eq!(resolved_by(&result), &InputResolvedBy::Absent);
}

#[test]
fn a_cell_the_input_does_not_name_is_not_asked() {
    // A college keeps the same reduction, but art. 39 names the inspecteur.
    let mut service = service();
    let dir = tempfile::tempdir().unwrap();
    common::write_identity(dir.path(), "name: wilgendaal\ncategory: college\n");
    common::keep_corpus_reduction(dir.path(), REDUCTION);
    let mut college = Cell::in_memory("college_wilgendaal", dir.path()).unwrap();
    grant(&mut service, &mut college, "2024-02-01");
    let answerer = college.answerer();
    let result = evaluate(&service, vec![&answerer], "2024-06-01").unwrap();
    assert_eq!(resolved_by(&result), &InputResolvedBy::Absent);
    // Its own reductions say what they answer, and the law does not make a
    // college competent for it.
    let e = college.check_inputs(&service).unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("whose source names 'inspecteur', and the cell is 'wilgendaal' (college)")),
        "{e}"
    );
}

#[test]
fn two_competent_cells_in_the_call_are_an_error() {
    let service = service();
    let one = inspecteur();
    let dir = tempfile::tempdir().unwrap();
    common::write_identity(
        dir.path(),
        "name: Inspecteur Oostmeer\ncategory: inspecteur\n",
    );
    let other = Cell::in_memory("inspecteur_oostmeer", dir.path()).unwrap();
    let (a, b) = (one.answerer(), other.answerer());
    let e = evaluate(&service, vec![&a, &b], "2024-06-01").unwrap_err();
    assert!(
        e.to_string()
            .contains("2 cells of the call are named by it"),
        "{e}"
    );
}

#[test]
fn the_fixture_cell_is_competent_for_what_it_answers() {
    inspecteur().check_inputs(&service()).unwrap();
}

#[test]
fn a_reduction_takes_the_day_asked_about_as_peildatum() {
    // A reduction that declares `peildatum` gets the calculation date.
    let mut service = service();
    let yaml = r#"
$id: reductie_peildatum_fictief
regulatory_layer: REDUCTIE
publication_date: '2024-01-01'
valid_from: '2024-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Geen regelgeving.
    machine_readable:
      implements:
        - law: wet_op_de_accijns/accijnsplicht_alcohol
          article: '39'
          input: heeft_agp_vergunning_register
      execution:
        parameters:
          - {name: kvk_nummer, type: string}
          - {name: peildatum, type: date}
        output:
          - {name: heeft_agp_vergunning_register, type: boolean}
        actions:
          - output: heeft_agp_vergunning_register
            value: {operation: EQUALS, subject: $peildatum, value: '2024-06-01'}
"#;
    service.load_law(yaml).unwrap();
    let dir = tempfile::tempdir().unwrap();
    common::write_identity(
        dir.path(),
        "name: Inspecteur Oostmeer\ncategory: inspecteur\n",
    );
    common::keep_reduction(dir.path(), "reductie_peildatum_fictief", yaml);
    let cell = Cell::in_memory("inspecteur_oostmeer", dir.path()).unwrap();
    let answerer = cell.answerer();
    for (on, expected) in [("2024-06-01", true), ("2024-06-02", false)] {
        let result = evaluate(&service, vec![&answerer], on).unwrap();
        assert_eq!(permitted(&result).0, expected, "on {on}");
    }
}

#[test]
fn a_competent_cell_without_a_reduction_for_the_input_is_silent() {
    // The cell is named, but none of its reductions implements the input:
    // it says nothing, and the input is what it is without data.
    let service = service();
    let dir = tempfile::tempdir().unwrap();
    common::write_identity(
        dir.path(),
        "name: Inspecteur Oostmeer\ncategory: inspecteur\n",
    );
    let cell = Cell::in_memory("inspecteur_oostmeer", dir.path()).unwrap();
    let answerer = cell.answerer();
    let result = evaluate(&service, vec![&answerer], "2024-06-01").unwrap();
    assert_eq!(resolved_by(&result), &InputResolvedBy::Absent);
}

/// A reduction that answers art. 39 with `output` as its output declaration.
fn answering_with(output: &str) -> String {
    format!(
        r#"
$id: reductie_fictief
regulatory_layer: REDUCTIE
publication_date: '2024-01-01'
valid_from: '2024-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Geen regelgeving.
    machine_readable:
      implements:
        - law: wet_op_de_accijns/accijnsplicht_alcohol
          article: '39'
          input: heeft_agp_vergunning_register
      execution:
        output:
          - {output}
        actions:
          - output: heeft_agp_vergunning_register
            value: null
"#
    )
}

#[test]
fn the_answer_has_the_type_unit_and_nullability_of_the_input() {
    for (output, says) in [
        (
            "{name: heeft_agp_vergunning_register, type: number}",
            "[T6] 'heeft_agp_vergunning_register' answers input 'heeft_agp_vergunning_register' of \
             wet_op_de_accijns/accijnsplicht_alcohol article 39, which is boolean, but is \
             declared number",
        ),
        (
            "{name: heeft_agp_vergunning_register, type: boolean, type_spec: {unit: eurocent}}",
            "whose unit is no unit, but has eurocent",
        ),
        (
            "{name: heeft_agp_vergunning_register, type: boolean, nullable: true}",
            "[N5] 'heeft_agp_vergunning_register' answers input 'heeft_agp_vergunning_register' \
             of wet_op_de_accijns/accijnsplicht_alcohol article 39, which is never absent, and \
             may be null",
        ),
    ] {
        let mut service = service();
        let e = service.load_law(&answering_with(output)).unwrap_err();
        assert!(e.to_string().contains(says), "{output}: {e}");
    }
}

#[test]
fn the_loader_holds_implements_and_the_source_to_their_place() {
    let mut service = service();
    let law = |layer: &str, implements: &str, source: &str| {
        format!(
            r#"
$id: fictief
regulatory_layer: {layer}
publication_date: '2024-01-01'
valid_from: '2024-01-01'
url: https://example.org/fictief
articles:
  - number: '1'
    text: Tekst.
    machine_readable:
      {implements}
      execution:
        input:
          - {{name: x, type: boolean, source: {source}}}
        output:
          - {{name: y, type: boolean}}
        actions:
          - {{output: y, value: $x}}
"#
        )
    };
    for (yaml, says) in [
        (
            law(
                "WET",
                "implements: [{law: wet_op_de_accijns/accijnsplicht_alcohol, article: '39', input: heeft_agp_vergunning_register}]",
                "{}",
            ),
            "only a REDUCTIE answers an input",
        ),
        (
            law(
                "REDUCTIE",
                "implements: [{law: wet_op_de_accijns/accijnsplicht_alcohol, article: '39'}]",
                "{decisions: wet_op_de_accijns/accijnsplicht_alcohol#43}",
            ),
            "without naming an open term or an input",
        ),
        (
            law(
                "WET",
                "",
                "{competent_authority: {name: inspecteur}, regulation: andere_wet}",
            ),
            "which no regulation can stand beside",
        ),
    ] {
        let e = service.load_law(&yaml).unwrap_err();
        assert!(e.to_string().contains(says), "{e}");
    }
}

#[test]
fn a_cycle_through_two_reductions_of_one_cell_is_an_error() {
    // Each reduction answers one input of a fictional law and asks the law
    // for the other: the cell refuses to answer itself without end.
    let law = r#"
$id: wet_kring_fictief
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
url: https://example.org/kring
articles:
  - number: '1'
    text: Tekst.
    machine_readable:
      execution:
        input:
          - {name: x, type: boolean, source: {competent_authority: {name: inspecteur, type: CATEGORY}}}
        output:
          - {name: x_uit, type: boolean}
        actions:
          - {output: x_uit, value: $x}
  - number: '2'
    text: Tekst.
    machine_readable:
      execution:
        input:
          - {name: y, type: boolean, source: {competent_authority: {name: inspecteur, type: CATEGORY}}}
        output:
          - {name: y_uit, type: boolean}
        actions:
          - {output: y_uit, value: $y}
"#;
    let reduction = |id: &str, answers: (&str, &str), reads: &str| {
        format!(
            r#"
$id: {id}
regulatory_layer: REDUCTIE
publication_date: '2024-01-01'
valid_from: '2024-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Geen regelgeving.
    machine_readable:
      implements:
        - {{law: wet_kring_fictief, article: '{}', input: {}}}
      execution:
        input:
          - {{name: gelezen, type: boolean, source: {{regulation: wet_kring_fictief, output: {reads}}}}}
        output:
          - {{name: {}, type: boolean}}
        actions:
          - {{output: {}, value: $gelezen}}
"#,
            answers.0, answers.1, answers.1, answers.1
        )
    };
    let a = reduction("reductie_kring_a", ("1", "x"), "y_uit");
    let b = reduction("reductie_kring_b", ("2", "y"), "x_uit");
    let service = common::service_of(&[law, &a, &b]);
    let dir = tempfile::tempdir().unwrap();
    common::write_identity(
        dir.path(),
        "name: Inspecteur Oostmeer\ncategory: inspecteur\n",
    );
    common::keep_reduction(dir.path(), "reductie_kring_a", &a);
    common::keep_reduction(dir.path(), "reductie_kring_b", &b);
    let cell = Cell::in_memory("inspecteur_oostmeer", dir.path()).unwrap();
    cell.check_inputs(&service).unwrap();
    let answerer = cell.answerer();
    let call = Call {
        sphere: vec![&answerer],
        ..Call::default()
    };
    let e = service
        .evaluate_law_in(
            "wet_kring_fictief",
            &["x_uit"],
            BTreeMap::new(),
            call,
            "2024-06-01",
        )
        .unwrap_err();
    assert!(e.to_string().contains("while answering it"), "{e}");
}
