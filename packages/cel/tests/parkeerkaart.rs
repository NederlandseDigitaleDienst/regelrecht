//! A second case on the same cell: the college of a municipality grants a
//! gehandicaptenparkeerkaart on application (BABW art. 49, with the criteria
//! of the Regeling gehandicaptenparkeerkaart and the validity of art. 51),
//! with the Awb on it, a reduction over its own decisions and the objection
//! under Awb 7:11. Nothing in the cell knows this case: it follows from the
//! law, the municipality's policy and its reduction, as vakantieverlof does.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use regelrecht_cel::{decision, shape, Cell, Error, Gram, Source};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const LAW: &str = "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw";
const ARTICLE: &str = "49";
const AWB: &str = "algemene_wet_bestuursrecht";
const OBJECTION: &str = "7:11";
/// A fictional municipality.
const RECORDING_ACTOR: &str = "gemeente_wilgendaal";

/// The municipality's fictitious policy, which fills the open term of art.
/// 49 on how the college uses its power, and declares an open term of its
/// own: whether the applicant already holds a valid card.
const POLICY: &str = "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal";
/// The open term of art. 49 the policy fills.
const POWER: &str = "bevoegdheid_tot_verstrekken_gebruikt";
/// The open term of the policy that the municipality's reduction fills.
const VALID: &str = "aanvrager_heeft_geldige_gehandicaptenparkeerkaart";
const REDUCTION: &str = "reductie_geldige_gehandicaptenparkeerkaart";

/// The four judgements the college gives after the medical examination.
const WALKING: &str = "langdurige_loopbeperking";
const HELP: &str = "continu_afhankelijk_van_hulp_bestuurder";
const WHEELCHAIR: &str = "permanent_rolstoelgebonden";
const OTHER: &str = "ernstige_beperkingen_andere_dan_loopbeperkingen";

const ADMISSIBLE: &str = "bezwaar_ontvankelijk";
const CAUSE: &str = "heroverweging_geeft_aanleiding";
const REVOKED: &str = "bestreden_besluit_herroepen";
const NEW: &str = "nieuw_besluit";

