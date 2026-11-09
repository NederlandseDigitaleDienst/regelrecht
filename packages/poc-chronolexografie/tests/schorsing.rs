//! A suspension by the voorzieningenrechter (Awb 8:81) of one of the two
//! business parking permits of a company, and what it does to the
//! reductions of the municipality: the one per registration number
//! (`kenteken`) and the one per company (`kvk_nummer`). Both read the
//! aggregates of the municipality (schema v0.8.17): one per decision on an
//! application, and one per copy of a ruling of the voorzieningenrechter.
//!
//! The company gets its permits as the game plays them (tests/spel.rs). A
//! neighbour objects against the first. What follows is not played through
//! the court's cell: the schema has no submission kind for a request for a
//! provisional measure (8:81), so no cell can decide on one. The municipality
//! receives the copy of the ruling (8:84, fourth paragraph) as
//! `Cell::record_receipt` records the receipt of a copy: an executogram of
//! the fact the article requires, that refers to the decision the copy
//! concerns. The lifting (8:87) arrives the same way, and refers to the
//! copy of the ruling it lifts, so it joins that suspension. That the measure
//! lapsed (8:85) is not recorded: it follows from the days the records
//! carry, such as the day of the judgment in the main case, whose copy the
//! municipality receives as the game plays it (8:79).
//!
//! Every question is asked on 15 January 2028, when every record is known,
//! about the day it names (`peildatum`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::reading::chronicle_records;
use regelrecht_cel::{EXECUTOGRAM, Gram, Received};
use regelrecht_engine::{LawExecutionService, Value as EngineValue};
use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde_json::{Map, Value, json};

const CASUS: &str = "bedrijfsparkeervergunning";
const BAKKERIJ: &str = "99990011";
const EERSTE: &str = "ZZ-001-Z";
const TWEEDE: &str = "ZZ-002-Z";
const AWB: &str = "algemene_wet_bestuursrecht";
const PER_VERGUNNING: &str = "reductie_bedrijfsparkeervergunning_op_peildatum";
const PER_BEDRIJF: &str = "reductie_geldige_bedrijfsparkeervergunningen";
const GEVRAAGD_OP: &str = "2028-01-15";

fn corpus_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus")
}

fn corpus() -> Corpus {
    let dir = corpus_dir();
    Corpus::alle(&dir, &dir.join("casussen"))
        .unwrap()
        .into_iter()
        .find(|c| c.casus.id == CASUS)
        .unwrap()
}

/// A game on the case, with the cells as they begin.
fn spel() -> Spel {
    let mut spel = Spel::new(Corpus::alle(&corpus_dir(), &corpus_dir().join("casussen")).unwrap());
    spel.kiezen(CASUS).unwrap();
    spel
}

