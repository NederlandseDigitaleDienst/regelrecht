//! The cell's actions (`packages/cel/bdd/grammar.yaml`): its additions to the
//! dispatch it shares with the engine's runner.
//!
//! A scenario has one cell with one chronicle in memory. Records are named by
//! the id in the scenario; the day they are recorded is the calculation date.
//! A feature in `packages/cel/bdd/features` runs against the corpus with the
//! fixtures in `packages/cel/bdd/fixtures` (the synthetic law, and the
//! reductions of a cell) next to it; a feature in a scenario set
//! (`crate::sets`) against the regelwerken of that set alone, its own and
//! those it names from the corpus.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::reading::Reader;
use regelrecht_cel::{
    Afschrift, Cell, Chronicle, Disk, EXECUTOGRAM, Gram, GramMoment, Received, shape,
};
use regelrecht_engine::projection::describe_applied;
use regelrecht_engine::{
    Aggregate, Article, ArticleResult, Authority, Call, LawExecutionService, ProjectionEvent,
    ProjectionOutcome, Value,
};
use regelrecht_law_model::stored;
use walkdir::WalkDir;

use crate::dispatch::{ArgValue, table_cell_value};
use crate::helpers::regulation_loader::load_all_regulations;
use crate::helpers::table::{Rows, rows_to_params};
use crate::sets::{ScenarioSet, load_regelwerk};
use crate::world::RegelrechtWorld;

/// The cell that records, as `recording_actor`.
const CELL: &str = "test_cel";

/// The court, a cell of its own with a chronicle of its own.
const COURT: &str = "test_rechtbank";

/// The cell's chronicle in this scenario, the court's, and the last
/// projection played over the cell's.
pub struct State {
    chronicle: Chronicle,
    court: Chronicle,
    projection: Option<ProjectionOutcome>,
    /// The aggregates the last evaluation read (`source.projections`), each
    /// played again for the assertions on its steps.
    aggregates: Vec<Aggregate>,
    /// Why the cell refused the last thing it was asked to record.
    refused: Option<String>,
}

impl State {
    /// An empty chronicle, and nothing projected or refused yet.
    pub fn new() -> Self {
        Self {
            chronicle: Chronicle::in_memory(),
            court: Chronicle::in_memory(),
            projection: None,
            aggregates: Vec::new(),
            refused: None,
        }
    }
}

