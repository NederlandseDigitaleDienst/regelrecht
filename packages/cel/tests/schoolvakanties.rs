//! Whether leave falls outside the school holidays (Leerplichtwet 1969 art.
//! 11, onder f, read in art. 13a) follows from what the school recorded and
//! the law, not from a judgement of the head (RFC-051). The Regeling
//! vaststelling schoolvakanties 2025–2030 fixes the kerst-, mei- and
//! zomervakantie, the last per region; the region follows from where the
//! school is established, an executogram in its own chronicle; the herfst-
//! and voorjaarsvakantie are a rule of the school, in its own policy, one
//! version per school year. The head's policy binds the open term to the
//! answer of the cell of OBS De Klimop, which gives it with its reduction
//! (RFC-048).

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::{Cell, Error, Gram};
use regelrecht_engine::{LawExecutionService, PathNode};
use serde_json::{json, Map, Value};

const LAW: &str = "leerplichtwet_1969";
const ARTICLE: &str = "13a";
const RECORDING_ACTOR: &str = "obs_de_klimop";
const REGELING: &str = "regeling_vaststelling_schoolvakanties_20252030";
const ROOSTER: &str = "fictief_beleid_vakantierooster_obs_de_klimop";
const PROFESSION: &str = "specifieke_aard_beroep_ouder";
const OUTSIDE_HOLIDAYS: &str = "verlof_valt_buiten_schoolvakanties";
/// The head's judgement on it where the cell is silent: an open term of his
/// policy (art. 4, RFC-048 §2).
const OUTSIDE_HOLIDAYS_HEAD: &str = "verlof_valt_buiten_schoolvakanties_volgens_hoofd";
const HOLIDAYS: &str = "reductie_verlof_valt_buiten_schoolvakanties";

/// The Leerplichtwet, the Awb, the regeling and the school's policies from
/// the corpus.
fn service() -> LawExecutionService {
    service_with(&[])
}

/// [`service`], with the versions `extra` loaded as well.
fn service_with(extra: &[String]) -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let laws = [
        format!("regulation/nl/wet/{LAW}"),
        "regulation/nl/wet/algemene_wet_bestuursrecht".to_string(),
        format!("regulation/nl/ministeriele_regeling/{REGELING}"),
        "regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaar_obs_de_klimop".to_string(),
        "regulation/nl/uitvoeringsbeleid/fictief_beleid_schooljaargrenzen_obs_de_klimop"
            .to_string(),
        format!("regulation/nl/uitvoeringsbeleid/{ROOSTER}"),
        "regulation/nl/uitvoeringsbeleid/fictief_beleid_vestiging_obs_de_klimop".to_string(),
        // The head's policy, which binds the terms to the answers of the
        // school's cell, and the reductions of that cell (RFC-048).
        "regulation/nl/uitvoeringsbeleid/fictief_beleid_vakantieverlof_obs_de_klimop".to_string(),
        common::reduction_dir("reductie_plaats_van_vestiging"),
        common::reduction_dir("reductie_verlof_al_verleend_in_schooljaar"),
        common::reduction_dir(HOLIDAYS),
        // Which decisions still stand after objection and appeal: the
        // reduction of the Awb, which the school's reduction calls.
        common::reduction_dir("reductie_algemene_wet_bestuursrecht"),
    ];
    common::load(&mut service, laws);
    for yaml in extra {
        service.load_law(yaml).unwrap();
    }
    service
}