fn dag(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn map(v: &Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// The company applies for a permit for `kenteken` on `datum` and the
/// college decides on `beslist`, with `bevindingen`. Gives the decision's id
/// and whether it grants the permit.
fn besluit(
    spel: &mut Spel,
    kenteken: &str,
    datum: &str,
    beslist: &str,
    bevindingen: &Map<String, Value>,
) -> (String, bool) {
    let casus = spel.casus().unwrap();
    let mut aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    aanvraag.insert("kvk_nummer".into(), json!(BAKKERIJ));
    aanvraag.insert("kenteken".into(), json!(kenteken));
    aanvraag.insert("dagtekening".into(), json!(datum));
    let ingediend = spel.aanvragen(&aanvraag, dag(datum)).unwrap();
    let besluit = spel
        .besluiten(ingediend["id"].as_str().unwrap(), bevindingen, dag(beslist))
        .unwrap();
    let verleend = besluit["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"]
        .as_bool()
        .unwrap_or_else(|| panic!("{besluit}"));
    (
        besluit["besluit"]["id"].as_str().unwrap().to_string(),
        verleend,
    )
}

/// [`besluit`] for a permit the college grants. Gives the decision's id.
fn vergunning(spel: &mut Spel, kenteken: &str, datum: &str, beslist: &str) -> String {
    let (id, verleend) = besluit(spel, kenteken, datum, beslist, &Map::new());
    assert!(verleend, "the permit for {kenteken} is refused");
    id
}

/// What else happens in the chronicle of [`chronicle`] after the two
/// permits.
#[derive(Default)]
struct Verder {
    /// A third application for the first registration number, refused at
    /// the maximum of two in the city centre.
    weigering: bool,
    /// After the first permit expired, a new one for the same number.
    verlenging: bool,
    /// The college decides on the objection, the neighbour appeals, and the
    /// court rules on 11 January 2027; the municipality receives the copy
    /// on 12 January.
    hoofdzaak: bool,
    /// With [`Verder::hoofdzaak`]: the court finds that the college broke a
    /// rule of law, so the appeal is well founded and the decision on the
    /// objection is annulled (Awb 8:72).
    gegrond: bool,
}

/// The chronicle of the municipality after two permits and an objection by
/// a neighbour against the first (Awb 8:81, first paragraph: an objection
/// is pending). Gives the chronicle and the id of the first permit.
fn twee_vergunningen_en_een_bezwaar() -> (Vec<Gram>, String) {
    chronicle(&Verder::default())
}

/// [`twee_vergunningen_en_een_bezwaar`], with what `verder` adds.
fn chronicle(verder: &Verder) -> (Vec<Gram>, String) {
    let mut spel = spel();
    let eerste = vergunning(&mut spel, EERSTE, "2026-10-13", "2026-10-20");
    vergunning(&mut spel, TWEEDE, "2026-10-21", "2026-10-22");
    if verder.weigering {
        let hardheid = map(&json!({ "toepassing_leidt_tot_onaanvaardbare_gevolgen": false }));
        let (_, verleend) = besluit(&mut spel, EERSTE, "2026-10-23", "2026-10-24", &hardheid);
        assert!(!verleend, "the third permit is granted");
    }
    let casus = spel.casus().unwrap();
    let mut bezwaar = map(&casus["voorbeeld"]["bezwaar"]);
    bezwaar.insert("naam_indiener".into(), json!("Buurtvereniging Kerkstraat"));
    bezwaar.insert(
        "omschrijving_bestreden_besluit".into(),
        json!("bedrijfsparkeervergunning voor ZZ-001-Z"),
    );
    bezwaar.insert(
        "gronden_bezwaar_of_beroep".into(),
        json!("de straat heeft te weinig parkeerplaatsen voor bewoners"),
    );
    bezwaar.insert(
        "dagtekening_bezwaar_of_beroepschrift".into(),
        json!("2026-10-28"),
    );
    let bezwaar = spel.bezwaren(&eerste, &bezwaar, dag("2026-10-28")).unwrap();
    if verder.hoofdzaak {
        let beslissing = spel
            .beslissen_op_bezwaar(
                bezwaar["id"].as_str().unwrap(),
                &Map::new(),
                dag("2026-11-25"),
            )
            .unwrap();
        let mut beroep = map(&casus["voorbeeld"]["beroep"]);
        beroep.insert("naam_indiener".into(), json!("Buurtvereniging Kerkstraat"));
        beroep.insert(
            "dagtekening_bezwaar_of_beroepschrift".into(),
            json!("2026-12-01"),
        );
        let beroep = spel
            .beroep_instellen(
                beslissing["beslissing"]["id"].as_str().unwrap(),
                &beroep,
                dag("2026-12-01"),
            )
            .unwrap();
        let mut oordeel = map(&casus["voorbeeld"]["uitspraak"]);
        if verder.gegrond {
            // The legal effects do not stay in place, the ruling does not
            // take the decision's place, and the college gets a new term
            // (8:72, third and fourth paragraph).
            oordeel.insert("rechtsregel_geschonden".into(), json!(true));
            oordeel.insert(
                "geschonden_rechtsregel".into(),
                json!("artikel 7:12, eerste lid, van de Algemene wet bestuursrecht"),
            );
            oordeel.insert("rechtsgevolgen_in_stand".into(), json!(false));
            oordeel.insert("uitspraak_treedt_in_de_plaats".into(), json!(false));
            oordeel.insert("termijn_nieuw_besluit".into(), json!("2027-03-08"));
        }
        let uitspraak = spel
            .uitspraak_doen(beroep["id"].as_str().unwrap(), &oordeel, dag("2027-01-11"))
            .unwrap();
        if verder.gegrond {
            assert_eq!(
                uitspraak["uitspraak"]["outputs"]["bestreden_besluit_vernietigd"],
                json!(true),
                "{uitspraak}"
            );
        }
        spel.toezenden(
            uitspraak["uitspraak"]["id"].as_str().unwrap(),
            dag("2027-01-12"),
        )
        .unwrap();
    }
    if verder.verlenging {
        // The first permit runs to 19 October 2027.
        vergunning(&mut spel, EERSTE, "2027-10-21", "2027-10-25");
    }
    let kroniek = spel.kroniek().unwrap();
    let grams: Vec<Gram> = serde_json::from_value(kroniek["grammen"].clone()).unwrap();
    (grams, eerste)
}

/// `grams` with `extra` placed by the day it was recorded, after the
/// records of the same day: the order in which the cell would have recorded
/// them.
fn met(mut grams: Vec<Gram>, extra: impl IntoIterator<Item = Gram>) -> Vec<Gram> {
    grams.extend(extra);
    grams.sort_by_key(|g| g.recorded_on);
    grams
}

/// The receipt of a copy of a ruling of the voorzieningenrechter, in the
/// shape `Cell::record_receipt` gives it: an executogram of `fact` under
/// `article` of the Awb, referring to the decision `besluit` the copy
/// concerns, with the fields of the fact. Built by hand, because no court
/// decision exists to receive; so the checks of `record_receipt` (the copy
/// concerns `besluit`, the receipt is not dated before the ruling, the copy
/// holds every field of the fact) do not run here. The projection still
/// refuses a fact its article does not require.
fn ontvangst(article: &str, fact: &str, besluit: &str, on: &str, velden: Value) -> Gram {
    let fields: BTreeMap<String, Value> = map(&velden).into_iter().collect();
    Gram {
        id: format!("{fact}-{on}"),
        kind: EXECUTOGRAM.to_string(),
        basis: format!("{AWB}#{article}"),
        fact: Some(fact.to_string()),
        refers_to: Some(besluit.to_string()),
        received: Some(Received {
            from: "rechtbank_wilgendaal".to_string(),
            id: format!("uitspraak-{on}"),
        }),
        after: None,
        recording_actor: "gemeente_wilgendaal".to_string(),
        recorded_on: dag(on),
        moment: None,
        fields,
        missing: Vec::new(),
        outputs: BTreeMap::new(),
    }
}

/// The voorzieningenrechter suspends the first permit from 10 November 2026
/// without an end of its own (8:81, 8:84).
fn schorsing(besluit: &str) -> Gram {
    ontvangst(
        "8:84",
        "afschrift_van_de_uitspraak_van_de_voorzieningenrechter",
        besluit,
        "2026-11-10",
        json!({
            "uitspraak_strekt_tot": "gehele of gedeeltelijke toewijzing van het verzoek",
            "besluit_geschorst_vanaf": "2026-11-10",
            "voorziening_vervalt_op": null,
        }),
    )
}

/// The voorzieningenrechter lifts the measure of `voorziening`, the copy
/// of its ruling, on 15 December 2026 (8:87). The copy of the lifting
/// refers to that ruling, so it joins the aggregate of the suspension.
fn opheffing(voorziening: &Gram) -> Gram {
    ontvangst(
        "8:87",
        "afschrift_van_de_uitspraak_tot_opheffing",
        &voorziening.id,
        "2026-12-15",
        json!({ "voorziening_opgeheven_op": "2026-12-15" }),
    )
}

fn service() -> LawExecutionService {
    corpus().service().unwrap()
}

/// `output` of the reduction `law` over `grams`, with `params`.
fn antwoord(
    service: &LawExecutionService,
    law: &str,
    output: &str,
    grams: &[Gram],
    params: &[(&str, EngineValue)],
) -> EngineValue {
    let records = chronicle_records(grams).unwrap();
    let params = params
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect();
    let outcome = service
        .evaluate_projection(law, &[output], &records, params, GEVRAAGD_OP, false)
        .unwrap_or_else(|e| panic!("{law}: {e}"));
    outcome.result.outputs[output].clone()
}

fn s(v: &str) -> EngineValue {
    EngineValue::String(v.to_string())
}

/// Whether the permit for `kenteken` is valid on `peildatum`.
fn geldig(service: &LawExecutionService, grams: &[Gram], kenteken: &str, peildatum: &str) -> bool {
    match antwoord(
        service,
        PER_VERGUNNING,
        "bedrijfsparkeervergunning_geldig_op_peildatum",
        grams,
        &[("kenteken", s(kenteken)), ("peildatum", s(peildatum))],
    ) {
        EngineValue::Bool(b) => b,
        other => panic!("no boolean: {other:?}"),
    }
}

/// How many permits of the company are valid on `peildatum`. The reduction
/// counts on the day of the decision that asks it (`besluit_datum`); here
/// that day is the day asked about.
fn aantal(service: &LawExecutionService, grams: &[Gram], peildatum: &str) -> EngineValue {
    antwoord(
        service,
        PER_BEDRIJF,
        "aantal_geldige_bedrijfsparkeervergunningen",
        grams,
        &[("kvk_nummer", s(BAKKERIJ)), ("besluit_datum", s(peildatum))],
    )
}

fn twee() -> EngineValue {
    EngineValue::Int(2)
}

#[test]
fn per_permit_the_suspended_permit_is_not_valid_while_the_suspension_holds() {
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let voorziening = schorsing(&eerste);
    grams.push(opheffing(&voorziening));
    grams.insert(grams.len() - 1, voorziening);
    // Before the suspension, both are valid.
    assert!(geldig(&service, &grams, EERSTE, "2026-11-09"));
    // From its first day the first is suspended; the second is not.
    assert!(!geldig(&service, &grams, EERSTE, "2026-11-10"));
    assert!(!geldig(&service, &grams, EERSTE, "2026-12-14"));
    assert!(geldig(&service, &grams, TWEEDE, "2026-11-20"));
    // Lifted on 15 December (8:87): valid again from that day.
    assert!(geldig(&service, &grams, EERSTE, "2026-12-15"));
}

#[test]
fn per_permit_the_judgment_in_the_main_case_ends_the_suspension() {
    // Awb 8:85, second paragraph, under c: the measure lapses as soon as the
    // court rules in the main case, on 11 January 2027. Nothing records the
    // lapse; article 4 of the policy reads it from the day of the judgment
    // in its copy, received a day later.
    let service = service();
    let (grams, eerste) = chronicle(&Verder {
        hoofdzaak: true,
        ..Verder::default()
    });
    let grams = met(grams, [schorsing(&eerste)]);
    assert!(!geldig(&service, &grams, EERSTE, "2027-01-10"));
    assert!(geldig(&service, &grams, EERSTE, "2027-01-11"));
    // A lifting after the lapse does not lengthen the suspension.
    let mut later = opheffing(&schorsing(&eerste));
    later.recorded_on = dag("2027-02-01");
    later
        .fields
        .insert("voorziening_opgeheven_op".into(), json!("2027-02-01"));
    let grams = met(grams, [later]);
    assert!(geldig(&service, &grams, EERSTE, "2027-01-20"));
}

#[test]
fn per_permit_an_end_set_in_the_ruling_ends_the_suspension() {
    // Awb 8:85, first paragraph: the ruling itself says when it lapses.
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let mut met_einde = schorsing(&eerste);
    met_einde
        .fields
        .insert("voorziening_vervalt_op".into(), json!("2026-12-20"));
    grams.push(met_einde);
    assert!(!geldig(&service, &grams, EERSTE, "2026-12-19"));
    assert!(geldig(&service, &grams, EERSTE, "2026-12-20"));
}

/// Outside the suspension the copies of 8:84 and 8:87 leave the count
/// alone: before the suspension begins and after it is lifted, both
/// permits count. The test below counts on a day inside it.
#[test]
fn per_company_the_copies_of_rulings_leave_the_count_alone() {
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let voorziening = schorsing(&eerste);
    grams.push(opheffing(&voorziening));
    grams.insert(grams.len() - 1, voorziening);
    assert_eq!(aantal(&service, &grams, "2026-11-09"), twee());
    assert_eq!(aantal(&service, &grams, "2026-12-15"), twee());
}

/// The per-company question on a day inside the suspension. One permit is
/// valid that day, so the answer is 1: each permit is an aggregate of its
/// own, and so is the suspension, which names the permit it concerns.
#[test]
fn per_company_one_permit_counts_while_the_other_is_suspended() {
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let voorziening = schorsing(&eerste);
    grams.push(opheffing(&voorziening));
    grams.insert(grams.len() - 1, voorziening);
    assert_eq!(aantal(&service, &grams, "2026-11-20"), EngineValue::Int(1));
}

#[test]
fn the_reduction_tab_shows_the_copies_the_permit_reduction_reads() {
    // A step on an executogram reads the facts its article requires; the
    // tab Reductie lists them beside the decisions, also those of the
    // aggregates the reduction reads (source.projections): the permit's,
    // with the judgment in the main case and the decision a suspension is
    // about, then the suspension's.
    let mut spel = spel();
    let stand = spel.reducties(dag("2026-12-01")).unwrap();
    let reductie = stand["reducties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == json!(PER_VERGUNNING))
        .unwrap();
    let feiten: Vec<&str> = reductie["invoer"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["executogrammen_van"].as_str())
        .collect();
    assert_eq!(
        feiten,
        [
            "algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak",
            "algemene_wet_bestuursrecht#8:84#afschrift_van_de_uitspraak_van_de_voorzieningenrechter",
            "algemene_wet_bestuursrecht#8:84#afschrift_van_de_uitspraak_van_de_voorzieningenrechter",
            "algemene_wet_bestuursrecht#8:87#afschrift_van_de_uitspraak_tot_opheffing",
        ]
    );
}

/// A ruling of the voorzieningenrechter on `on` that rejects the request
/// (8:84, second paragraph, under c): it suspends nothing, even with a day.
fn afwijzing(besluit: &str, on: &str, vanaf: Value) -> Gram {
    ontvangst(
        "8:84",
        "afschrift_van_de_uitspraak_van_de_voorzieningenrechter",
        besluit,
        on,
        json!({
            "uitspraak_strekt_tot": "afwijzing van het verzoek",
            "besluit_geschorst_vanaf": vanaf,
            "voorziening_vervalt_op": null,
        }),
    )
}

#[test]
fn per_permit_a_rejected_request_suspends_nothing() {
    let service = service();
    let (grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let grams = met(
        grams,
        [afwijzing(&eerste, "2026-11-10", json!("2026-11-10"))],
    );
    assert!(geldig(&service, &grams, EERSTE, "2026-11-20"));
}

#[test]
fn per_permit_a_later_ruling_that_suspends_nothing_leaves_the_suspension() {
    let service = service();
    let (grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let grams = met(
        grams,
        [
            schorsing(&eerste),
            afwijzing(&eerste, "2026-11-20", json!(null)),
        ],
    );
    assert!(!geldig(&service, &grams, EERSTE, "2026-11-25"));
}

#[test]
fn per_permit_a_refusal_for_the_same_number_leaves_the_permit() {
    let service = service();
    let (grams, _) = chronicle(&Verder {
        weigering: true,
        ..Verder::default()
    });
    assert!(geldig(&service, &grams, EERSTE, "2026-11-01"));
}

#[test]
fn per_permit_an_expired_permit_is_not_valid() {
    // Granted on 20 October 2026, valid up to and including 19 October 2027.
    let service = service();
    let (grams, _) = twee_vergunningen_en_een_bezwaar();
    assert!(geldig(&service, &grams, EERSTE, "2027-10-19"));
    assert!(!geldig(&service, &grams, EERSTE, "2027-10-20"));
}

#[test]
fn per_permit_a_new_grant_for_the_number_starts_without_the_suspension() {
    // The suspension has no end and concerns the first decision; the new
    // permit of 25 October 2027 is another decision.
    let service = service();
    let (grams, eerste) = chronicle(&Verder {
        verlenging: true,
        ..Verder::default()
    });
    let grams = met(grams, [schorsing(&eerste)]);
    assert!(!geldig(&service, &grams, EERSTE, "2027-10-01"));
    assert!(geldig(&service, &grams, EERSTE, "2027-11-01"));
}

/// A second suspension of the first permit, from 1 March 2027 without an
/// end.
fn tweede_schorsing(besluit: &str) -> Gram {
    let mut gram = schorsing(besluit);
    gram.id = "tweede-schorsing".to_string();
    gram.recorded_on = dag("2027-03-01");
    gram.fields
        .insert("besluit_geschorst_vanaf".into(), json!("2027-03-01"));
    gram
}

#[test]
fn per_permit_a_later_suspension_leaves_the_day_of_the_first() {
    let service = service();
    let (grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let grams = met(
        grams,
        [
            schorsing(&eerste),
            opheffing(&schorsing(&eerste)),
            tweede_schorsing(&eerste),
        ],
    );
    assert!(!geldig(&service, &grams, EERSTE, "2026-11-20"));
    assert!(geldig(&service, &grams, EERSTE, "2027-01-10"));
    assert!(!geldig(&service, &grams, EERSTE, "2027-03-10"));
}

/// The copy of a ruling on the first decision for a number, received after
/// a new permit for that number. The ruling concerns the first decision, so
/// the new permit stays valid: the suspension names the first decision,
/// and the new permit is an aggregate of its own.
#[test]
fn per_permit_a_copy_about_the_earlier_decision_leaves_the_new_permit() {
    let service = service();
    let (grams, eerste) = chronicle(&Verder {
        verlenging: true,
        ..Verder::default()
    });
    let mut late = schorsing(&eerste);
    late.recorded_on = dag("2027-11-01");
    late.fields
        .insert("besluit_geschorst_vanaf".into(), json!("2027-11-01"));
    let grams = met(grams, [late]);
    assert!(geldig(&service, &grams, EERSTE, "2027-11-10"));
}

#[test]
fn per_permit_annulling_the_decision_on_objection_leaves_the_permit_and_ends_the_suspension() {
    // The appeal is well founded: the court annuls the decision on the
    // objection with its legal effects (Awb 8:72, first and second
    // paragraph). That revokes the decision on the objection, not the
    // permit it upheld; the objection is open again. The measure lapses all
    // the same, because the court ruled in the main case (8:85, second
    // paragraph, under c).
    let service = service();
    let (grams, eerste) = chronicle(&Verder {
        hoofdzaak: true,
        gegrond: true,
        ..Verder::default()
    });
    let grams = met(grams, [schorsing(&eerste)]);
    assert!(!geldig(&service, &grams, EERSTE, "2027-01-10"));
    assert!(geldig(&service, &grams, EERSTE, "2027-01-11"));
}

#[test]
fn per_permit_a_permit_granted_on_objection_can_be_suspended() {
    // The bakery reaches the maximum of two in the city centre, and the
    // college refuses a third permit. On objection it applies the hardship
    // clause: the refusal is revoked (Awb 7:11, second paragraph, as an
    // effect) and a new decision grants the permit. The voorzieningenrechter
    // then suspends that decision on the objection, which holds the new
    // decision: one act, one record. The request and the appeal it needs
    // (8:81, first paragraph) are not played, as elsewhere in this file.
    const DERDE: &str = "ZZ-003-Z";
    let service = service();
    let mut spel = spel();
    vergunning(&mut spel, EERSTE, "2026-10-13", "2026-10-20");
    vergunning(&mut spel, TWEEDE, "2026-10-21", "2026-10-22");
    let hardheid =
        |oordeel: bool| map(&json!({ "toepassing_leidt_tot_onaanvaardbare_gevolgen": oordeel }));
    let (weigering, verleend) = besluit(
        &mut spel,
        DERDE,
        "2026-10-23",
        "2026-10-27",
        &hardheid(false),
    );
    assert!(!verleend, "the third permit is granted");
    let casus = spel.casus().unwrap();
    let mut bezwaar = map(&casus["voorbeeld"]["bezwaar"]);
    bezwaar.insert(
        "dagtekening_bezwaar_of_beroepschrift".into(),
        json!("2026-11-01"),
    );
    let bezwaar = spel
        .bezwaren(&weigering, &bezwaar, dag("2026-11-01"))
        .unwrap();
    let gegrond = spel
        .beslissen_op_bezwaar(
            bezwaar["id"].as_str().unwrap(),
            &hardheid(true),
            dag("2026-11-20"),
        )
        .unwrap();
    assert_eq!(
        gegrond["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
        json!(true),
        "{gegrond}"
    );
    let beslissing = gegrond["beslissing"]["id"].as_str().unwrap().to_string();
    let kroniek = spel.kroniek().unwrap();
    let grams: Vec<Gram> = serde_json::from_value(kroniek["grammen"].clone()).unwrap();

    let mut geschorst = schorsing(&beslissing);
    geschorst.recorded_on = dag("2026-12-01");
    geschorst
        .fields
        .insert("besluit_geschorst_vanaf".into(), json!("2026-12-01"));
    let grams = met(grams, [geschorst]);
    // Before the decision on the objection there is no permit: the refusal
    // gives none, and its revocation does not reach back to a day before
    // the new decision.
    assert!(!geldig(&service, &grams, DERDE, "2026-11-10"));
    assert!(geldig(&service, &grams, DERDE, "2026-11-25"));
    assert!(!geldig(&service, &grams, DERDE, "2026-12-05"));
}

/// The first suspension, lifted on 15 December 2026, and a second from
/// 1 March 2027, lifted on 1 April 2027. On 10 January 2027 the permit is
/// valid: the first suspension has ended and the second has not begun. The
/// step for the second copy skips it, because it begins after the day
/// asked about. Each suspension is an aggregate of its own, and a lifting
/// refers to the suspension it ends, so lifting the second leaves the end
/// of the first as it was.
#[test]
fn per_permit_lifting_a_later_suspension_leaves_the_end_of_the_first() {
    let service = service();
    let (grams, eerste) = twee_vergunningen_en_een_bezwaar();
    let mut tweede_opheffing = opheffing(&tweede_schorsing(&eerste));
    tweede_opheffing.id = "tweede-opheffing".to_string();
    tweede_opheffing.recorded_on = dag("2027-04-01");
    tweede_opheffing
        .fields
        .insert("voorziening_opgeheven_op".into(), json!("2027-04-01"));
    let grams = met(
        grams,
        [
            schorsing(&eerste),
            opheffing(&schorsing(&eerste)),
            tweede_schorsing(&eerste),
            tweede_opheffing,
        ],
    );
    assert!(geldig(&service, &grams, EERSTE, "2027-01-10"));
    assert!(!geldig(&service, &grams, EERSTE, "2027-03-10"));
    assert!(geldig(&service, &grams, EERSTE, "2027-04-01"));
}
