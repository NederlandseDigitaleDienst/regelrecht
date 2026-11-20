//! The lifecycle of a submission in a cell: recorded,
//! decided, counted by a reduction, objected to, and replayed under the
//! regulation versions it was taken under (RFC-045, RFC-046, RFC-048). The
//! case is leave from school.
//!
//! A cell records an application for vakantieverlof under a fictional
//! school leave act (article 13a, after the Leerplichtwet 1969), with the
//! hooks of a mini general administrative act on it (RFC-045, RFC-046), and
//! the head of the school decides on it (RFC-046). Both acts are fixtures
//! (`tests/fixtures/corpus`), not the corpus.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod application;
mod cell_directory;
#[path = "../common/mod.rs"]
mod common;
mod decision_on_application;
mod objection;
mod own_reduction;
mod regelwerk_values;
mod regulation_versions;
mod replay;

use common::{answered, cell_tempdir, day, object_map};

use std::path::Path;

use regelrecht_cel::{Cell, Disk, Error, Gram, LawSince, Reference, Source, decision, shape};
use regelrecht_engine::LawExecutionService;
use serde_json::{Map, Value, json};

const LAW: &str = "test_school_leave_act";
const ARTICLE: &str = "13a";
const AWB: &str = "test_general_administrative_act";
/// A fictional school.
const RECORDING_ACTOR: &str = "obs_de_klimop";

/// The fictitious policy of the school that says when its school year
/// starts.
const POLICY: &str = "fictief_beleid_schooljaar_obs_de_klimop";

/// The fictitious policy of the school that says in which school year a
/// day falls; the head's policy calls it (RFC-048).
const SCHOOL_YEAR: &str = "fictief_beleid_schooljaargrenzen_obs_de_klimop";

/// The head's policy on vakantieverlof: the norms the reduction applies,
/// and the bindings of the open terms of art. 13a to the answers of the
/// school's cell (RFC-048).
const LEAVE_POLICY: &str = "fictief_beleid_vakantieverlof_obs_de_klimop";

/// The school's reduction for lid 2, and the answer it gives: the open term
/// of art. 13a that the head's policy binds to it.
const REDUCTION: &str = "reductie_verlof_al_verleend_in_schooljaar";
const ONCE: &str = "verlof_al_verleend_in_schooljaar";
/// What the head finds on it where his cell gives no answer: an open term of
/// his policy (art. 3, RFC-048 §2).
const ONCE_HEAD: &str = "verlof_al_verleend_in_schooljaar_volgens_hoofd";

/// The school leave act, the general act and the school's policies from the
/// fixtures.
fn service() -> LawExecutionService {
    service_without(&[])
}

/// What the article of the submission `gram` gives on it, with the hooks on
/// it: not in its chronolexogram, but computed again by the cell.
fn given(cell: &Cell, gram: &Gram) -> std::collections::BTreeMap<String, Value> {
    cell.submission_outputs(&mut service(), &[], &gram.id)
        .unwrap()
}

/// As [`service`], without the regelwerken under `without`.
fn service_without(without: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let laws = [
        format!("regulation/nl/wet/{LAW}"),
        "regulation/nl/wet/test_general_administrative_act".to_string(),
        format!("regulation/nl/uitvoeringsbeleid/{POLICY}"),
        format!("regulation/nl/uitvoeringsbeleid/{SCHOOL_YEAR}"),
        format!("regulation/nl/uitvoeringsbeleid/{LEAVE_POLICY}"),
        // The school's reduction for lid 2, with its cell (RFC-048).
        common::reduction_dir(REDUCTION),
        // The decisions under an article as aggregates, with what the Awb
        // gives a decision on an objection and the receipt of a ruling.
        common::reduction_dir("reductie_verlof"),
        "regulation/nl/uitvoeringsbeleid/fictief_beleid_verleend_verlof_obs_de_klimop".to_string(),
    ]
    .into_iter()
    .filter(|law| !without.iter().any(|w| law.ends_with(w)));
    common::load(&mut service, laws);
    service
}

/// The school's cell directory, with its identity and catalogue (RFC-048).
fn cell_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/corpus/cells/{RECORDING_ACTOR}"))
}

/// A directory of the school's cell that gives the answers `answers`, each
/// with its reduction: its own identity, so the head's policy, which names
/// the head as its holder, asks it.
fn answers_dir(answers: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = cell_tempdir();
    let mut yaml = "name: Het hoofd van OBS De Klimop\ncategory: het hoofd\n".to_string();
    if !answers.is_empty() {
        yaml.push_str("answers:\n");
    }
    for (answer, reduction) in answers {
        yaml.push_str(&format!("  {answer}: {reduction}\n"));
    }
    common::write_identity(dir.path(), &yaml);
    dir
}

/// A directory of the school's cell that gives only the answer for lid 2
/// (RFC-048). Its own directory also answers art. 11, onder f, from its
/// holidays (RFC-051); the tests here leave that judgement to the head, and
/// `tests/answer_from_executograms.rs` decides in the school's own cell.
fn lid_2_dir() -> tempfile::TempDir {
    answers_dir(&[(ONCE, REDUCTION)])
}

