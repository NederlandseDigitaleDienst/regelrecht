//! A cell records an application for vakantieverlof (Leerplichtwet 1969
//! art. 13a) on the real corpus, with the Awb hooks on it (RFC-045, RFC-046),
//! and the head of the school decides on it (RFC-047).

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::{decision, shape, Cell, Error, Gram, Reference, Source};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

mod common;
use common::cell_tempdir;

const LAW: &str = "leerplichtwet_1969";
const ARTICLE: &str = "13a";
const AWB: &str = "algemene_wet_bestuursrecht";
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

/// The Leerplichtwet, the Awb and the school's policies from the corpus.
fn service() -> LawExecutionService {
    service_without(&[])
}

/// As [`service`], without the regelwerken under `without`.
fn service_without(without: &[&str]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let laws = [
        format!("regulation/nl/wet/{LAW}"),
        format!("regulation/nl/wet/{AWB}"),
        format!("regulation/nl/uitvoeringsbeleid/{POLICY}"),
        format!("regulation/nl/uitvoeringsbeleid/{SCHOOL_YEAR}"),
        format!("regulation/nl/uitvoeringsbeleid/{LEAVE_POLICY}"),
        // The school's reduction for lid 2, with its cell (RFC-048).
        common::reduction_dir(REDUCTION),
        // Which decisions still stand after objection and appeal: the
        // reduction of the Awb, which the school's reduction calls.
        common::reduction_dir("reductie_algemene_wet_bestuursrecht"),
    ]
    .into_iter()
    .filter(|law| !without.iter().any(|w| law.ends_with(w)));
    common::load(&mut service, laws);
    service
}

/// The school's cell directory, with its identity and catalogue (RFC-048).
fn cell_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{RECORDING_ACTOR}"))
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
/// `tests/schoolvakanties.rs` decides in the school's own cell.
fn lid_2_dir() -> tempfile::TempDir {
    answers_dir(&[(ONCE, REDUCTION)])
}

/// The school's cell with an empty chronicle that gives only the answer for
/// lid 2 ([`lid_2_dir`]). The cell reads its catalogue when it opens, so the
/// directory may go after.
fn cell() -> Cell {
    Cell::in_memory(RECORDING_ACTOR, lid_2_dir().path()).unwrap()
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
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

#[test]
fn the_shape_of_the_application_comes_from_the_law() {
    let shape = shape::derive(&service(), LAW, ARTICLE, day("2026-10-13")).unwrap();
    assert_eq!(shape.kind, "AANVRAAG");
    // Art. 13a is also the decision: recording the application runs only
    // the hooks, at the stage AANVRAAG of the Awb procedure.
    assert_eq!(shape.stage.as_deref(), Some("AANVRAAG"));
    assert!(shape.outputs.is_empty());
    // Every parameter of art. 13a and of the Awb hooks on its application,
    // but the moment, which the cell fills.
    let supplied: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    assert_eq!(
        supplied,
        vec![
            ("bsn_jongere", "leerplichtwet_1969#13a"),
            ("eerste_verlofdag", "leerplichtwet_1969#13a"),
            ("laatste_verlofdag", "leerplichtwet_1969#13a"),
            ("aanvraag_ondertekend", "algemene_wet_bestuursrecht#4:2"),
            ("naam_aanvrager", "algemene_wet_bestuursrecht#4:2"),
            ("adres_aanvrager", "algemene_wet_bestuursrecht#4:2"),
            ("dagtekening", "algemene_wet_bestuursrecht#4:2"),
            ("gevraagde_beschikking", "algemene_wet_bestuursrecht#4:2"),
        ]
    );
    let moment = shape.moment.unwrap();
    assert_eq!(moment.parameter.as_deref(), Some("datum_ontvangst"));
    assert_eq!(moment.declared_by, "algemene_wet_bestuursrecht#4:13");
}

#[test]
fn an_application_is_recorded_with_its_basis_and_the_awb_deadline() {
    let mut cell = cell();
    let gram = record(&mut cell, &application(), "2026-10-13").unwrap();

    assert_eq!(gram.kind, "AANVRAAG");
    assert_eq!(gram.basis, "leerplichtwet_1969#13a");
    assert_eq!(gram.recording_actor, RECORDING_ACTOR);
    assert_eq!(gram.recorded_on, day("2026-10-13"));
    assert_eq!(uuid_version(&gram.id), 7);

    // What the parent supplied; the receipt is no part of it.
    assert_eq!(gram.fields["eerste_verlofdag"], json!("2027-02-01"));
    assert_eq!(gram.fields.len(), 8);
    assert!(!gram.fields.contains_key("datum_ontvangst"));

    // The receipt the cell filled in.
    let moment = gram.moment.as_ref().unwrap();
    assert_eq!(moment.parameter.as_deref(), Some("datum_ontvangst"));
    assert_eq!(moment.on, day("2026-10-13"));
    assert_eq!(moment.declared_by, "algemene_wet_bestuursrecht#4:13");

    // What the Awb hooks gave. Art. 13a follows the Awb procedure, so
    // recording the application runs its stage AANVRAAG: the outputs of
    // art. 13a belong to the decision (`produces`), and no hook on the
    // decision fires yet.
    assert_eq!(gram.outputs.len(), 2);
    assert_eq!(
        gram.outputs["aanvraag_bevat_wat_artikel_4_2_eist"],
        json!(true)
    );
    assert_eq!(
        gram.outputs["redelijke_beslistermijn_einddatum"],
        json!("2026-12-08")
    );
    // The chronolexogram does not repeat the law: what each field and output
    // rests on follows from the law in force on `recorded_on`.
    let serialized = serde_json::to_value(&gram).unwrap();
    assert!(serialized.get("field_basis").is_none());
    assert!(serialized.get("output_basis").is_none());
    assert!(serialized["moment"].get("basis").is_none());
    assert_eq!(serialized["basis"], "leerplichtwet_1969#13a");

    assert_eq!(cell.grams(), [gram]);
}

#[test]
fn an_application_without_an_address_is_recorded_and_4_2_says_it_lacks() {
    let mut cell = cell();
    let mut empty = application();
    empty.insert("adres_aanvrager".into(), Value::Null);
    let mut left_out = application();
    left_out.remove("adres_aanvrager");
    let grams: Vec<_> = [empty, left_out]
        .iter()
        .map(|supplied| record(&mut cell, supplied, "2026-10-13").unwrap())
        .collect();
    for gram in &grams {
        assert_eq!(
            gram.outputs["aanvraag_bevat_wat_artikel_4_2_eist"],
            json!(false)
        );
        // Left out or `null`, the chronolexogram shows what the engine
        // received.
        assert_eq!(gram.fields["adres_aanvrager"], Value::Null);
    }
    assert_eq!(grams[0].fields, grams[1].fields);
}

#[test]
fn a_field_the_law_does_not_ask_is_refused() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.insert("bestemming".into(), json!("Lissabon"));
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not ask 'bestemming'")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn the_moment_that_counts_is_not_the_applicants_to_supply() {
    let mut cell = cell();
    let mut supplied = application();
    // The receipt is the moment that counts; the cell records it.
    supplied.insert("datum_ontvangst".into(), json!("2026-10-01"));
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("moment that counts")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn a_value_of_the_wrong_type_is_refused() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.insert("eerste_verlofdag".into(), json!("volgende week"));
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not fit a non-nullable Date")),
        "{e}"
    );
}

#[test]
fn null_is_refused_for_a_field_that_cannot_be_absent() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.insert("bsn_jongere".into(), Value::Null);
    let e = record(&mut cell, &supplied, "2026-10-13").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("'bsn_jongere' is null")),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn an_article_that_establishes_no_submission_is_refused() {
    let mut cell = cell();
    let e = cell
        .record_submission(
            &mut service(),
            &[],
            LAW,
            "2",
            None,
            &Map::new(),
            day("2026-10-13"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("establishes no submission")),
        "{e}"
    );
}

#[test]
fn a_gram_dated_before_the_last_one_is_refused() {
    let mut cell = cell();
    record(&mut cell, &application(), "2026-10-13").unwrap();
    // The same day is fine.
    record(&mut cell, &application(), "2026-10-13").unwrap();
    let e = record(&mut cell, &application(), "2026-10-12").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn a_chronicle_in_a_file_reads_back_what_was_recorded() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("aanvragen.jsonl");
    let recorded = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        assert!(cell.grams().is_empty());
        record(&mut cell, &application(), "2026-10-13").unwrap()
    };
    let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    assert_eq!(cell.grams(), [recorded]);
    // The clock of the file holds after reopening.
    let e = record(&mut cell, &application(), "2026-10-01").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );

    // Another recording actor does not open this chronicle.
    let e = Cell::open("obs_het_anker", &path, dir.path()).unwrap_err();
    assert!(matches!(e, Error::Setup(_)), "{e}");
}

