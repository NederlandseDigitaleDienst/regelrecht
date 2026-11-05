//! The cell's actions (`packages/cel/bdd/grammar.yaml`): its additions to the
//! dispatch it shares with the engine's runner.
//!
//! A scenario has one cell with one chronicle in memory. Records are named by
//! the id in the scenario; the day they are recorded is the calculation date.
//! The fixtures in `packages/cel/bdd/fixtures` (the synthetic law, and the
//! reductions of a cell) are loaded next to the corpus.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::{effects, Chronicle, Gram};
use regelrecht_engine::{Article, LawExecutionService, Value};
use regelrecht_law_model::stored;
use walkdir::WalkDir;

use crate::dispatch::{table_cell_value, ArgValue};
use crate::helpers::table::{rows_to_params, Rows};
use crate::world::RegelrechtWorld;

/// The cell that records, as `recording_actor`.
const CELL: &str = "test_cel";

/// The cell's chronicle in this scenario.
pub struct State {
    chronicle: Chronicle,
}

impl State {
    /// An empty chronicle, after loading the fixtures: every regelwerk under
    /// `fixtures/regulation`, and the reductions under `fixtures/cells`.
    pub fn new(service: &mut LawExecutionService) -> Self {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures");
        for entry in WalkDir::new(&fixtures).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            let rel = path.strip_prefix(&fixtures).unwrap();
            let regelwerk = rel.starts_with("regulation")
                || rel.components().any(|c| c.as_os_str() == "reductions");
            if regelwerk && path.extension().is_some_and(|x| x == "yaml") {
                let mut yaml = std::fs::read_to_string(path).unwrap();
                // As the corpus loader does, so `BDD_REVERSE_ACTIONS` covers
                // the reductions too.
                if crate::reverse_actions::enabled() {
                    yaml = crate::reverse_actions::reversed_actions(&yaml)
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                }
                service
                    .load_law(&yaml)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
        Self {
            chronicle: Chronicle::in_memory(),
        }
    }
}

/// Run `action` if it is one of the cell's steps; `false` otherwise.
pub fn dispatch(
    world: &mut RegelrechtWorld,
    action: &str,
    args: Vec<ArgValue>,
    table: Option<Rows>,
) -> bool {
    match action {
        "record_submission" => {
            let fields = rows_to_params(&table.expect("fields table"), table_cell_value);
            record_submission(
                world,
                args[0].as_str(),
                args[1].as_str(),
                args[2].as_str(),
                fields,
            );
        }
        "decide_on_submission" => decide_on_submission(world, args[0].as_str(), args[1].as_str()),
        "decide_under_article" => {
            let parameters = rows_to_params(&table.expect("parameters table"), table_cell_value);
            let basis = format!("{}#{}", args[2].as_str(), args[1].as_str());
            decide(world, args[0].as_str(), &basis, None, parameters);
        }
        "evaluate_over_counting_decisions" => {
            let counting = counting_rows(world);
            world
                .parameters
                .insert(args[2].as_str().to_string(), Value::Array(counting));
            world.run_evaluation(args[1].as_str(), &[args[0].as_str().to_string()]);
        }
        "assert_decision_counts" => {
            let id = args[0].as_str();
            assert!(
                world.extension.chronicle.grams().iter().any(|g| g.id == id),
                "no record '{id}' in the chronicle"
            );
            let counts = effects::counting(&world.service, &world.extension.chronicle)
                .expect("play the effects")
                .iter()
                .any(|g| g.id == id);
            assert_eq!(
                counts,
                args[1].as_bool(),
                "whether the consequences of '{id}' count"
            );
        }
        _ => return false,
    }
    true
}

/// The day a record is made: the calculation date.
fn today(world: &RegelrechtWorld) -> NaiveDate {
    NaiveDate::parse_from_str(&world.calculation_date, "%Y-%m-%d").expect("calculation date")
}

/// Article `basis` (`<law>#<number>`) of the law in force on the calculation date.
fn article<'w>(world: &'w RegelrechtWorld, basis: &str) -> &'w Article {
    let (law, number) = basis.split_once('#').expect("<law>#<article>");
    world
        .service
        .resolver()
        .get_law_for_date(law, Some(today(world)))
        .and_then(|l| l.find_article_by_number(number))
        .unwrap_or_else(|| panic!("no article {basis} on {}", world.calculation_date))
}