/// The BABW, the Regeling, the Awb and the municipality's policy from the
/// corpus.
fn service() -> LawExecutionService {
    let regulation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/regulation/nl");
    let mut service = LawExecutionService::new();
    for law in [
        format!("amvb/{LAW}"),
        "ministeriele_regeling/regeling_gehandicaptenparkeerkaart".to_string(),
        format!("wet/{AWB}"),
        format!("uitvoeringsbeleid/{POLICY}"),
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

/// The municipality's cell directory, with its reduction (RFC-048).
fn cell_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{RECORDING_ACTOR}"))
}

/// The municipality's cell, with an empty chronicle and its reductions.
fn cell() -> Cell {
    Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap()
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
    cell.record_submission(
        &mut service(),
        LAW,
        ARTICLE,
        None,
        &application(bsn),
        day(on),
    )
    .unwrap()
}

/// The college's findings after the medical examination: only a long-term
/// walking limitation, or none.
fn examined(walking: bool) -> Map<String, Value> {
    object_map(json!({ WALKING: walking, HELP: false, WHEELCHAIR: false, OTHER: false }))
}

/// Decide on `application` on `on` with the college's `findings`; the
/// decision on the application.
fn decide_with(
    cell: &mut Cell,
    application: &str,
    findings: &Map<String, Value>,
    on: &str,
) -> Result<Gram, Error> {
    cell.decide(
        &mut service(),
        LAW,
        ARTICLE,
        application,
        &Map::new(),
        findings,
        day(on),
    )
    .map(|mut grams| grams.remove(0))
}

fn decide(cell: &mut Cell, application: &str, walking: bool, on: &str) -> Gram {
    decide_with(cell, application, &examined(walking), on).unwrap()
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
        Some(format!("{LAW}#{ARTICLE}").as_str())
    );
    let fields: Vec<(&str, &str)> = shape
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_declared_by()))
        .collect();
    let babw = format!("{LAW}#{ARTICLE}");
    assert_eq!(
        fields,
        vec![
            ("bsn_aanvrager", babw.as_str()),
            ("kaart_als_bestuurder", babw.as_str()),
            ("kaart_als_passagier", babw.as_str()),
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
    // The outcome; the last day of validity is derived, no outcome (RFC-050).
    assert_eq!(
        shape.outputs,
        [
            "gehandicaptenparkeerkaart_verstrekt",
            "kaart_vermeldt_bestuurder",
            "kaart_vermeldt_passagier"
        ]
    );
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
            // The day of the decision (a parameter of art. 49 that the Awb
            // stage also needs) and the day it is made known, both from the
            // cell.
            ("besluit_datum", &Source::Cell, false),
            ("bekendmaking_datum", &Source::Cell, false),
            (WALKING, &Source::Caller, false),
            (HELP, &Source::Caller, false),
            (WHEELCHAIR, &Source::Caller, false),
            (OTHER, &Source::Caller, false),
            // The college's power, which its policy fills first.
            (POWER, &Source::Caller, true),
        ]
    );
    assert_eq!(shape.judgements, [WALKING, HELP, WHEELCHAIR, OTHER]);
    // Whether the applicant holds a valid card is no input of art. 49: the
    // policy asks it, and in the cell the reduction answers it there.
    assert!(shape.input(VALID).is_none());
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
    // the decision (own choice). Derived, and recorded with the decision.
    assert_eq!(gram.outputs["kaart_geldig_tot"], json!("2031-10-19"));
    // The Awb on the decision.
    assert_eq!(gram.outputs["motivering_vereist"], json!(true));
    assert_eq!(
        gram.outputs["bezwaartermijn_einddatum"],
        json!("2026-12-01")
    );
    // What the policy filled for art. 49, and the college's findings.
    assert_eq!(gram.fields[POWER], json!(true));
    assert_eq!(gram.fields[WALKING], json!(true));
    // The cell's answer to the policy's term is no field of the decision;
    // a replay reports it (see the trace tests below).
    assert!(!gram.fields.contains_key(VALID));
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
    let e = decide_with(&mut cell, &app.id, &partial, "2026-10-20").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains(&format!("asks '{OTHER}'"))),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn the_reduction_sets_a_finding_of_the_college_aside() {
    // The college finds that the applicant holds a valid card. The cell's
    // reduction fills the policy's term and so goes first (RFC-048): there
    // is none in its chronicle, and the card is granted.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut given = examined(true);
    given.insert(VALID.into(), json!(true));
    let gram = decide_with(&mut cell, &app.id, &given, "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // The finding set aside is no finding the decision used.
    let found = cell.findings(&mut service(), &gram.id).unwrap();
    assert!(!found.contains(&VALID.to_string()), "{found:?}");
    assert!(found.contains(&WALKING.to_string()), "{found:?}");
}

#[test]
fn a_second_application_while_the_card_is_valid_is_refused() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let second = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    // The municipality's policy: no card next to a valid one.
    assert_eq!(second.fields[POWER], json!(false));
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
    assert_eq!(last_day.fields[POWER], json!(false));
    assert_eq!(
        last_day.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    let expired = apply_and_decide(&mut cell, APPLICANT, true, "2031-10-20");
    assert_eq!(expired.fields[POWER], json!(true));
    assert_eq!(
        expired.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(expired.outputs["kaart_geldig_tot"], json!("2036-10-19"));
}

#[test]
fn a_refusal_does_not_count() {
    // A refusal has no last day of validity (`kaart_geldig_tot` is null);
    // only a card that was granted counts. Without the reduction's
    // `verstrekt` filter this test fails through the engine (DATE_DIFF on an
    // absent operand), not on the assertion below: the filter guards that
    // comparison as well.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let gram = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-27");
    assert_eq!(gram.fields[POWER], json!(true));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_card_of_someone_else_does_not_count() {
    let mut cell = cell();
    apply_and_decide(&mut cell, "999990032", true, "2026-10-20");
    let gram = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-27");
    assert_eq!(gram.fields[POWER], json!(true));
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_cell_lists_the_lexostatus_it_answers() {
    // The term the reduction fills is the policy's, not the BABW's.
    let answers = cell().answers();
    assert_eq!(answers.len(), 1);
    let answer = &answers[0];
    assert_eq!(
        (
            answer.term.law.as_str(),
            answer.term.article.as_str(),
            answer.term.open_term.as_str()
        ),
        (POLICY, "1", VALID)
    );
    assert_eq!(answer.reduction, REDUCTION);
    assert_eq!(answer.valid_from, day("2026-01-01"));
}

// ---------------------------------------------------------------------------
// The objection and the decision on it (Awb 7:11, RFC-050)
// ---------------------------------------------------------------------------

/// A complete objection on the ground `ground`, and `with`.
fn objection(ground: &str, with: &[(&str, Value)]) -> Map<String, Value> {
    let mut supplied = object_map(json!({
        "bezwaarschrift_ondertekend": true,
        "naam_indiener": "A. de Vries",
        "adres_indiener": "Voorbeeldlaan 2",
        "dagtekening_bezwaarschrift": "2026-11-01",
        "omschrijving_bestreden_besluit": "besluit op de aanvraag om een gehandicaptenparkeerkaart",
        "gronden_bezwaar": ground,
    }));
    for (name, value) in with {
        supplied.insert((*name).into(), value.clone());
    }
    supplied
}

/// Record the objection against the decision `contested` on `on`.
fn object(
    cell: &mut Cell,
    contested: &str,
    supplied: &Map<String, Value>,
    on: &str,
) -> Result<Gram, Error> {
    cell.record_submission(
        &mut service(),
        AWB,
        OBJECTION,
        Some(contested),
        supplied,
        day(on),
    )
}

/// Decide on the objection on `on`, with what the college finds anew: the
/// decision on the objection first, then the new decision in the place of
/// the revoked one, if any.
fn reconsider(
    cell: &mut Cell,
    objection: &str,
    findings: &Map<String, Value>,
    on: &str,
) -> Vec<Gram> {
    cell.decide(
        &mut service(),
        AWB,
        OBJECTION,
        objection,
        &Map::new(),
        findings,
        day(on),
    )
    .unwrap()
}

/// The college's finding on the walking limitation, given anew.
fn walking(judged: bool) -> Map<String, Value> {
    object_map(json!({ WALKING: judged }))
}

#[test]
fn an_objection_does_not_change_the_application() {
    // Awb 6:5 and 7:11 lid 1: the objection gives grounds, the college
    // reconsiders on the application as it was. A card as a passenger is a
    // new application.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let amended = objection(
        "ik kan niet verder lopen dan vijftig meter",
        &[("kaart_als_passagier", json!(true))],
    );
    let e = object(&mut cell, &refused.id, &amended, "2026-11-01").unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("does not ask 'kaart_als_passagier'")),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 2);
}

#[test]
fn ongegrond_the_same_examination_refuses_again_and_the_refusal_stands() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let bezwaar = object(
        &mut cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    assert_eq!(
        bezwaar.fields["bezwaartermijn_einddatum"],
        json!("2026-12-01")
    );
    assert_eq!(
        bezwaar.outputs["bezwaarschrift_tijdig_ingediend"],
        json!(true)
    );

    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided.len(), 1);
    let decision = &decided[0];
    assert_eq!(decision.grondslag, "algemene_wet_bestuursrecht#7:11");
    assert_eq!(decision.outputs[ADMISSIBLE], json!(true));
    assert_eq!(decision.outputs[CAUSE], json!(false));
    assert_eq!(decision.outputs[REVOKED], json!(false));
    assert_eq!(decision.outputs[NEW], Value::Null);
}