fn uuid_version(id: &str) -> usize {
    uuid::Uuid::parse_str(id).unwrap().get_version_num()
}

#[test]
fn a_left_out_field_that_cannot_be_absent_is_named_as_missing() {
    // Awb 4:5: an incomplete application is still an application. The
    // chronolexogram records it, and says what it lacks.
    let mut cell = cell();
    let mut supplied = application();
    supplied.remove("eerste_verlofdag");
    let gram = record(&mut cell, &supplied, "2026-10-13").unwrap();
    assert_eq!(gram.missing, vec!["eerste_verlofdag"]);
    assert!(!gram.fields.contains_key("eerste_verlofdag"));

    let complete = record(&mut cell, &application(), "2026-10-13").unwrap();
    assert!(complete.missing.is_empty());
}

// ---------------------------------------------------------------------------
// The decision on the application (RFC-047)
// ---------------------------------------------------------------------------

/// The two facts of art. 11, onder f, both judged by the head.
const PROFESSION: &str = "specifieke_aard_beroep_ouder";
const OUTSIDE_HOLIDAYS: &str = "verlof_valt_buiten_schoolvakanties";

/// An application for leave from `first` to `last`, recorded on `on`.
fn applied(cell: &mut Cell, first: &str, last: &str, on: &str) -> Gram {
    let mut supplied = application();
    supplied.insert("eerste_verlofdag".into(), json!(first));
    supplied.insert("laatste_verlofdag".into(), json!(last));
    record(cell, &supplied, on).unwrap()
}

/// The head's judgements under art. 11, onder f: the profession of a parent,
/// and leave outside the school holidays.
fn judgement(profession: bool, outside_holidays: bool) -> Map<String, Value> {
    json!({ PROFESSION: profession, OUTSIDE_HOLIDAYS: outside_holidays })
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
    let (findings, supplied): (Map<String, Value>, Map<String, Value>) = given
        .clone()
        .into_iter()
        .partition(|(name, _)| [PROFESSION, OUTSIDE_HOLIDAYS, ONCE].contains(&name.as_str()));
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

#[test]
fn the_decision_asks_from_the_application_the_policy_the_head_and_the_cell() {
    let shape = decision::derive(&service(), LAW, ARTICLE, day("2026-10-20")).unwrap();
    assert_eq!(shape.kind, "BESCHIKKING");
    assert_eq!(shape.submission_kind, "AANVRAAG");
    let asked: Vec<(&str, &Source)> = shape
        .inputs
        .iter()
        .map(|i| (i.field.name.as_str(), &i.source))
        .collect();
    let policy = Source::Policy;
    assert_eq!(
        asked,
        vec![
            ("bsn_jongere", &Source::Application),
            ("eerste_verlofdag", &Source::Application),
            ("laatste_verlofdag", &Source::Application),
            ("besluit_datum", &Source::Cell),
            ("bekendmaking_datum", &Source::Cell),
            ("eerste_lesdag_schooljaar", &policy),
            (PROFESSION, &Source::Caller),
            // Findings of the head (art. 11 onder f, lid 2) that his policy
            // binds to an answer of his cell (RFC-048): where the cell gives
            // it, the binding fills the term and sets the finding aside;
            // where it is silent, or there is no cell, the head finds it.
            (OUTSIDE_HOLIDAYS, &Source::Caller),
            (ONCE, &Source::Caller),
        ]
    );
    let first = |name: &str| shape.input(name).unwrap().policy_first;
    assert!(first(ONCE) && first(OUTSIDE_HOLIDAYS) && !first(PROFESSION));
    assert_eq!(shape.judgements, [PROFESSION]);
}

#[test]
fn five_school_days_outside_the_first_two_weeks_on_the_ground_of_11f_are_granted() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();

    assert_eq!(gram.kind, "BESCHIKKING");
    assert_eq!(gram.basis, "leerplichtwet_1969#13a");
    assert_eq!(gram.refers_to.as_deref(), Some(app.id.as_str()));
    assert_eq!(gram.recorded_on, day("2026-10-20"));
    assert_eq!(uuid_version(&gram.id), 7);

    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    assert_eq!(gram.outputs["verlofdagen"], json!(5));
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
    // The chronolexogram does not repeat the law: the article is `basis`, and
    // what rests on which provision follows from the law on `recorded_on`.
    let json = serde_json::to_value(&gram).unwrap();
    assert!(json.get("field_basis").is_none() && json.get("output_basis").is_none());
    // Nor where each value came from: that follows from the law and
    // `refers_to`, so there is no `field_source`.
    assert!(json.get("field_source").is_none());

    // The values the decision used.
    assert_eq!(gram.fields["eerste_verlofdag"], json!("2027-02-01"));
    assert_eq!(gram.fields["eerste_lesdag_schooljaar"], json!("2026-08-31"));
    for judged in [PROFESSION, OUTSIDE_HOLIDAYS] {
        assert_eq!(gram.fields[judged], json!(true), "{judged}");
    }
    assert_eq!(gram.fields["besluit_datum"], json!("2026-10-20"));
    // What the application holds and the decision does not ask stays in
    // the application gram.
    assert!(!gram.fields.contains_key("naam_aanvrager"));
}

#[test]
fn the_awb_hooks_on_the_decision_land_in_its_gram_and_not_in_the_application() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();

    // Which Awb articles take part at which stage follows from the law.
    let shape = decision::derive(&service(), LAW, ARTICLE, day("2026-10-20")).unwrap();
    let taking_part = |stage: usize| -> Vec<String> {
        shape.stages[stage]
            .articles
            .iter()
            .map(|a| format!("{}#{}", a.law_id, a.article_number))
            .collect()
    };
    let awb = |article: &str| format!("{AWB}#{article}");
    for (output, value, article) in [
        ("motivering_vereist", json!(true), "3:46"),
        ("termijn_weken", json!(6), "6:7"),
        // Made known on the day of the decision (assumption of the cell).
        ("termijn_startdatum", json!("2026-10-21"), "6:8"),
        ("termijn_einddatum", json!("2026-12-01"), "6:8"),
    ] {
        assert_eq!(gram.outputs[output], value, "{output}");
        let stage = usize::from(article == "6:8");
        assert!(taking_part(stage).contains(&awb(article)), "{output}");
        assert!(!app.outputs.contains_key(output), "{output}");
    }
    // And the hooks on the application do not fire again.
    assert!(!gram
        .outputs
        .contains_key("redelijke_beslistermijn_einddatum"));
    assert!(!gram
        .outputs
        .contains_key("aanvraag_bevat_wat_artikel_4_2_eist"));
    assert_eq!(cell.grams(), [app, gram]);
}

#[test]
fn eleven_school_days_are_refused() {
    let mut cell = cell();
    // Monday 1 up to and including Monday 15 February.
    let app = applied(&mut cell, "2027-02-01", "2027-02-15", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlofdagen"], json!(11));
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    // A refusal is a decision too: it is motivated and open to objection.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(gram.outputs["termijn_weken"], json!(6));
}

#[test]
fn two_weeks_with_the_weekend_between_are_ten_school_days() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-12", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlofdagen"], json!(10));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn leave_in_the_first_two_weeks_of_the_school_year_is_refused() {
    let mut cell = cell();
    // The school year starts on Monday 31 August; the second week ends on
    // Sunday 13 September.
    let app = applied(&mut cell, "2026-09-10", "2026-09-11", "2026-08-20");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-25").unwrap();
    assert_eq!(gram.outputs["verlofdagen"], json!(2));
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(true)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));

    // The week after is fine.
    let app = applied(&mut cell, "2026-09-14", "2026-09-18", "2026-08-25");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-25").unwrap();
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn without_a_profession_of_a_specific_nature_leave_is_refused() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(false, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    assert_eq!(gram.fields[PROFESSION], json!(false));
}

#[test]
fn leave_within_the_school_holidays_is_refused() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, &judgement(true, false), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
}