fn record_submission(
    world: &mut RegelrechtWorld,
    id: &str,
    number: &str,
    law: &str,
    fields: BTreeMap<String, Value>,
) {
    let basis = format!("{law}#{number}");
    let kind = article(world, &basis)
        .get_submission()
        .and_then(|s| s.submission_kind())
        .unwrap_or_else(|| panic!("article {basis} establishes no submission"))
        .to_string();
    append(
        world,
        gram(world, id, kind, basis, None, fields, BTreeMap::new()),
    );
}

/// The decision the submission asks for, with its fields as parameters.
fn decide_on_submission(world: &mut RegelrechtWorld, id: &str, submission: &str) {
    let gram = world
        .extension
        .chronicle
        .grams()
        .iter()
        .find(|g| g.id == submission)
        .unwrap_or_else(|| panic!("no submission '{submission}' in the chronicle"))
        .clone();
    let requests = article(world, &gram.basis)
        .get_submission()
        .and_then(|s| s.requests())
        .unwrap_or_else(|| panic!("submission '{submission}' asks for no decision"))
        .to_string();
    let fields = gram
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), stored::from_stored(v).expect("stored field")))
        .collect();
    decide(world, id, &requests, Some(submission), fields);
}

/// Execute the decision of article `basis` with `parameters` and record it.
fn decide(
    world: &mut RegelrechtWorld,
    id: &str,
    basis: &str,
    refers_to: Option<&str>,
    parameters: BTreeMap<String, Value>,
) {
    let deciding = article(world, basis);
    let decision = deciding
        .get_decision()
        .unwrap_or_else(|| panic!("article {basis} takes no decision"));
    let kind = decision.legal_character().expect("a decision").to_string();
    let outputs: Vec<&str> = deciding.outputs_of(decision);
    let law = basis.split_once('#').expect("<law>#<article>").0;
    let result = world
        .service
        .evaluate_law(law, &outputs, parameters.clone(), &world.calculation_date)
        .unwrap_or_else(|e| panic!("decision '{id}' under {basis}: {e}"));
    let gram = gram(
        world,
        id,
        kind,
        basis.to_string(),
        refers_to,
        parameters,
        result.outputs,
    );
    append(world, gram);
}

fn gram(
    world: &RegelrechtWorld,
    id: &str,
    kind: String,
    basis: String,
    refers_to: Option<&str>,
    fields: BTreeMap<String, Value>,
    outputs: BTreeMap<String, Value>,
) -> Gram {
    let to_stored = |values: BTreeMap<String, Value>| {
        values
            .into_iter()
            .map(|(k, v)| {
                let json = stored::to_stored(&v).expect("storable value");
                (k, json)
            })
            .collect()
    };
    Gram {
        id: id.to_string(),
        kind,
        basis,
        fact: None,
        refers_to: refers_to.map(str::to_string),
        received: None,
        after: None,
        recording_actor: CELL.to_string(),
        recorded_on: today(world),
        moment: None,
        fields: to_stored(fields),
        missing: Vec::new(),
        outputs: to_stored(outputs),
    }
}

fn append(world: &mut RegelrechtWorld, gram: Gram) {
    world
        .extension
        .chronicle
        .append(gram)
        .unwrap_or_else(|e| panic!("record in the chronicle: {e}"));
}

/// The decisions whose consequences count, as the collection a reduction
/// is given: per decision its `id`, its `basis`, its fields and its outputs.
fn counting_rows(world: &RegelrechtWorld) -> Vec<Value> {
    effects::counting(&world.service, &world.extension.chronicle)
        .expect("play the effects")
        .into_iter()
        .map(|g| {
            let mut row: BTreeMap<String, Value> = g
                .fields
                .iter()
                .chain(&g.outputs)
                .map(|(k, v)| (k.clone(), stored::from_stored(v).expect("stored value")))
                .collect();
            row.insert("id".to_string(), Value::String(g.id.clone()));
            row.insert("basis".to_string(), Value::String(g.basis.clone()));
            Value::Object(row)
        })
        .collect()
}
