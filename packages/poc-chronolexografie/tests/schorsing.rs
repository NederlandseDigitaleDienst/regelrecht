//! A suspension by the voorzieningenrechter (Awb 8:81) of one of the two
//! business parking permits of a company, and what it does to the
//! reductions of the municipality: the one per permit (key `kenteken`) and
//! the one per company (key `kvk_nummer`).
//!
//! The company gets its permits as the game plays them (tests/spel.rs). A
//! neighbour objects against the first. What follows is not played through
//! the court's cell: the schema has no submission kind for a request for a
//! provisional measure (8:81), so no cell can decide on one. The municipality
//! receives the copy of the ruling (8:84, fourth paragraph) as
//! `Cell::record_receipt` records the receipt of a copy: an executogram of
//! the fact the article requires, that refers to the decision the copy
//! concerns. The lifting (8:87) and the lapse (8:85, second paragraph) arrive
//! the same way.
//!
//! Every question is asked on 15 January 2027, when every record is known,
//! about the day it names (`peildatum`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_cel::reading::chronicle_records;
use regelrecht_cel::{Gram, Received, EXECUTOGRAM};
use regelrecht_engine::{LawExecutionService, Value as EngineValue};
use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde_json::{json, Map, Value};

const CASUS: &str = "bedrijfsparkeervergunning";
const BAKKERIJ: &str = "99990011";
const EERSTE: &str = "ZZ-001-Z";
const TWEEDE: &str = "ZZ-002-Z";
const AWB: &str = "algemene_wet_bestuursrecht";
const PER_VERGUNNING: &str = "reductie_bedrijfsparkeervergunning_op_peildatum";
const PER_BEDRIJF: &str = "reductie_geldige_bedrijfsparkeervergunningen";
const GEVRAAGD_OP: &str = "2027-01-15";

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

fn dag(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn map(v: &Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// The company applies for a permit for `kenteken` on `datum` and the
/// college decides on `beslist`. Gives the decision's id.
fn vergunning(spel: &mut Spel, kenteken: &str, datum: &str, beslist: &str) -> String {
    let casus = spel.casus().unwrap();
    let mut aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    aanvraag.insert("kvk_nummer".into(), json!(BAKKERIJ));
    aanvraag.insert("kenteken".into(), json!(kenteken));
    aanvraag.insert("dagtekening".into(), json!(datum));
    let ingediend = spel.aanvragen(&aanvraag, dag(datum)).unwrap();
    let besluit = spel
        .besluiten(ingediend["id"].as_str().unwrap(), &Map::new(), dag(beslist))
        .unwrap();
    assert_eq!(
        besluit["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"],
        json!(true),
        "{besluit}"
    );
    besluit["besluit"]["id"].as_str().unwrap().to_string()
}

/// The chronicle of the municipality after two permits and an objection by
/// a neighbour against the first (Awb 8:81, first paragraph: an objection
/// is pending). Gives the chronicle and the id of the first permit.
fn twee_vergunningen_en_een_bezwaar() -> (Vec<Gram>, String) {
    let mut spel = Spel::new(Corpus::alle(&corpus_dir(), &corpus_dir().join("casussen")).unwrap());
    spel.kiezen(CASUS).unwrap();
    let eerste = vergunning(&mut spel, EERSTE, "2026-10-13", "2026-10-20");
    vergunning(&mut spel, TWEEDE, "2026-10-21", "2026-10-22");
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
    spel.bezwaren(&eerste, &bezwaar, dag("2026-10-28")).unwrap();
    let kroniek = spel.kroniek().unwrap();
    let grams: Vec<Gram> = serde_json::from_value(kroniek["grammen"].clone()).unwrap();
    (grams, eerste)
}

/// The receipt of a copy of a ruling of the voorzieningenrechter, as
/// `Cell::record_receipt` records it: an executogram of `fact` under
/// `article` of the Awb, referring to the decision `besluit` the copy
/// concerns, with the fields of the fact.
fn ontvangst(article: &str, fact: &str, besluit: &str, on: &str, velden: Value) -> Gram {
    let mut fields: BTreeMap<String, Value> = map(&velden).into_iter().collect();
    fields.insert("kenmerk_besluit".into(), json!(besluit));
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

/// The voorzieningenrechter lifts the measure on 15 December 2026 (8:87).
fn opheffing(besluit: &str) -> Gram {
    ontvangst(
        "8:87",
        "afschrift_van_de_uitspraak_tot_opheffing",
        besluit,
        "2026-12-15",
        json!({ "voorziening_opgeheven_op": "2026-12-15" }),
    )
}

/// The measure lapses on 1 December 2026: the objection was withdrawn
/// (8:85, second paragraph, under b).
fn verval(besluit: &str) -> Gram {
    ontvangst(
        "8:85",
        "voorlopige_voorziening_vervallen",
        besluit,
        "2026-12-01",
        json!({
            "grond_verval": "het bezwaar is ingetrokken (onder b)",
            "voorziening_vervallen_op": "2026-12-01",
        }),
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
    grams.push(schorsing(&eerste));
    grams.push(opheffing(&eerste));
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
fn per_permit_a_measure_that_lapses_ends_the_suspension_on_that_day() {
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    grams.push(schorsing(&eerste));
    grams.push(verval(&eerste));
    assert!(!geldig(&service, &grams, EERSTE, "2026-11-30"));
    assert!(geldig(&service, &grams, EERSTE, "2026-12-01"));
    // A lifting after the lapse does not lengthen the suspension.
    grams.push(opheffing(&eerste));
    assert!(geldig(&service, &grams, EERSTE, "2026-12-10"));
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

#[test]
fn per_company_two_permits_count_before_and_after_the_suspension() {
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    grams.push(schorsing(&eerste));
    grams.push(opheffing(&eerste));
    assert_eq!(aantal(&service, &grams, "2026-11-09"), twee());
    assert_eq!(aantal(&service, &grams, "2026-12-15"), twee());
}

/// The per-company question on a day inside the suspension. One permit is
/// valid that day, so the answer is 1. The reduction per company cannot say
/// so with a state of single values, and answers 2: see the question on a
/// state that holds a list in
/// `docs/src/content/docs/concepts/chronolexography/open-questions.md`.
#[test]
#[ignore = "pending: the per-company count needs a state that holds a list (chronolexography open questions)"]
fn per_company_one_permit_counts_while_the_other_is_suspended() {
    let service = service();
    let (mut grams, eerste) = twee_vergunningen_en_een_bezwaar();
    grams.push(schorsing(&eerste));
    grams.push(opheffing(&eerste));
    assert_eq!(aantal(&service, &grams, "2026-11-20"), EngineValue::Int(1));
}

#[test]
fn the_reduction_tab_shows_the_copies_the_permit_reduction_reads() {
    // A step on an executogram reads the facts its article requires; the
    // tab Reductie lists them beside the decisions.
    let mut spel = Spel::new(Corpus::alle(&corpus_dir(), &corpus_dir().join("casussen")).unwrap());
    spel.kiezen(CASUS).unwrap();
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
            "algemene_wet_bestuursrecht#8:84#afschrift_van_de_uitspraak_van_de_voorzieningenrechter",
            "algemene_wet_bestuursrecht#8:85#voorlopige_voorziening_vervallen",
            "algemene_wet_bestuursrecht#8:87#afschrift_van_de_uitspraak_tot_opheffing",
        ]
    );
}