#[test]
fn a_decision_needs_both_judgements_of_the_head() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    // The cell asks the head for the profession itself; whether the leave
    // falls outside the holidays the policy binds to an answer this cell
    // does not give, so the engine says the judgement is missing.
    for (judged, other) in [
        (PROFESSION, OUTSIDE_HOLIDAYS),
        (OUTSIDE_HOLIDAYS, PROFESSION),
    ] {
        let mut supplied = judgement(true, true);
        supplied.remove(judged);
        let e = decide(&mut cell, &app.id, &supplied, "2026-10-20").unwrap_err();
        let says = e.to_string();
        assert!(
            (says.contains(&format!("asks '{judged}'"))
                || says.contains(&format!("'{judged}' on {LAW}#{ARTICLE}"))
                    && says.contains("no judgement was given"))
                && !says.contains(&format!("'{other}'")),
            "{e}"
        );
    }
    let mut wrong = judgement(true, true);
    wrong.insert(PROFESSION.into(), json!("ja"));
    let e = decide(&mut cell, &app.id, &wrong, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not fit a Boolean")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn the_head_does_not_supply_what_comes_from_elsewhere() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    for (name, value, from) in [
        ("eerste_verlofdag", json!("2027-02-02"), "the application"),
        (
            "eerste_lesdag_schooljaar",
            json!("2026-09-07"),
            "the policy",
        ),
        ("besluit_datum", json!("2026-10-01"), "the cell"),
    ] {
        let mut supplied = judgement(true, true);
        supplied.insert(name.into(), value);
        let e = decide(&mut cell, &app.id, &supplied, "2026-10-20").unwrap_err();
        assert!(
            matches!(&e, Error::Refused(m) if m.contains(&format!("'{name}'")) && m.contains(from)),
            "{e}"
        );
    }
    let mut supplied = judgement(true, true);
    supplied.insert("bestemming".into(), json!("Lissabon"));
    let e = decide(&mut cell, &app.id, &supplied, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not ask 'bestemming'")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn a_decision_refers_to_an_application_of_this_cell_that_is_not_decided_yet() {
    let mut cell = cell();
    let e = decide(
        &mut cell,
        "019a0000-0000-7000-8000-000000000000",
        &judgement(true, true),
        "2026-10-20",
    )
    .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("no chronolexogram")),
        "{e}"
    );

    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let decided = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    let e = decide(&mut cell, &app.id, &judgement(false, true), "2026-10-21").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("already decided on")),
        "{e}"
    );
    // A decision is no application to decide on.
    let e = decide(&mut cell, &decided.id, &judgement(true, true), "2026-10-21").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("is a BESCHIKKING")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn a_decision_is_not_dated_before_its_application() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-12").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );
    // The same day is fine.
    decide(&mut cell, &app.id, &judgement(true, true), "2026-10-13").unwrap();
}

#[test]
fn a_decision_reads_back_from_the_file() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("kroniek.jsonl");
    let recorded = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
        let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
        vec![app, gram]
    };
    let cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    assert_eq!(cell.grams(), recorded);
}

#[test]
fn leave_that_ends_on_the_first_lesdag_touches_the_first_two_weeks() {
    let mut cell = cell();
    // Ending on Monday 31 August, the first lesdag: refused.
    let app = applied(&mut cell, "2026-08-24", "2026-08-31", "2026-08-03");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-03").unwrap();
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(true)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));

    // Ending the day before: it does not touch them.
    let app = applied(&mut cell, "2026-08-24", "2026-08-30", "2026-08-03");
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-08-03").unwrap();
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
}

#[test]
fn an_application_that_lacks_what_the_decision_needs_is_not_decided_on() {
    let mut cell = cell();
    let mut supplied = application();
    supplied.remove("laatste_verlofdag");
    let app = record(&mut cell, &supplied, "2026-10-13").unwrap();
    assert_eq!(app.missing, vec!["laatste_verlofdag"]);
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("lacks 'laatste_verlofdag'")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
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

#[test]
fn a_first_application_in_the_school_year_is_granted() {
    let mut cell = cell();
    let gram = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    // The reduction's answer, kept with the decision like an application
    // field.
    assert_eq!(gram.fields[ONCE], json!(false));
}

#[test]
fn a_second_application_in_the_same_school_year_is_refused() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "2026-10-05", "2026-10-09", "2026-09-21");
    // Five days in May: on their own they would be granted.
    let gram = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(gram.fields[ONCE], json!(true));
    assert_eq!(gram.outputs["verlofdagen"], json!(5));
    assert_eq!(
        gram.outputs["verlof_raakt_eerste_twee_lesweken"],
        json!(false)
    );
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
}

#[test]
fn an_application_in_the_next_school_year_is_granted() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    // Leave in October 2027 is in the school year 2027-2028, whose first
    // lesdag comes from next year's policy.
    let gram = apply_and_decide(&mut cell, "2027-10-04", "2027-10-08", "2027-09-20");
    assert_eq!(gram.fields["eerste_lesdag_schooljaar"], json!("2027-08-30"));
    assert_eq!(gram.fields[ONCE], json!(false));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn the_school_year_runs_from_august_to_july() {
    let mut cell = cell();
    // Granted leave at the end of the school year 2026-2027.
    apply_and_decide(&mut cell, "2027-07-26", "2027-07-30", "2027-06-01");
    // Leave starting in July is in the same school year.
    let july = apply_and_decide(&mut cell, "2027-07-19", "2027-07-23", "2027-06-02");
    assert_eq!(july.fields[ONCE], json!(true));
    assert_eq!(july.outputs["verlof_verleend"], json!(false));
    // Leave starting on 2 August is in the next one. Decided in that school
    // year, the first lesdag comes from next year's policy too.
    let august = apply_and_decide(&mut cell, "2027-08-02", "2027-08-06", "2027-08-02");
    assert_eq!(
        august.fields["eerste_lesdag_schooljaar"],
        json!("2027-08-30")
    );
    assert_eq!(august.fields[ONCE], json!(false));
    assert_eq!(august.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_refusal_does_not_count() {
    let mut cell = cell();
    // Eleven school days: refused. The reduction leaves it out of the
    // granted leave.
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-15", "2026-10-20");
    assert_eq!(refused.outputs["verlof_verleend"], json!(false));
    let gram = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-27");
    assert_eq!(gram.fields[ONCE], json!(false));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn leave_for_another_child_does_not_count() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let mut supplied = application();
    supplied.insert("bsn_jongere".into(), json!("999990020"));
    let app = record(&mut cell, &supplied, "2026-10-21").unwrap();
    let gram = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-21").unwrap();
    assert_eq!(gram.fields[ONCE], json!(false));
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
}

#[test]
fn the_school_year_ends_on_31_july_and_the_next_begins_on_1_august() {
    let mut cell = cell();
    // Granted leave whose first day is Saturday 31 July 2027.
    let last = apply_and_decide(&mut cell, "2027-07-31", "2027-08-06", "2027-06-01");
    assert_eq!(last.outputs["verlof_verleend"], json!(true));
    // So the school year 2026-2027 has its leave.
    let july = apply_and_decide(&mut cell, "2027-07-12", "2027-07-16", "2027-06-02");
    assert_eq!(july.fields[ONCE], json!(true));
    assert_eq!(july.outputs["verlof_verleend"], json!(false));
    // Leave whose first day is Sunday 1 August 2027 is in the next one.
    let first = apply_and_decide(&mut cell, "2027-08-01", "2027-08-06", "2027-06-03");
    assert_eq!(first.fields[ONCE], json!(false));
    assert_eq!(first.outputs["verlof_verleend"], json!(true));
}

#[test]
fn the_reduction_sets_a_finding_of_the_head_aside() {
    // The reduction fills the term (implements), so it wins over what the
    // head finds (RFC-047): leave was granted already this school year.
    let mut cell = cell();
    apply_and_decide(&mut cell, "2026-10-05", "2026-10-09", "2026-09-21");
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let mut given = judgement(true, true);
    given.insert(ONCE.into(), json!(false));
    let gram = decide(&mut cell, &app.id, &given, "2026-10-20").unwrap();
    assert_eq!(gram.fields[ONCE], json!(true));
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
    // The finding set aside is no finding the decision used.
    let found = cell.findings(&mut service(), &[], &gram.id).unwrap();
    assert!(!found.contains(&ONCE.to_string()), "{found:?}");
}

#[test]
fn the_decisions_the_reduction_reads_are_bound_for_the_decision_only() {
    let mut service = service();
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    cell.decide(
        &mut service,
        &[],
        LAW,
        ARTICLE,
        &app.id,
        &Map::new(),
        &judgement(true, true),
        day("2026-10-20"),
    )
    .unwrap();
    assert!(service.data_registry().list_sources().is_empty());
    // The reduction is rulework of the corpus, as before the decision.
    assert!(service.has_law(REDUCTION));
}

/// The cell gives its reduction the decisions itself, so a data source in
/// the service that answers the same input does not reach it.
#[test]
fn a_source_for_the_decisions_does_not_reach_the_cells_reduction() {
    let mut service = service();
    service.add_dict_source(
        "elders",
        10,
        [(
            "999990019".to_string(),
            [("besluiten".to_string(), regelrecht_engine::Value::Null)].into(),
        )]
        .into(),
    );
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let decided = cell
        .decide(
            &mut service,
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &judgement(true, true),
            day("2026-10-20"),
        )
        .unwrap();
    assert_eq!(decided[0].fields[ONCE], json!(false));
    assert_eq!(decided[0].outputs["verlof_verleend"], json!(true));
    assert_eq!(service.data_registry().list_sources(), ["elders"]);
}

/// A cell that gives no answers, as a school without a reduction.
fn without_reductions() -> Cell {
    let dir = answers_dir(&[]);
    Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap()
}

#[test]
fn without_the_answer_the_head_finds_it() {
    // A cell that gives no answer: the binding of the head's policy gets
    // none, so it fills nothing, and the head finds it per case
    // (decided_per_case_by).
    let mut cell = without_reductions();
    let app = applied(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    let mut supplied = judgement(true, true);
    supplied.insert(ONCE.into(), json!(true));
    let gram = decide(&mut cell, &app.id, &supplied, "2027-03-01").unwrap();
    assert_eq!(gram.fields[ONCE], json!(true));
    assert_eq!(gram.outputs["verlof_verleend"], json!(false));
}

#[test]
fn without_the_answer_or_the_head_it_is_not_known() {
    let mut cell = without_reductions();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap_err();
    assert!(
        e.to_string()
            .contains(&format!("'{ONCE}' on {LAW}#{ARTICLE}"))
            && e.to_string().contains("no judgement was given"),
        "{e}"
    );
}

#[test]
fn a_cell_directory_that_is_not_there_is_refused() {
    let e = Cell::in_memory(RECORDING_ACTOR, cell_dir().join("elders")).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("no directory of the cell")),
        "{e}"
    );
}

#[test]
fn a_cell_reads_the_reductions_it_keeps_from_its_directory() {
    let cell = Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap();
    assert_eq!(
        cell.reductions()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "reductie_plaats_van_vestiging",
            REDUCTION,
            "reductie_verlof_valt_buiten_schoolvakanties"
        ]
    );
}