/// A fictional next version of the school's policy, for the school year
/// 2027–2028, from 1 August 2027.
fn rooster_2027() -> String {
    let current = std::fs::read_to_string(common::corpus().join(format!(
        "regulation/nl/uitvoeringsbeleid/{ROOSTER}/2026-08-01.yaml"
    )))
    .unwrap();
    let next = [
        (
            "publication_date: '2026-07-01'",
            "publication_date: '2027-07-01'",
        ),
        ("valid_from: '2026-08-01'", "valid_from: '2027-08-01'"),
        ("value: 2026\n", "value: 2027\n"),
        ("'2026-10-17'", "'2027-10-16'"),
        ("'2026-10-25'", "'2027-10-24'"),
        ("'2027-02-20'", "'2028-02-19'"),
        ("'2027-02-28'", "'2028-02-27'"),
    ]
    .iter()
    .fold(current.clone(), |yaml, (from, to)| {
        assert!(yaml.contains(from), "{from}");
        yaml.replace(from, to)
    });
    assert_ne!(next, current);
    next
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn map(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

/// The school's own cell, with all its reductions, and an empty chronicle.
fn school() -> Cell {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{RECORDING_ACTOR}"));
    Cell::in_memory(RECORDING_ACTOR, dir).unwrap()
}

/// Record that the school is established in `gemeente` of `provincie` since
/// `sinds`, on `on` (an executogram).
fn established(cell: &mut Cell, gemeente: &str, provincie: &str, sinds: &str, on: &str) -> Gram {
    let fact = map(json!({ "gemeente": gemeente, "provincie": provincie, "sinds": sinds }));
    cell.record_executogram(
        &service(),
        "fictief_beleid_vestiging_obs_de_klimop#1#vestiging",
        &fact,
        day(on),
    )
    .unwrap()
}

/// The school, established in `gemeente` since 2020, recorded on 1 August
/// 2026.
fn school_in(gemeente: &str, provincie: &str) -> Cell {
    let mut cell = school();
    established(&mut cell, gemeente, provincie, "2020-08-01", "2026-08-01");
    cell
}

/// An application for leave from `first` to `last`, recorded on `on`, with
/// fictional data.
fn applied(cell: &mut Cell, first: &str, last: &str, on: &str) -> Gram {
    let supplied = map(json!({
        "bsn_jongere": "999990019",
        "eerste_verlofdag": first,
        "laatste_verlofdag": last,
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "J. Jansen",
        "adres_aanvrager": "Voorbeeldstraat 1",
        "dagtekening": on,
        "gevraagde_beschikking": "vakantieverlof",
    }));
    cell.record_submission(&mut service(), &[], LAW, ARTICLE, None, &supplied, day(on))
        .unwrap()
}

/// The head decides on `application` on `on`, finding only the profession
/// of a parent: whether the leave falls outside the school holidays, and
/// whether it already had leave this school year, the cell answers.
fn decide(cell: &mut Cell, application: &str, on: &str) -> Result<Gram, Error> {
    let findings = map(json!({ PROFESSION: true }));
    cell.decide(
        &mut service(),
        &[],
        LAW,
        ARTICLE,
        application,
        &Map::new(),
        &findings,
        day(on),
    )
    .map(|mut grams| grams.remove(0))
}

/// The decision asks the head's finding on whether the leave falls outside
/// the school holidays: the cell was silent, and the head's policy leaves
/// the case to him.
fn asks_the_head(e: &Error) {
    let text = e.to_string();
    assert!(
        matches!(e, Error::Engine(_))
            && text.contains(&format!("'{OUTSIDE_HOLIDAYS_HEAD}'"))
            && text.contains("no judgement was given for this case"),
        "{e}"
    );
}

fn granted(gram: &Gram) -> bool {
    gram.outputs["verlof_verleend"] == json!(true)
}

/// The nodes of a trace where an answerer filled an open term.
fn answered(node: &PathNode) -> Vec<&PathNode> {
    let mut found: Vec<&PathNode> = node.children.iter().flat_map(answered).collect();
    if node.answer.is_some() {
        found.insert(0, node);
    }
    found
}

#[test]
fn leave_in_the_christmas_holiday_is_refused_and_nobody_judged_it() {
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2026-12-21", "2026-12-23", "2026-11-02");
    let gram = decide(&mut cell, &app.id, "2026-11-09").unwrap();
    assert!(!granted(&gram));
    // What the engine used: the cell's answer, not a finding of the head.
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
    let found = cell.findings(&mut service(), &[], &gram.id).unwrap();
    assert_eq!(found, vec![PROFESSION.to_string()]);

    // In the trace of the decision, the answer is one node, given by the
    // school: its cell, with the reduction and its version; the head's
    // policy filled the term with it.
    let explained = cell.explain(&mut service(), &[], &gram.id).unwrap();
    let trace = explained.trace.as_ref().unwrap();
    let nodes = answered(trace);
    let node = nodes
        .iter()
        .find(|n| n.name == OUTSIDE_HOLIDAYS)
        .unwrap_or_else(|| panic!("{}", trace.render_box_drawing()));
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, RECORDING_ACTOR);
    assert_eq!(answer.law_id, HOLIDAYS);
    assert_eq!(answer.valid_from, "2026-08-01");
    let rendered = trace.render_box_drawing();
    assert!(
        rendered.contains(&format!("with the answer of {RECORDING_ACTOR}")),
        "{rendered}"
    );

    // Why: the cell's own account runs its reduction, which read the
    // kerstvakantie from the regeling.
    let why = cell
        .explain_answer(&mut service(), &[], &gram.id, OUTSIDE_HOLIDAYS)
        .unwrap();
    assert_eq!(why.len(), 1);
    assert_eq!(why[0].outputs[OUTSIDE_HOLIDAYS].to_string(), "false");
    let reduction = why[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(reduction.contains("2026-12-19"), "{reduction}");
}

#[test]
fn leave_outside_every_holiday_is_granted() {
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let gram = decide(&mut cell, &app.id, "2026-10-20").unwrap();
    assert!(granted(&gram), "{:?}", gram.outputs);
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(true));
}

#[test]
fn the_summer_holiday_follows_the_region_of_the_school() {
    // 12 to 16 July 2027: summer in regio Noord (from 10 July), not yet in
    // regio Zuid (from 24 July).
    let mut noord = school_in("Groningen", "Groningen");
    let mut zuid = school_in("Maastricht", "Limburg");
    let leave = |cell: &mut Cell| {
        let app = applied(cell, "2027-07-12", "2027-07-16", "2027-05-31");
        decide(cell, &app.id, "2027-06-01").unwrap()
    };
    assert!(!granted(&leave(&mut noord)));
    assert!(granted(&leave(&mut zuid)));
}

#[test]
fn the_herfstvakantie_of_the_school_policy_counts() {
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2026-10-19", "2026-10-23", "2026-09-21");
    let gram = decide(&mut cell, &app.id, "2026-10-01").unwrap();
    assert!(!granted(&gram));
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
    // One day of it is enough: 25 October is its last day.
    let app = applied(&mut cell, "2026-10-25", "2026-10-29", "2026-10-01");
    let gram = decide(&mut cell, &app.id, "2026-10-05").unwrap();
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
    // The week after is outside it.
    let app = applied(&mut cell, "2026-10-26", "2026-10-30", "2026-10-05");
    assert!(granted(&decide(&mut cell, &app.id, "2026-10-05").unwrap()));
}

#[test]
fn a_school_that_moves_before_the_leave_is_judged_on_the_leave_days() {
    // Utrecht is in regio Midden, whose summer of 2027 starts on 17 July;
    // Maastricht is in regio Zuid, whose summer starts on 24 July. Recorded
    // on 1 April: the school moves to Maastricht on 1 May. The head decides
    // in April on leave in July: the holidays that count are those of the
    // school on the leave days, so those of Zuid.
    let mut cell = school_in("Utrecht", "Utrecht");
    established(
        &mut cell,
        "Maastricht",
        "Limburg",
        "2027-05-01",
        "2027-04-01",
    );
    let app = applied(&mut cell, "2027-07-19", "2027-07-23", "2027-04-10");
    let gram = decide(&mut cell, &app.id, "2027-04-15").unwrap();
    assert!(granted(&gram), "{:?}", gram.outputs);
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(true));

    // The cell explains where the school was: the reduction for its
    // vestiging, asked inside the one for the holidays, read the move.
    let why = cell
        .explain_answer(&mut service(), &[], &gram.id, "gemeente_van_vestiging")
        .unwrap();
    assert_eq!(
        why[0].outputs["gemeente_van_vestiging"].to_string(),
        "Maastricht"
    );
    let rendered = why[0].trace.as_ref().unwrap().render_box_drawing();
    // The policy of the head read the executograms, inside the reduction
    // (RFC-048).
    assert!(rendered.contains("$VESTIGINGEN"), "{rendered}");

    // Leave that ends before the move is judged on Utrecht.
    let early = applied(&mut cell, "2027-04-19", "2027-04-23", "2027-04-15");
    let gram = decide(&mut cell, &early.id, "2027-04-15").unwrap();
    let why = cell
        .explain_answer(&mut service(), &[], &gram.id, "gemeente_van_vestiging")
        .unwrap();
    assert_eq!(
        why[0].outputs["gemeente_van_vestiging"].to_string(),
        "Utrecht"
    );
}

