//! A decision that takes the answers of another cell along (RFC-048): the
//! records of a second cell, an examination and an advice, fill the terms of
//! the deciding cell's policy, beside the answer of a reduction over the
//! deciding cell's own decisions.
//!
//! The case: the college of a municipality grants a gehandicaptenparkeerkaart
//! on application (art. 49 of a fictional traffic decree after the BABW, with
//! the criteria of a fictional parking card regulation after the Regeling
//! gehandicaptenparkeerkaart, and the validity of art. 51), with a mini
//! general administrative act on it, a reduction over its own decisions and
//! the objection under Awb 7:11. Nothing in the cell knows this case: it
//! follows from the law, the municipality's policy and its reduction, as the
//! leave case of `tests/submission_lifecycle/` does.
//!
//! The medical examination is the GGD's (Regeling art. 3): a cell of its
//! own records it, and the municipality's decisions take that cell along
//! as their sphere (RFC-048). Each test has its own GGD ([`GGD`]). The GGD
//! also records its advice, with per criterion whether the applicant meets
//! it (the municipality's fictitious policy, art. 5); the policy fills the
//! college's four judgements from it (art. 6), and without an advice the
//! college judges.

// Test helpers outside a `#[test]` fn may unwrap and panic too; that is how
// a failing fixture reports itself.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod answer_bound_by_policy;
#[path = "../common/mod.rs"]
mod common;
mod decision_on_application;
mod fact_of_another_cell;
mod objection;
mod own_reduction;
mod trace;

use common::{day, object_map};

use std::path::{Path, PathBuf};

use regelrecht_cel::{Cell, Error, Gram, Reference, Source, decision, shape};
use regelrecht_engine::LawExecutionService;
use serde_json::{Map, Value, json};

const LAW: &str = "test_traffic_decree";
const ARTICLE: &str = "49";
const AWB: &str = "test_general_administrative_act";
const OBJECTION: &str = "7:11";
/// A fictional municipality.
const RECORDING_ACTOR: &str = "gemeente_wilgendaal";

/// The municipality's fictitious policy, which fills the open term of art.
/// 49 on how the college uses its power, declares an open term of its own
/// (whether the applicant already holds a valid card), and binds that term
/// and two of the Regeling to the answers of cells (RFC-048).
const POLICY: &str = "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal";
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
const REGELING: &str = "test_parking_card_regulation";
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
const EXAMINATION: &str = "test_parking_card_regulation#3#geneeskundig_onderzoek_verricht";
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
    "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";
const ADVICE_REDUCTION: &str = "reductie_geneeskundig_advies";
const ADVICE_POLICY: &str = "beleid_geneeskundig_advies_ggd_wilgendaal";
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
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/corpus/cells/{GGD_ACTOR}")),
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

/// The traffic decree, the parking card regulation, the general act and the
/// municipality's policy from the fixtures.
fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let laws = [
        format!("regulation/nl/amvb/{LAW}"),
        "regulation/nl/ministeriele_regeling/test_parking_card_regulation".to_string(),
        "regulation/nl/wet/test_general_administrative_act".to_string(),
        format!("regulation/nl/uitvoeringsbeleid/{POLICY}"),
        // The reductions of the municipality and of the GGD, each with its
        // cell (RFC-048).
        common::reduction_dir(REDUCTION),
        common::reduction_dir(REPORTS),
        common::reduction_dir(ADVICE_REDUCTION),
        format!("regulation/nl/uitvoeringsbeleid/{ADVICE_POLICY}"),
        // The decisions under an article as aggregates, with what the Awb
        // gives a decision on an objection and the receipt of a ruling.
        common::reduction_dir("reductie_gehandicaptenparkeerkaart"),
        "regulation/nl/uitvoeringsbeleid/beleid_verstrekte_gehandicaptenparkeerkaarten_gemeente_wilgendaal".to_string(),
    ];
    common::load(&mut service, laws);
    service
}

/// What the article of the submission `gram` gives on it, with the hooks on
/// it: not in its chronolexogram, but computed again by the cell.
fn given(cell: &Cell, gram: &Gram) -> std::collections::BTreeMap<String, Value> {
    cell.submission_outputs(&mut service(), &[&ggd()], &gram.id)
        .unwrap()
}

/// The municipality's cell directory, with its catalogue of answers
/// (RFC-048).
fn cell_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/corpus/cells/{RECORDING_ACTOR}"))
}

/// The municipality's cell, with an empty chronicle.
fn cell() -> Cell {
    Cell::in_memory(RECORDING_ACTOR, cell_dir()).unwrap()
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

// ---------------------------------------------------------------------------
// The objection and the decision on it (Awb 7:11, RFC-045 §7)
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
        &common::on_objection(findings),
        day(on),
    )
    .unwrap()
}

/// The college's finding on the walking limitation, given anew.
fn walking(judged: bool) -> Map<String, Value> {
    object_map(json!({ WALKING_C: judged }))
}

/// The fictional court, and the fact that a party holds the copy of its
/// judgment (Awb 8:79).
const COURT: &str = "rechtbank_wilgendaal";
const JUDGMENTS: &str = "test_general_administrative_act#8:79#afschrift_van_de_uitspraak";

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
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/corpus/cells/{COURT}")),
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
            &common::on_appeal(findings),
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

/// In `cell`, a refusal of `bsn` on 2026-10-20 that the objection leaves
/// standing; the court then decides the case itself (Awb 8:72 lid 3 onder
/// b): it revokes the refusal in the decision on the objection it sets in
/// place, and grants the card in a new decision on the application. It
/// gives the facts the cell would otherwise answer (Awb 8:69 lid 3). The
/// card the court granted, as the decision value inside its judgment.
fn granted_by_the_court(cell: &mut Cell, bsn: &str) -> Value {
    let app = applied(cell, bsn, "2026-10-20");
    reported(cell, bsn, "2026-10-20", "2026-10-20");
    let ggd = ggd();
    let sphere = [&ggd];
    let refused = decide(cell, &app.id, false, "2026-10-20");
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