/// The school's cell with an empty chronicle that gives only the answer for
/// lid 2 ([`lid_2_dir`]). The cell reads its catalogue when it opens, so the
/// directory may go after.
fn cell() -> Cell {
    Cell::in_memory(RECORDING_ACTOR, lid_2_dir().path()).unwrap()
}

/// A complete application, with fictional data (BSNs from the test range).
fn application() -> Map<String, Value> {
    json!({
        "bsn_jongere": "999990019",
        "eerste_verlofdag": "2027-02-01",
        "laatste_verlofdag": "2027-02-05",
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "J. Jansen",
        "adres_aanvrager": "Voorbeeldstraat 1",
        "dagtekening": "2026-10-12",
        "gevraagde_beschikking": "vakantieverlof",
    })
    .as_object()
    .unwrap()
    .clone()
}

fn record(
    cell: &mut Cell,
    supplied: &Map<String, Value>,
    on: &str,
) -> Result<regelrecht_cel::Gram, Error> {
    cell.record_submission(&mut service(), &[], LAW, ARTICLE, None, supplied, day(on))
}

fn uuid_version(id: &str) -> usize {
    uuid::Uuid::parse_str(id).unwrap().get_version_num()
}

// ---------------------------------------------------------------------------
// The decision on the application (RFC-045)
// ---------------------------------------------------------------------------

/// The two facts of art. 11, onder f, both judged by the head.
const PROFESSION: &str = "specifieke_aard_beroep_ouder";
const OUTSIDE_HOLIDAYS: &str = "verlof_valt_buiten_schoolvakanties";
/// The head's judgement on it where his cell gives no answer: an open term
/// of his policy (art. 4, RFC-048 §2).
const OUTSIDE_HOLIDAYS_HEAD: &str = "verlof_valt_buiten_schoolvakanties_volgens_hoofd";

/// An application for leave from `first` to `last`, recorded on `on`.
fn applied(cell: &mut Cell, first: &str, last: &str, on: &str) -> Gram {
    let mut supplied = application();
    supplied.insert("eerste_verlofdag".into(), json!(first));
    supplied.insert("laatste_verlofdag".into(), json!(last));
    record(cell, &supplied, on).unwrap()
}

/// The head's judgements under art. 11, onder f: the profession of a parent,
/// and leave outside the school holidays, which his policy leaves to him
/// where his cell gives no answer.
fn judgement(profession: bool, outside_holidays: bool) -> Map<String, Value> {
    json!({ PROFESSION: profession, OUTSIDE_HOLIDAYS_HEAD: outside_holidays })
        .as_object()
        .unwrap()
        .clone()
}

/// Decide with what the head gives: its findings under art. 11 onder f and
/// art. 13a lid 2 apart, the rest as what it supplies.
fn decide(
    cell: &mut Cell,
    application: &str,
    given: &Map<String, Value>,
    on: &str,
) -> Result<Gram, Error> {
    let (findings, supplied): (Map<String, Value>, Map<String, Value>) =
        given.clone().into_iter().partition(|(name, _)| {
            [
                PROFESSION,
                OUTSIDE_HOLIDAYS,
                OUTSIDE_HOLIDAYS_HEAD,
                ONCE,
                ONCE_HEAD,
            ]
            .contains(&name.as_str())
        });
    cell.decide(
        &mut service(),
        &[],
        LAW,
        ARTICLE,
        application,
        &supplied,
        &findings,
        day(on),
    )
    .map(|mut grams| grams.remove(0))
}

// ---------------------------------------------------------------------------
// Once per school year: the school's reduction reads the chronicle back,
// calls the school's policy for the school year, and fills the open term of
// art. 13a with its answer (implements, RFC-048)
// ---------------------------------------------------------------------------

/// Apply for leave from `first` to `last` on `on`, and decide on it the
/// same day with the ground of art. 11 onder f.
fn apply_and_decide(cell: &mut Cell, first: &str, last: &str, on: &str) -> Gram {
    let app = applied(cell, first, last, on);
    decide(cell, &app.id, &judgement(true, true), on).unwrap()
}

/// A cell that gives no answers, as a school without a reduction.
fn without_reductions() -> Cell {
    let dir = answers_dir(&[]);
    Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap()
}

/// The service with the school's reduction for lid 2 in force from `from`.
fn service_with_reduction_from(from: &str) -> LawExecutionService {
    let mut service = service_without(&[REDUCTION]);
    let yaml = std::fs::read_to_string(
        common::corpus()
            .join(common::reduction_dir(REDUCTION))
            .join("2026-08-01.yaml"),
    )
    .unwrap();
    let later = yaml.replace("valid_from: '2026-08-01'", &format!("valid_from: '{from}'"));
    assert_ne!(later, yaml);
    service.load_law(&later).unwrap();
    service
}