#[test]
fn leave_during_which_the_school_moves_is_refused_loudly() {
    // From 1 June 2027 the school is in Maastricht. Leave from 31 May to 4
    // June has two vestigingen, so two regions; the regeling gives one per
    // school, and the decision fails rather than pick one.
    let mut cell = school_in("Utrecht", "Utrecht");
    established(
        &mut cell,
        "Maastricht",
        "Limburg",
        "2027-06-01",
        "2027-04-01",
    );
    let app = applied(&mut cell, "2027-05-31", "2027-06-04", "2027-04-10");
    let e = decide(&mut cell, &app.id, "2027-04-15").unwrap_err();
    assert!(
        matches!(&e, Error::Engine(m) if m.to_string().contains("ONLY")),
        "{e}"
    );
}

#[test]
fn a_school_year_the_policy_does_not_know_leaves_it_to_the_head() {
    // The policy in force knows the herfst- and voorjaarsvakantie of
    // 2026–2027 only. For leave in 2027–2028 the cell does not know them and
    // is silent, so the head judges as without the reduction.
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2028-02-07", "2028-02-11", "2027-07-01");
    let e = decide(&mut cell, &app.id, "2027-07-05").unwrap_err();
    asks_the_head(&e);
    let findings = map(json!({ PROFESSION: true, OUTSIDE_HOLIDAYS_HEAD: false }));
    let gram = cell
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &findings,
            day("2027-07-05"),
        )
        .unwrap()
        .remove(0);
    assert!(!granted(&gram));
    let found = cell.findings(&mut service(), &[], &gram.id).unwrap();
    assert!(
        found.contains(&OUTSIDE_HOLIDAYS_HEAD.to_string()),
        "{found:?}"
    );
}