#[test]
fn a_cell_keeps_each_reduction_as_a_directory_with_its_versions() {
    // `reductions/<$id>/<valid_from>.yaml`, and nothing else.
    for make in [
        |p: &Path| std::fs::write(p.join("los.yaml"), "").unwrap(),
        |p: &Path| std::fs::create_dir(p.join("reductie_zonder_versie")).unwrap(),
        |p: &Path| {
            std::fs::create_dir(p.join("reductie_x")).unwrap();
            std::fs::write(p.join("reductie_x/notities.txt"), "").unwrap();
        },
        |p: &Path| {
            std::fs::create_dir(p.join("reductie_x")).unwrap();
            std::fs::write(p.join("reductie_x/eerste.yaml"), "").unwrap();
        },
    ] {
        let dir = lid_2_dir();
        make(&dir.path().join("reductions"));
        let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
        assert!(
            matches!(&e, Error::Setup(m) if m.contains("a file per version")),
            "{e}"
        );
    }
}

#[test]
fn an_answer_from_a_reduction_the_cell_does_not_keep_is_refused() {
    let dir = cell_tempdir();
    std::fs::write(
        dir.path().join("cel.yaml"),
        format!("name: Het hoofd van OBS De Klimop\nanswers:\n  {ONCE}: {REDUCTION}\n"),
    )
    .unwrap();
    let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains(REDUCTION)
            && m.contains("only the answers of its own reductions")),
        "{e}"
    );
}

#[test]
fn the_reductions_of_no_cell_are_no_cell() {
    let shared = common::corpus().join("cells/reductions");
    let e = Cell::in_memory(RECORDING_ACTOR, shared).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("reductions of no cell")),
        "{e}"
    );
}

#[test]
fn a_cell_lists_the_answers_it_gives_and_the_terms_they_fill() {
    // The school's own cell: five answers from three reductions.
    let own = Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap();
    let listed: Vec<(&str, &str)> = own
        .answers()
        .iter()
        .map(|(answer, reduction)| (answer.as_str(), reduction.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "deel_van_gemeente_van_vestiging",
                "reductie_plaats_van_vestiging"
            ),
            ("gemeente_van_vestiging", "reductie_plaats_van_vestiging"),
            ("provincie_van_vestiging", "reductie_plaats_van_vestiging"),
            (ONCE, REDUCTION),
            (
                OUTSIDE_HOLIDAYS,
                "reductie_verlof_valt_buiten_schoolvakanties"
            ),
        ]
    );

    // The head's policy binds both terms of art. 13a; the cell for lid 2
    // gives the answer of one of them.
    let service = service();
    let terms: Vec<String> = own
        .bound_terms(&service, day("2026-10-20"))
        .into_iter()
        .filter(|t| t.law == LAW)
        .map(|t| t.open_term)
        .collect();
    assert_eq!(terms, [ONCE, OUTSIDE_HOLIDAYS]);
    let terms: Vec<String> = cell()
        .bound_terms(&service, day("2026-10-20"))
        .into_iter()
        .map(|t| t.open_term)
        .collect();
    assert_eq!(terms, [ONCE]);
    // Before the policy is in force, it binds nothing.
    assert!(cell().bound_terms(&service, day("2026-07-01")).is_empty());

    // A cell without answers gives none.
    assert!(without_reductions().answers().is_empty());
}

#[test]
fn a_catalogue_that_names_an_empty_answer_or_reduction_is_refused() {
    for answers in [[("''", REDUCTION)], [(ONCE, "''")]] {
        let dir = answers_dir(&answers);
        let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
        assert!(
            matches!(&e, Error::Setup(m) if m.contains("neither may be empty")),
            "{e}"
        );
    }
    let dir = lid_2_dir();
    common::write_identity(
        dir.path(),
        "name: Het hoofd van OBS De Klimop\nantwoorden: {}\n",
    );
    let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("antwoorden")),
        "{e}"
    );
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

