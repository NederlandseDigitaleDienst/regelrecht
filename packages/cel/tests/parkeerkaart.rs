//! A second case on the same cell: the college of a municipality grants a
//! gehandicaptenparkeerkaart on application (BABW art. 49, with the criteria
//! of the Regeling gehandicaptenparkeerkaart and the validity of art. 51),
//! with the Awb on it, a reduction over its own decisions and the objection
//! under Awb 7:11. Nothing in the cell knows this case: it follows from the
//! law, the municipality's policy and its reduction, as vakantieverlof does.
//!
//! The medical examination is the GGD's (Regeling art. 3): a cell of its
//! own records it, and the municipality's decisions take that cell along
//! as their sphere (RFC-052). Each test has its own GGD ([`GGD`]). The GGD
//! also records its advice, with per criterion whether the applicant meets
//! it (the municipality's fictitious policy, art. 5); the policy fills the
//! college's four judgements from it (art. 6), and without an advice the
//! college judges.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{day, object_map};

use std::path::{Path, PathBuf};

use regelrecht_cel::{decision, shape, Cell, Error, Gram, Reference, Source};
use regelrecht_engine::LawExecutionService;
use serde_json::{json, Map, Value};

const LAW: &str = "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw";
const ARTICLE: &str = "49";
const AWB: &str = "algemene_wet_bestuursrecht";
const OBJECTION: &str = "7:11";
/// A fictional municipality.
const RECORDING_ACTOR: &str = "gemeente_wilgendaal";

/// The municipality's fictitious policy, which fills the open term of art.
/// 49 on how the college uses its power, declares an open term of its own
/// (whether the applicant already holds a valid card), and binds that term
/// and two of the Regeling to the answers of cells (RFC-048).
const POLICY: &str = "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal";
/// The open term of art. 49 the policy fills.
const POWER: &str = "bevoegdheid_tot_verstrekken_gebruikt";
/// The open term of the policy that the policy binds to the answer of the
/// municipality's cell, from its reduction.
const VALID: &str = "aanvrager_heeft_geldige_gehandicaptenparkeerkaart";
const REDUCTION: &str = "reductie_geldige_gehandicaptenparkeerkaart";
/// The answer of the same cell for the bedrijfsparkeervergunning, another
/// case of the municipality: how many valid permits a company holds.
const PERMITS: &str = "aantal_geldige_bedrijfsparkeervergunningen";
const PERMITS_REDUCTION: &str = "reductie_geldige_bedrijfsparkeervergunningen";

/// Art. 2 of the Regeling: no card before a medical examination (lid 1),
/// unless it is left out for someone who had a card before (lid 2, onder a
/// en b). The cell answers its two facts.
const REGELING: &str = "regeling_gehandicaptenparkeerkaart";
const EXAMINED: &str = "geneeskundig_onderzoek_heeft_plaatsgehad";
const EARLIER: &str = "aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt";
/// What the college establishes itself where no cell answers: open terms of
/// its policy (art. 2 to 4, RFC-048 §2).
const VALID_C: &str = "aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college";
const EXAMINED_C: &str = "geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college";
const EARLIER_C: &str = "aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt_volgens_college";
/// The fact art. 3 lid 1 of the Regeling requires: the medical
/// examination. The GGD records it as an executogram in its own cell, and
/// its reduction reads those executograms.
const EXAMINATION: &str = "regeling_gehandicaptenparkeerkaart#3#geneeskundig_onderzoek_verricht";
const REPORTS: &str = "reductie_geneeskundig_onderzoek";
/// The fictitious GGD that examines for the municipality.
const GGD_ACTOR: &str = "ggd_wilgendaal";
/// The fact art. 5 of the municipality's policy requires: the advice of who
/// examines, with per criterion of Regeling art. 1 whether the applicant
/// meets it. The GGD records it, and its reduction gives the four answers
/// art. 7 binds to the terms of that article (RFC-048); art. 6 of the
/// municipality's policy fills the college's four judgements on art. 49
/// from them.
const ADVISED: &str =
    "fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";
const ADVICE_REDUCTION: &str = "reductie_geneeskundig_advies";
const ADVICE_POLICY: &str = "fictief_beleid_geneeskundig_advies_ggd_wilgendaal";
/// The four terms of policy art. 5 the GGD answers.
const ADVICE: [&str; 4] = [
    "advies_langdurige_loopbeperking",
    "advies_continu_afhankelijk_van_hulp_bestuurder",
    "advies_permanent_rolstoelgebonden",
    "advies_ernstige_beperkingen_andere_dan_loopbeperkingen",
];

thread_local! {
    /// The GGD's cell of this test: tests run on threads of their own.
    static GGD: std::cell::RefCell<Cell> = std::cell::RefCell::new(
        Cell::in_memory(
            GGD_ACTOR,
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{GGD_ACTOR}")),
        )
        .unwrap(),
    );
}

/// A copy of the GGD's cell as it stands, to take along as the sphere.
fn ggd() -> Cell {
    GGD.with(|g| g.borrow().sandbox())
}
/// The college's power to leave the examination out, and the two
/// judgements of lid 2, onder a en b, all on art. 49.
const LEFT_OUT: &str = "geneeskundig_onderzoek_achterwege_gelaten";
const KNOWN: &str = "bekend_dat_aanvrager_nog_steeds_voldoet";
const JUDGED: &str = "keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet";

/// The four judgements the college gives after the medical examination.
const WALKING: &str = "langdurige_loopbeperking";
const HELP: &str = "continu_afhankelijk_van_hulp_bestuurder";
const WHEELCHAIR: &str = "permanent_rolstoelgebonden";
const OTHER: &str = "ernstige_beperkingen_andere_dan_loopbeperkingen";
/// The college's own judgements on them without an advice: open terms of
/// its policy (art. 6, lid 2, RFC-048 §2).
const WALKING_C: &str = "langdurige_loopbeperking_volgens_college";
const HELP_C: &str = "continu_afhankelijk_van_hulp_bestuurder_volgens_college";
const WHEELCHAIR_C: &str = "permanent_rolstoelgebonden_volgens_college";
const OTHER_C: &str = "ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college";

const ADMISSIBLE: &str = "bezwaar_ontvankelijk";
const CAUSE: &str = "heroverweging_geeft_aanleiding";
const REVOKED: &str = "bestreden_besluit_herroepen";
const NEW: &str = "nieuw_besluit";