#[test]
fn gegrond_a_new_examination_revokes_the_refusal_and_grants_the_card() {
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let bezwaar = object(
        &mut cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    // A new medical examination finds a long-term walking limitation; the
    // college gives that finding anew, and the others stand.
    let decided = reconsider(&mut cell, &bezwaar.id, &walking(true), "2026-11-20");
    assert_eq!(decided.len(), 2);
    let (decision, new) = (&decided[0], &decided[1]);

    assert_eq!(decision.outputs[CAUSE], json!(true));
    assert_eq!(decision.outputs[REVOKED], json!(true));
    assert_eq!(decision.outputs[NEW], json!(new.id));
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
    assert_eq!(later.fields[POWER], json!(false));
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
    let bezwaar = object(
        &mut cell,
        &granted.id,
        &objection("de kaart vermeldt alleen de letter B", &[]),
        "2026-11-01",
    )
    .unwrap();
    let decided = reconsider(&mut cell, &bezwaar.id, &walking(false), "2026-11-20");
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    assert_eq!(
        decided[1].outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    // Neither the revoked card nor the refusal in its place counts.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(true));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

/// Grant the card on 2026-10-20 and object against it on 2026-11-01.
fn granted_and_objected(cell: &mut Cell) -> (Gram, Gram) {
    let granted = apply_and_decide(cell, APPLICANT, true, "2026-10-20");
    assert_eq!(granted.outputs["kaart_geldig_tot"], json!("2031-10-19"));
    let bezwaar = object(
        cell,
        &granted.id,
        &objection("de kaart vermeldt alleen de letter B", &[]),
        "2026-11-01",
    )
    .unwrap();
    (granted, bezwaar)
}

#[test]
fn an_unfounded_objection_against_a_grant_leaves_the_decision_standing() {
    // The heroverweging runs art. 49 on the day of the decision on the
    // objection, a month after the grant, so it would issue a card valid a
    // month longer. That end is derived data (`derived`, RFC-050), no
    // outcome, so it gives no cause: the outcome is the same, and the grant
    // stands with its own validity. During the heroverweging the cell reads
    // the chronicle without the contested grant, so the reduction does not
    // refuse the card for being next to itself.
    let mut cell = cell();
    let (granted, bezwaar) = granted_and_objected(&mut cell);
    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided.len(), 1);
    assert_eq!(decided[0].outputs[ADMISSIBLE], json!(true));
    assert_eq!(decided[0].outputs[CAUSE], json!(false));
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    assert_eq!(decided[0].outputs[NEW], Value::Null);

    // The grant is as it was, and it still counts.
    let recorded = cell.grams().iter().find(|g| g.id == granted.id).unwrap();
    assert_eq!(recorded.outputs["kaart_geldig_tot"], json!("2031-10-19"));
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(false));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

// ---------------------------------------------------------------------------
// The cell's answer in the trace (RFC-048 §3)
// ---------------------------------------------------------------------------

/// The nodes of `node` and below where the cell answered an open term.
fn answered(node: &regelrecht_engine::PathNode) -> Vec<&regelrecht_engine::PathNode> {
    let mut found: Vec<_> = node.children.iter().flat_map(answered).collect();
    if node.answer.is_some() {
        found.insert(0, node);
    }
    found
}

#[test]
fn the_cell_answers_the_policy_and_explains_the_card_it_counted() {
    let mut cell = cell();
    let first = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let second = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(
        second.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    // Step 1: the trace of the second decision holds one node for the term,
    // within the policy that fills art. 49: the cell's answer, with the
    // reduction, its version and how far it read the chronicle.
    let result = cell.explain(&mut service(), &second.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let nodes = answered(root);
    assert_eq!(nodes.len(), 1, "{}", root.render_box_drawing());
    let node = nodes[0];
    assert_eq!(node.name, VALID);
    assert_eq!(
        node.result,
        Some(regelrecht_engine::Value::from(json!(true)))
    );
    assert!(node.children.is_empty(), "{node:?}");
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, RECORDING_ACTOR);
    assert_eq!(answer.law_id, REDUCTION);
    assert_eq!(answer.valid_from, "2026-01-01");
    // Everything before this decision: the first application and decision,
    // and the second application.
    assert_eq!(answer.records_read, 3);
    assert_eq!(answer.last_record.as_deref(), second.refers_to.as_deref());
    // The reduction's FOREACH is not in the decision's trace.
    let trace = root.render_box_drawing();
    assert!(!trace.contains("FOREACH"), "{trace}");
    // The report says the same: the cell answered the policy's term.
    assert!(
        result.open_terms.iter().any(|f| f.term == VALID
            && f.law_id == POLICY
            && matches!(&f.filled_by, regelrecht_engine::FilledBy::Answer(by) if by == answer)),
        "{:?}",
        result.open_terms
    );

    // Step 2: asked why, the cell runs its reduction again with its own
    // trace, which shows the first card it counted.
    let explained = cell
        .explain_answer(&mut service(), &second.id, VALID)
        .unwrap();
    assert_eq!(explained.len(), 1);
    assert_eq!(
        explained[0].outputs[VALID],
        regelrecht_engine::Value::from(json!(true))
    );
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&first.id), "{why}");

    // Nothing stays loaded.
    let mut kept = service();
    cell.explain_answer(&mut kept, &second.id, VALID).unwrap();
    assert!(!kept.has_law(REDUCTION));
}

#[test]
fn on_a_refusal_on_the_criteria_the_cell_is_not_asked() {
    // Art. 49 reads the college's power only for an applicant who meets the
    // criteria (AND stops at the first false), so the policy and its term
    // are not read, and the cell answers nothing.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let result = cell.explain(&mut service(), &refused.id).unwrap();
    assert!(answered(result.trace.as_ref().unwrap()).is_empty());
    let e = cell
        .explain_answer(&mut service(), &refused.id, VALID)
        .unwrap_err();
    assert!(
        matches!(&e, Error::Refused(m) if m.contains("did not ask")),
        "{e}"
    );
}

#[test]
fn at_the_heroverweging_the_cell_answers_without_the_contested_grant() {
    let mut cell = cell();
    let (granted, bezwaar) = granted_and_objected(&mut cell);
    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");

    // One node for the term, within art. 49 called anew: the cell read
    // everything before this decision but the contested grant.
    let result = cell.explain(&mut service(), &decided[0].id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let nodes = answered(root);
    assert_eq!(nodes.len(), 1, "{}", root.render_box_drawing());
    assert_eq!(
        nodes[0].result,
        Some(regelrecht_engine::Value::from(json!(false)))
    );
    let answer = nodes[0].answer.as_ref().unwrap();
    let before = cell
        .grams()
        .iter()
        .take_while(|g| g.id != decided[0].id)
        .filter(|g| g.id != granted.id)
        .count();
    assert_eq!(answer.records_read, before);
    assert_eq!(answer.last_record.as_deref(), Some(bezwaar.id.as_str()));

    // The reduction's own account does not read the contested grant.
    let explained = cell
        .explain_answer(&mut service(), &decided[0].id, VALID)
        .unwrap();
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(!why.contains(&granted.id), "{why}");
}
