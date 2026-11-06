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
use regelrecht_cel::{shape, Chronicle, Gram, EXECUTOGRAM};
use regelrecht_engine::projection::{cause_word, describe_applied};
use regelrecht_engine::{
    Article, ChronicleRecord, LawExecutionService, ProjectionEvent, ProjectionOutcome, Value,
};
use regelrecht_law_model::stored;
use walkdir::WalkDir;

use crate::dispatch::{table_cell_value, ArgValue};
use crate::helpers::table::{rows_to_params, Rows};
use crate::world::RegelrechtWorld;

/// The cell that records, as `recording_actor`.
const CELL: &str = "test_cel";

/// The cell's chronicle in this scenario, and the last projection played
/// over it.
pub struct State {
    chronicle: Chronicle,
    projection: Option<ProjectionOutcome>,
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
            projection: None,
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
        "decide_on_submission" => {
            let found = table
                .map(|t| rows_to_params(&t, table_cell_value))
                .unwrap_or_default();
            decide_on_submission(world, args[0].as_str(), args[1].as_str(), found);
        }
        "decide_under_article" => {
            let parameters = rows_to_params(&table.expect("parameters table"), table_cell_value);
            let basis = format!("{}#{}", args[2].as_str(), args[1].as_str());
            decide(world, args[0].as_str(), &basis, None, parameters);
        }
        "record_fact" => {
            let fields = rows_to_params(&table.expect("fields table"), table_cell_value);
            record_fact(
                world,
                args[0].as_str(),
                args[1].as_str(),
                args[2].as_str(),
                args[3].as_str(),
                fields,
            );
        }
        "evaluate_over_chronicle" => {
            evaluate_over_chronicle(world, args[0].as_str(), args[1].as_str())
        }
        "assert_projection_trace" => assert_projection_trace(world, &table.expect("trace table")),
        "assert_last_set_by" => {
            let (name, id) = (args[0].as_str(), args[1].as_str());
            let set_by = projection(world)
                .set_by
                .get(name)
                .unwrap_or_else(|| panic!("no state property '{name}'"));
            assert_eq!(
                set_by.as_deref(),
                Some(id),
                "the record that set '{name}' last"
            );
        }
        "assert_record_applied" => {
            let id = args[0].as_str();
            let events = last_replay(&projection(world).events);
            let applied = events
                .iter()
                .any(|e| matches!(e, ProjectionEvent::Applied { record, .. } if record == id));
            let skipped = events
                .iter()
                .any(|e| matches!(e, ProjectionEvent::Skipped { record, .. } if record == id));
            assert!(
                applied || skipped,
                "record '{id}' is not in the last replay"
            );
            assert_eq!(
                applied,
                args[1].as_bool(),
                "whether record '{id}' is applied"
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

/// The decision the submission asks for, with its fields and what the
/// authority `found` besides them as parameters.
fn decide_on_submission(
    world: &mut RegelrechtWorld,
    id: &str,
    submission: &str,
    found: BTreeMap<String, Value>,
) {
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
    let mut parameters: BTreeMap<String, Value> = gram
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), stored::from_stored(v).expect("stored field")))
        .collect();
    for (name, value) in found {
        // The table gives what was found besides the submission.
        assert!(
            !parameters.contains_key(&name),
            "'{name}' is a field of submission '{submission}'; the table gives what the authority found besides it"
        );
        parameters.insert(name, value);
    }
    decide(world, id, &requests, Some(submission), parameters);
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

/// Record an executogram of `fact`, required by the article, with the
/// values of its fields. The fields must be exactly those the fact holds.
fn record_fact(
    world: &mut RegelrechtWorld,
    fact: &str,
    id: &str,
    number: &str,
    law: &str,
    fields: BTreeMap<String, Value>,
) {
    let basis = format!("{law}#{number}");
    let declared = shape::fact_fields(
        &world.service,
        &format!("{basis}#{fact}"),
        today(world),
        None,
    )
    .unwrap_or_else(|e| panic!("fact {fact} under {basis}: {e}"));
    let mut names: Vec<&str> = declared.iter().map(|f| f.name.as_str()).collect();
    names.sort_unstable();
    let given: Vec<&str> = fields.keys().map(String::as_str).collect();
    assert_eq!(given, names, "the fields of fact {fact} under {basis}");
    let mut gram = gram(
        world,
        id,
        EXECUTOGRAM.to_string(),
        basis,
        None,
        fields,
        BTreeMap::new(),
    );
    gram.fact = Some(fact.to_string());
    append(world, gram);
}

/// Play every record of the chronicle through the projection of `law` and
/// keep the answer, with the engine's trace.
fn evaluate_over_chronicle(world: &mut RegelrechtWorld, output: &str, law: &str) {
    let records: Vec<ChronicleRecord> = world
        .extension
        .chronicle
        .grams()
        .iter()
        .map(|g| ChronicleRecord {
            id: g.id.clone(),
            kind: g.kind.clone(),
            basis: g.basis.clone(),
            fact: g.fact.clone(),
            recorded_on: g.recorded_on,
            refers_to: g.refers_to.clone(),
            fields: read_back(&g.fields),
            outputs: read_back(&g.outputs),
        })
        .collect();
    let outcome = world.service.evaluate_projection(
        law,
        &[output],
        &records,
        world.parameters.clone(),
        &world.calculation_date,
        true,
    );
    world.requested_outputs = vec![output.to_string()];
    match outcome {
        Ok(outcome) => {
            world.result = Some(outcome.result.clone());
            world.error = None;
            world.extension.projection = Some(outcome);
        }
        Err(e) => {
            world.result = None;
            world.error = Some(e);
            world.extension.projection = None;
        }
    }
}

fn read_back(values: &BTreeMap<String, serde_json::Value>) -> BTreeMap<String, Value> {
    values
        .iter()
        .map(|(k, v)| (k.clone(), stored::from_stored(v).expect("stored value")))
        .collect()
}

fn projection(world: &RegelrechtWorld) -> &ProjectionOutcome {
    world.extension.projection.as_ref().unwrap_or_else(|| {
        panic!(
            "no projection was played: {:?}",
            world.error.as_ref().map(ToString::to_string)
        )
    })
}

/// The events of the last replay: those after the last rewind.
fn last_replay(events: &[ProjectionEvent]) -> &[ProjectionEvent] {
    let start = events
        .iter()
        .rposition(|e| matches!(e, ProjectionEvent::Rewound { .. }))
        .map_or(0, |i| i + 1);
    &events[start..]
}

/// The events as rows `record | event | detail`.
fn trace_rows(events: &[ProjectionEvent]) -> Vec<[String; 3]> {
    events
        .iter()
        .map(|e| match e {
            ProjectionEvent::Applied {
                record,
                article,
                changes,
            } => [
                record.clone(),
                "applied".to_string(),
                describe_applied(article.as_deref(), changes),
            ],
            ProjectionEvent::Skipped { record, undone_by } => [
                record.clone(),
                "skipped".to_string(),
                format!("undone by {}", undone_by.join(", ")),
            ],
            ProjectionEvent::EffectsOnly { record, reason } => {
                [record.clone(), "effects only".to_string(), reason.clone()]
            }
            ProjectionEvent::Rewound {
                record,
                cause,
                target,
            } => [
                record.clone(),
                "rewound".to_string(),
                format!("{} {target}", cause_word(*cause)),
            ],
        })
        .collect()
}

/// The trace table must match the events exactly, and the engine's trace
/// must hold them as the children of its `PROJECTION` node.
fn assert_projection_trace(world: &RegelrechtWorld, table: &Rows) {
    let outcome = projection(world);
    let header: Vec<&str> = table
        .first()
        .map(|row| row.iter().map(|c| c.trim()).collect())
        .unwrap_or_default();
    assert_eq!(
        header,
        ["record", "event", "detail"],
        "the header of the trace table"
    );
    let expected: Vec<[String; 3]> = table
        .iter()
        .skip(1)
        .map(|row| {
            assert_eq!(row.len(), 3, "a trace row is record | event | detail");
            [
                row[0].trim().to_string(),
                row[1].trim().to_string(),
                row[2].trim().to_string(),
            ]
        })
        .collect();
    let actual = trace_rows(&outcome.events);
    let show = |rows: &[[String; 3]]| {
        rows.iter()
            .map(|r| format!("| {} | {} | {} |", r[0], r[1], r[2]))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(expected == actual, "projection trace:\n{}", show(&actual));

    let root = outcome.result.trace.as_ref().expect("the engine trace");
    let node = root
        .children
        .iter()
        .find(|n| n.name == "PROJECTION")
        .expect("a PROJECTION node in the engine trace");
    assert_eq!(
        node.children.len(),
        outcome.events.len(),
        "one trace node per event"
    );
}