/// The BABW, the Regeling, the Awb and the municipality's policy from the
/// corpus.
fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let laws = [
        format!("regulation/nl/amvb/{LAW}"),
        "regulation/nl/ministeriele_regeling/regeling_gehandicaptenparkeerkaart".to_string(),
        format!("regulation/nl/wet/{AWB}"),
        format!("regulation/nl/uitvoeringsbeleid/{POLICY}"),
        // The reductions of every issuing body and of the GGD, each with
        // its cell (RFC-048).
        common::reduction_dir(REDUCTION),
        common::reduction_dir(
            "reductie_eerder_verstrekte_gehandicaptenparkeerkaart_gemeente_oostmeer",
        ),
        common::reduction_dir("reductie_eerder_verstrekte_gehandicaptenparkeerkaart_cbr"),
        common::reduction_dir(REPORTS),
        common::reduction_dir(ADVICE_REDUCTION),
        format!("regulation/nl/uitvoeringsbeleid/{ADVICE_POLICY}"),
        // Which decisions still stand after objection and appeal: the
        // reduction of the Awb, which the reductions above call.
        common::reduction_dir("reductie_algemene_wet_bestuursrecht"),
    ];
    common::load(&mut service, laws);
    service
}

/// The municipality's cell directory, with its catalogue of answers
/// (RFC-048).
fn cell_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{RECORDING_ACTOR}"))
}

/// The municipality's cell, with an empty chronicle.
fn cell() -> Cell {
    Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap()
}

/// A cell of Wilgendaal, for a helper whose own `cell` names another.
fn cell_of_wilgendaal() -> Cell {
    cell()
}

/// A complete application of `bsn` for a card as a driver, dated
/// `dagtekening`, with fictional data (BSNs from the test range).
fn application(bsn: &str, dagtekening: &str) -> Map<String, Value> {
    object_map(json!({
        "bsn_aanvrager": bsn,
        "kaart_als_bestuurder": true,
        "kaart_als_passagier": false,
        "aanvraag_ondertekend": true,
        "naam_aanvrager": "A. de Vries",
        "adres_aanvrager": "Voorbeeldlaan 2",
        "dagtekening": dagtekening,
        "gevraagde_beschikking": "gehandicaptenparkeerkaart",
    }))
}

const APPLICANT: &str = "999990020";

fn applied(cell: &mut Cell, bsn: &str, on: &str) -> Gram {
    cell.record_submission(
        &mut service(),
        &[&ggd()],
        LAW,
        ARTICLE,
        None,
        &application(bsn, on),
        day(on),
    )
    .unwrap()
}

/// The GGD records on `on` that it examined `bsn` on `examined_on` (art. 3
/// lid 1 of the Regeling), in its own chronicle.
fn reported(_municipality: &mut Cell, bsn: &str, examined_on: &str, on: &str) -> Gram {
    let supplied = object_map(json!({
        "bsn_aanvrager": bsn,
        "datum_onderzoek": examined_on,
        "verricht_door": "Gemeentelijke Gezondheidsdienst",
    }));
    GGD.with(|g| {
        g.borrow_mut()
            .record_executogram(&service(), EXAMINATION, &supplied, day(on))
            .unwrap()
    })
}

