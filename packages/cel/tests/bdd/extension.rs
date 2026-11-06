//! The cell's actions (`packages/cel/bdd/grammar.yaml`): its additions to the
//! dispatch it shares with the engine's runner.
//!
//! A scenario has one cell with one chronicle in memory, and a court whose
//! chronicle holds its appeals and rulings. Records are named by the id in
//! the scenario; the day they are recorded is the calculation date. The
//! fixtures in `packages/cel/bdd/fixtures` (the synthetic law, a policy, and
//! the reductions of a cell) are loaded next to the corpus.
//!
//! Most steps record directly in the chronicle, so a scenario names its
//! records. Three go through the cell itself ([`Cell`]): a decision taken
//! by the cell, the receipt of a copy of a court ruling, and an answer the
//! cell gives. Those run the cell's own code, as the engine meets it when a
//! policy binds an open term to an answer of the cell.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::reading::chronicle_records;
use regelrecht_cel::{
    shape, Afschrift, Cell, Chronicle, Disk, Gram, Received, Reference, EXECUTOGRAM,
};
use regelrecht_engine::projection::{cause_word, show_ids};
use regelrecht_engine::{
    Answer, Article, LawExecutionService, ProjectionEvent, ProjectionOptions, ProjectionOutcome,
    Value,
};
use regelrecht_law_model::stored;
use walkdir::WalkDir;

use crate::dispatch::{table_cell_value, ArgValue};
use crate::helpers::table::{rows_to_params, Rows};
use crate::world::RegelrechtWorld;

/// The cell that records, as `recording_actor`.
const CELL: &str = "test_cel";

/// The court whose rulings the cell receives a copy of.
const COURT: &str = "test_rechtbank";

/// The directory of the cell, with its `cel.yaml` and reductions.
fn cell_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("bdd/fixtures/cells/{CELL}"))
}

/// The cell's chronicle in this scenario, the court's, the last projection
/// played over the cell's, and the last answer the cell gave.
pub struct State {
    chronicle: Chronicle,
    court: Chronicle,
    projection: Option<ProjectionOutcome>,
    answer: Option<Answer>,
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
            court: Chronicle::in_memory(),
            projection: None,
            answer: None,
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
            evaluate_over_chronicle(world, args[0].as_str(), args[1].as_str(), None)
        }
        "evaluate_over_chronicle_known_on" => evaluate_over_chronicle(
            world,
            args[0].as_str(),
            args[1].as_str(),
            Some(args[2].as_str()),
        ),
        "record_submission_through_cell" => {
            let fields = table
                .map(|t| rows_to_params(&t, table_cell_value))
                .unwrap_or_default();
            record_submission_through_cell(
                world,
                args[0].as_str(),
                args[1].as_str(),
                args[2].as_str(),
                args[3].as_str(),
                fields,
            );
        }
        "decide_through_cell" => {
            decide_through_cell(world, args[0].as_str(), args[1].as_str());
        }
        "court_records_appeal" => {
            let fields = rows_to_params(&table.expect("fields table"), table_cell_value);
            court_records_appeal(
                world,
                args[0].as_str(),
                args[1].as_str(),
                args[2].as_str(),
                fields,
            );
        }
        "court_rules" => {
            let found = table
                .map(|t| rows_to_params(&t, table_cell_value))
                .unwrap_or_default();
            court_rules(world, args[0].as_str(), args[1].as_str(), found);
        }
        "cell_receives_ruling" => cell_receives_ruling(
            world,
            args[0].as_str(),
            args[1].as_str(),
            args[2].as_str(),
            args[3].as_str(),
            args[4].as_str(),
        ),
        "cell_answers" => {
            let known_on = args.get(1).map(|a| a.as_str().to_string());
            cell_answers(world, args[0].as_str(), known_on.as_deref());
        }
        "assert_cell_answer" => {
            let answer = world.extension.answer.as_ref().unwrap_or_else(|| {
                panic!(
                    "the cell gave no answer: {:?}",
                    world.error.as_ref().map(ToString::to_string)
                )
            });
            assert_eq!(
                answer.value,
                Value::Bool(args[0].as_bool()),
                "the cell's answer ({})",
                answer.provenance
            );
        }
        "assert_record_output" => {
            let (id, name) = (args[0].as_str(), args[1].as_str());
            let gram = world
                .extension
                .chronicle
                .grams()
                .iter()
                .find(|g| g.id == id)
                .unwrap_or_else(|| panic!("no record '{id}' in the chronicle"));
            assert_eq!(
                gram.outputs.get(name),
                Some(&serde_json::Value::Bool(args[2].as_bool())),
                "output '{name}' of record '{id}'"
            );
        }
        "assert_projection_trace" => assert_projection_trace(world, &table.expect("trace table")),
        "assert_in_force" => {
            let ids = show_ids(&projection(world).in_force);
            assert_eq!(ids, args[0].as_str(), "the records in force");
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
    let (kind, outputs) = take(world, id, basis, &parameters);
    let gram = gram(
        world,
        id,
        kind,
        basis.to_string(),
        refers_to,
        parameters,
        outputs,
    );
    append(world, gram);
}