/// Load the regelwerken a scenario of the feature at `feature` runs against.
/// A feature in a scenario set ([`ScenarioSet`]) gets the regelwerken of that
/// set alone: its own and those it names from the corpus. Any other feature
/// (`bdd/features`) gets the corpus and the fixtures: every regelwerk under
/// `fixtures/regulation`, and the reductions under `fixtures/cells`.
pub fn load(service: &mut LawExecutionService, feature: Option<&Path>) {
    if let Some(set) = feature.and_then(ScenarioSet::containing) {
        set.load(service);
        return;
    }
    if let Err(e) = load_all_regulations(service) {
        panic!("Failed to load regulations: {e}");
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures");
    for entry in WalkDir::new(&fixtures).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let rel = path.strip_prefix(&fixtures).unwrap();
        let regelwerk = rel.starts_with("regulation")
            || rel.components().any(|c| c.as_os_str() == "reductions");
        if regelwerk && path.extension().is_some_and(|x| x == "yaml") {
            load_regelwerk(service, path);
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
        "decide_under_article_about" => {
            let parameters = rows_to_params(&table.expect("parameters table"), table_cell_value);
            decide_ex_officio(
                world,
                args[0].as_str(),
                args[1].as_str(),
                args[2].as_str(),
                args[3].as_str(),
                parameters,
            );
        }
        "record_submission_about" => {
            let fields = rows_to_params(&table.expect("fields table"), table_cell_value);
            record_submission_referring(
                world,
                args[0].as_str(),
                Some(args[1].as_str()),
                args[2].as_str(),
                args[3].as_str(),
                fields,
            );
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
        "record_fact_about" => {
            let fields = rows_to_params(&table.expect("fields table"), table_cell_value);
            record_fact_about(
                world,
                args[0].as_str(),
                args[1].as_str(),
                args[2].as_str(),
                args[3].as_str(),
                args[4].as_str(),
                fields,
            );
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
            let found = rows_to_params(&table.expect("findings table"), table_cell_value);
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
        "evaluate_over_chronicle" => {
            evaluate_over_chronicle(world, args[0].as_str(), args[1].as_str())
        }
        "evaluate_with_cell" => evaluate_with_cell(
            world,
            args[0].as_str(),
            args[1].as_str(),
            args[2].as_str(),
            args[3].as_str(),
        ),
        "assert_projection_trace" => assert_projection_trace(world, &table.expect("trace table")),
        "assert_cell_refuses" => {
            let message = args[0].as_str();
            let refused = world
                .extension
                .refused
                .as_deref()
                .unwrap_or_else(|| panic!("the cell refused nothing"));
            assert!(
                refused.contains(message),
                "the cell refused with: {refused}"
            );
        }
        "assert_last_set_by" => {
            // In one of the aggregates read, or in the projection played.
            let (name, id) = (args[0].as_str(), args[1].as_str());
            let setters: Vec<Option<String>> = played(world)
                .into_iter()
                .map(|(_, set_by)| {
                    set_by
                        .get(name)
                        .unwrap_or_else(|| panic!("no state property '{name}'"))
                        .clone()
                })
                .collect();
            assert!(
                setters.iter().any(|s| s.as_deref() == Some(id)),
                "the record that set '{name}' last: {setters:?}"
            );
        }
        "assert_record_applied" => {
            let id = args[0].as_str();
            let events: Vec<ProjectionEvent> = played(world)
                .into_iter()
                .flat_map(|(events, _)| events)
                .collect();
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
        "assert_submission_fields" => assert_submission_fields(
            world,
            args[0].as_str(),
            args[1].as_str(),
            &table.expect("fields table"),
        ),
        "assert_submission_moment" => {
            let (number, law) = (args[0].as_str(), args[1].as_str());
            let parameter = moment(world, law, number)
                .and_then(|m| m.parameter)
                .unwrap_or_else(|| {
                    panic!("the submission under {law}#{number} names no moment parameter")
                });
            assert_eq!(
                parameter,
                args[2].as_str(),
                "the moment of the submission under {law}#{number}"
            );
        }
        "assert_decision_within" => {
            assert_decision_within(world, args[0].as_str(), args[1].as_str(), args[2].as_bool())
        }
        _ => return false,
    }
    true
}

/// The fields of an application under article `number` of `law` are exactly
/// those in the table (header `field`), in any order.
fn assert_submission_fields(world: &RegelrechtWorld, number: &str, law: &str, table: &Rows) {
    let shape = shape::derive(&world.service, law, number, today(world))
        .unwrap_or_else(|e| panic!("the submission under {law}#{number}: {e}"));
    let mut held: Vec<&str> = shape.fields.iter().map(|f| f.name.as_str()).collect();
    held.sort_unstable();
    let (header, rows) = table.split_first().expect("a header row `field`");
    assert_eq!(
        header.as_slice(),
        ["field"],
        "the header of the fields table"
    );
    let mut expected: Vec<&str> = rows.iter().map(|row| row[0].as_str()).collect();
    expected.sort_unstable();
    assert_eq!(
        held, expected,
        "the fields of the submission under {law}#{number}"
    );
}

/// Whether decision `id` was recorded on or before the date its output
/// `output` holds (`within`), or after it.
fn assert_decision_within(world: &RegelrechtWorld, id: &str, output: &str, within: bool) {
    let gram = world
        .extension
        .chronicle
        .grams()
        .iter()
        .find(|g| g.id == id)
        .unwrap_or_else(|| panic!("no decision '{id}' in the chronicle"));
    // A submission or a fact holds no decision date to compare.
    assert!(
        !matches!(
            gram.kind.as_str(),
            "AANVRAAG" | "BEZWAAR" | "BEROEP" | EXECUTOGRAM
        ),
        "'{id}' is a {}, not a decision",
        gram.kind
    );
    let last = gram
        .outputs
        .get(output)
        .and_then(serde_json::Value::as_str)
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or_else(|| {
            panic!(
                "decision '{id}' holds no date '{output}': {:?}",
                gram.outputs.get(output)
            )
        });
    assert_eq!(
        gram.recorded_on <= last,
        within,
        "decision '{id}' given on {} against '{output}' {last}",
        gram.recorded_on
    );
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
    record_submission_referring(world, id, None, number, law, fields);
}

/// [`record_submission`], about the earlier record `refers_to`.
fn record_submission_referring(
    world: &mut RegelrechtWorld,
    id: &str,
    refers_to: Option<&str>,
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
    let moment = moment(world, law, number);
    // As the cell does, refuse the moment as a field: the cell fills it.
    if let Some(name) = moment.as_ref().and_then(|m| m.parameter.as_ref()) {
        assert!(
            !fields.contains_key(name),
            "'{name}' is the moment of the submission under {basis}; the cell records it"
        );
    }
    let mut gram = gram(world, id, kind, basis, refers_to, fields, BTreeMap::new());
    gram.moment = moment;
    append(world, gram);
}

/// The moment that counts for a submission under article `number` of `law`,
/// such as the receipt (Awb 4:13): the day it is recorded, as the cell fills
/// it ([`Cell::record_submission`]). `None` when no article names one.
fn moment(world: &RegelrechtWorld, law: &str, number: &str) -> Option<GramMoment> {
    let on = today(world);
    let shape = shape::derive(&world.service, law, number, on)
        .unwrap_or_else(|e| panic!("the submission under {law}#{number}: {e}"));
    shape.moment.map(|m| GramMoment {
        parameter: m.parameter,
        on,
        declared_by: m.declared_by,
    })
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
    // The moment of the submission, as the cell hands it to the decision.
    // The cell sends it only where the decision asks it
    // (`regelrecht_cel::DecisionShape`); this step always sends it, which
    // matters only for a decision under another article than the
    // submission's, and the engine passes a hook only what it declares.
    if let Some(m) = &gram.moment
        && let Some(name) = &m.parameter
    {
        let day = serde_json::Value::String(m.on.to_string());
        parameters.insert(name.clone(), stored::from_stored(&day).expect("a date"));
    }
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
/// The canonical output steps then read what the decision gave, with what
/// the hooks on it gave (such as the last day of a decision term).
fn decide(
    world: &mut RegelrechtWorld,
    id: &str,
    basis: &str,
    refers_to: Option<&str>,
    parameters: BTreeMap<String, Value>,
) {
    let Decided { gram, result } = decided(world, id, basis, refers_to, parameters);
    append(world, gram);
    world.result = Some(result);
    world.error = None;
}

/// A decision of the cell, not yet recorded, and the engine's result it
/// came from.
struct Decided {
    gram: Gram,
    result: ArticleResult,
}

/// The decision of article `basis` with `parameters`, as a chronolexogram
/// of the cell, not yet recorded.
fn decided(
    world: &RegelrechtWorld,
    id: &str,
    basis: &str,
    refers_to: Option<&str>,
    parameters: BTreeMap<String, Value>,
) -> Decided {
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
        result.outputs.clone(),
    );
    Decided { gram, result }
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
        law_versions: Vec::new(),
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

/// The court records an appeal under article `number` of `law`, in its own
/// chronicle.
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

/// The court rules on an appeal: the decision the appeal asks for, with the
/// appeal's fields and what the court `found` as parameters.
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
    let mut parameters: BTreeMap<String, Value> = appeal
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), stored::from_stored(v).expect("stored field")))
        .collect();
    parameters.extend(found);
    // The ruling is the court's, not a decision of the cell: it does not
    // become the result the canonical output steps read.
    let mut gram = decided(world, id, &basis, Some(&appeal.id), parameters).gram;
    gram.recording_actor = COURT.to_string();
    world
        .extension
        .court
        .append(gram)
        .unwrap_or_else(|e| panic!("record in the court's chronicle: {e}"));
}

/// The cell records that it received a copy of the court's ruling, as the
/// fact `fact` of the article, through [`Cell::record_receipt`]. The copy
/// concerns the decision of the cell that the appeal contested (its
/// `bestreden_besluit`). The court's cell would make the copy with
/// [`Cell::afschrift`]; here the scenario builds it from the court's
/// chronicle, as `packages/cel/tests/beroep.rs` tests the copy itself.
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
            "taken_on": appeal.basis,
            "fields": ruling.fields,
            "findings": {},
            "outputs": ruling.outputs,
        }),
        gram: ruling,
        about: Some(Received {
            from: CELL.to_string(),
            id: contested.clone(),
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

/// The cell of the scenario, with its chronicle.
fn cell(world: &RegelrechtWorld) -> Cell {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("bdd/fixtures/cells");
    Cell::restore(
        &Disk,
        CELL,
        fixtures.join(CELL),
        world.extension.chronicle.grams().to_vec(),
    )
    .unwrap_or_else(|e| panic!("the cell: {e}"))
}

/// Keep the cell's chronicle, with the chronolexogram it just recorded
/// named `id`: the cell names it by a uuid, the scenario by `id`.
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

/// Record a fact that refers to the earlier record `about`, through the
/// cell ([`Cell::record_executogram_referring`]), as the fact `fact` of the
/// article, with the values of its fields.
fn record_fact_about(
    world: &mut RegelrechtWorld,
    fact: &str,
    id: &str,
    about: &str,
    number: &str,
    law: &str,
    fields: BTreeMap<String, Value>,
) {
    let supplied: serde_json::Map<String, serde_json::Value> = fields
        .into_iter()
        .map(|(k, v)| (k, stored::to_stored(&v).expect("storable value")))
        .collect();
    let mut cell = cell(world);
    let gram = cell
        .record_executogram_referring(
            &world.service,
            &format!("{law}#{number}#{fact}"),
            &supplied,
            Some(about),
            today(world),
        )
        .unwrap_or_else(|e| panic!("fact {fact} about {about}: {e}"));
    keep(world, &cell, &gram.id, id);
}

/// Take a decision ex officio about the earlier record `about`, through the
/// cell ([`Cell::decide_ex_officio`]), under article `number` of `law`,
/// with the parameters the authority supplies. A refusal is kept for
/// `assert_cell_refuses`, and nothing is recorded.
fn decide_ex_officio(
    world: &mut RegelrechtWorld,
    id: &str,
    about: &str,
    number: &str,
    law: &str,
    parameters: BTreeMap<String, Value>,
) {
    let supplied: serde_json::Map<String, serde_json::Value> = parameters
        .into_iter()
        .map(|(k, v)| (k, stored::to_stored(&v).expect("storable value")))
        .collect();
    let mut cell = cell(world);
    let on = today(world);
    let taken = cell.decide_ex_officio(
        &mut world.service,
        &[],
        law,
        number,
        Some(about),
        &supplied,
        &serde_json::Map::new(),
        on,
    );
    match taken {
        Ok(grams) => {
            let [gram] = &grams[..] else {
                panic!("decision {id}: {} chronolexograms", grams.len());
            };
            world.extension.refused = None;
            keep(world, &cell, &gram.id.clone(), id);
        }
        Err(e) => world.extension.refused = Some(e.to_string()),
    }
}

/// The authority of the cell, as the fixture policy names its holder.
const AUTHORITY: &str = "het college van de testcel";

/// Evaluate `output` of `law` as a decision does, with the cell as the
/// answerer of the call. Its catalogue gives `answer` from `reduction`.
fn evaluate_with_cell(
    world: &mut RegelrechtWorld,
    output: &str,
    law: &str,
    answer: &str,
    reduction: &str,
) {
    let authority = Authority {
        name: AUTHORITY.to_string(),
        category: None,
    };
    let answers = BTreeMap::from([(answer.to_string(), reduction.to_string())]);
    let reductions = BTreeSet::from([reduction.to_string()]);
    let grams = world.extension.chronicle.grams().to_vec();
    let reader = Reader::new(CELL, &authority, &answers, &reductions, grams, None);
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&reader),
        decides_for: None,
        chronicle: Some(&reader),
        sphere: Vec::new(),
    };
    let outcome = world.service.evaluate_law_with_trace_in(
        law,
        &[output],
        world.parameters.clone(),
        call,
        &world.calculation_date,
    );
    world.requested_outputs = vec![output.to_string()];
    match outcome {
        Ok(result) => {
            world.result = Some(result);
            world.error = None;
        }
        Err(e) => {
            world.result = None;
            world.error = Some(e);
        }
    }
}

