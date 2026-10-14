//! A second case on the same cell: the college of a municipality grants a
//! gehandicaptenparkeerkaart on application (BABW art. 49, with the criteria
//! of the Regeling gehandicaptenparkeerkaart and the validity of art. 51),
//! with the Awb on it, a reduction over its own decisions and the objection
//! under Awb 7:11. Nothing in the cell knows this case: it follows from the
//! law, the municipality's policy and its reduction, as vakantieverlof does.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::{decision, shape, Cell, Error, Gram, ObjectionDecision, Source};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const LAW: &str = "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw";
const ARTICLE: &str = "49";
const AWB: &str = "algemene_wet_bestuursrecht";
const OBJECTION: &str = "7:11";
/// A fictional municipality.
const RECORDING_ACTOR: &str = "gemeente_wilgendaal";

/// The input of the decision that the municipality's reduction yields.
const VALID: &str = "aanvrager_heeft_geldige_gehandicaptenparkeerkaart";
/// The open term the municipality's fictitious policy fills.
const POLICY_TERM: &str = "kaart_naast_geldige_kaart_verstrekken";
/// The four judgements the college gives after the medical examination.
const WALKING: &str = "langdurige_loopbeperking";
const HELP: &str = "continu_afhankelijk_van_hulp_bestuurder";
const WHEELCHAIR: &str = "permanent_rolstoelgebonden";
const OTHER: &str = "ernstige_beperkingen_andere_dan_loopbeperkingen";
const CAUSE: &str = "heroverweging_geeft_aanleiding_tot_herroeping";