// ---------------------------------------------------------------------------
// The objection and the decision on it (RFC-045 §7)
// ---------------------------------------------------------------------------

/// Awb 7:11 decides on the objection and establishes it.
const OBJECTION: &str = "7:11";
const TIMELY: &str = "bezwaar_of_beroepschrift_tijdig_ingediend";
const ADMISSIBLE: &str = "bezwaar_ontvankelijk";
const CAUSE: &str = "heroverweging_geeft_aanleiding";
const REVOKED: &str = "bestreden_besluit_herroepen";
const NEW: &str = "nieuw_besluit";

/// A complete objection, with fictional data, and `with`.
fn objection(with: &[(&str, &str)]) -> Map<String, Value> {
    let mut supplied = json!({
        "bezwaar_of_beroepschrift_ondertekend": true,
        "naam_indiener": "J. Jansen",
        "adres_indiener": "Voorbeeldstraat 1",
        "dagtekening_bezwaar_of_beroepschrift": "2026-11-01",
        "omschrijving_bestreden_besluit": "weigering vakantieverlof",
        "gronden_bezwaar_of_beroep": "verklaring van de werkgever: geen verlof in de schoolvakanties",
    })
    .as_object()
    .unwrap()
    .clone();
    for (name, value) in with {
        supplied.insert((*name).into(), json!(value));
    }
    supplied
}

/// Record the objection against the decision `contested`: a submission
/// under Awb 7:11 that refers to it.
fn object(
    cell: &mut Cell,
    contested: &str,
    supplied: &Map<String, Value>,
    on: &str,
) -> Result<Gram, Error> {
    cell.record_submission(
        &mut service(),
        &[],
        AWB,
        OBJECTION,
        Some(Reference::Local(contested)),
        supplied,
        day(on),
    )
}

fn decide_objection(cell: &mut Cell, objection: &str, on: &str) -> Result<Vec<Gram>, Error> {
    reconsider(cell, objection, &Map::new(), on)
}

/// Decide on the objection with what the head finds anew, as any
/// decision: the decision on the objection first, then the new decision in
/// the place of the revoked one, if any.
fn reconsider(
    cell: &mut Cell,
    objection: &str,
    findings: &Map<String, Value>,
    on: &str,
) -> Result<Vec<Gram>, Error> {
    cell.decide(
        &mut service(),
        &[],
        AWB,
        OBJECTION,
        objection,
        &Map::new(),
        &common::on_objection(findings),
        day(on),
    )
}

/// Five school days, refused on 20 October: the head judged that the
/// profession of a parent is not of the kind art. 11 onder f asks.
fn refused_on_profession(cell: &mut Cell) -> Gram {
    let app = applied(cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let refused = decide(cell, &app.id, &judgement(false, true), "2026-10-20").unwrap();
    assert_eq!(refused.outputs["verlof_verleend"], json!(false));
    refused
}

/// The head's finding on the profession of a parent, given anew at the
/// heroverweging, apart from the parameters.
fn profession(judged: bool) -> Map<String, Value> {
    object_map(json!({ PROFESSION: judged }))
}

// ---------------------------------------------------------------------------
// The regulation versions a record was taken under
// ---------------------------------------------------------------------------

/// The school year 2026-2027 of the school, as the fixtures hold it.
fn school_year() -> String {
    std::fs::read_to_string(common::corpus().join(format!(
        "regulation/nl/uitvoeringsbeleid/{POLICY}/2026-08-01.yaml"
    )))
    .unwrap()
}

/// [`service`], with the school year 2027-2028 as the fixtures hold it and
/// `versions` of the school year in place of 2026-2027.
fn service_with_school_year(versions: &[String]) -> LawExecutionService {
    let mut service = service_without(&[POLICY]);
    let next = common::corpus().join(format!(
        "regulation/nl/uitvoeringsbeleid/{POLICY}/2027-08-01.yaml"
    ));
    service
        .load_law(&std::fs::read_to_string(next).unwrap())
        .unwrap();
    for version in versions {
        service.load_law(version).unwrap();
    }
    service
}

/// [`service`], where a version of the school's policy from 5 December
/// 2026 no longer gives the first lesdag: art. 13a has no answer for it from
/// then on. A decision taken before still replays under the version of its
/// own day.
fn service_with_policy_lapsed() -> LawExecutionService {
    let path = common::corpus().join(format!(
        "regulation/nl/uitvoeringsbeleid/{POLICY}/2026-08-01.yaml"
    ));
    let yaml = std::fs::read_to_string(path).unwrap();
    // The article keeps its text and loses its machine-readable part.
    let start = yaml.find("    machine_readable:").unwrap();
    let lapsed = yaml[..start]
        .to_string()
        .replace("valid_from: '2026-08-01'", "valid_from: '2026-12-05'")
        .replace(
            "publication_date: '2026-07-01'",
            "publication_date: '2026-12-01'",
        );
    let mut service = service();
    service.load_law(&lapsed).unwrap();
    service
}