/// The kind and outputs of the decision of article `basis` with
/// `parameters`, evaluated on the calculation date.
fn take(
    world: &RegelrechtWorld,
    id: &str,
    basis: &str,
    parameters: &BTreeMap<String, Value>,
) -> (String, BTreeMap<String, Value>) {
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
    (kind, result.outputs)
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
/// keep the answer, with the engine's trace. The answer is about the
/// calculation date, as known on `known_on` when it is given.
fn evaluate_over_chronicle(
    world: &mut RegelrechtWorld,
    output: &str,
    law: &str,
    known_on: Option<&str>,
) {
    let records = chronicle_records(&world.service, world.extension.chronicle.grams())
        .unwrap_or_else(|e| panic!("the chronicle as records: {e}"));
    let options = ProjectionOptions {
        known_on,
        trace: true,
        ..ProjectionOptions::default()
    };
    let outcome = world.service.evaluate_projection_with(
        law,
        &[output],
        &records,
        world.parameters.clone(),
        &world.calculation_date,
        options,
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

/// The cell of this scenario, with the chronicle as it stands.
fn cell(world: &RegelrechtWorld) -> Cell {
    Cell::restore(
        &Disk,
        CELL,
        cell_dir(),
        world.extension.chronicle.grams().to_vec(),
    )
    .unwrap_or_else(|e| panic!("the cell: {e}"))
}

/// Keep the cell's chronicle, with the chronolexogram the cell just
/// recorded named `id`: the cell gives it a uuid, a scenario a name.
fn keep(world: &mut RegelrechtWorld, cell: &Cell, recorded: &str, id: &str) {
    let grams = cell
        .grams()
        .iter()
        .cloned()
        .map(|mut g| {
            if g.id == recorded {
                g.id = id.to_string();
            }
            g
        })
        .collect();
    world.extension.chronicle =
        Chronicle::in_memory_with(grams).unwrap_or_else(|e| panic!("the chronicle: {e}"));
}

/// The cell itself takes the decision its submission asks for
/// ([`Cell::decide`]), on the calculation date: where a policy binds an open
/// term to an answer of the cell, the cell answers from its reduction.
fn decide_through_cell(world: &mut RegelrechtWorld, id: &str, submission: &str) {
    let mut cell = cell(world);
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
    let (law, number) = requests.split_once('#').expect("<law>#<article>");
    let on = today(world);
    let empty = serde_json::Map::new();
    let decided = cell
        .decide(
            &mut world.service,
            &[],
            law,
            number,
            submission,
            &empty,
            &empty,
            on,
        )
        .unwrap_or_else(|e| panic!("the cell decides '{id}': {e}"));
    let recorded = decided[0].id.clone();
    keep(world, &cell, &recorded, id);
}

/// The cell itself records a submission of article `number` of `law`
/// against its decision `decision` ([`Cell::record_submission`]), with what
/// the submitter supplies in `fields`.
fn record_submission_through_cell(
    world: &mut RegelrechtWorld,
    id: &str,
    decision: &str,
    number: &str,
    law: &str,
    fields: BTreeMap<String, Value>,
) {
    let mut cell = cell(world);
    let supplied: serde_json::Map<String, serde_json::Value> = fields
        .iter()
        .map(|(k, v)| (k.clone(), stored::to_stored(v).expect("storable value")))
        .collect();
    let on = today(world);
    let gram = cell
        .record_submission(
            &mut world.service,
            &[],
            law,
            number,
            Some(Reference::Local(decision)),
            &supplied,
            on,
        )
        .unwrap_or_else(|e| panic!("the cell records submission '{id}': {e}"));
    keep(world, &cell, &gram.id, id);
}

/// The court records an appeal against a decision of the cell, under the
/// article that establishes it, with the fields in the table.
fn court_records_appeal(
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
    let mut gram = gram(world, id, kind, basis, None, fields, BTreeMap::new());
    gram.recording_actor = COURT.to_string();
    world
        .extension
        .court
        .append(gram)
        .unwrap_or_else(|e| panic!("record in the court's chronicle: {e}"));
}

/// The court rules on an appeal: the decision the appeal asks for, with
/// the appeal's fields and what the court found as parameters.
fn court_rules(
    world: &mut RegelrechtWorld,
    id: &str,
    appeal: &str,
    found: BTreeMap<String, Value>,
) {
    let appeal = world
        .extension
        .court
        .grams()
        .iter()
        .find(|g| g.id == appeal)
        .unwrap_or_else(|| panic!("no appeal '{appeal}' in the court's chronicle"))
        .clone();
    let basis = article(world, &appeal.basis)
        .get_submission()
        .and_then(|s| s.requests())
        .unwrap_or_else(|| panic!("appeal '{}' asks for no ruling", appeal.id))
        .to_string();
    let mut parameters = read_back(&appeal.fields);
    parameters.extend(found);
    let (kind, outputs) = take(world, id, &basis, &parameters);
    let mut gram = gram(
        world,
        id,
        kind,
        basis,
        Some(&appeal.id),
        parameters,
        outputs,
    );
    gram.recording_actor = COURT.to_string();
    world
        .extension
        .court
        .append(gram)
        .unwrap_or_else(|e| panic!("record in the court's chronicle: {e}"));
}

/// The cell records that it received a copy of the court's ruling, as the
/// fact `fact` of the article ([`Cell::record_receipt`]). The copy concerns
/// the decision of the cell the appeal contested (its `bestreden_besluit`).
fn cell_receives_ruling(
    world: &mut RegelrechtWorld,
    ruling: &str,
    fact: &str,
    id: &str,
    number: &str,
    law: &str,
) {
    let court = world.extension.court.grams();
    let ruling = court
        .iter()
        .find(|g| g.id == ruling)
        .unwrap_or_else(|| panic!("no ruling '{ruling}' in the court's chronicle"))
        .clone();
    let appeal = court
        .iter()
        .find(|g| Some(&g.id) == ruling.refers_to.as_ref())
        .expect("the appeal the ruling is on");
    let contested = appeal
        .fields
        .get("bestreden_besluit")
        .and_then(serde_json::Value::as_str)
        .expect("the appeal names the contested decision")
        .to_string();
    let copy = Afschrift {
        from: COURT.to_string(),
        value: serde_json::json!({
            "basis": ruling.basis,
            "fields": ruling.fields,
            "findings": {},
            "outputs": ruling.outputs,
        }),
        gram: ruling,
        about: Some(Received {
            from: CELL.to_string(),
            id: contested.clone(),
            record: None,
        }),
    };
    let mut cell = cell(world);
    let receipt = cell
        .record_receipt(
            &world.service,
            &format!("{law}#{number}#{fact}"),
            &copy,
            &contested,
            today(world),
        )
        .unwrap_or_else(|e| panic!("the cell receives the ruling: {e}"));
    keep(world, &cell, &receipt.id, id);
}

/// The cell gives its answer `name` ([`Cell::answer`]), with the scenario's
/// parameters as the binding passes them, about the calculation date and as
/// known on `known_on` (the calculation date without it).
fn cell_answers(world: &mut RegelrechtWorld, name: &str, known_on: Option<&str>) {
    let on = today(world);
    let known_on = known_on.map_or(on, |d| {
        NaiveDate::parse_from_str(d, "%Y-%m-%d").expect("a date known on")
    });
    match cell(world).answer(&world.service, name, &world.parameters, on, known_on) {
        Ok(answer) => {
            world.extension.answer = answer;
            world.error = None;
        }
        Err(e) => panic!("the cell answers '{name}': {e}"),
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
            ProjectionEvent::Applied { record } => {
                [record.clone(), "applied".to_string(), "-".to_string()]
            }
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