#[test]
fn a_decision_from_before_the_reduction_is_the_heads_and_replays() {
    // The reduction comes into force on 2027-08-01: before that the cell
    // gives no answer, and the head finds the term.
    let dir = lid_2_dir();
    let path = dir.path().join("kroniek.jsonl");
    let mut cell = Cell::open(RECORDING_ACTOR, &path, dir.path()).unwrap();
    let app = applied(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    let mut supplied = judgement(true, true);
    supplied.insert(ONCE.into(), json!(false));
    let gram = cell
        .decide(
            &mut service_with_reduction_from("2027-08-01"),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &supplied,
            day("2027-03-01"),
        )
        .unwrap()
        .remove(0);
    assert_eq!(gram.fields[ONCE], json!(false));
    let mut later = service_with_reduction_from("2027-08-01");
    cell.explain(&mut later, &[], &gram.id).unwrap();
    let found = cell.findings(&mut later, &[], &gram.id).unwrap();
    assert!(found.contains(&ONCE.to_string()), "{found:?}");
}

#[test]
fn an_answer_whose_reduction_the_service_does_not_hold_is_refused() {
    // Never silently the head's: the cell gives this answer, and cannot.
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let mut supplied = judgement(true, true);
    supplied.insert(ONCE.into(), json!(false));
    let e = cell
        .decide(
            &mut service_without(&[REDUCTION]),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &supplied,
            day("2026-10-20"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("which is not loaded"), "{e}");
    assert_eq!(cell.grams().len(), 1);
}

// ---------------------------------------------------------------------------
// The objection and the decision on it (RFC-050)
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
        findings,
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
    json!({ PROFESSION: judged }).as_object().unwrap().clone()
}

#[test]
fn the_shape_of_the_objection_comes_from_the_awb() {
    let shape = shape::derive(&service(), AWB, OBJECTION, day("2026-11-01")).unwrap();
    assert_eq!(shape.kind, "BEZWAAR");
    assert_eq!(shape.stage.as_deref(), Some("BEZWAAR"));
    let fields: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    assert_eq!(
        fields,
        vec![
            ("bestreden_besluit", "algemene_wet_bestuursrecht#7:11"),
            (
                "bezwaar_of_beroepschrift_ondertekend",
                "algemene_wet_bestuursrecht#6:5"
            ),
            ("naam_indiener", "algemene_wet_bestuursrecht#6:5"),
            ("adres_indiener", "algemene_wet_bestuursrecht#6:5"),
            (
                "dagtekening_bezwaar_of_beroepschrift",
                "algemene_wet_bestuursrecht#6:5"
            ),
            (
                "omschrijving_bestreden_besluit",
                "algemene_wet_bestuursrecht#6:5"
            ),
            (
                "gronden_bezwaar_of_beroep",
                "algemene_wet_bestuursrecht#6:5"
            ),
            ("termijn_einddatum", "algemene_wet_bestuursrecht#6:9"),
        ]
    );
    // What 6:9 gives is no field, though 7:11 asks it.
    assert!(shape.given.iter().any(|g| g == TIMELY));
    // The objection asks for the decision of 7:11 (`requests`).
    assert_eq!(
        shape.requests.as_deref(),
        Some("algemene_wet_bestuursrecht#7:11")
    );
    let moment = shape.moment.unwrap();
    assert_eq!(
        moment.parameter.as_deref(),
        Some("datum_ontvangst_bezwaar_of_beroepschrift")
    );
    assert_eq!(moment.declared_by, "algemene_wet_bestuursrecht#6:9");
}

#[test]
fn the_decision_on_the_objection_asks_the_contested_decision() {
    let shape = decision::derive(&service(), AWB, OBJECTION, day("2026-11-15")).unwrap();
    assert_eq!(shape.kind, "BESCHIKKING");
    assert_eq!(shape.submission_kind, "BEZWAAR");
    assert_eq!(shape.outputs, [ADMISSIBLE, CAUSE, REVOKED, NEW]);
    let source = |name: &str| shape.input(name).map(|i| &i.source);
    assert_eq!(source("bestreden_besluit"), Some(&Source::Application));
    assert_eq!(source(TIMELY), Some(&Source::Application));
    // What the heroverweging asks of art. 13a is no input of 7:11: the head
    // gives it among the findings.
    assert_eq!(source(PROFESSION), None);
}

#[test]
fn an_objection_refers_to_the_decision_and_takes_its_period_from_it() {
    let mut cell = cell();
    // Twelve school days, refused on 20 October; the period ends 1 December.
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    assert_eq!(refused.outputs["verlofdagen"], json!(12));
    let gram = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();

    assert_eq!(gram.kind, "BEZWAAR");
    assert_eq!(gram.basis, "algemene_wet_bestuursrecht#7:11");
    assert_eq!(gram.refers_to.as_deref(), Some(refused.id.as_str()));
    // The contested decision, by its id.
    assert_eq!(gram.fields["bestreden_besluit"], json!(refused.id));
    // The period comes from the contested decision.
    assert_eq!(gram.fields["termijn_einddatum"], json!("2026-12-01"));
    assert_eq!(
        gram.fields["termijn_einddatum"],
        refused.outputs["termijn_einddatum"]
    );
    assert_eq!(gram.moment.as_ref().unwrap().on, day("2026-11-01"));
    assert_eq!(gram.outputs[TIMELY], json!(true));
    assert_eq!(
        gram.outputs["bezwaar_of_beroepschrift_bevat_wat_artikel_6_5_eist"],
        json!(true)
    );
    // Awb 7:10: six weeks from the day after the period ends.
    assert_eq!(
        gram.outputs["beslistermijn_bezwaar_einddatum"],
        json!("2027-01-12")
    );
    // The hooks on the decision do not fire on the objection.
    assert!(!gram.outputs.contains_key("motivering_vereist"));
}

#[test]
fn the_objector_does_not_supply_what_the_decision_holds_or_the_law_does_not_ask() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    for (name, value, says) in [
        (
            "termijn_einddatum",
            "2027-01-01",
            "comes from the decision it refers to",
        ),
        (
            "bestreden_besluit",
            "elders",
            "comes from the decision it refers to",
        ),
        ("bestemming", "Lissabon", "does not ask 'bestemming'"),
        (
            "datum_ontvangst_bezwaar_of_beroepschrift",
            "2026-10-25",
            "moment that counts",
        ),
        // An objection does not change the application (Awb 6:5, 7:11
        // lid 1), nor what a hook on it gave.
        ("laatste_verlofdag", "2027-02-12", "does not ask"),
        (
            "aanvraag_bevat_wat_artikel_4_2_eist",
            "true",
            "does not ask",
        ),
        ("bezwaar_of_beroepschrift_ondertekend", "ja", "does not fit"),
    ] {
        let e = object(
            &mut cell,
            &refused.id,
            &objection(&[(name, value)]),
            "2026-11-01",
        )
        .unwrap_err();
        assert!(matches!(&e, Error::Refused(m) if m.contains(says)), "{e}");
    }
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn an_objection_against_what_is_no_decision_is_refused() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = object(&mut cell, &app.id, &objection(&[]), "2026-11-01").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("not a decision")),
        "{e}"
    );
    // Nor is an objection a decision to object to.
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let e = object(&mut cell, &bezwaar.id, &objection(&[]), "2026-11-02").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("BEZWAAR under algemene_wet_bestuursrecht#7:11, not a decision")),
        "{e}"
    );
}

#[test]
fn an_objection_without_the_contested_decision_is_recorded_and_not_decided_on() {
    // An objection that refers to no decision lacks the contested decision
    // and the end of the period; it is recorded (Awb 4:5 does not stop
    // that), but the decision on it needs the contested decision.
    let mut cell = cell();
    let bezwaar = cell
        .record_submission(
            &mut service(),
            &[],
            AWB,
            OBJECTION,
            None,
            &objection(&[]),
            day("2026-11-01"),
        )
        .unwrap();
    assert_eq!(bezwaar.refers_to, None);
    assert!(bezwaar.missing.iter().any(|m| m == "bestreden_besluit"));
    let e = decide_objection(&mut cell, &bezwaar.id, "2026-11-15").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("lacks 'bestreden_besluit'")),
        "{e}"
    );
}

#[test]
fn a_second_application_upheld_on_objection_stands() {
    // Ongegrond: nothing changed, so 13a refuses again and the decision stands.
    let mut cell = cell();
    apply_and_decide(&mut cell, "2026-10-05", "2026-10-09", "2026-09-21");
    let second = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(second.outputs["verlof_verleend"], json!(false));
    let bezwaar = object(&mut cell, &second.id, &objection(&[]), "2027-03-10").unwrap();
    let decided = decide_objection(&mut cell, &bezwaar.id, "2027-04-01").unwrap();
    assert_eq!(decided.len(), 1);

    let decision = &decided[0];
    assert_eq!(decision.kind, "BESCHIKKING");
    assert_eq!(decision.basis, "algemene_wet_bestuursrecht#7:11");
    assert_eq!(decision.refers_to.as_deref(), Some(bezwaar.id.as_str()));
    // The contested decision by its id; whether the objection came in on
    // time comes from the objection (Awb 6:9).
    assert_eq!(decision.fields["bestreden_besluit"], json!(second.id));
    assert_eq!(decision.fields[TIMELY], json!(true));
    assert_eq!(decision.fields[TIMELY], bezwaar.outputs[TIMELY]);
    assert!(!decision.fields.contains_key(PROFESSION));
    // The heroverweging gives no cause.
    assert_eq!(decision.outputs[ADMISSIBLE], json!(true));
    assert_eq!(decision.outputs[CAUSE], json!(false));
    assert_eq!(decision.outputs[REVOKED], json!(false));
    assert_eq!(decision.outputs[NEW], Value::Null);
    // The decision on the objection is a beschikking: motivated (3:46), and
    // open to appeal within six weeks (6:7, 6:8).
    assert_eq!(decision.outputs["motivering_vereist"], json!(true));
    assert_eq!(decision.outputs["termijn_einddatum"], json!("2027-05-13"));
    assert_eq!(cell.grams().len(), 6);

    // Decided once.
    let e = decide_objection(&mut cell, &bezwaar.id, "2027-04-02").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("already decided on")),
        "{e}"
    );
}

#[test]
fn grounds_on_objection_lead_the_head_to_judge_anew_and_revoke_the_refusal() {
    // Gegrond: refused because the head judged the profession of a parent
    // not of the kind art. 11 onder f asks. In the objection the parent
    // gives grounds, a statement of the employer; at the heroverweging the
    // head judges it anew, and art. 13a grants.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    assert_eq!(decided.len(), 2);
    let (decision, new) = (&decided[0], &decided[1]);

    // What the head found anew is recorded with the decision on it.
    assert_eq!(decision.fields[PROFESSION], json!(true));
    assert_eq!(decision.outputs[CAUSE], json!(true));
    assert_eq!(decision.outputs[REVOKED], json!(true));
    // The decision on the objection holds the new decision by its id.
    assert_eq!(decision.outputs[NEW], json!(new.id));
    assert_eq!(new.kind, "BESCHIKKING");
    assert_eq!(new.basis, "leerplichtwet_1969#13a");
    assert_eq!(new.refers_to.as_deref(), Some(bezwaar.id.as_str()));
    assert_eq!(new.recorded_on, day("2026-11-15"));
    // The application's own fields.
    assert_eq!(new.fields["eerste_verlofdag"], json!("2027-02-01"));
    assert_eq!(new.fields["laatste_verlofdag"], json!("2027-02-05"));
    // The judgement given anew replaces that one; the other stands from
    // the contested decision.
    assert_eq!(new.fields[PROFESSION], json!(true));
    assert_eq!(new.fields[OUTSIDE_HOLIDAYS], json!(true));
    assert_eq!(new.outputs["verlofdagen"], json!(5));
    assert_eq!(new.outputs["verlof_verleend"], json!(true));
    // The new decision has its own period, from its own announcement (Awb
    // 6:7, 6:8), not that of the revoked one.
    assert_eq!(new.outputs["termijn_einddatum"], json!("2026-12-27"));
    assert_ne!(
        new.outputs["termijn_einddatum"],
        refused.outputs["termijn_einddatum"]
    );
    assert_eq!(cell.grams().len(), 5);

    // The new decision counts: a later application this school year is refused.
    let later = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(later.fields[ONCE], json!(true));
    assert_eq!(later.outputs["verlof_verleend"], json!(false));
}