/// The BABW, the Regeling, the Awb and the municipality's policy from the
/// corpus.
fn service() -> LawExecutionService {
    let regulation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/regulation/nl");
    let mut service = LawExecutionService::new();
    for law in [
        format!("amvb/{LAW}"),
        "ministeriele_regeling/regeling_gehandicaptenparkeerkaart".to_string(),
        format!("wet/{AWB}"),
        "uitvoeringsbeleid/fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
            .to_string(),
    ] {
        for entry in std::fs::read_dir(regulation.join(law)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|x| x == "yaml") {
                service
                    .load_law(&std::fs::read_to_string(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
    }
    service
}

/// The municipality's cell, with an empty chronicle and its reductions.
fn cell() -> Cell {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{RECORDING_ACTOR}"));
    Cell::in_memory(RECORDING_ACTOR)
        .with_reductions(dir)
        .unwrap()
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn object_map(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

/// A complete application of `bsn` for a card as a driver, with fictional
/// data (BSNs from the test range).
fn application(bsn: &str) -> Map<String, Value> {
    object_map(json!({
        "bsn_aanvrager": bsn,
        "kaart_als_bestuurder": true,
        "kaart_als_passagier": false,
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "A. de Vries",
        "adres_aanvrager": "Voorbeeldlaan 2",
        "dagtekening": "2026-10-12",
        "gevraagde_beschikking": "gehandicaptenparkeerkaart",
    }))
}

const APPLICANT: &str = "999990020";

fn applied(cell: &mut Cell, bsn: &str, on: &str) -> Gram {
    cell.record_submission(&service(), LAW, ARTICLE, &application(bsn), day(on))
        .unwrap()
}

/// The college's judgements after the medical examination: only a long-term
/// walking limitation, or none.
fn examined(walking: bool) -> Map<String, Value> {
    object_map(json!({ WALKING: walking, HELP: false, WHEELCHAIR: false, OTHER: false }))
}

fn decide(cell: &mut Cell, application: &str, walking: bool, on: &str) -> Gram {
    cell.decide(
        &mut service(),
        LAW,
        ARTICLE,
        application,
        &examined(walking),
        day(on),
    )
    .unwrap()
}

fn apply_and_decide(cell: &mut Cell, bsn: &str, walking: bool, on: &str) -> Gram {
    let app = applied(cell, bsn, on);
    decide(cell, &app.id, walking, on)
}

#[test]
fn the_shape_of_the_application_comes_from_the_babw_and_the_awb() {
    let shape = shape::derive(&service(), LAW, ARTICLE, day("2026-10-13")).unwrap();
    assert_eq!(shape.kind, "AANVRAAG");
    assert_eq!(shape.stage.as_deref(), Some("AANVRAAG"));
    assert_eq!(
        shape.requests.as_deref(),
        Some("besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49")
    );
    let fields: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    let babw = "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49";
    assert_eq!(
        fields,
        vec![
            ("bsn_aanvrager", babw),
            ("kaart_als_bestuurder", babw),
            ("kaart_als_passagier", babw),
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
fn an_application_is_recorded_with_the_awb_deadline() {
    let mut cell = cell();
    let gram = applied(&mut cell, APPLICANT, "2026-10-13");
    assert_eq!(gram.kind, "AANVRAAG");
    assert_eq!(gram.recording_actor, RECORDING_ACTOR);
    assert_eq!(gram.fields["kaart_als_bestuurder"], json!(true));
    assert_eq!(
        gram.outputs["aanvraag_bevat_wat_artikel_4_2_eist"],
        json!(true)
    );
    assert_eq!(
        gram.outputs["redelijke_beslistermijn_einddatum"],
        json!("2026-12-08")
    );
    // The decision's outputs wait for the decision.
    assert!(!gram
        .outputs
        .contains_key("gehandicaptenparkeerkaart_verstrekt"));
}

#[test]
fn the_decision_asks_from_the_application_the_college_the_policy_and_the_cell() {
    let shape = decision::derive(&service(), LAW, ARTICLE, day("2026-10-20")).unwrap();
    assert_eq!(shape.kind, "BESCHIKKING");
    let asked: Vec<(&str, &Source, bool)> = shape
        .inputs
        .iter()
        .map(|i| (i.field.name.as_str(), &i.source, i.policy_first))
        .collect();
    assert_eq!(
        asked,
        vec![
            ("bsn_aanvrager", &Source::Application, false),
            ("kaart_als_bestuurder", &Source::Application, false),
            ("kaart_als_passagier", &Source::Application, false),
            // Not an application field: the cell's reduction yields it.
            (VALID, &Source::Caller, false),
            // A parameter of art. 49 that the Awb stage requires too: the
            // day of the decision.
            ("besluit_datum", &Source::Cell, false),
            ("bekendmaking_datum", &Source::Cell, false),
            (WALKING, &Source::Caller, false),
            (HELP, &Source::Caller, false),
            (WHEELCHAIR, &Source::Caller, false),
            (OTHER, &Source::Caller, false),
            // The college's own discretion, which its policy fills first.
            (POLICY_TERM, &Source::Caller, true),
        ]
    );
    assert_eq!(shape.judgements, [WALKING, HELP, WHEELCHAIR, OTHER]);
}

#[test]
fn a_driver_with_a_long_term_walking_limitation_gets_a_card_for_five_years() {
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");

    assert_eq!(gram.kind, "BESCHIKKING");
    assert_eq!(gram.refers_to.as_deref(), Some(app.id.as_str()));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(gram.outputs["kaart_vermeldt_bestuurder"], json!(true));
    assert_eq!(gram.outputs["kaart_vermeldt_passagier"], json!(false));
    // Art. 51 lid 1: five years from the day of issue, which is the day of
    // the decision (own choice).
    assert_eq!(gram.outputs["kaart_geldig_tot"], json!("2031-10-19"));
    // The Awb on the decision.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(
        gram.outputs["bezwaartermijn_einddatum"],
        json!("2026-12-01")
    );
    // The reduction's answer, kept with the decision like an application
    // field.
    assert_eq!(gram.fields[VALID], json!(false));
    assert_eq!(gram.fields[WALKING], json!(true));
}

#[test]
fn without_a_limitation_from_the_medical_examination_the_card_is_refused() {
    let mut cell = cell();
    let gram = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    assert_eq!(gram.outputs["kaart_geldig_tot"], Value::Null);
    // A refusal is a decision too: motivated and open to objection.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(gram.outputs["bezwaartermijn_weken"], json!(6));
}

#[test]
fn the_college_judges_the_criteria_itself() {
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut partial = examined(true);
    partial.remove(OTHER);
    let e = cell
        .decide(
            &mut service(),
            LAW,
            ARTICLE,
            &app.id,
            &partial,
            day("2026-10-20"),
        )
        .unwrap_err();
    assert!(matches!(&e, Error::Refused(m) if m.contains(OTHER)), "{e}");
    // What the reduction yields, the college does not supply.
    let mut answer = examined(true);
    answer.insert(VALID.into(), json!(false));
    let e = cell
        .decide(
            &mut service(),
            LAW,
            ARTICLE,
            &app.id,
            &answer,
            day("2026-10-20"),
        )
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("reduction")),
        "{e}"
    );
}

#[test]
fn a_second_application_while_the_card_is_valid_is_refused() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let second = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(second.fields[VALID], json!(true));
    // The municipality's policy: no card next to a valid one. The decision
    // keeps the value it used.
    assert_eq!(second.fields[POLICY_TERM], json!(false));
    assert_eq!(
        second.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn on_its_last_day_the_card_is_valid_and_the_day_after_it_is_not() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let last_day = apply_and_decide(&mut cell, APPLICANT, true, "2031-10-19");
    assert_eq!(last_day.fields[VALID], json!(true));
    assert_eq!(
        last_day.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    let expired = apply_and_decide(&mut cell, APPLICANT, true, "2031-10-20");
    assert_eq!(expired.fields[VALID], json!(false));
    assert_eq!(
        expired.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(expired.outputs["kaart_geldig_tot"], json!("2036-10-19"));
}

#[test]
fn a_refusal_and_a_card_of_someone_else_do_not_count() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    apply_and_decide(&mut cell, "999990032", true, "2026-10-20");
    let gram = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-27");
    assert_eq!(gram.fields[VALID], json!(false));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

// ---------------------------------------------------------------------------
// The objection and the decision on it (Awb 7:11, RFC-050)
// ---------------------------------------------------------------------------

fn objection() -> Map<String, Value> {
    objection_on("ik kan niet verder lopen dan vijftig meter")
}

/// A complete objection on the ground `ground`.
fn objection_on(ground: &str) -> Map<String, Value> {
    object_map(json!({
        "bezwaarschrift_ondertekend": true,
        "naam_indiener": "A. de Vries",
        "adres_indiener": "Voorbeeldlaan 2",
        "dagtekening_bezwaarschrift": "2026-11-01",
        "omschrijving_bestreden_besluit": "weigering gehandicaptenparkeerkaart",
        "gronden_bezwaar": ground,
    }))
}

fn object(cell: &mut Cell, contested: &str, on: &str) -> Gram {
    cell.record_objection(&service(), AWB, OBJECTION, contested, &objection(), day(on))
        .unwrap()
}

/// Decide on the objection on `on`, with the judgements the college gives
/// anew in `anew`.
fn decide_objection(
    cell: &mut Cell,
    objection: &str,
    anew: &Map<String, Value>,
    on: &str,
) -> ObjectionDecision {
    cell.decide_on_objection(&mut service(), AWB, OBJECTION, objection, anew, day(on))
        .unwrap()
}

#[test]
fn ongegrond_the_same_examination_refuses_again_and_the_refusal_stands() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let bezwaar = object(&mut cell, &refused.id, "2026-11-01");
    assert_eq!(
        bezwaar.fields["bezwaartermijn_einddatum"],
        json!("2026-12-01")
    );
    assert_eq!(
        bezwaar.outputs["bezwaarschrift_tijdig_ingediend"],
        json!(true)
    );

    let decided = decide_objection(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(
        decided.decision.grondslag,
        "algemene_wet_bestuursrecht#7:11"
    );
    assert_eq!(decided.decision.fields[CAUSE], json!(false));
    assert_eq!(
        decided.decision.outputs["bezwaar_ontvankelijk"],
        json!(true)
    );
    assert_eq!(
        decided.decision.outputs["bestreden_besluit_herroepen"],
        json!(false)
    );
    assert!(decided.new_decision.is_none());
}

#[test]
fn gegrond_a_new_examination_revokes_the_refusal_and_grants_the_card() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let bezwaar = object(&mut cell, &refused.id, "2026-11-01");
    // A new medical examination finds a long-term walking limitation; the
    // college gives that judgement anew, and the others stand.
    let anew = object_map(json!({ WALKING: true }));
    let decided = decide_objection(&mut cell, &bezwaar.id, &anew, "2026-11-20");

    assert_eq!(decided.decision.fields[CAUSE], json!(true));
    assert_eq!(
        decided.decision.outputs["bestreden_besluit_herroepen"],
        json!(true)
    );
    let new = decided.new_decision.unwrap();
    assert_eq!(new.grondslag, format!("{LAW}#{ARTICLE}"));
    assert_eq!(new.refers_to.as_deref(), Some(bezwaar.id.as_str()));
    assert_eq!(new.fields[WALKING], json!(true));
    assert_eq!(new.fields[OTHER], json!(false));
    assert_eq!(
        new.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // Its own validity, from its own day of issue.
    assert_eq!(new.outputs["kaart_geldig_tot"], json!("2031-11-19"));

    // The new decision counts: a later application is refused.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[VALID], json!(true));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn a_revoked_card_does_not_count_as_a_valid_one() {
    // Constructed: on objection against the grant, a new examination finds
    // no limitation, so the grant is revoked and a refusal takes its place.
    let mut cell = cell();
    let granted = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let bezwaar = cell
        .record_objection(
            &service(),
            AWB,
            OBJECTION,
            &granted.id,
            &objection_on("de kaart vermeldt alleen de letter B"),
            day("2026-11-01"),
        )
        .unwrap();
    let anew = object_map(json!({ WALKING: false }));
    let decided = decide_objection(&mut cell, &bezwaar.id, &anew, "2026-11-20");
    let new = decided.new_decision.unwrap();
    assert_eq!(
        new.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    // Neither the revoked card nor the refusal in its place counts.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[VALID], json!(false));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}