/// Play every record of the chronicle through the projection of `law` and
/// keep the answer, with the engine's trace.
fn evaluate_over_chronicle(world: &mut RegelrechtWorld, output: &str, law: &str) {
    let records = regelrecht_cel::reading::chronicle_records(world.extension.chronicle.grams())
        .expect("the chronicle as records");
    let outcome = world.service.evaluate_projection(
        law,
        &[output],
        &records,
        world.parameters.clone(),
        &world.calculation_date,
        true,
    );
    world.requested_outputs = vec![output.to_string()];
    world.extension.aggregates = Vec::new();
    match outcome {
        Ok(outcome) => {
            world.result = Some(outcome.result.clone());
            world.error = None;
            world.extension.projection = Some(outcome);
            world.extension.aggregates = aggregates_read(world, output, law, &records);
        }
        Err(e) => {
            world.result = None;
            world.error = Some(e);
            world.extension.projection = None;
        }
    }
}

/// The aggregates the article of `law` that gives `output` reads with
/// `source.projections`, picked by `where` from the parameters of the
/// scenario, as the engine plays them.
fn aggregates_read(
    world: &RegelrechtWorld,
    output: &str,
    law: &str,
    records: &[regelrecht_engine::ChronicleRecord],
) -> Vec<Aggregate> {
    let Some(article) = world
        .service
        .resolver()
        .get_law_for_date(law, Some(today(world)))
        .and_then(|l| l.find_article_by_output(output))
    else {
        return Vec::new();
    };
    let mut uit = Vec::new();
    for source in article
        .get_inputs()
        .iter()
        .filter_map(|i| i.source.as_ref())
    {
        let Some(reduction) = &source.projections else {
            continue;
        };
        let filter = source
            .where_
            .iter()
            .flatten()
            .map(|(name, reference)| {
                let parameter = reference.trim_start_matches('$');
                let value = world
                    .parameters
                    .get(parameter)
                    .unwrap_or_else(|| panic!("no parameter '{parameter}' for where"))
                    .clone();
                (name.clone(), value)
            })
            .collect();
        uit.extend(
            world
                .service
                .aggregates(reduction, records, &filter, &world.calculation_date)
                .unwrap_or_else(|e| panic!("the aggregates of {reduction}: {e}")),
        );
    }
    uit
}