/// The college's findings after the medical examination: only a long-term
/// walking limitation, or none.
fn examined(walking: bool) -> Map<String, Value> {
    object_map(json!({ WALKING_C: walking, HELP_C: false, WHEELCHAIR_C: false, OTHER_C: false }))
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
        &[&ggd()],
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

/// Apply on `on`, receive the GGD's report of an examination that day, and
/// decide.
fn apply_and_decide(cell: &mut Cell, bsn: &str, walking: bool, on: &str) -> Gram {
    let app = applied(cell, bsn, on);
    reported(cell, bsn, on, on);
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
fn the_decision_takes_the_application_and_the_cell_and_the_policy_says_what_the_college_finds() {
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
    let asked: Vec<(&str, &Source)> = shape
        .inputs
        .iter()
        .map(|i| (i.field.name.as_str(), &i.source))
        .collect();
    assert_eq!(
        asked,
        vec![
            ("bsn_aanvrager", &Source::Application),
            ("kaart_als_bestuurder", &Source::Application),
            ("kaart_als_passagier", &Source::Application),
            // The day of the decision (a parameter of art. 49 that the Awb
            // stage also needs) and the day it is made known, both from the
            // cell.
            ("besluit_datum", &Source::Cell),
            // The date of the application (Awb 4:2): an examination counts
            // for it from that day (art. 2 of the Regeling, own choice).
            ("dagtekening", &Source::Application),
            ("bekendmaking_datum", &Source::Cell),
        ]
    );
    // The four criteria and the college's power: the municipality's policy
    // fills them, so no finding takes their place (RFC-048 §2).
    let college = cell();
    let by = college.authority();
    for name in [WALKING, HELP, WHEELCHAIR, OTHER, POWER] {
        assert!(shape.filled(name), "{name}");
        assert!(shape.finding(name, by).is_none(), "{name}");
    }
    // The findings: art. 2 lid 2 of the Regeling (whether to leave the
    // examination out, and the judgements under a and b), and where the
    // policy leaves a case to the college itself: the criteria without an
    // advice (policy art. 6, lid 2) and a valid card without an answer of
    // the cell (policy art. 2).
    let mut findings: Vec<&str> = shape
        .terms
        .iter()
        .filter(|t| t.is_finding(by))
        .map(|t| t.id.as_str())
        .collect();
    findings.sort_unstable();
    let mut expected = vec![
        LEFT_OUT,
        KNOWN,
        JUDGED,
        "langdurige_loopbeperking_volgens_college",
        "continu_afhankelijk_van_hulp_bestuurder_volgens_college",
        "permanent_rolstoelgebonden_volgens_college",
        "ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college",
        "aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college",
    ];
    expected.sort_unstable();
    assert_eq!(findings, expected);
    // Each is the college's: the GGD answers none of them (RFC-002), so a
    // finding of hers on them would be refused.
    let ggd = ggd();
    assert!(shape.terms.iter().all(|t| !t.is_finding(ggd.authority())));
    // The college may leave the three of lid 2 out: then it does not leave
    // the examination out.
    for name in [LEFT_OUT, KNOWN, JUDGED] {
        let term = shape.finding(name, by).unwrap();
        assert!(
            regelrecht_cel::decision::finding_field(term).nullable,
            "{name}"
        );
    }
    // Whether the examination took place is no input of art. 49: the
    // Regeling asks it, and in the cell the reduction answers it there.
    assert!(shape.input(EXAMINED).is_none());
    // Whether the applicant holds a valid card is no input of art. 49: the
    // policy asks it, and in the cell the reduction answers it there.
    assert!(shape.input(VALID).is_none());
}

#[test]
fn a_driver_with_a_long_term_walking_limitation_gets_a_card_for_five_years() {
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
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
    assert_eq!(gram.outputs["termijn_einddatum"], json!("2026-12-01"));
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
    assert_eq!(gram.outputs["termijn_weken"], json!(6));
}

#[test]
fn the_college_judges_the_criteria_itself() {
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut partial = examined(true);
    partial.remove(OTHER_C);
    // Without an advice the policy is silent, and the engine misses the
    // college's judgement (policy art. 6 lid 2).
    let e = decide_with(&mut cell, &app.id, &partial, "2026-10-20").unwrap_err();
    assert!(
        e.to_string()
            .contains(&format!("Required open term '{OTHER_C}'"))
            && e.to_string().contains("no judgement was given"),
        "{e}"
    );
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn a_finding_of_the_college_on_a_term_its_policy_fills_is_asked_by_none() {
    // The college finds that the applicant holds a valid card. Its policy
    // fills that term with the cell's answer, so no finding of the college
    // takes its place (RFC-048 §2); without one the cell's answer counts:
    // there is no card in its chronicle, and the card is granted.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    let mut given = examined(true);
    given.insert(VALID.into(), json!(true));
    let e = decide_with(&mut cell, &app.id, &given, "2026-10-20").unwrap_err();
    assert!(e.to_string().contains("is asked by none"), "{e}");
    let gram = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let found = cell.findings(&mut service(), &[&ggd()], &gram.id).unwrap();
    assert!(!found.contains(&VALID_C.to_string()), "{found:?}");
    assert!(found.contains(&WALKING_C.to_string()), "{found:?}");
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
fn a_cell_lists_the_answers_it_gives_and_the_terms_they_fill() {
    // The answers of the municipality's reduction: whether the applicant
    // holds a valid card, and whether a card was issued before (Regeling
    // art. 2 lid 2, RFC-052). The examination is the GGD's, in a cell of its
    // own. The catalogue also gives the count of the bedrijfsparkeervergunning:
    // a cell's answers are the cell's, whatever case asks. The service here
    // loads none of the rulework that binds it, so it fills no term.
    let cell = cell();
    let answers: Vec<(&str, &str)> = cell
        .answers()
        .iter()
        .map(|(answer, reduction)| (answer.as_str(), reduction.as_str()))
        .collect();
    assert_eq!(
        answers,
        [
            (PERMITS, PERMITS_REDUCTION),
            (EARLIER, REDUCTION),
            (VALID, REDUCTION)
        ]
    );
    let terms: Vec<(String, String)> = cell
        .bound_terms(&service(), day("2026-10-20"))
        .into_iter()
        .map(|t| (t.law, t.open_term))
        .collect();
    assert_eq!(
        terms,
        [
            (POLICY.into(), VALID.into()),
            (REGELING.into(), EARLIER.into())
        ]
    );
    // The GGD's answer fills the examination; the policy binds it too.
    let ggd: Vec<String> = ggd()
        .bound_terms(&service(), day("2026-10-20"))
        .into_iter()
        .map(|t| t.open_term)
        .collect();
    let mut expected = vec![EXAMINED];
    expected.extend(ADVICE);
    assert_eq!(ggd, expected);
}

// ---------------------------------------------------------------------------
// The objection and the decision on it (Awb 7:11, RFC-050)
// ---------------------------------------------------------------------------

/// A complete objection on the ground `ground`, and `with`.
fn objection(ground: &str, with: &[(&str, Value)]) -> Map<String, Value> {
    let mut supplied = object_map(json!({
        "bezwaar_of_beroepschrift_ondertekend": true,
        "naam_indiener": "A. de Vries",
        "adres_indiener": "Voorbeeldlaan 2",
        "dagtekening_bezwaar_of_beroepschrift": "2026-11-01",
        "omschrijving_bestreden_besluit": "besluit op de aanvraag om een gehandicaptenparkeerkaart",
        "gronden_bezwaar_of_beroep": ground,
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
        &[&ggd()],
        AWB,
        OBJECTION,
        Some(Reference::Local(contested)),
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
    reconsider_in(cell, &[&ggd()], objection, findings, on)
}

/// [`reconsider`] with `sphere` as the other cells.
fn reconsider_in(
    cell: &mut Cell,
    sphere: &[&Cell],
    objection: &str,
    findings: &Map<String, Value>,
    on: &str,
) -> Vec<Gram> {
    cell.decide(
        &mut service(),
        sphere,
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
    object_map(json!({ WALKING_C: judged }))
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
    // The application and the refusal; the examination is in the GGD's
    // chronicle.
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
    assert_eq!(bezwaar.fields["termijn_einddatum"], json!("2026-12-01"));
    assert_eq!(
        bezwaar.outputs["bezwaar_of_beroepschrift_tijdig_ingediend"],
        json!(true)
    );

    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided.len(), 1);
    let decision = &decided[0];
    assert_eq!(decision.basis, "algemene_wet_bestuursrecht#7:11");
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
    assert_eq!(new.basis, format!("{LAW}#{ARTICLE}"));
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

/// The fictional court, and the fact that a party holds the copy of its
/// judgment (Awb 8:79).
const COURT: &str = "rechtbank_wilgendaal";
const JUDGMENTS: &str = "algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak";

/// The court's judgement on a well-founded appeal: Awb 7:12 lid 1 is
/// breached, the legal effects do not stay, and the college gets two weeks.
fn annulled() -> Map<String, Value> {
    object_map(json!({
        "indiener_is_belanghebbende": true,
        "rechtsregel_geschonden": true,
        "geschonden_rechtsregel": "artikel 7:12, eerste lid, van de Algemene wet bestuursrecht",
        "rechtsgevolgen_in_stand": false,
        "uitspraak_treedt_in_de_plaats": false,
        "termijn_nieuw_besluit": "2027-01-25",
    }))
}

/// The court judges on 2027-01-11 the appeal against the decision on the
/// objection `on_objection` of `cell`, with `findings`, and `cell` records
/// the receipt of the copy the next day (Awb 8:79). The judgment.
fn judged_by_the_court(
    cell: &mut Cell,
    on_objection: &Gram,
    findings: &Map<String, Value>,
) -> Gram {
    judged_by_the_court_in(cell, &[&ggd()], on_objection, findings)
}

/// [`judged_by_the_court`], where `cell` takes `sphere` along.
fn judged_by_the_court_in(
    cell: &mut Cell,
    sphere: &[&Cell],
    on_objection: &Gram,
    findings: &Map<String, Value>,
) -> Gram {
    let mut court = Cell::in_memory(
        COURT,
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{COURT}")),
    )
    .unwrap();
    let copy = cell
        .afschrift(&mut service(), sphere, &on_objection.id)
        .unwrap();
    let appeal = court
        .record_submission(
            &mut service(),
            &[],
            AWB,
            "8:70",
            Some(Reference::Received(&copy)),
            &object_map(json!({
                "bezwaar_of_beroepschrift_ondertekend": true,
                "naam_indiener": "A. de Vries",
                "adres_indiener": "Voorbeeldlaan 2",
                "dagtekening_bezwaar_of_beroepschrift": "2026-11-30",
                "omschrijving_bestreden_besluit": "beslissing op bezwaar tegen de kaart",
                "gronden_bezwaar_of_beroep": "de beslissing is niet deugdelijk gemotiveerd",
            })),
            day("2026-12-01"),
        )
        .unwrap();
    let judgment = court
        .decide(
            &mut service(),
            &[],
            AWB,
            "8:70",
            &appeal.id,
            &Map::new(),
            findings,
            day("2027-01-11"),
        )
        .unwrap()
        .remove(0);
    let copy = court.afschrift(&mut service(), &[], &judgment.id).unwrap();
    cell.record_receipt(
        &service(),
        JUDGMENTS,
        &copy,
        &on_objection.id,
        day("2027-01-12"),
    )
    .unwrap();
    judgment
}

#[test]
fn a_card_whose_revocation_the_court_annulled_counts_again() {
    // As above, the grant is revoked on objection. The court annuls that
    // decision on the objection with its legal effects (Awb 8:72): the
    // revocation no longer holds, nor the refusal in its place, and the card
    // is valid again.
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

    let judgment = judged_by_the_court(&mut cell, &decided[0], &annulled());
    assert_eq!(judgment.outputs["rechtsgevolgen_vernietigd"], json!(true));

    // A second application while the revived card is valid is refused.
    let later = apply_and_decide(&mut cell, APPLICANT, true, "2027-03-01");
    assert_eq!(later.fields[POWER], json!(false));
    assert_eq!(
        later.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
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
// The cell's answer in the trace (RFC-048 §5)
// ---------------------------------------------------------------------------

/// The nodes of `node` and below where a cell answered an open term,
/// besides the GGD's advice. Every decision on the criteria asks the GGD for
/// its advice (policy art. 5 and 6); the tests below are about the other
/// terms, and the advice has tests of its own.
fn answered(node: &regelrecht_engine::PathNode) -> Vec<&regelrecht_engine::PathNode> {
    common::answered(node)
        .into_iter()
        .filter(|n| !ADVICE.contains(&n.name.as_str()))
        .collect()
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
    // reduction, its version and how far it read the chronicle. The policy
    // refuses, so art. 2 of the Regeling is not asked.
    let result = cell.explain(&mut service(), &[&ggd()], &second.id).unwrap();
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
    let second_application = &cell.grams()[2];
    assert_eq!(second_application.kind, "AANVRAAG");
    assert_eq!(
        answer.last_record.as_deref(),
        Some(second_application.id.as_str())
    );
    // The reduction's FOREACH is not in the decision's trace.
    let trace = root.render_box_drawing();
    assert!(!trace.contains("FOREACH"), "{trace}");
    // The report says the same: the cell answered the policy's term.
    assert!(
        result.open_terms.iter().any(|f| f.term == VALID
            && f.law_id == POLICY
            && matches!(&f.filled_by, regelrecht_engine::FilledBy::Answer { law_id, article, answers }
                if law_id == POLICY && article == "2" && answers == std::slice::from_ref(answer))),
        "{:?}",
        result.open_terms
    );

    // Step 2: asked why, the cell runs its reduction again with its own
    // trace, which shows the first card it counted.
    let explained = cell
        .explain_answer(&mut service(), &[&ggd()], &second.id, VALID)
        .unwrap();
    assert_eq!(explained.len(), 1);
    assert_eq!(
        explained[0].outputs[VALID],
        regelrecht_engine::Value::from(json!(true))
    );
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&first.id), "{why}");

    // Nothing stays behind in the service.
    let mut kept = service();
    let laws = kept.law_count();
    cell.explain_answer(&mut kept, &[&ggd()], &second.id, VALID)
        .unwrap();
    assert_eq!(kept.law_count(), laws);
}

#[test]
fn on_a_refusal_on_the_criteria_the_cell_is_not_asked() {
    // Art. 49 reads the college's power only for an applicant who meets the
    // criteria (AND stops at the first false), so the policy and its term
    // are not read, and the cell answers nothing.
    let mut cell = cell();
    let refused = apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let result = cell
        .explain(&mut service(), &[&ggd()], &refused.id)
        .unwrap();
    assert!(answered(result.trace.as_ref().unwrap()).is_empty());
    let e = cell
        .explain_answer(&mut service(), &[&ggd()], &refused.id, VALID)
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
    // everything before this decision but the contested grant. Art. 49
    // would issue the card again, so it asks art. 2 of the Regeling too,
    // and the cell answers from the report it received for the grant.
    let result = cell
        .explain(&mut service(), &[&ggd()], &decided[0].id)
        .unwrap();
    let root = result.trace.as_ref().unwrap();
    let all = answered(root);
    let names: Vec<&str> = all.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, [VALID, EXAMINED], "{}", root.render_box_drawing());
    assert_eq!(
        all[1].answer.as_ref().unwrap().law_id,
        REPORTS,
        "{}",
        root.render_box_drawing()
    );
    let nodes = &all[..1];
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
        .explain_answer(&mut service(), &[&ggd()], &decided[0].id, VALID)
        .unwrap();
    let why = explained[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(!why.contains(&granted.id), "{why}");
}

// ---------------------------------------------------------------------------
// The medical examination (art. 2 and 3 of the Regeling)
// ---------------------------------------------------------------------------

/// The college's findings after the examination, with what it decides on
/// lid 2: whether it leaves the examination out, and the judgements under
/// a and b.
fn lid_2(left_out: bool, known: bool, judged: bool) -> Map<String, Value> {
    let mut findings = examined(true);
    findings.insert(LEFT_OUT.into(), json!(left_out));
    findings.insert(KNOWN.into(), json!(known));
    findings.insert(JUDGED.into(), json!(judged));
    findings
}

/// The decision fails on the term `term`: nobody answered it, and nothing
/// is recorded.
fn blocked_on(e: &Error, term: &str) {
    assert!(
        matches!(e, Error::Engine(regelrecht_engine::EngineError::ResolutionError(m))
            if m.contains(&format!("'{term}'")) && m.contains("no judgement was given")),
        "{e}"
    );
}

#[test]
fn without_a_report_the_college_cannot_issue_the_card_yet() {
    // Art. 2 lid 1: "wordt niet afgegeven alvorens een geneeskundig onderzoek
    // heeft plaatsgehad". Not a ground for refusal: the decision waits for
    // the report, and the cell records nothing.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);
    assert_eq!(cell.grams().len(), 1);

    // Once the GGD has recorded the examination, the card is issued; the
    // GGD's cell answered the term of art. 3 (RFC-052).
    let report = reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    assert_eq!(report.kind, "EXECUTOGRAM");
    assert_eq!(report.basis, format!("{REGELING}#3"));
    assert_eq!(
        report.fact.as_deref(),
        Some("geneeskundig_onderzoek_verricht")
    );
    let gram = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let result = cell.explain(&mut service(), &[&ggd()], &gram.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let node = answered(root)
        .into_iter()
        .find(|n| n.name == EXAMINED)
        .unwrap_or_else(|| panic!("{}", root.render_box_drawing()));
    assert_eq!(
        node.result,
        Some(regelrecht_engine::Value::from(json!(true)))
    );
    let answer = node.answer.as_ref().unwrap();
    assert_eq!(answer.answerer, GGD_ACTOR);
    assert_eq!(answer.law_id, REPORTS);
    assert_eq!(answer.last_record.as_deref(), Some(report.id.as_str()));
    // The municipality did not answer it, so it has nothing to explain;
    // the GGD's reasoning is its own.
    let e = cell
        .explain_answer(&mut service(), &[&ggd()], &gram.id, EXAMINED)
        .unwrap_err();
    assert!(e.to_string().contains("did not ask the cell"), "{e}");
}

#[test]
fn a_refusal_does_not_wait_for_the_examination() {
    // Art. 2 lid 1 is about issuing a card. Art. 49 asks it only for a card
    // it would issue, so a refusal on the policy needs no report.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2027-03-01");
    let second = decide_with(&mut cell, &app.id, &examined(true), "2027-03-01").unwrap();
    assert_eq!(second.fields[POWER], json!(false));
    assert_eq!(
        second.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn only_an_examination_of_the_applicant_for_this_application_counts() {
    // Own choice: from the date of the application (Awb 4:2) up to the day
    // of issue. Not before: lid 2 onder a en b would mean nothing if an
    // examination for an earlier card still counted.
    let mut cell = cell();
    reported(&mut cell, APPLICANT, "2026-10-01", "2026-10-02");
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, "999990032", "2026-10-16", "2026-10-16");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);

    // An examination after the day of issue does not count either. The cell
    // records the report on the day it receives it, and the examination
    // date in it is the GGD's; it does not check that the examination came
    // before the report, so this constructed report tests the upper bound.
    reported(&mut cell, APPLICANT, "2026-10-25", "2026-10-19");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);

    // On the date of the application, it counts.
    reported(&mut cell, APPLICANT, "2026-10-13", "2026-10-19");
    let gram = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn the_cell_refuses_a_report_that_is_not_whole() {
    let partial =
        object_map(json!({ "bsn_aanvrager": APPLICANT, "datum_onderzoek": "2026-10-16" }));
    GGD.with(|ggd| {
        let mut ggd = ggd.borrow_mut();
        let e = ggd
            .record_executogram(&service(), EXAMINATION, &partial, day("2026-10-19"))
            .unwrap_err();
        assert!(e.to_string().contains("verricht_door"), "{e}");
        assert!(ggd.grams().is_empty());
    });
}

#[test]
fn only_who_examines_records_the_examination() {
    // Regeling art. 3 lid 1: the GGD or the designated expert. The
    // municipality receives the report; it does not record the examination.
    let mut cell = cell();
    let whole = object_map(json!({
        "bsn_aanvrager": APPLICANT,
        "datum_onderzoek": "2026-10-16",
        "verricht_door": "GGD Wilgendaal",
    }));
    let e = cell
        .record_executogram(&service(), EXAMINATION, &whole, day("2026-10-19"))
        .unwrap_err();
    assert!(
        e.to_string().contains("is established by 'Gemeentelijke Gezondheidsdienst' or 'vanwege het gemeentelijk gezag aangewezen deskundige'"),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn lid_2_onder_a_an_earlier_card_and_the_college_knows_the_applicant_still_qualifies() {
    // The card of 2026 has expired. The college knows the applicant still
    // meets the criteria and leaves the examination out: no report needed.
    // That the applicant had a card before, the cell answers from its own
    // decisions.
    let mut cell = cell();
    let first = apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2031-10-20");
    let gram = decide_with(&mut cell, &app.id, &lid_2(true, true, false), "2031-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(gram.fields[LEFT_OUT], json!(true));
    assert_eq!(gram.fields[KNOWN], json!(true));

    let result = cell.explain(&mut service(), &[&ggd()], &gram.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    let names: Vec<&str> = answered(root).iter().map(|n| n.name.as_str()).collect();
    // The examination is not asked: lid 2 decides first.
    assert_eq!(names, [VALID, EARLIER], "{}", root.render_box_drawing());
    let why = cell
        .explain_answer(&mut service(), &[&ggd()], &gram.id, EARLIER)
        .unwrap();
    let why = why[0].trace.as_ref().unwrap().render_box_drawing();
    assert!(why.contains(&first.id), "{why}");
}

#[test]
fn lid_2_onder_b_the_judgement_of_the_examining_body() {
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2031-10-20");
    let gram = decide_with(&mut cell, &app.id, &lid_2(true, false, true), "2031-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn lid_2_leaving_the_examination_out_without_its_ground_falls_back_on_lid_1() {
    // Neither a nor b: the college may not leave it out, so the card waits
    // for a report.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, true, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2031-10-20");
    let e = decide_with(&mut cell, &app.id, &lid_2(true, false, false), "2031-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);
    // The college that knows but does not leave it out, waits too.
    let e = decide_with(&mut cell, &app.id, &lid_2(false, true, true), "2031-10-20").unwrap_err();
    blocked_on(&e, EXAMINED_C);
}

#[test]
fn lid_2_without_an_earlier_card_in_the_chronicle_the_cell_is_silent() {
    // A card from another municipality counts too, so the cell does not say
    // there was none: the college would have to find it, and without that
    // finding the decision fails. A refused or revoked card is no card
    // issued.
    let mut cell = cell();
    apply_and_decide(&mut cell, APPLICANT, false, "2026-10-20");
    let app = applied(&mut cell, APPLICANT, "2027-03-01");
    let e = decide_with(&mut cell, &app.id, &lid_2(true, true, false), "2027-03-01").unwrap_err();
    blocked_on(&e, EARLIER_C);
}

#[test]
fn without_a_report_the_college_may_establish_the_examination_itself() {
    // The term is the college's to decide per case: the cell is silent, so
    // the college's own finding counts. Finding that no examination took
    // place (the applicant did not appear, say) refuses the card: own
    // choice, a decision of the college and no consequence of lid 1.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut given = examined(true);
    given.insert(EXAMINED_C.into(), json!(false));
    let refused = decide_with(&mut cell, &app.id, &given, "2026-10-20").unwrap();
    assert_eq!(
        refused.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );

    let app = applied(&mut cell, "999990032", "2026-10-21");
    given.insert(EXAMINED_C.into(), json!(true));
    let granted = decide_with(&mut cell, &app.id, &given, "2026-10-21").unwrap();
    assert_eq!(
        granted.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let found = cell
        .findings(&mut service(), &[&ggd()], &granted.id)
        .unwrap();
    assert!(found.contains(&EXAMINED_C.to_string()), "{found:?}");
}

// Synthesis (RFC-052): whether a card was issued before is a fact of every
// issuing body (Regeling art. 2 lid 2: "de verstrekkende instantie"), and
// each answers from its own chronicle.

/// Another issuing body: the fictitious municipality of Oostmeer, or the CBR.
fn issuer(actor: &str) -> Cell {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/cells/{actor}"));
    Cell::in_memory(actor, dir).unwrap()
}

/// `issuer` issues `bsn` a card on `on`, after an examination and an advice
/// of the GGD. The policy of Wilgendaal is unscoped (it says so), so the
/// other body decides under it. It gives no finding on that policy's own
/// terms: the college of Wilgendaal, its holder, answers those. So the
/// cells answer them all: the GGD the examination and the advice, the
/// cell of Wilgendaal, without a card yet, whether the applicant holds one.
fn issued_by(issuer: &mut Cell, bsn: &str, on: &str) -> Gram {
    let app = applied(issuer, bsn, on);
    reported(issuer, bsn, on, on);
    advised(bsn, on, true, on);
    let wilgendaal = cell();
    let gram = issuer
        .decide(
            &mut service(),
            &[&ggd(), &wilgendaal],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &Map::new(),
            day(on),
        )
        .map(|mut grams| grams.remove(0))
        .unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    gram
}

/// The college of Wilgendaal decides with `sphere` and leaves the
/// examination out because it knows the applicant still qualifies (Regeling
/// art. 2 lid 2 onder a), which needs an earlier card.
fn without_examination(
    cell: &mut Cell,
    sphere: &[&Cell],
    application: &str,
    on: &str,
) -> Result<Gram, Error> {
    let mut findings = examined(true);
    findings.insert(LEFT_OUT.into(), json!(true));
    findings.insert(KNOWN.into(), json!(true));
    cell.decide(
        &mut service(),
        sphere,
        LAW,
        ARTICLE,
        application,
        &Map::new(),
        &findings,
        day(on),
    )
    .map(|mut grams| grams.remove(0))
}

/// Who filled whether a card was issued before, in the decision `id`.
fn earlier_filled_by(cell: &Cell, sphere: &[&Cell], id: &str) -> regelrecht_engine::FilledBy {
    cell.explain(&mut service(), sphere, id)
        .unwrap()
        .open_terms
        .into_iter()
        .find(|f| f.term == EARLIER)
        .map(|f| f.filled_by)
        .expect("the term was filled")
}

#[test]
fn a_card_of_another_municipality_counts() {
    let mut oostmeer = issuer("gemeente_oostmeer");
    issued_by(&mut oostmeer, "999990044", "2026-01-15");
    let mut cell = cell();
    let app = applied(&mut cell, "999990044", "2026-10-13");
    let gram = without_examination(&mut cell, &[&ggd(), &oostmeer], &app.id, "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let regelrecht_engine::FilledBy::Answer { answers, .. } =
        earlier_filled_by(&cell, &[&ggd(), &oostmeer], &gram.id)
    else {
        panic!("one answer")
    };
    let [by] = answers.as_slice() else {
        panic!("one answer: {answers:?}")
    };
    assert_eq!(by.answerer, "gemeente_oostmeer");
}

#[test]
fn a_card_of_the_cbr_counts() {
    let mut cbr = issuer("cbr");
    issued_by(&mut cbr, "999990056", "2026-02-02");
    let mut cell = cell();
    let app = applied(&mut cell, "999990056", "2026-10-13");
    let gram = without_examination(&mut cell, &[&ggd(), &cbr], &app.id, "2026-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let regelrecht_engine::FilledBy::Answer { answers, .. } =
        earlier_filled_by(&cell, &[&ggd(), &cbr], &gram.id)
    else {
        panic!("one answer")
    };
    let [by] = answers.as_slice() else {
        panic!("one answer: {answers:?}")
    };
    assert_eq!(by.answerer, "cbr");
}

#[test]
fn cards_of_two_issuers_combine_with_or() {
    // Wilgendaal issued a card in 2026 and Oostmeer one before; after the
    // card of Wilgendaal expired, both say true. Without `combine: OR` on
    // the term of Regeling art. 2 the two answers would be an error.
    let mut oostmeer = issuer("gemeente_oostmeer");
    issued_by(&mut oostmeer, "999990044", "2026-01-15");
    let mut cell = cell();
    let app = applied(&mut cell, "999990044", "2026-10-13");
    without_examination(&mut cell, &[&ggd(), &oostmeer], &app.id, "2026-10-20").unwrap();
    let again = applied(&mut cell, "999990044", "2031-10-20");
    let gram =
        without_examination(&mut cell, &[&ggd(), &oostmeer], &again.id, "2031-10-20").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let regelrecht_engine::FilledBy::Answer { answers: all, .. } =
        earlier_filled_by(&cell, &[&ggd(), &oostmeer], &gram.id)
    else {
        panic!("two answers")
    };
    let names: Vec<&str> = all.iter().map(|p| p.answerer.as_str()).collect();
    assert_eq!(names, ["gemeente_oostmeer", RECORDING_ACTOR]);
}

/// In `cell`, a refusal of `bsn` on 2026-10-20 that the objection leaves
/// standing; the court then decides the case itself (Awb 8:72 lid 3 onder
/// b): it revokes the refusal in the decision on the objection it sets in
/// place, and grants the card in a new decision on the application. It
/// gives the facts the cell would otherwise answer (Awb 8:69 lid 3). The
/// card the court granted, as the decision value inside its judgment.
fn granted_by_the_court(cell: &mut Cell, bsn: &str) -> Value {
    // Wilgendaal decides with its own findings; another issuer gives none on
    // the policy of Wilgendaal, so the cells answer them: the GGD the
    // examination and the advice, the cell of Wilgendaal the card.
    let other = cell.recording_actor() != RECORDING_ACTOR;
    let app = applied(cell, bsn, "2026-10-20");
    reported(cell, bsn, "2026-10-20", "2026-10-20");
    if other {
        advised(bsn, "2026-10-20", false, "2026-10-20");
    }
    let wilgendaal = other.then(cell_of_wilgendaal);
    let mut sphere = vec![ggd()];
    sphere.extend(wilgendaal);
    let sphere: Vec<&Cell> = sphere.iter().collect();
    let refused = if !other {
        decide(cell, &app.id, false, "2026-10-20")
    } else {
        cell.decide(
            &mut service(),
            &sphere,
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &Map::new(),
            day("2026-10-20"),
        )
        .unwrap()
        .remove(0)
    };
    let bezwaar = object(
        cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    let decided = reconsider_in(cell, &sphere, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided[0].outputs[REVOKED], json!(false));
    // The court decides in the place of the college (Awb 8:72 lid 3 onder
    // b), without the cells of the college: so it gives the college's own
    // findings, those the policy leaves to the college without an answer.
    let mut findings = annulled();
    findings.remove("termijn_nieuw_besluit");
    findings.insert("uitspraak_treedt_in_de_plaats".into(), json!(true));
    findings.extend(examined(true));
    findings.insert(VALID_C.into(), json!(false));
    findings.insert(EXAMINED_C.into(), json!(true));
    let judgment = judged_by_the_court_in(cell, &sphere, &decided[0], &findings);
    let instead = &judgment.outputs["uitspraak_in_de_plaats"];
    assert_eq!(instead["outputs"][REVOKED], json!(true));
    let grant = instead["outputs"][NEW].clone();
    assert_eq!(grant["basis"], json!(format!("{LAW}#{ARTICLE}")));
    assert_eq!(grant["fields"]["bsn_aanvrager"], json!(bsn));
    assert_eq!(
        grant["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    grant
}

#[test]
fn a_card_the_court_granted_counts_as_an_earlier_card_but_not_as_a_valid_one() {
    let mut cell = cell();
    granted_by_the_court(&mut cell, APPLICANT);
    let app = applied(&mut cell, APPLICANT, "2027-03-01");

    // Whether that card is still valid the cell does not know: the decision
    // the court set in place carries no kaart_geldig_tot. It is silent, so
    // the college judges, and without that finding the decision fails.
    let e = without_examination(&mut cell, &[&ggd()], &app.id, "2027-03-01").unwrap_err();
    blocked_on(&e, VALID_C);

    // The college finds the card no longer valid. That a card was issued
    // before, the cell answers from the judgment, so the examination may be
    // left out (Regeling art. 2 lid 2 onder a).
    let mut findings = examined(true);
    findings.insert(LEFT_OUT.into(), json!(true));
    findings.insert(KNOWN.into(), json!(true));
    findings.insert(VALID_C.into(), json!(false));
    let gram = decide_with(&mut cell, &app.id, &findings, "2027-03-01").unwrap();
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    let regelrecht_engine::FilledBy::Answer { answers, .. } =
        earlier_filled_by(&cell, &[&ggd()], &gram.id)
    else {
        panic!("one answer")
    };
    let [by] = answers.as_slice() else {
        panic!("one answer: {answers:?}")
    };
    assert_eq!(by.answerer, RECORDING_ACTOR);
}

#[test]
fn a_card_the_court_granted_in_place_of_another_issuer_counts_there() {
    // One applicant per issuer: the GGD of these tests advises each once.
    for (actor, bsn) in [("gemeente_oostmeer", "999990044"), ("cbr", "999990056")] {
        let mut other = issuer(actor);
        granted_by_the_court(&mut other, bsn);
        let mut cell = cell();
        let app = applied(&mut cell, bsn, "2027-03-01");
        let gram =
            without_examination(&mut cell, &[&ggd(), &other], &app.id, "2027-03-01").unwrap();
        assert_eq!(
            gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(true),
            "{actor}"
        );
        let regelrecht_engine::FilledBy::Answer { answers, .. } =
            earlier_filled_by(&cell, &[&ggd(), &other], &gram.id)
        else {
            panic!("one answer from {actor}")
        };
        let [by] = answers.as_slice() else {
            panic!("one answer: {answers:?}")
        };
        assert_eq!(by.answerer, actor);
    }
}

#[test]
fn regeling_2_combines_the_answers_of_the_issuers_with_or() {
    let service = service();
    let law = service
        .resolver()
        .get_law_for_date(REGELING, Some(day("2026-10-20")))
        .unwrap();
    let term = law
        .find_article_by_number("2")
        .and_then(|a| a.get_open_terms())
        .and_then(|terms| terms.iter().find(|t| t.id == EARLIER))
        .expect("art. 2 declares the term");
    assert_eq!(
        term.combine,
        Some(regelrecht_engine::article::CombineOp::Or)
    );
}

#[test]
fn a_municipality_that_gives_the_examination_answer_is_not_asked_for_it() {
    // The examination is the GGD's (Regeling art. 3): the term names who is
    // competent, so the binding asks the GGD and not the municipality, even
    // where its catalogue gives the answer (RFC-052). Without the GGD the
    // college finds it.
    let dir = tempfile::tempdir().unwrap();
    let identity = std::fs::read_to_string(cell_dir().join("cel.yaml")).unwrap();
    common::write_identity(dir.path(), &format!("{identity}  {EXAMINED}: {REPORTS}\n"));
    let mut cell = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap();
    assert_eq!(cell.answers().len(), 4);
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let e = cell
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &examined(true),
            day("2026-10-20"),
        )
        .unwrap_err();
    assert!(
        e.to_string().contains(EXAMINED) && e.to_string().contains("no judgement was given"),
        "{e}"
    );
}

// ---------------------------------------------------------------------------
// The advice of the GGD (fictitious policy of the municipality, art. 5 and 6)
// ---------------------------------------------------------------------------

/// The GGD records on `on` its advice on `bsn`, of an examination on
/// `examined_on`: only a long-term walking limitation, or none.
fn advised(bsn: &str, examined_on: &str, walking: bool, on: &str) -> Gram {
    let supplied = object_map(json!({
        "bsn_aanvrager": bsn,
        "datum_onderzoek": examined_on,
        WALKING: walking,
        HELP: false,
        WHEELCHAIR: false,
        OTHER: false,
    }));
    GGD.with(|g| {
        g.borrow_mut()
            .record_executogram(&service(), ADVISED, &supplied, day(on))
            .unwrap()
    })
}

/// Who filled `term` of art. 49 in the decision `id`.
fn filled_by(cell: &Cell, id: &str, term: &str) -> regelrecht_engine::FilledBy {
    let result = cell.explain(&mut service(), &[&ggd()], id).unwrap();
    result
        .open_terms
        .iter()
        .find(|f| f.term == term && f.law_id == LAW)
        .map(|f| f.filled_by.clone())
        .unwrap_or_else(|| panic!("{term}: {:?}", result.open_terms))
}

#[test]
fn an_advice_with_a_long_term_walking_limitation_grants_the_card() {
    // The college would find no limitation, but the advice does. The policy
    // fills the four judgements from the advice, and reads the college's own
    // judgement only without one (RFC-048 §2): the college cannot deviate
    // (Awb 4:84 is open).
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    advised(APPLICANT, "2026-10-16", true, "2026-10-19");
    let gram = decide(&mut cell, &app.id, false, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // The decision holds what the engine used: the advice.
    assert_eq!(gram.fields[WALKING], json!(true));
    assert_eq!(gram.fields[OTHER], json!(false));
    assert_eq!(
        filled_by(&cell, &gram.id, WALKING),
        regelrecht_engine::FilledBy::Regulation {
            law_id: POLICY.into(),
            article: "6".into()
        }
    );
    // The policy did not read the college's own judgement, so the decision
    // holds no finding on the walking limitation.
    let found = cell.findings(&mut service(), &[&ggd()], &gram.id).unwrap();
    assert!(!found.contains(&WALKING.to_string()), "{found:?}");
    assert!(!found.contains(&WALKING_C.to_string()), "{found:?}");
    // Asked why: in the trace, art. 7 asks the GGD's cell for the answer,
    // which its reduction gave from its advice.
    let result = cell.explain(&mut service(), &[&ggd()], &gram.id).unwrap();
    let root = result.trace.as_ref().unwrap();
    fn find<'a>(
        node: &'a regelrecht_engine::PathNode,
        name: &str,
    ) -> Option<&'a regelrecht_engine::PathNode> {
        if node.name == name && node.answer.is_some() {
            return Some(node);
        }
        node.children.iter().find_map(|c| find(c, name))
    }
    let node = find(root, ADVICE[0]).unwrap_or_else(|| panic!("{}", root.render_box_drawing()));
    let by = node.answer.as_ref().unwrap();
    assert_eq!(by.answerer, GGD_ACTOR);
    assert_eq!(by.answer, ADVICE[0]);
    assert_eq!(by.law_id, ADVICE_REDUCTION);
}

#[test]
fn an_advice_without_a_criterion_refuses_the_card() {
    // The college would find a limitation; the advice finds none, and the
    // advice decides.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    advised(APPLICANT, "2026-10-16", false, "2026-10-19");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    assert_eq!(gram.fields[WALKING], json!(false));
}

#[test]
fn without_an_advice_the_college_judges() {
    // The GGD is silent for this application, so the policy is too (policy
    // art. 6 lid 2), and the college's finding counts.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    // The policy fills the criterion, with the college's own judgement
    // where there is no advice (art. 6, lid 2).
    assert_eq!(
        filled_by(&cell, &gram.id, WALKING),
        regelrecht_engine::FilledBy::Regulation {
            law_id: POLICY.to_string(),
            article: "6".to_string(),
        }
    );
    let found = cell.findings(&mut service(), &[&ggd()], &gram.id).unwrap();
    assert!(found.contains(&WALKING_C.to_string()), "{found:?}");
}

#[test]
fn only_the_last_advice_for_this_application_counts() {
    // An advice from before the date of the application does not count, and
    // of two in the term the later one does (policy art. 5 lid 3).
    let mut cell = cell();
    advised(APPLICANT, "2026-10-01", true, "2026-10-02");
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-14", "2026-10-15");
    advised(APPLICANT, "2026-10-14", true, "2026-10-15");
    advised(APPLICANT, "2026-10-16", false, "2026-10-17");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
}

#[test]
fn an_advice_from_before_the_application_or_after_the_decision_does_not_count() {
    // Policy art. 5 lid 3: only an examination between the date of the
    // application and the day of the decision. Without one the policy is
    // silent and the college's finding counts.
    let mut cell = cell();
    advised(APPLICANT, "2026-10-01", false, "2026-10-02");
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-14", "2026-10-15");
    advised(APPLICANT, "2026-10-21", false, "2026-10-21");
    let gram = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        gram.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
    assert_eq!(
        filled_by(&cell, &gram.id, WALKING),
        regelrecht_engine::FilledBy::Regulation {
            law_id: POLICY.to_string(),
            article: "6".to_string(),
        }
    );
}

#[test]
fn two_advices_from_the_same_day_are_an_error() {
    // Which of two advices of one examination day counts, the policy does
    // not say: the GGD's policy (art. 1) refuses to pick (FOREACH combine
    // ONLY), and the decision is not taken.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    reported(&mut cell, APPLICANT, "2026-10-16", "2026-10-19");
    advised(APPLICANT, "2026-10-16", true, "2026-10-19");
    advised(APPLICANT, "2026-10-16", false, "2026-10-19");
    let e = decide_with(&mut cell, &app.id, &examined(true), "2026-10-20").unwrap_err();
    assert!(e.to_string().contains("ONLY"), "{e}");
    assert_eq!(cell.grams().len(), 1);
}

#[test]
fn only_who_examines_records_the_advice() {
    // The advice is a fact of who examines (policy art. 5): the
    // municipality cannot record it, and so cannot give itself an advice.
    let mut cell = cell();
    let supplied = object_map(json!({
        "bsn_aanvrager": APPLICANT,
        "datum_onderzoek": "2026-10-16",
        WALKING_C: true,
        HELP_C: false,
        WHEELCHAIR_C: false,
        OTHER_C: false,
    }));
    let e = cell
        .record_executogram(&service(), ADVISED, &supplied, day("2026-10-19"))
        .unwrap_err();
    assert!(
        e.to_string().contains("is established by 'Gemeentelijke Gezondheidsdienst' or 'vanwege het gemeentelijk gezag aangewezen deskundige'"),
        "{e}"
    );
    assert!(cell.grams().is_empty());
}

#[test]
fn on_objection_a_new_advice_revokes_the_refusal() {
    // The advice refuses; on objection the GGD examines again and finds the
    // limitation. The new advice counts at the heroverweging, which falls
    // later (policy art. 5 lid 3), and the refusal is revoked.
    let mut cell = cell();
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    advised(APPLICANT, "2026-10-16", false, "2026-10-19");
    let refused = decide(&mut cell, &app.id, true, "2026-10-20");
    assert_eq!(
        refused.outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(false)
    );
    let bezwaar = object(
        &mut cell,
        &refused.id,
        &objection("ik kan niet verder lopen dan vijftig meter", &[]),
        "2026-11-01",
    )
    .unwrap();
    reported(&mut cell, APPLICANT, "2026-11-10", "2026-11-11");
    advised(APPLICANT, "2026-11-10", true, "2026-11-11");
    let decided = reconsider(&mut cell, &bezwaar.id, &Map::new(), "2026-11-20");
    assert_eq!(decided[0].outputs[REVOKED], json!(true));
    assert_eq!(
        decided[1].outputs["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn a_municipality_that_gives_the_advice_answer_is_not_asked_for_it() {
    // The advice is the GGD's (policy art. 5): its terms name who is
    // competent, so the binding of art. 7 asks the GGD and not the
    // municipality, even where its catalogue gives the answer (RFC-051).
    // Without the GGD the college judges the criteria itself.
    let dir = tempfile::tempdir().unwrap();
    let identity = std::fs::read_to_string(cell_dir().join("cel.yaml")).unwrap();
    common::write_identity(
        dir.path(),
        &format!("{identity}  {}: {ADVICE_REDUCTION}\n", ADVICE[0]),
    );
    let mut cell = Cell::in_memory(RECORDING_ACTOR, dir.path()).unwrap();
    assert_eq!(cell.answers().len(), 4);
    let app = applied(&mut cell, APPLICANT, "2026-10-13");
    let mut findings = examined(true);
    findings.insert(EXAMINED_C.into(), json!(true));
    let gram = cell
        .decide(
            &mut service(),
            &[],
            LAW,
            ARTICLE,
            &app.id,
            &Map::new(),
            &findings,
            day("2026-10-20"),
        )
        .unwrap();
    let result = cell.explain(&mut service(), &[], &gram[0].id).unwrap();
    let walking = result
        .open_terms
        .iter()
        .find(|f| f.term == WALKING_C && f.law_id == POLICY)
        .unwrap();
    assert_eq!(walking.filled_by, regelrecht_engine::FilledBy::Finding);
    // Nowhere in the trace did the municipality give the advice.
    fn by_municipality(node: &regelrecht_engine::PathNode) -> bool {
        node.answer
            .as_ref()
            .is_some_and(|a| a.answer == ADVICE[0] && a.answerer == RECORDING_ACTOR)
            || node.children.iter().any(by_municipality)
    }
    let root = result.trace.as_ref().unwrap();
    assert!(!by_municipality(root), "{}", root.render_box_drawing());
}
