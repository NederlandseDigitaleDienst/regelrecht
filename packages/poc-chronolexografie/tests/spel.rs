//! The game as the app plays it, one step at a time, on the repository's
//! corpus. In the browser the same steps run behind wasm-bindgen
//! (`src/wasm.rs`), one game per tab.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use chrono::NaiveDate;
use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::{Fout, Spel};
use serde_json::{json, Map, Value};

fn spel() -> Spel {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    Spel::new(Corpus::alle(&dir.join("corpus"), &dir.join("corpus/casussen")).unwrap())
}

/// A game with the case `id` chosen.
fn spel_met(id: &str) -> Spel {
    let mut spel = spel();
    spel.kiezen(id).unwrap();
    spel
}

fn dag(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn map(v: &Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn soorten(k: &Value) -> Vec<String> {
    k["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_forms_come_from_the_chosen_case() {
    let mut spel = spel_met("vakantieverlof");
    let vormen = spel.vormen(dag("2026-10-14")).unwrap();
    assert_eq!(vormen["aanvraag"]["basis"], json!("leerplichtwet_1969#13a"));
}

#[test]
fn a_refusal_gives_the_reason_and_reset_starts_from_the_school() {
    let mut spel = spel_met("vakantieverlof");
    let e = spel
        .aanvragen(
            &map(&json!({ "bestemming": "Lissabon" })),
            dag("2026-10-13"),
        )
        .unwrap_err();
    assert!(matches!(e, Fout::Cel(_)));
    assert!(e.to_string().contains("bestemming"), "{e}");

    spel.aanvragen(
        &map(&json!({ "bsn_jongere": "999990019" })),
        dag("2026-10-13"),
    )
    .unwrap();
    // The school's vestiging, and the application.
    assert_eq!(
        soorten(&spel.kroniek().unwrap()),
        ["EXECUTOGRAM", "AANVRAAG"]
    );
    // The log has the application on top; it is no decision.
    let logboek = spel.logboek().unwrap();
    let handelingen = logboek["handelingen"].as_array().unwrap();
    assert_eq!(handelingen.len(), 2);
    assert_eq!(handelingen[0]["gram"]["kind"], json!("AANVRAAG"));
    assert!(handelingen[0]["waarom"].is_null());
    // A reset starts from what the school recorded before the scenarios.
    let terug = spel.reset().unwrap();
    let vooraf = ["EXECUTOGRAM"];
    assert_eq!(soorten(&terug), vooraf);
    assert_eq!(soorten(&spel.kroniek().unwrap()), vooraf);
}

#[test]
fn without_a_chosen_case_a_step_is_refused_and_choosing_one_starts_its_cell() {
    let mut spel = spel();
    let lijst = spel.casussen().unwrap();
    assert!(lijst["gekozen"].is_null());
    let ids: Vec<&str> = lijst["casussen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "bedrijfsparkeervergunning",
            "gehandicaptenparkeerkaart",
            "vakantieverlof",
            "zorgtoeslag"
        ]
    );

    for e in [spel.kroniek().unwrap_err(), spel.casus().unwrap_err()] {
        assert!(matches!(e, Fout::GeenCasus));
        assert!(e.to_string().contains("no case is chosen"), "{e}");
    }
    assert!(matches!(
        spel.kiezen("x").unwrap_err(),
        Fout::OnbekendeCasus(_)
    ));

    // Choosing a case gives what follows for it from the law.
    let casus = spel.kiezen("vakantieverlof").unwrap();
    assert_eq!(casus["besluit"], json!("leerplichtwet_1969#13a"));
    assert_eq!(
        casus["beslissing_op_bezwaar"],
        json!("algemene_wet_bestuursrecht#7:11")
    );
    assert_eq!(casus["rol"], json!("het hoofd"));
    assert_eq!(spel.casussen().unwrap()["gekozen"], json!("vakantieverlof"));
}

#[test]
fn two_games_are_apart_and_a_reset_touches_only_its_own() {
    // Two visitors, each with a game of their own: what one records and
    // resets stays out of the other's chronicle.
    let mut verlof = spel_met("vakantieverlof");
    let mut kaart = spel_met("gehandicaptenparkeerkaart");
    let mut tweede_verlof = spel_met("vakantieverlof");

    verlof
        .aanvragen(
            &map(&json!({ "bsn_jongere": "999990019" })),
            dag("2026-10-13"),
        )
        .unwrap();
    let casus = kaart.casus().unwrap();
    kaart
        .aanvragen(&map(&casus["voorbeeld"]["aanvraag"]), dag("2026-10-20"))
        .unwrap();
    tweede_verlof
        .aanvragen(
            &map(&json!({ "bsn_jongere": "999990019" })),
            dag("2026-10-13"),
        )
        .unwrap();

    verlof.reset().unwrap();
    assert_eq!(soorten(&verlof.kroniek().unwrap()), ["EXECUTOGRAM"]);
    assert_eq!(
        soorten(&tweede_verlof.kroniek().unwrap()),
        ["EXECUTOGRAM", "AANVRAAG"]
    );
    assert_eq!(soorten(&kaart.kroniek().unwrap()), ["AANVRAAG"]);
}

#[test]
fn a_saved_game_plays_on_where_it_was() {
    let mut spel = spel_met("gehandicaptenparkeerkaart");
    let casus = spel.casus().unwrap();
    let aanvraag = spel
        .aanvragen(&map(&casus["voorbeeld"]["aanvraag"]), dag("2026-10-20"))
        .unwrap();
    let bewaard = spel.bewaren().unwrap();
    assert_eq!(bewaard["casus"], json!("gehandicaptenparkeerkaart"));

    // A new tab, or the same one after a reload.
    let mut terug = self::spel();
    let hersteld = terug.herstellen(bewaard.clone()).unwrap();
    assert_eq!(hersteld["id"], json!("gehandicaptenparkeerkaart"));
    assert_eq!(terug.kroniek().unwrap(), spel.kroniek().unwrap());
    // And it plays on: the application is decided in the restored game.
    let besluit = terug
        .besluiten(
            aanvraag["id"].as_str().unwrap(),
            &map(&casus["voorbeeld"]["bevindingen"]),
            dag("2026-10-20"),
        )
        .unwrap();
    assert_eq!(
        besluit["besluit"]["outputs"][casus["uitkomst"].as_str().unwrap()],
        json!(true)
    );

    // A saved game the cell does not take back is an error, and the game
    // stays as it was.
    let mut kapot = bewaard.clone();
    kapot["kronieken"][0][0]["recording_actor"] = json!("iemand_anders");
    let e = terug.herstellen(kapot).unwrap_err();
    assert!(e.to_string().contains("was recorded by"), "{e}");
    assert_eq!(soorten(&terug.kroniek().unwrap()).len(), 2);
    let mut te_weinig = bewaard;
    te_weinig["kronieken"] = json!([[]]);
    assert!(terug.herstellen(te_weinig).is_err());
    assert!(terug.herstellen(json!({ "casus": 1 })).is_err());
}

#[test]
fn the_appeal_from_the_objection_to_the_decision_taken_again() {
    // The vakantieverlof scenario with the court, as the app plays it, on
    // the case's own example values.
    let mut spel = spel_met("vakantieverlof");
    let casus = spel.casus().unwrap();
    assert_eq!(casus["beroep"], json!("algemene_wet_bestuursrecht#8:70"));
    assert_eq!(
        casus["ontvangst"],
        json!("algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak")
    );
    assert_eq!(casus["rechter"]["cel"], json!("rechtbank_wilgendaal"));
    let voorbeeld = &casus["voorbeeld"];
    let aanvraag = spel
        .aanvragen(&map(&voorbeeld["aanvraag"]), dag("2026-10-14"))
        .unwrap();
    let besluit = spel
        .besluiten(
            aanvraag["id"].as_str().unwrap(),
            &map(&json!({ "specifieke_aard_beroep_ouder": false })),
            dag("2026-10-14"),
        )
        .unwrap();
    let mut bezwaar = voorbeeld["bezwaar"].clone();
    bezwaar["dagtekening_bezwaar_of_beroepschrift"] = json!("2026-11-01");
    let bezwaar = spel
        .bezwaren(
            besluit["besluit"]["id"].as_str().unwrap(),
            &map(&bezwaar),
            dag("2026-11-01"),
        )
        .unwrap();
    let ongegrond = spel
        .beslissen_op_bezwaar(
            bezwaar["id"].as_str().unwrap(),
            &Map::new(),
            dag("2026-11-15"),
        )
        .unwrap();
    let mut beroep = voorbeeld["beroep"].clone();
    beroep["dagtekening_bezwaar_of_beroepschrift"] = json!("2026-12-01");
    let beroep = spel
        .beroep_instellen(
            ongegrond["beslissing"]["id"].as_str().unwrap(),
            &map(&beroep),
            dag("2026-12-01"),
        )
        .unwrap();
    let beroep = beroep["id"].as_str().unwrap();
    let bevindingen = map(&voorbeeld["uitspraak"]);
    // The preview of a judgment is that judgment, and records nothing.
    let voorlopig = spel
        .voorvertoning_uitspraak(beroep, &bevindingen, dag("2027-01-11"))
        .unwrap();
    let gedaan = spel
        .uitspraak_doen(beroep, &bevindingen, dag("2027-01-11"))
        .unwrap();
    assert_eq!(
        voorlopig["voorlopig"]["uitspraak"]["outputs"],
        gedaan["uitspraak"]["outputs"]
    );
    let uitspraak = gedaan["uitspraak"]["id"].as_str().unwrap().to_string();
    spel.waarom_uitspraak(&uitspraak).unwrap();
    let ontvangst = spel.toezenden(&uitspraak, dag("2027-01-11")).unwrap();
    let ontvangst = ontvangst["id"].as_str().unwrap().to_string();
    let toets = spel.heroverweging_na_uitspraak(&ontvangst).unwrap();
    assert_eq!(toets["bezwaar"], bezwaar["id"]);
    let bevindingen = map(&json!({ "specifieke_aard_beroep_ouder": true }));
    let voorlopig = spel
        .voorvertoning_na_uitspraak(&ontvangst, &bevindingen, dag("2027-01-18"))
        .unwrap();
    assert_eq!(
        voorlopig["voorlopig"]["nieuw_besluit"]["outputs"]["verlof_verleend"],
        json!(true)
    );
    let opnieuw = spel
        .opnieuw_beslissen_op_bezwaar(&ontvangst, &bevindingen, dag("2027-01-18"))
        .unwrap();
    assert_eq!(
        opnieuw["nieuw_besluit"]["outputs"]["verlof_verleend"],
        json!(true)
    );

    // Both chronicles: the court's is that of another cell of the case. A
    // reset empties it too.
    let kroniek = spel.kroniek().unwrap();
    assert_eq!(kroniek["grammen"].as_array().unwrap().len(), 8);
    let rechter = &kroniek["andere"][0];
    assert_eq!(rechter["cel"], json!("rechtbank_wilgendaal"));
    assert_eq!(rechter["cel_naam"], json!("Rechtbank Wilgendaal"));
    assert_eq!(soorten(rechter), ["BEROEP", "UITSPRAAK"]);
    spel.reset().unwrap();
    assert_eq!(spel.kroniek().unwrap()["andere"][0]["grammen"], json!([]));

    // A judgment the court did not give is not sent.
    let e = spel.toezenden(&uitspraak, dag("2027-01-20")).unwrap_err();
    assert!(matches!(e, Fout::Cel(_)), "{e}");
}

#[test]
fn parkeerkaart_a_second_card_next_to_a_valid_one() {
    // Scenario 3 of the parkeerkaart, as the app plays it: the case's own
    // example values, two applications and two decisions.
    let mut spel = spel_met("gehandicaptenparkeerkaart");
    let casus = spel.casus().unwrap();
    assert_eq!(
        casus["rol"],
        json!("het college van burgemeester en wethouders")
    );
    // The other cells the case takes along (RFC-052), from the case file,
    // the court among them (RFC-053).
    let andere: Vec<&str> = casus["andere_cellen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["cel"].as_str().unwrap())
        .collect();
    assert_eq!(andere, ["ggd_wilgendaal", "rechtbank_wilgendaal"]);
    assert_eq!(casus["rechter"]["cel"], json!("rechtbank_wilgendaal"));
    let aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    let bevindingen = map(&casus["voorbeeld"]["bevindingen"]);
    let uitkomst = casus["uitkomst"].as_str().unwrap().to_string();

    let mut besluiten = Vec::new();
    for datum in ["2026-10-20", "2027-03-01"] {
        let gram = spel.aanvragen(&aanvraag, dag(datum)).unwrap();
        let besluit = spel
            .besluiten(gram["id"].as_str().unwrap(), &bevindingen, dag(datum))
            .unwrap();
        besluiten.push(besluit);
    }
    assert_eq!(besluiten[0]["besluit"]["outputs"][&uitkomst], json!(true));
    assert_eq!(besluiten[1]["besluit"]["outputs"][&uitkomst], json!(false));
    let antwoord = besluiten[1]["waarom"]["antwoorden"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["begrip"] == json!("aanvrager_heeft_geldige_gehandicaptenparkeerkaart"))
        .unwrap();
    assert_eq!(antwoord["waarde"], json!(true));

    let id = besluiten[1]["besluit"]["id"].as_str().unwrap();
    let hoe = spel
        .hoe_beantwoord(id, "aanvrager_heeft_geldige_gehandicaptenparkeerkaart")
        .unwrap();
    let eerste = besluiten[0]["besluit"]["id"].as_str().unwrap();
    assert!(hoe["antwoorden"][0]["spoor"]
        .as_str()
        .unwrap()
        .contains(eerste));
}

#[test]
fn the_preview_answers_a_refusal_as_a_state_and_records_nothing() {
    // The app asks for a preview after every change to a finding, also
    // while a finding is still wrong: the cell's refusal is then the
    // preview's answer, not an error. (For the example applicant the GGD's
    // advice fills every criterion, so no finding is missing.)
    let mut spel = spel_met("gehandicaptenparkeerkaart");
    let casus = spel.casus().unwrap();
    let datum = dag("2026-10-20");
    let aanvraag = spel
        .aanvragen(&map(&casus["voorbeeld"]["aanvraag"]), datum)
        .unwrap();
    let op = aanvraag["id"].as_str().unwrap();
    let voorlopig = spel
        .voorvertoning_besluit(op, &map(&casus["voorbeeld"]["bevindingen"]), datum)
        .unwrap();
    let uitkomst = casus["uitkomst"].as_str().unwrap();
    assert_eq!(
        voorlopig["voorlopig"]["besluit"]["outputs"][uitkomst],
        json!(true)
    );
    assert!(voorlopig["voorlopig"]["waarom"]["keten"].is_array());
    assert_eq!(voorlopig["stil"], json!([]));

    // A finding of the wrong type: the cell refuses it.
    let geweigerd = spel
        .voorvertoning_besluit(
            op,
            &map(&json!({ "geneeskundig_onderzoek_achterwege_gelaten": "ja" })),
            datum,
        )
        .unwrap();
    assert!(geweigerd["geweigerd"].as_str().unwrap().contains("refused"));
    assert!(geweigerd.get("voorlopig").is_none());

    let kroniek = spel.kroniek().unwrap();
    // The application; the GGD's examination from the case file is in the
    // GGD's own chronicle (RFC-052).
    assert_eq!(soorten(&kroniek), ["AANVRAAG"]);
    let ggd = &kroniek["andere"][0];
    assert_eq!(ggd["cel"], json!("ggd_wilgendaal"));
    assert_eq!(ggd["grammen"][0]["kind"], json!("EXECUTOGRAM"));
}

#[test]
fn the_ggd_records_an_examination_and_the_card_follows() {
    // Scenario "zonder onderzoek", continued: the GGD records the
    // examination in its own cell, and then the college can issue the card
    // (RFC-052).
    let mut spel = spel_met("gehandicaptenparkeerkaart");
    let casus = spel.casus().unwrap();
    let mut aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    aanvraag.insert("bsn_aanvrager".into(), json!("999990032"));
    let bevindingen = map(&casus["voorbeeld"]["bevindingen"]);
    let ingediend = spel.aanvragen(&aanvraag, dag("2026-10-13")).unwrap();
    let op = ingediend["id"].as_str().unwrap().to_string();
    assert!(spel
        .besluiten(&op, &bevindingen, dag("2026-10-20"))
        .is_err());

    let vormen = spel.vormen(dag("2026-10-16")).unwrap();
    let ggd = &vormen["andere_cellen"][0];
    assert_eq!(ggd["cel"], json!("ggd_wilgendaal"));
    // The GGD records its advice (the municipality's fictitious policy, art.
    // 5), which also proves the examination.
    let feit =
        "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";
    let feiten: Vec<&Value> = ggd["handelingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| &h["feit"])
        .collect();
    assert!(feiten.contains(&&json!(feit)), "{feiten:?}");
    spel.vastleggen(
        "ggd_wilgendaal",
        feit,
        &map(&json!({
            "bsn_aanvrager": "999990032",
            "datum_onderzoek": "2026-10-16",
            "langdurige_loopbeperking": true,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        })),
        dag("2026-10-16"),
    )
    .unwrap();
    let genomen = spel
        .besluiten(&op, &bevindingen, dag("2026-10-20"))
        .unwrap();
    assert_eq!(
        genomen["besluit"]["outputs"]["gehandicaptenparkeerkaart_verstrekt"],
        json!(true)
    );
}

#[test]
fn parkeerkaart_a_card_granted_on_objection_counts_at_the_next_application() {
    // The scenario "Een kaart na bezwaar telt bij de volgende aanvraag", as
    // the app plays it. The refusal is revoked on objection and a card takes
    // its place (Awb 7:11). At the second application the municipality's
    // reduction finds that card among the decisions that hold, which the
    // reduction of the Awb gives.
    const BSN: &str = "999990068";
    let mut spel = spel_met("gehandicaptenparkeerkaart");
    let casus = spel.casus().unwrap();
    let voorbeeld = &casus["voorbeeld"];
    let uitkomst = casus["uitkomst"].as_str().unwrap().to_string();
    let mut aanvraag = map(&voorbeeld["aanvraag"]);
    aanvraag.insert("bsn_aanvrager".into(), json!(BSN));
    let bevindingen = map(&voorbeeld["bevindingen"]);

    let eerste = spel.aanvragen(&aanvraag, dag("2026-10-13")).unwrap();
    let geweigerd = spel
        .besluiten(
            eerste["id"].as_str().unwrap(),
            &bevindingen,
            dag("2026-10-20"),
        )
        .unwrap();
    assert_eq!(geweigerd["besluit"]["outputs"][&uitkomst], json!(false));

    let mut bezwaar = voorbeeld["bezwaar"].clone();
    bezwaar["dagtekening_bezwaar_of_beroepschrift"] = json!("2026-11-01");
    let bezwaar = spel
        .bezwaren(
            geweigerd["besluit"]["id"].as_str().unwrap(),
            &map(&bezwaar),
            dag("2026-11-01"),
        )
        .unwrap();
    spel.vastleggen(
        "ggd_wilgendaal",
        "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht",
        &map(&json!({
            "bsn_aanvrager": BSN,
            "datum_onderzoek": "2026-11-10",
            "langdurige_loopbeperking": true,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        })),
        dag("2026-11-10"),
    )
    .unwrap();
    let gegrond = spel
        .beslissen_op_bezwaar(
            bezwaar["id"].as_str().unwrap(),
            &Map::new(),
            dag("2026-11-20"),
        )
        .unwrap();
    assert_eq!(
        gegrond["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
        json!(true)
    );
    let nieuw = gegrond["beslissing"]["outputs"]["nieuw_besluit"]
        .as_str()
        .unwrap()
        .to_string();

    let tweede = spel.aanvragen(&aanvraag, dag("2027-03-01")).unwrap();
    let besluit = spel
        .besluiten(
            tweede["id"].as_str().unwrap(),
            &bevindingen,
            dag("2027-03-01"),
        )
        .unwrap();
    assert_eq!(besluit["besluit"]["outputs"][&uitkomst], json!(false));
    let antwoord = besluit["waarom"]["antwoorden"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["begrip"] == json!("aanvrager_heeft_geldige_gehandicaptenparkeerkaart"))
        .unwrap();
    assert_eq!(antwoord["waarde"], json!(true));

    // The reduction of the Awb gives the refusal as revoked. The only decision
    // that holds is the one the decision on objection put in its place, and
    // that one holds the card the cell counted.
    let refusal = geweigerd["besluit"]["id"].as_str().unwrap();
    let id = besluit["besluit"]["id"].as_str().unwrap();
    let hoe = spel
        .hoe_beantwoord(id, "aanvrager_heeft_geldige_gehandicaptenparkeerkaart")
        .unwrap();
    let uitvoer = &hoe["antwoorden"][0]["uitvoer"];
    assert_eq!(uitvoer["herroepen_besluiten"], json!([refusal]));
    let geldend: Vec<&str> = uitvoer["geldende_besluiten"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap())
        .collect();
    assert_eq!(geldend, [nieuw.as_str()]);
}

#[test]
fn parkeerkaart_the_ggd_examines_after_the_application_and_the_card_follows() {
    // The first demo scenario, in the order of the law: the application,
    // then the examination and the advice of the GGD (Regeling art. 2 and 3),
    // then the decision.
    const BSN: &str = "999990032";
    let mut spel = spel_met("gehandicaptenparkeerkaart");
    let casus = spel.casus().unwrap();
    let mut aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    aanvraag.insert("bsn_aanvrager".into(), json!(BSN));
    let bevindingen = map(&casus["voorbeeld"]["bevindingen"]);
    let uitkomst = casus["uitkomst"].as_str().unwrap().to_string();

    let ingediend = spel.aanvragen(&aanvraag, dag("2026-10-13")).unwrap();
    spel.vastleggen(
        "ggd_wilgendaal",
        "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht",
        &map(&json!({
            "bsn_aanvrager": BSN,
            "datum_onderzoek": "2026-10-16",
            "langdurige_loopbeperking": true,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        })),
        dag("2026-10-16"),
    )
    .unwrap();
    let besluit = spel
        .besluiten(
            ingediend["id"].as_str().unwrap(),
            &bevindingen,
            dag("2026-10-20"),
        )
        .unwrap();
    assert_eq!(besluit["besluit"]["outputs"][&uitkomst], json!(true));
    // The decision is in the municipality's chronicle, after the application.
    let kroniek = spel.kroniek().unwrap();
    assert_eq!(soorten(&kroniek), ["AANVRAAG", "BESCHIKKING"]);
}

/// The answer of the cell's sphere to `begrip` in the reasons of `besluit`.
fn antwoord<'a>(besluit: &'a Value, begrip: &str) -> &'a Value {
    &besluit["waarom"]["antwoorden"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["begrip"] == json!(begrip))
        .expect(begrip)["waarde"]
}

/// Apply for a permit for the company `kvk` with the case's example values,
/// on `datum`, and decide on `beslist`.
fn bedrijf_vraagt(spel: &mut Spel, kvk: &str, kenteken: &str, datum: &str, beslist: &str) -> Value {
    bedrijf_vraagt_met(spel, kvk, kenteken, datum, beslist, &Map::new())
}

/// The college's judgement under the hardship clause (article 6 of the
/// fictitious ordinance), which the decision asks only at the maximum.
const HARDHEID: &str = "toepassing_leidt_tot_onaanvaardbare_gevolgen";

/// The college's findings with its judgement under the hardship clause.
fn hardheid(oordeel: bool) -> Map<String, Value> {
    map(&json!({ HARDHEID: oordeel }))
}

/// As [`bedrijf_vraagt`], with the college's `bevindingen`.
fn bedrijf_vraagt_met(
    spel: &mut Spel,
    kvk: &str,
    kenteken: &str,
    datum: &str,
    beslist: &str,
    bevindingen: &Map<String, Value>,
) -> Value {
    let casus = spel.casus().unwrap();
    let mut aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    aanvraag.insert("kvk_nummer".into(), json!(kvk));
    aanvraag.insert("kenteken".into(), json!(kenteken));
    aanvraag.insert("dagtekening".into(), json!(datum));
    let ingediend = spel.aanvragen(&aanvraag, dag(datum)).unwrap();
    spel.besluiten(ingediend["id"].as_str().unwrap(), bevindingen, dag(beslist))
        .unwrap()
}

#[test]
fn bedrijfsparkeervergunning_a_company_in_the_city_centre_gets_a_permit() {
    // The first scenario: the example values, a company the Chamber of
    // Commerce registered in the city centre, and a decision a week later.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let casus = spel.casus().unwrap();
    assert_eq!(
        casus["rol"],
        json!("het college van burgemeester en wethouders")
    );
    assert_eq!(casus["rechter"]["cel"], json!("rechtbank_wilgendaal"));
    let besluit = bedrijf_vraagt(
        &mut spel,
        "99990011",
        "ZZ-001-Z",
        "2026-10-13",
        "2026-10-20",
    );
    let uit = &besluit["besluit"]["outputs"];
    assert_eq!(
        uit["bedrijfsparkeervergunning_verleend"],
        json!(true),
        "{besluit}"
    );
    assert_eq!(uit["vergunning_geldig_tot"], json!("2027-10-19"));
    assert_eq!(
        antwoord(&besluit, "postcode_vestiging_volgens_handelsregister"),
        &json!("7711 AB")
    );
    assert_eq!(
        antwoord(&besluit, "aantal_geldige_bedrijfsparkeervergunningen"),
        &json!(0)
    );
    // The registration is in the Chamber's own chronicle (RFC-052).
    let kroniek = spel.kroniek().unwrap();
    assert_eq!(soorten(&kroniek), ["AANVRAAG", "BESCHIKKING"]);
    assert_eq!(kroniek["andere"][0]["cel"], json!("kamer_van_koophandel"));
    assert_eq!(
        soorten(&kroniek["andere"][0]),
        ["EXECUTOGRAM", "EXECUTOGRAM", "EXECUTOGRAM", "EXECUTOGRAM"]
    );
}

const INSCHRIJVING: &str = "beleid_handelsregister_kamer_van_koophandel#1#vestiging_ingeschreven";

/// The Chamber of Commerce registers the establishment of `kvk` anew on
/// `datum`.
fn kvk_schrijft_in(spel: &mut Spel, kvk: &str, postcode: &str, personen: u32, datum: &str) {
    spel.vastleggen(
        "kamer_van_koophandel",
        INSCHRIJVING,
        &map(&json!({
            "kvk_nummer": kvk,
            "postcode": postcode,
            "aantal_werkzame_personen": personen,
            "datum_inschrijving": datum,
        })),
        dag(datum),
    )
    .unwrap();
}

#[test]
fn bedrijfsparkeervergunning_the_maximum_in_the_ring_and_a_permit_on_objection() {
    // The scenario "Het maximum in de schil, en een vergunning na bezwaar":
    // one person works at the company, so it gets one permit and a second is
    // refused. After the objection the Chamber registers two, the refusal is
    // revoked and a permit takes its place (Awb 7:11). At the third
    // application the municipality counts that permit and the first, among
    // the decisions the reduction of the Awb gives.
    const KVK: &str = "99990022";
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let casus = spel.casus().unwrap();
    let uitkomst = casus["uitkomst"].as_str().unwrap().to_string();

    let eerste = bedrijf_vraagt(&mut spel, KVK, "ZZ-001-Z", "2026-10-13", "2026-10-20");
    assert_eq!(eerste["besluit"]["outputs"][&uitkomst], json!(true));
    let geweigerd = bedrijf_vraagt_met(
        &mut spel,
        KVK,
        "ZZ-002-Z",
        "2026-10-21",
        "2026-10-22",
        &hardheid(false),
    );
    assert_eq!(geweigerd["besluit"]["outputs"][&uitkomst], json!(false));
    assert_eq!(
        antwoord(
            &geweigerd,
            "aantal_werkzame_personen_volgens_handelsregister"
        ),
        &json!(1)
    );
    assert_eq!(
        antwoord(&geweigerd, "aantal_geldige_bedrijfsparkeervergunningen"),
        &json!(1)
    );

    let mut bezwaar = casus["voorbeeld"]["bezwaar"].clone();
    bezwaar["dagtekening_bezwaar_of_beroepschrift"] = json!("2026-11-01");
    let bezwaar = spel
        .bezwaren(
            geweigerd["besluit"]["id"].as_str().unwrap(),
            &map(&bezwaar),
            dag("2026-11-01"),
        )
        .unwrap();
    kvk_schrijft_in(&mut spel, KVK, "7713 AA", 2, "2026-11-10");
    let gegrond = spel
        .beslissen_op_bezwaar(
            bezwaar["id"].as_str().unwrap(),
            &Map::new(),
            dag("2026-11-20"),
        )
        .unwrap();
    assert_eq!(
        gegrond["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
        json!(true),
        "{gegrond}"
    );
    let nieuw = gegrond["beslissing"]["outputs"]["nieuw_besluit"]
        .as_str()
        .unwrap()
        .to_string();

    let derde = bedrijf_vraagt_met(
        &mut spel,
        KVK,
        "ZZ-003-Z",
        "2026-12-01",
        "2026-12-01",
        &hardheid(false),
    );
    assert_eq!(derde["besluit"]["outputs"][&uitkomst], json!(false));
    assert_eq!(
        antwoord(&derde, "aantal_geldige_bedrijfsparkeervergunningen"),
        &json!(2)
    );
    let hoe = spel
        .hoe_beantwoord(
            derde["besluit"]["id"].as_str().unwrap(),
            "aantal_geldige_bedrijfsparkeervergunningen",
        )
        .unwrap();
    let uitvoer = &hoe["antwoorden"][0]["uitvoer"];
    let refusal = geweigerd["besluit"]["id"].as_str().unwrap();
    assert_eq!(uitvoer["herroepen_besluiten"], json!([refusal]));
    let geldend: Vec<&str> = uitvoer["geldende_besluiten"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        geldend,
        [eerste["besluit"]["id"].as_str().unwrap(), nieuw.as_str()]
    );
}

#[test]
fn bedrijfsparkeervergunning_outside_the_area_and_after_the_move_inside() {
    // The scenario "Buiten het gebied, en na de verhuizing erin": the
    // Chamber answers with the registration that holds on the day of each
    // decision, and the old one stays in its chronicle.
    const KVK: &str = "99990033";
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let buiten = bedrijf_vraagt(&mut spel, KVK, "ZZ-001-Z", "2026-10-13", "2026-10-20");
    let uit = &buiten["besluit"]["outputs"];
    assert_eq!(uit["bedrijfsparkeervergunning_verleend"], json!(false));
    assert_eq!(uit["vergunning_geldig_tot"], json!(null));
    assert_eq!(
        antwoord(&buiten, "postcode_vestiging_volgens_handelsregister"),
        &json!("7720 BC")
    );

    kvk_schrijft_in(&mut spel, KVK, "7711 AC", 3, "2026-11-01");
    let binnen = bedrijf_vraagt(&mut spel, KVK, "ZZ-001-Z", "2026-11-02", "2026-11-02");
    assert_eq!(
        binnen["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"],
        json!(true)
    );
    assert_eq!(
        antwoord(&binnen, "postcode_vestiging_volgens_handelsregister"),
        &json!("7711 AC")
    );
    let kvk = &spel.kroniek().unwrap()["andere"][0];
    assert_eq!(soorten(kvk).len(), 5);
}

#[test]
fn bedrijfsparkeervergunning_a_company_the_register_does_not_know_is_refused() {
    // Without a registration the Chamber is silent, so it does not appear
    // from the register that the company is established in an area.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let besluit = bedrijf_vraagt(
        &mut spel,
        "99990099",
        "ZZ-001-Z",
        "2026-10-13",
        "2026-10-20",
    );
    assert_eq!(
        besluit["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"],
        json!(false)
    );
}

#[test]
fn bedrijfsparkeervergunning_two_permits_in_the_city_centre_and_no_third() {
    // Article 3, first paragraph, of the fictitious implementing decision:
    // at most two in the city centre, whatever the number of people. Every
    // decision gets the college's judgement under the hardship clause; only
    // the third reaches the maximum and uses it, and an unasked finding
    // changes nothing.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let mut verleend = Vec::new();
    for (i, datum) in ["2026-10-13", "2026-10-14", "2026-10-15"]
        .iter()
        .enumerate()
    {
        let besluit = bedrijf_vraagt_met(
            &mut spel,
            "99990011",
            &format!("ZZ-00{}-Z", i + 1),
            datum,
            datum,
            &hardheid(false),
        );
        verleend.push(besluit["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"].clone());
    }
    assert_eq!(verleend, [json!(true), json!(true), json!(false)]);
}

/// The home care company in the city centre, which drives to its clients.
const THUISZORG: &str = "99990044";

/// The first steps of the scenario "Het maximum in de binnenstad, en de
/// hardheidsclausule na bezwaar": two permits, a third refused at the
/// maximum, and an objection that names the special circumstance. Gives
/// the refusal and the objection.
fn thuiszorg_tot_bezwaar(spel: &mut Spel) -> (Value, Value) {
    let casus = spel.casus().unwrap();
    let uitkomst = casus["uitkomst"].as_str().unwrap().to_string();
    for (kenteken, datum, beslist) in [
        ("ZZ-001-Z", "2026-10-13", "2026-10-20"),
        ("ZZ-002-Z", "2026-10-21", "2026-10-22"),
    ] {
        let besluit = bedrijf_vraagt(spel, THUISZORG, kenteken, datum, beslist);
        assert_eq!(
            besluit["besluit"]["outputs"][&uitkomst],
            json!(true),
            "{besluit}"
        );
        // Below the maximum the decision asks no judgement under the
        // hardship clause: the condition stops at the first true.
        assert!(
            besluit["besluit"]["fields"].get(HARDHEID).is_none(),
            "{besluit}"
        );
    }

    // The third application reaches the maximum. Only now the preview
    // shows that the decision asks the college's judgement.
    let mut aanvraag = map(&casus["voorbeeld"]["aanvraag"]);
    aanvraag.insert("kvk_nummer".into(), json!(THUISZORG));
    aanvraag.insert("kenteken".into(), json!("ZZ-003-Z"));
    aanvraag.insert("dagtekening".into(), json!("2026-10-23"));
    let derde = spel.aanvragen(&aanvraag, dag("2026-10-23")).unwrap();
    let derde = derde["id"].as_str().unwrap().to_string();
    let vooraf = spel
        .voorvertoning_besluit(&derde, &Map::new(), dag("2026-10-27"))
        .unwrap();
    let gevraagd: Vec<&str> = vooraf["stil"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["naam"].as_str().unwrap())
        .collect();
    assert_eq!(gevraagd, [HARDHEID], "{vooraf}");
    assert_eq!(
        vooraf["stil"][0]["basis"],
        json!("parkeerverordening_gemeente_wilgendaal#6")
    );
    let geweigerd = spel
        .besluiten(&derde, &hardheid(false), dag("2026-10-27"))
        .unwrap();
    assert_eq!(geweigerd["besluit"]["outputs"][&uitkomst], json!(false));
    assert_eq!(
        antwoord(&geweigerd, "aantal_geldige_bedrijfsparkeervergunningen"),
        &json!(2)
    );
    // The decision records the college's judgement as a given.
    assert_eq!(geweigerd["besluit"]["fields"][HARDHEID], json!(false));

    let mut bezwaar = casus["voorbeeld"]["bezwaar"].clone();
    bezwaar["dagtekening_bezwaar_of_beroepschrift"] = json!("2026-11-01");
    bezwaar["gronden_bezwaar_of_beroep"] =
        json!("de wijkverpleging rijdt dagelijks met drie auto's naar cliënten");
    let bezwaar = spel
        .bezwaren(
            geweigerd["besluit"]["id"].as_str().unwrap(),
            &map(&bezwaar),
            dag("2026-11-01"),
        )
        .unwrap();
    // At the reconsideration the college judges anew; the form gives what
    // it judged before.
    let toets = spel
        .heroverweging(bezwaar["id"].as_str().unwrap(), dag("2026-11-20"))
        .unwrap();
    assert_eq!(toets["ontvankelijk"], json!(true), "{toets}");
    let opnieuw: Vec<(&str, &Value)> = toets["bevindingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["naam"].as_str().unwrap(), &v["eerder"]))
        .collect();
    assert_eq!(opnieuw, [(HARDHEID, &json!(false))], "{toets}");
    (geweigerd, bezwaar)
}

#[test]
fn bedrijfsparkeervergunning_the_hardship_clause_on_objection_grants_a_third_permit() {
    // The scenario "Het maximum in de binnenstad, en de hardheidsclausule na
    // bezwaar". The facts stay the same; on reconsideration (Awb 7:11) the
    // college judges that the maximum leads to unacceptable consequences
    // (article 6 of the fictitious ordinance, after article 13 of the
    // Deventer ordinance). The refusal is revoked, a new decision grants the
    // permit, and the next application counts three permits.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let (geweigerd, bezwaar) = thuiszorg_tot_bezwaar(&mut spel);
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
    let nieuw = &gegrond["nieuw_besluit"];
    assert_eq!(
        nieuw["outputs"]["bedrijfsparkeervergunning_verleend"],
        json!(true),
        "{gegrond}"
    );
    assert_eq!(
        nieuw["outputs"]["vergunning_geldig_tot"],
        json!("2027-11-19")
    );
    // The new decision records the judgement that carries it.
    assert_eq!(nieuw["fields"][HARDHEID], json!(true), "{nieuw}");

    let vierde = bedrijf_vraagt_met(
        &mut spel,
        THUISZORG,
        "ZZ-004-Z",
        "2026-12-01",
        "2026-12-01",
        &hardheid(false),
    );
    assert_eq!(
        vierde["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"],
        json!(false)
    );
    assert_eq!(
        antwoord(&vierde, "aantal_geldige_bedrijfsparkeervergunningen"),
        &json!(3)
    );
    let hoe = spel
        .hoe_beantwoord(
            vierde["besluit"]["id"].as_str().unwrap(),
            "aantal_geldige_bedrijfsparkeervergunningen",
        )
        .unwrap();
    let uitvoer = &hoe["antwoorden"][0]["uitvoer"];
    assert_eq!(
        uitvoer["herroepen_besluiten"],
        json!([geweigerd["besluit"]["id"]])
    );
}

#[test]
fn bedrijfsparkeervergunning_the_hardship_clause_not_applied_leaves_the_refusal() {
    // The same objection, and the college judges again that the maximum
    // leads to no unacceptable consequences: unfounded, no new decision, and
    // the next application counts two permits.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let (_, bezwaar) = thuiszorg_tot_bezwaar(&mut spel);
    let ongegrond = spel
        .beslissen_op_bezwaar(
            bezwaar["id"].as_str().unwrap(),
            &hardheid(false),
            dag("2026-11-20"),
        )
        .unwrap();
    assert_eq!(
        ongegrond["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
        json!(false),
        "{ongegrond}"
    );
    assert_eq!(ongegrond["nieuw_besluit"], json!(null));
    let vierde = bedrijf_vraagt_met(
        &mut spel,
        THUISZORG,
        "ZZ-004-Z",
        "2026-12-01",
        "2026-12-01",
        &hardheid(false),
    );
    assert_eq!(
        antwoord(&vierde, "aantal_geldige_bedrijfsparkeervergunningen"),
        &json!(2)
    );
}

#[test]
fn bedrijfsparkeervergunning_the_ordinance_sets_a_term_of_four_weeks() {
    // Article 4 of the fictitious ordinance replaces the reasonable term of
    // Awb 4:13 (eight weeks) with four weeks, only for its own applications.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let casus = spel.casus().unwrap();
    let aanvraag = spel
        .aanvragen(&map(&casus["voorbeeld"]["aanvraag"]), dag("2026-10-13"))
        .unwrap();
    // The chronolexogram holds what was submitted; the term follows from it
    // and the cell computes it when asked.
    assert!(aanvraag.get("outputs").is_none());
    let id = aanvraag["id"].as_str().unwrap();
    assert_eq!(
        spel.wat_volgt(id).unwrap()["redelijke_beslistermijn_einddatum"],
        json!("2026-11-10")
    );
    let mut kaart = spel_met("gehandicaptenparkeerkaart");
    let casus = kaart.casus().unwrap();
    let aanvraag = kaart
        .aanvragen(&map(&casus["voorbeeld"]["aanvraag"]), dag("2026-10-13"))
        .unwrap();
    let id = aanvraag["id"].as_str().unwrap();
    assert_eq!(
        kaart.wat_volgt(id).unwrap()["redelijke_beslistermijn_einddatum"],
        json!("2026-12-08")
    );
}

#[test]
fn bedrijfsparkeervergunning_an_objection_against_a_permit_at_the_cap_does_not_count_it() {
    // A company in the city centre holds two permits and objects against
    // the second. The reconsideration leaves the contested decision out, so
    // that permit does not count against itself: the objection is unfounded.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let casus = spel.casus().unwrap();
    bedrijf_vraagt(
        &mut spel,
        "99990011",
        "ZZ-001-Z",
        "2026-10-13",
        "2026-10-13",
    );
    let tweede = bedrijf_vraagt(
        &mut spel,
        "99990011",
        "ZZ-002-Z",
        "2026-10-14",
        "2026-10-14",
    );
    assert_eq!(
        tweede["besluit"]["outputs"]["bedrijfsparkeervergunning_verleend"],
        json!(true)
    );
    let mut bezwaar = casus["voorbeeld"]["bezwaar"].clone();
    bezwaar["dagtekening_bezwaar_of_beroepschrift"] = json!("2026-11-01");
    let bezwaar = spel
        .bezwaren(
            tweede["besluit"]["id"].as_str().unwrap(),
            &map(&bezwaar),
            dag("2026-11-01"),
        )
        .unwrap();
    let beslissing = spel
        .beslissen_op_bezwaar(
            bezwaar["id"].as_str().unwrap(),
            &Map::new(),
            dag("2026-11-20"),
        )
        .unwrap();
    assert_eq!(
        beslissing["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
        json!(false),
        "{beslissing}"
    );
}

#[test]
fn bedrijfsparkeervergunning_a_permit_granted_on_29_february_runs_to_28_february() {
    // Article 3 of the fictitious ordinance: a year from the day of granting.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    let besluit = bedrijf_vraagt(
        &mut spel,
        "99990011",
        "ZZ-001-Z",
        "2028-02-29",
        "2028-02-29",
    );
    assert_eq!(
        besluit["besluit"]["outputs"]["vergunning_geldig_tot"],
        json!("2029-02-28")
    );
}

#[test]
fn every_case_shows_the_reductions_of_its_cell() {
    // The tab Reductie lists every reduction the cell's catalogue names,
    // with the decisions each reads. A reduction can name the article whose
    // decisions it reads through a parameter of the reduction it calls
    // (`basis` of the reduction of the Awb). The municipality's catalogue
    // serves two cases, so in each case one of its reductions reads the
    // decisions under the other case's article: that article has to be in
    // the case too.
    for id in [
        "bedrijfsparkeervergunning",
        "gehandicaptenparkeerkaart",
        "vakantieverlof",
    ] {
        let mut spel = spel_met(id);
        let stand = spel.reducties(dag("2026-12-01"));
        assert!(stand.is_ok(), "{id}: {}", stand.unwrap_err());
        let stand = stand.unwrap();
        assert!(!stand["reducties"].as_array().unwrap().is_empty(), "{id}");
    }
}

#[test]
fn the_log_holds_the_facts_the_other_cells_recorded() {
    // The registrations of the Chamber of Commerce are facts the decisions
    // read: the log shows them beside the municipality's own steps, those
    // from the case file and one recorded during play, merged by day.
    let mut spel = spel_met("bedrijfsparkeervergunning");
    bedrijf_vraagt(
        &mut spel,
        "99990033",
        "ZZ-001-Z",
        "2026-10-13",
        "2026-10-20",
    );
    kvk_schrijft_in(&mut spel, "99990033", "7711 AC", 3, "2026-11-01");
    let logboek = spel.logboek().unwrap();
    let stappen: Vec<(String, String)> = logboek["handelingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            (
                h["gram"]["recording_actor"].as_str().unwrap().to_string(),
                h["gram"]["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let kvk = (
        "kamer_van_koophandel".to_string(),
        "EXECUTOGRAM".to_string(),
    );
    let eigen = |kind: &str| ("gemeente_wilgendaal".to_string(), kind.to_string());
    assert_eq!(
        stappen,
        [
            kvk.clone(),
            eigen("BESCHIKKING"),
            eigen("AANVRAAG"),
            kvk.clone(),
            kvk.clone(),
            kvk.clone(),
            kvk
        ]
    );
}