/// The events and setters of what the last evaluation played: each
/// aggregate it read, or else the projection it played.
type Played = (Vec<ProjectionEvent>, BTreeMap<String, Option<String>>);

fn played(world: &RegelrechtWorld) -> Vec<Played> {
    if !world.extension.aggregates.is_empty() {
        return world
            .extension
            .aggregates
            .iter()
            .map(|a| (a.events.clone(), a.set_by.clone()))
            .collect();
    }
    let outcome = projection(world);
    vec![(outcome.events.clone(), outcome.set_by.clone())]
}

fn projection(world: &RegelrechtWorld) -> &ProjectionOutcome {
    world.extension.projection.as_ref().unwrap_or_else(|| {
        panic!(
            "no projection was played: {:?}",
            world.error.as_ref().map(ToString::to_string)
        )
    })
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
                format!("revoked by {}", undone_by.join(", ")),
            ],
            ProjectionEvent::EffectsOnly { record, reason } => {
                [record.clone(), "effects only".to_string(), reason.clone()]
            }
        })
        .collect()
}

/// The trace table must match the events exactly, and the engine's trace
/// must hold them as the children of its `PROJECTION` node.
fn assert_projection_trace(world: &RegelrechtWorld, table: &Rows) {
    let header: Vec<&str> = table
        .first()
        .map(|row| row.iter().map(|c| c.trim()).collect())
        .unwrap_or_default();
    let show = |rows: &[Vec<String>]| {
        rows.iter()
            .map(|r| format!("| {} |", r.join(" | ")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let expected: Vec<Vec<String>> = table
        .iter()
        .skip(1)
        .map(|row| row.iter().map(|c| c.trim().to_string()).collect())
        .collect();
    if header == ["aggregate", "record", "event", "detail"] {
        // The steps of each aggregate the reduction read, by its root.
        let actual: Vec<Vec<String>> = world
            .extension
            .aggregates
            .iter()
            .flat_map(|a| {
                trace_rows(&a.events)
                    .into_iter()
                    .map(|[record, event, detail]| vec![a.root.clone(), record, event, detail])
            })
            .collect();
        assert!(expected == actual, "aggregate trace:\n{}", show(&actual));
        return;
    }
    let outcome = projection(world);
    assert_eq!(
        header,
        ["record", "event", "detail"],
        "the header of the trace table"
    );
    let actual: Vec<Vec<String>> = trace_rows(&outcome.events)
        .into_iter()
        .map(|r| r.to_vec())
        .collect();
    assert!(expected == actual, "projection trace:\n{}", show(&actual));

    let root = outcome.result.trace.as_ref().expect("the engine trace");
    let node = root
        .children
        .iter()
        .find(|n| n.name == "PROJECTION")
        .expect("a PROJECTION node in the engine trace");
    // Phase 1 has a node per record; phase 2 one per record applied.
    let phase = |name: &str| {
        node.children
            .iter()
            .find(|n| n.name == name)
            .map_or(0, |n| n.children.len())
    };
    assert_eq!(
        phase("COUNTS"),
        outcome.events.len(),
        "a COUNTS node per record"
    );
    let applied = outcome
        .events
        .iter()
        .filter(|e| matches!(e, ProjectionEvent::Applied { .. }))
        .count();
    assert_eq!(phase("FOLD"), applied, "a FOLD node per record applied");
}