#[test]
fn a_school_without_a_vestiging_has_no_region_and_the_head_judges() {
    let mut cell = school();
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, "2026-10-20").unwrap_err();
    asks_the_head(&e);
}

#[test]
fn two_vestigingen_from_the_same_day_are_refused() {
    let mut cell = school_in("Utrecht", "Utrecht");
    established(
        &mut cell,
        "Maastricht",
        "Limburg",
        "2020-08-01",
        "2026-08-01",
    );
    let app = applied(&mut cell, "2027-02-01", "2027-02-05", "2026-10-13");
    let e = decide(&mut cell, &app.id, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Engine(m) if m.to_string().contains("ONLY")),
        "{e}"
    );
}

#[test]
fn the_voorjaarsvakantie_of_the_school_policy_counts() {
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2027-02-22", "2027-02-26", "2026-11-02");
    let gram = decide(&mut cell, &app.id, "2026-11-09").unwrap();
    assert!(!granted(&gram));
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
}

#[test]
fn the_summer_of_the_school_year_before_runs_into_the_next() {
    // 30 August to 3 September 2027 falls in schooljaar 2027–2028, and in
    // regio Zuid still in the summer of 2027 (until 5 September). In regio
    // Noord that summer ended on 22 August. Decided in August 2027, the
    // school's policy for 2027–2028 is in force.
    let next = [rooster_2027()];
    let outside = |gemeente: &str, provincie: &str| {
        let mut cell = school_in(gemeente, provincie);
        let app = applied(&mut cell, "2027-08-30", "2027-09-03", "2027-08-02");
        let findings = map(json!({ PROFESSION: true }));
        let gram = cell
            .decide(
                &mut service_with(&next),
                &[],
                LAW,
                ARTICLE,
                &app.id,
                &Map::new(),
                &findings,
                day("2027-08-06"),
            )
            .unwrap()
            .remove(0);
        gram.fields[OUTSIDE_HOLIDAYS].clone()
    };
    assert_eq!(outside("Maastricht", "Limburg"), json!(false));
    assert_eq!(outside("Groningen", "Groningen"), json!(true));
}

#[test]
fn the_next_version_of_the_policy_holds_the_herfstvakantie_of_its_year() {
    // In force from 1 August 2027, the policy for 2027–2028 puts the
    // herfstvakantie on 16 to 24 October 2027.
    let next = [rooster_2027()];
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2027-10-18", "2027-10-22", "2027-09-01");
    let findings = map(json!({ PROFESSION: true }));
    let gram = cell
        .decide(
            &mut service_with(&next),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &findings,
            day("2027-09-06"),
        )
        .unwrap()
        .remove(0);
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
}

#[test]
fn a_sandbox_keeps_the_executograms_and_answers_as_the_cell() {
    let mut cell = school_in("Utrecht", "Utrecht");
    let app = applied(&mut cell, "2026-12-21", "2026-12-23", "2026-11-02");
    let mut sandbox = cell.sandbox();
    assert_eq!(sandbox.grams(), cell.grams());
    let gram = decide(&mut sandbox, &app.id, "2026-11-09").unwrap();
    assert_eq!(gram.fields[OUTSIDE_HOLIDAYS], json!(false));
    // A move recorded in the sandbox stays out of the cell.
    established(
        &mut sandbox,
        "Groningen",
        "Groningen",
        "2026-11-10",
        "2026-11-10",
    );
    assert_eq!(cell.grams().len(), 2);
}