#[test]
fn an_objection_against_a_decision_on_objection_is_not_admissible() {
    // Against a decision on objection beroep is open, not bezwaar (Awb 7:1
    // lid 1 onder a). Art. 7:11 says so in its admissibility: the cell
    // records the objection and the decision on it, without a heroverweging.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let upheld = decide_objection(&mut cell, &bezwaar.id, "2026-11-15")
        .unwrap()
        .remove(0);
    assert_eq!(upheld.outputs[REVOKED], json!(false));

    let again = object(&mut cell, &upheld.id, &objection(&[]), "2026-11-20").unwrap();
    assert_eq!(again.fields["bestreden_besluit"], json!(upheld.id));
    // Its period comes from the decision on the objection.
    assert_eq!(again.outputs[TIMELY], json!(true));
    let decided = decide_objection(&mut cell, &again.id, "2026-11-25").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
}

#[test]
fn an_objection_against_the_decision_taken_on_objection_is_not_admissible() {
    // The new decision rests on art. 13a, but it was taken on the objection,
    // in the place of the revoked refusal: against it too beroep is open,
    // not bezwaar (Awb 7:1 lid 1 onder a). Its decision value says so in
    // `taken_on`, which the cell derives from the submission it refers to.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    let new = &decided[1];
    assert_eq!(new.basis, "leerplichtwet_1969#13a");
    assert_eq!(new.outputs["verlof_verleend"], json!(true));

    let again = object(&mut cell, &new.id, &objection(&[]), "2026-11-20").unwrap();
    assert_eq!(again.outputs[TIMELY], json!(true));
    // The objection is not reconsidered, so a finding anew is asked by no
    // article and refused; without one it is decided on.
    let e = reconsider(&mut cell, &again.id, &profession(false), "2026-11-25").unwrap_err();
    assert!(e.to_string().contains(&format!("'{PROFESSION}'")), "{e}");
    let decided = decide_objection(&mut cell, &again.id, "2026-11-25").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn a_decision_already_replaced_on_objection_is_not_replaced_again() {
    // Two objections against one grant, both on time. On the first, the
    // head judges the profession anew and a refusal takes the grant's place.
    // The second, decided after it, would revoke the grant again and take a
    // second refusal in its place: the cell refuses that decision, and
    // appends nothing.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(granted.outputs["verlof_verleend"], json!(true));
    let first = object(&mut cell, &granted.id, &objection(&[]), "2026-11-01").unwrap();
    let second = object(&mut cell, &granted.id, &objection(&[]), "2026-11-02").unwrap();
    let decided = reconsider(&mut cell, &first.id, &profession(false), "2026-11-15").unwrap();
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    let replacement = &decided[1].id;
    let recorded = cell.grams().len();

    let e = reconsider(&mut cell, &second.id, &profession(false), "2026-11-16").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("already replaces") && m.contains(replacement.as_str()) && m.contains(&granted.id)),
        "{e}"
    );
    assert_eq!(cell.grams().len(), recorded);
    // Without cause, the second is decided on: the grant is not revoked again.
    let decided = decide_objection(&mut cell, &second.id, "2026-11-16").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
}

#[test]
fn a_finding_anew_on_an_objection_not_admissible_is_refused() {
    // The head gives a finding anew on a late objection. There is no
    // heroverweging (Awb 7:11 lid 1), so no article asks it: an error that
    // names it, and nothing is recorded.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let late = object(&mut cell, &refused.id, &objection(&[]), "2026-12-02").unwrap();
    let recorded = cell.grams().len();
    let e = reconsider(&mut cell, &late.id, &profession(true), "2026-12-10").unwrap_err();
    assert!(e.to_string().contains(&format!("'{PROFESSION}'")), "{e}");
    assert_eq!(cell.grams().len(), recorded);
    // Without it, the objection is decided on and not admissible.
    let decided = decide_objection(&mut cell, &late.id, "2026-12-10").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn a_refusal_on_another_ground_on_objection_stands() {
    // The head now judges the profession of the kind 11 onder f asks, but
    // the leave within the school holidays: still refused, on another
    // ground. No outcome of art. 13a differs, so the heroverweging gives no
    // cause and the refusal stands.
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(
        &mut cell,
        &bezwaar.id,
        &judgement(true, false),
        "2026-11-15",
    )
    .unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn a_smaller_request_after_a_refusal_is_a_new_application() {
    // Twelve school days are refused. Ten is a new application, not an
    // objection (Awb 6:5, 7:11 lid 1); the refusal does not count as leave
    // granted, so it is granted.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    assert_eq!(refused.outputs["verlof_verleend"], json!(false));
    let smaller = apply_and_decide(&mut cell, "2027-02-01", "2027-02-12", "2026-10-27");
    assert_eq!(smaller.outputs["verlofdagen"], json!(10));
    assert_eq!(smaller.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_revoked_grant_does_not_count_as_leave_granted() {
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(granted.outputs["verlof_verleend"], json!(true));
    // Constructed: at the heroverweging the head judges the profession
    // anew, not of the kind art. 11 onder f asks, so 13a refuses anew, the
    // grant is revoked and a refusal takes its place.
    let bezwaar = object(&mut cell, &granted.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(false), "2026-11-15").unwrap();
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    let new = &decided[1];
    assert_eq!(new.outputs["verlof_verleend"], json!(false));

    // The revoked grant stays in the chronicle, and its row is as it was.
    // The row of the decision on the objection names it by id and says it
    // revoked it; the reduction leaves it out (RFC-050).
    let rows = |basis: &str| {
        serde_json::to_value(
            regelrecht_cel::reading::rows(&service(), cell.grams(), basis, "BESCHIKKING").unwrap(),
        )
        .unwrap()
    };
    let granted_rows: Vec<Value> = rows("leerplichtwet_1969#13a")
        .as_array()
        .unwrap()
        .iter()
        .map(|row| json!([row["id"], row["verlof_verleend"]]))
        .collect();
    assert_eq!(
        granted_rows,
        [json!([granted.id, true]), json!([new.id, false])]
    );
    let on_objection = rows("algemene_wet_bestuursrecht#7:11");
    assert_eq!(on_objection[0]["bestreden_besluit"], json!(granted.id));
    assert_eq!(on_objection[0][REVOKED], json!(true));
    assert_eq!(on_objection[0][NEW], json!(new.id));

    // Neither the revoked grant nor the refusal in its place counts.
    let later = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2027-03-01");
    assert_eq!(later.fields[ONCE], json!(false));
    assert_eq!(later.outputs["verlof_verleend"], json!(true));
}

#[test]
fn a_late_objection_is_recorded_and_not_admissible() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    // On the last day of the period it is on time.
    let on_time = object(&mut cell, &refused.id, &objection(&[]), "2026-12-01").unwrap();
    assert_eq!(on_time.outputs[TIMELY], json!(true));
    // The day after it is late, and still recorded.
    let late = object(&mut cell, &refused.id, &objection(&[]), "2026-12-02").unwrap();
    assert_eq!(late.outputs[TIMELY], json!(false));

    // A late objection is not admissible (Awb 7:11 lid 1): it is not
    // reconsidered, so there is no judgement on cause, and the decision
    // stands.
    let decided = decide_objection(&mut cell, &late.id, "2026-12-10").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);
}

#[test]
fn an_objection_is_decided_with_decide_and_reads_back_from_the_file() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("kroniek.jsonl");
    let recorded = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        let refused = refused_on_profession(&mut cell);
        let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-15").unwrap();
        reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
        cell.grams().to_vec()
    };
    assert_eq!(recorded.len(), 5);
    let cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    assert_eq!(cell.grams(), recorded);
}

