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
    Spel::new(Corpus::alle(&dir.join("../../corpus"), &dir.join("casussen")).unwrap())
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
    assert_eq!(ids, ["gehandicaptenparkeerkaart", "vakantieverlof"]);

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
    // The other cells of the sphere (RFC-052), from the case file, the
    // court among them (RFC-053).
    let andere: Vec<&str> = casus["andere_cellen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["cel"].as_str().unwrap())
        .collect();
    assert_eq!(
        andere,
        [
            "ggd_wilgendaal",
            "gemeente_oostmeer",
            "cbr",
            "rechtbank_wilgendaal"
        ]
    );
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
    // The GGD records the examination (Regeling art. 3) and its advice
    // (the municipality's fictitious policy, art. 5).
    let feit = "regeling_gehandicaptenparkeerkaart#3#geneeskundig_onderzoek_verricht";
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
            "verricht_door": "Gemeentelijke Gezondheidsdienst",
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