#[test]
fn a_chronicle_appends_a_batch_whole_or_not_at_all() {
    let mut cell = cell();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let mut chronicle = regelrecht_cel::Chronicle::in_memory();
    let mut later = app.clone();
    later.id = "later".into();
    let mut earlier = app.clone();
    earlier.id = "earlier".into();
    earlier.recorded_on = day("2026-10-01");
    let e = chronicle.append_all(vec![later, earlier]).unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not go back")),
        "{e}"
    );
    assert!(chronicle.grams().is_empty());
}

#[test]
fn the_heroverweging_does_not_count_the_contested_grant() {
    // Read with the contested grant, art. 13a would refuse once per school
    // year and revoke the grant. Without it, it grants anew, and the grant
    // stands.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let bezwaar = object(&mut cell, &granted.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = decide_objection(&mut cell, &bezwaar.id, "2026-11-15").unwrap();
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
}

/// [`service`], where a version of the school's policy from 5 December
/// 2026 no longer gives the first lesdag: art. 13a has no answer for it from
/// then on. A decision taken before still replays under the version of its
/// own day.
fn service_with_policy_lapsed() -> LawExecutionService {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../corpus/regulation/nl/uitvoeringsbeleid/{POLICY}/2026-08-01.yaml"
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

#[test]
fn a_late_objection_is_not_admissible_where_the_heroverweging_would_fail() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, "2027-02-01", "2027-02-16", "2026-10-20");
    let late = object(&mut cell, &refused.id, &objection(&[]), "2026-12-02").unwrap();
    assert_eq!(late.outputs[TIMELY], json!(false));
    // Without the school's policy on the day of the decision, art. 13a anew
    // cannot be applied; the objection is not reconsidered, so that does not
    // stand in the way (Awb 7:11 lid 1).
    let mut without_policy = service_with_policy_lapsed();
    let decided = cell
        .decide(
            &mut without_policy,
            &[],
            AWB,
            OBJECTION,
            &late.id,
            &Map::new(),
            &Map::new(),
            day("2026-12-10"),
        )
        .unwrap();
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(false));
    assert_eq!(decided[0].outputs[CAUSE], Value::Null);
    assert_eq!(decided.len(), 1);
    // An objection on time is reconsidered, and there it fails.
    let mut timely = self::cell();
    let refused = apply_and_decide(&mut timely, "2027-02-01", "2027-02-16", "2026-10-20");
    let on_time = object(&mut timely, &refused.id, &objection(&[]), "2026-12-01").unwrap();
    let e = timely
        .decide(
            &mut service_with_policy_lapsed(),
            &[],
            AWB,
            OBJECTION,
            &on_time.id,
            &Map::new(),
            &Map::new(),
            day("2026-12-10"),
        )
        .unwrap_err();
    // It fails in the call to art. 13a, on the first lesdag the policy no
    // longer gives.
    assert!(e.to_string().contains("eerste_lesdag_schooljaar"), "{e}");
}

/// Lazy evaluation keeps the heroverweging from running for an objection
/// that is not admissible (Awb 7:11 lid 1): the trace of the decision on a
/// late objection does not reach art. 13a, that on a timely one does.
#[test]
fn the_heroverweging_runs_only_for_an_admissible_objection() {
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    // The head's findings per case under art. 13a are no fields.
    let (findings, fields): (Map<String, Value>, Map<String, Value>) = refused
        .fields
        .clone()
        .into_iter()
        .partition(|(name, _)| [PROFESSION, OUTSIDE_HOLIDAYS, ONCE].contains(&name.as_str()));
    let contested = json!({
        "basis": refused.basis,
        "taken_on": "leerplichtwet_1969#13a",
        "fields": fields,
        "findings": findings,
        "outputs": {
            "verlofdagen": refused.outputs["verlofdagen"],
            "verlof_raakt_eerste_twee_lesweken": refused.outputs["verlof_raakt_eerste_twee_lesweken"],
            "verlof_verleend": refused.outputs["verlof_verleend"],
        },
    });
    let steps = |timely: bool| {
        let parameters = [
            ("bestreden_besluit", contested.clone()),
            (TIMELY, json!(timely)),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_string(), regelrecht_engine::Value::from(value)))
        .collect();
        let result = service()
            .evaluate_law_output_with_trace(AWB, REVOKED, parameters, "2026-11-15")
            .unwrap();
        assert_eq!(
            result.outputs[REVOKED],
            regelrecht_engine::Value::from(json!(false))
        );
        // The first line echoes the parameters, the basis among them.
        let trace = result.trace.as_ref().unwrap().render_box_drawing();
        trace.lines().skip(1).collect::<Vec<_>>().join("\n")
    };
    let late = steps(false);
    assert!(
        !late.contains("leerplichtwet_1969#13a"),
        "the heroverweging must not run:\n{late}"
    );
    let timely = steps(true);
    assert!(timely.contains("leerplichtwet_1969#13a"), "{timely}");
}

#[test]
fn the_head_gives_at_the_decision_on_objection_only_findings_an_article_asks() {
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    for (given, says) in [
        // The contested decision comes from the objection: it is no finding.
        (
            json!({"bestreden_besluit": refused.id}),
            "'bestreden_besluit' of the decision of algemene_wet_bestuursrecht#7:11 is no finding",
        ),
        // What no article in the decision asks, misspelt or not, is an
        // error that names it: it is not quietly dropped.
        (
            json!({"specifieke_aard_beroep": true}),
            "'specifieke_aard_beroep'",
        ),
        (json!({ CAUSE: true }), "'heroverweging_geeft_aanleiding'"),
        // A finding of another type than its term.
        (
            json!({ PROFESSION: "ja" }),
            "a boolean for the finding 'specifieke_aard_beroep_ouder'",
        ),
    ] {
        let given = given.as_object().unwrap().clone();
        let e = reconsider(&mut cell, &bezwaar.id, &given, "2026-11-15").unwrap_err();
        assert!(e.to_string().contains(says), "{says}: {e}");
    }
    assert_eq!(cell.grams().len(), 3);
}

#[test]
fn a_reduction_reads_the_decisions_on_objection_with_their_own_outcome() {
    let mut cell = cell();
    let refused = refused_on_profession(&mut cell);
    let bezwaar = object(&mut cell, &refused.id, &objection(&[]), "2026-11-01").unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &profession(true), "2026-11-15").unwrap();
    let rows = serde_json::to_value(
        regelrecht_cel::reading::rows(
            &service(),
            cell.grams(),
            "algemene_wet_bestuursrecht#7:11",
            "BESCHIKKING",
        )
        .unwrap(),
    )
    .unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    // Its own outcome: it revoked the refusal and took a grant in its place.
    assert_eq!(rows[0]["id"], json!(decided[0].id));
    assert_eq!(rows[0]["bestreden_besluit"], json!(refused.id));
    assert_eq!(rows[0][REVOKED], json!(true));
    assert_eq!(rows[0][ADMISSIBLE], json!(true));
    assert_eq!(rows[0][NEW], json!(decided[1].id));
}

/// The nodes of `node` and below where the cell answered an open term.
fn answered(node: &regelrecht_engine::PathNode) -> Vec<&regelrecht_engine::PathNode> {
    let mut found: Vec<_> = node.children.iter().flat_map(answered).collect();
    if node.answer.is_some() {
        found.insert(0, node);
    }
    found
}

/// The owner's case (slice 4c): leave A is granted, leave B in the same
/// school year is refused because A counts, an objection against A revokes
/// A, and on the objection against B art. 13a called anew grants B. The
/// cell answers the term there; the engine records who answered. Why the
/// cell answered so is its own account: its reduction left A out, because
/// a decision on objection revoked it.
#[test]
fn a_grant_revoked_on_objection_no_longer_counts_on_the_objection_against_the_refusal() {
    let mut cell = cell();
    let a = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    assert_eq!(a.outputs["verlof_verleend"], json!(true));
    let b = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2026-10-27");
    assert_eq!(b.fields[ONCE], json!(true));
    assert_eq!(b.outputs["verlof_verleend"], json!(false));

    // At the heroverweging of A the head judges the profession anew: A is
    // revoked and a refusal takes its place.
    let against_a = object(&mut cell, &a.id, &objection(&[]), "2026-11-01").unwrap();
    let on_a = reconsider(&mut cell, &against_a.id, &profession(false), "2026-11-15").unwrap();
    assert_eq!(on_a[0].outputs[REVOKED], json!(true));
    assert_eq!(on_a[1].outputs["verlof_verleend"], json!(false));

    // On the objection against B, art. 13a anew no longer counts A.
    let against_b = object(&mut cell, &b.id, &objection(&[]), "2026-11-20").unwrap();
    let on_b = decide_objection(&mut cell, &against_b.id, "2026-12-01").unwrap();
    assert_eq!(on_b[0].outputs[CAUSE], json!(true));
    assert_eq!(on_b[0].outputs[REVOKED], json!(true));
    let new_b = &on_b[1];
    assert_eq!(new_b.outputs["verlof_verleend"], json!(true));
    // The cell filled the term, so the new decision has no finding on it;
    // its value is the cell's answer.
    assert_eq!(new_b.fields[ONCE], json!(false));
    let found = cell.findings(&mut service(), &[], &new_b.id).unwrap();
    assert!(!found.contains(&ONCE.to_string()), "{found:?}");

    // Step 1: the trace of the decision on B's objection holds one node for
    // the term, within art. 13a called anew: the cell's answer, with the
    // reduction, its version and how far it read the chronicle. It read
    // everything before this decision but B, the contested decision.
    let result = cell.explain(&mut service(), &[], &on_b[0].id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let nodes = answered(root);
    assert_eq!(nodes.len(), 1, "{}", root.render_box_drawing());
    let node = nodes[0];
    assert_eq!(node.name, ONCE);
    assert_eq!(
        node.result,
        Some(regelrecht_engine::Value::from(json!(false)))
    );
    assert!(node.children.is_empty(), "{node:?}");
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, RECORDING_ACTOR);
    assert_eq!(answer.law_id, REDUCTION);
    assert_eq!(answer.valid_from, "2026-08-01");
    let before: Vec<&str> = cell
        .grams()
        .iter()
        .take_while(|g| g.id != on_b[0].id)
        .filter(|g| g.id != b.id)
        .map(|g| g.id.as_str())
        .collect();
    assert_eq!(answer.records_read, before.len());
    assert_eq!(answer.last_record.as_deref(), Some(against_b.id.as_str()));
    // The reduction's FOREACH is not in the decision's trace.
    let trace = root.render_box_drawing();
    assert!(!trace.contains("FOREACH"), "{trace}");
    assert_eq!(
        result.outputs[REVOKED],
        regelrecht_engine::Value::from(json!(true))
    );
    // The report says the same: the cell answered the term.
    assert!(
        result.open_terms.iter().any(|f| f.term == ONCE
            && matches!(&f.filled_by, regelrecht_engine::FilledBy::Answer { law_id, article, answers }
                if law_id == LEAVE_POLICY && article == "2" && answers == std::slice::from_ref(answer))),
        "{:?}",
        result.open_terms
    );

    // Step 2: asked why, the cell runs its reduction for that term again,
    // with its own trace: the Awb reduction counts A among the revoked
    // decisions and leaves only the refusal in its place standing, which
    // the filter skips; B itself is not read.
    let explained = cell
        .explain_answer(&mut service(), &[], &on_b[0].id, ONCE)
        .unwrap();
    assert_eq!(explained.len(), 1);
    assert_eq!(
        explained[0].outputs[ONCE],
        regelrecht_engine::Value::from(json!(false))
    );
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(
        why.contains(&format!("herroepen_besluiten = ['{}']", a.id)),
        "A is not among the revoked:\n{why}"
    );
    let standing = why
        .lines()
        .find(|line| line.contains("Result: besluiten_in_stand ="))
        .unwrap_or_else(|| panic!("no decisions standing:\n{why}"));
    assert!(!standing.contains(&a.id), "A still stands:\n{why}");
    assert!(
        why.contains("FOREACH over 0 element(s), 1 skipped by filter = False"),
        "{why}"
    );
    // A term the decision did not ask the cell is refused.
    let e = cell
        .explain_answer(&mut service(), &[], &on_b[0].id, "verlofdagen")
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("did not ask")),
        "{e}"
    );

    // Explaining the new decision explains the decision that took it.
    let again = cell.explain(&mut service(), &[], &new_b.id).unwrap();
    assert_eq!(again.outputs, result.outputs);
    // Nothing stays behind in the service.
    let mut kept = service();
    let laws = kept.law_count();
    cell.explain(&mut kept, &[], &b.id).unwrap();
    cell.explain_answer(&mut kept, &[], &b.id, ONCE).unwrap();
    assert_eq!(kept.law_count(), laws);
    assert!(kept.data_registry().list_sources().is_empty());
}

/// The decision on B itself, explained: the cell answered that leave was
/// already granted, and its own account shows the grant A it counted.
#[test]
fn the_cell_explains_the_grant_its_reduction_counted_for_a_refusal() {
    let mut cell = cell();
    let a = apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
    let b = apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2026-10-27");
    let result = cell.explain(&mut service(), &[], &b.id).unwrap();
    let trace = result.trace.as_ref().unwrap().render_box_drawing();
    assert!(trace.contains(REDUCTION), "{trace}");
    assert!(!trace.contains(&a.id), "{trace}");
    assert_eq!(
        result.outputs["verlof_verleend"],
        regelrecht_engine::Value::from(json!(false))
    );
    let explained = cell
        .explain_answer(&mut service(), &[], &b.id, ONCE)
        .unwrap();
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&a.id), "{why}");
    assert_eq!(
        explained[0].outputs[ONCE],
        regelrecht_engine::Value::from(json!(true))
    );
}

/// A replay runs as the decision ran, or fails: read back with an outcome
/// the chronicle did not get from the law, the replay is not that decision.
#[test]
fn a_replay_that_does_not_run_as_the_decision_ran_fails() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join("kroniek.jsonl");
    let b = {
        let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
        apply_and_decide(&mut cell, "2027-02-01", "2027-02-05", "2026-10-20");
        apply_and_decide(&mut cell, "2027-05-10", "2027-05-14", "2026-10-27")
    };
    assert_eq!(b.outputs["verlof_verleend"], json!(false));
    let with = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    let found = with.findings(&mut service(), &[], &b.id).unwrap();
    assert!(!found.contains(&ONCE.to_string()), "{found:?}");

    // The last line is B; it now says leave was granted.
    let text = std::fs::read_to_string(&path).unwrap();
    let (rest, last) = text.trim_end().rsplit_once('\n').unwrap();
    assert!(last.contains(&b.id));
    let tampered = last.replace("\"verlof_verleend\":false", "\"verlof_verleend\":true");
    assert_ne!(tampered, last);
    std::fs::write(&path, format!("{rest}\n{tampered}\n")).unwrap();
    let cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    let e = cell.explain(&mut service(), &[], &b.id).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains("the replay is not this decision")
            && m.contains("verlof_verleend")),
        "{e}"
    );
    // Nor does the cell derive findings from it.
    assert!(cell.findings(&mut service(), &[], &b.id).is_err());
}

#[test]
fn a_decision_in_a_sandbox_stays_out_of_the_cell_and_its_file() {
    let dir = cell_tempdir();
    let school = lid_2_dir();
    let path = dir.path().join(RECORDING_ACTOR).join("kroniek.jsonl");
    let mut cell = Cell::open(RECORDING_ACTOR, &path, school.path()).unwrap();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let before = std::fs::read_to_string(&path).unwrap();

    let mut sandbox = cell.sandbox();
    assert_eq!(sandbox.answers(), cell.answers());
    let gram = decide(&mut sandbox, &app.id, &judgement(true, true), "2026-10-20").unwrap();
    assert_eq!(gram.outputs["verlof_verleend"], json!(true));
    assert_eq!(sandbox.grams().len(), 2);

    assert_eq!(cell.grams(), std::slice::from_ref(&app));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    // The cell can still take that decision itself.
    decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap();
}

/// The head's policy asks the cell of the head (RFC-048): a cell of another
/// school with the same reduction is not asked, and the head finds the term.
#[test]
fn the_heads_policy_does_not_ask_the_cell_of_another_school() {
    let dir = lid_2_dir();
    common::write_identity(
        dir.path(),
        &format!("name: Het hoofd van OBS Het Anker\nanswers:\n  {ONCE}: {REDUCTION}\n"),
    );
    let mut cell = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, &judgement(true, true), "2026-10-20").unwrap_err();
    assert!(e.to_string().contains("no judgement was given"), "{e}");
}

/// The reduction of the Awb is of no cell: a reduction calls it, and no
/// cell gives its output as an answer, not even the cell of a body that
/// applies the Awb. A catalogue that names it is refused when the cell
/// opens.
#[test]
fn no_cell_gives_the_reduction_of_the_awb_as_its_answer() {
    const AWB_REDUCTION: &str = "reductie_algemene_wet_bestuursrecht";
    let dir = answers_dir(&[("besluiten_in_stand", AWB_REDUCTION)]);
    let e = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap_err();
    assert!(
        matches!(&e, Error::Setup(m) if m.contains(AWB_REDUCTION)
            && m.contains("only the answers of its own reductions")),
        "{e}"
    );
}
