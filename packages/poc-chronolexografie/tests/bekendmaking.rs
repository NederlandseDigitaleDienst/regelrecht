//! Making a decision known as a step of its own, on the fictional procedure
//! of `tests/procedure-zonder-awb`: its article 6 says that the day the
//! service sent the decision is a fact it records afterwards. The decision
//! form does not ask that day; the step that makes the decision known
//! records it, referring to the decision, and the period follows from it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use chrono::NaiveDate;
use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde_json::{Map, Value, json};

const SENT: &str = "fictieve_vergunningwet#6#besluit_verzonden";

fn spel() -> Spel {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/procedure-zonder-awb");
    let mut spel = Spel::new(Corpus::alle(&dir, &dir.join("casussen")).unwrap());
    spel.kiezen("fictieve_vergunning").unwrap();
    spel
}

fn dag(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn map(v: &Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// Apply and decide on the same day; the id of the decision.
fn verleend(spel: &mut Spel, on: &str) -> String {
    let aanvraag = spel
        .indienen(
            "fictieve_vergunningwet#1",
            None,
            &map(&json!({"aanvrager": "D. Voorbeeld", "meerderjarig": true})),
            dag(on),
        )
        .unwrap();
    let id = aanvraag["id"].as_str().unwrap().to_string();
    let besluit = spel.beslissen(&id, None, &Map::new(), dag(on)).unwrap();
    besluit["besluit"]["id"].as_str().unwrap().to_string()
}

#[test]
fn the_decision_form_asks_nothing_of_the_announcement() {
    let mut spel = spel();
    let vormen = spel.vormen(dag("2026-02-02")).unwrap();
    let besluit = &vormen["besluiten"]["fictieve_vergunningwet#1"];
    let gevraagd: Vec<&str> = besluit["bevindingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["naam"].as_str().unwrap())
        .collect();
    assert!(!gevraagd.contains(&"bekendmaking_datum"), "{gevraagd:?}");
    // It comes from the fact, where it used to be the day of the decision.
    let bron = besluit["invoer"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["naam"] == json!("bekendmaking_datum"))
        .map(|i| i["bron"].clone());
    assert_eq!(bron, Some(json!("feit")));
    // The step after the decision: the stage, and the fact it records.
    assert_eq!(besluit["bekendmaking"]["fase"], json!("BEKENDMAKING"));
    assert_eq!(besluit["bekendmaking"]["feiten"][0]["feit"], json!(SENT));
    assert_eq!(
        besluit["bekendmaking"]["feiten"][0]["velden"][0]["naam"],
        json!("bekendmaking_datum")
    );
    // Recorded on a decision, so not among what the service records on its
    // own.
    let eigen: Vec<&str> = vormen["eigen"]["handelingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["feit"].as_str().unwrap())
        .collect();
    assert!(!eigen.contains(&SENT), "{eigen:?}");
    assert!(eigen.contains(&"fictieve_vergunningwet#4#leges_betaald"));
}

#[test]
fn the_period_follows_from_the_record_that_makes_it_known() {
    let mut spel = spel();
    let besluit = verleend(&mut spel, "2026-02-02");
    let voor = spel.bekendmaking(&besluit, dag("2026-02-02")).unwrap();
    assert_eq!(voor["bekendgemaakt"], json!(false));
    assert_eq!(voor["uitvoer"], json!({}));

    spel.vastleggen_verwijzend(
        "fictieve_dienst",
        SENT,
        &map(&json!({"bekendmaking_datum": "2026-02-09"})),
        Some(&besluit),
        dag("2026-02-09"),
    )
    .unwrap();
    let na = spel.bekendmaking(&besluit, dag("2026-02-09")).unwrap();
    assert_eq!(na["bekendgemaakt"], json!(true));
    assert_eq!(na["velden"]["bekendmaking_datum"], json!("2026-02-09"));
    assert_eq!(na["uitvoer"]["termijn_einde"], json!("2026-03-23"));
}

#[test]
fn the_fact_is_recorded_only_on_a_decision_it_makes_known() {
    let mut spel = spel();
    let besluit = verleend(&mut spel, "2026-02-02");
    let velden = map(&json!({"bekendmaking_datum": "2026-02-09"}));
    // Without a reference it is no fact the service records on its own.
    let e = spel
        .vastleggen_verwijzend("fictieve_dienst", SENT, &velden, None, dag("2026-02-09"))
        .unwrap_err();
    assert!(e.to_string().contains("records no fact"), "{e}");
    let betaling = spel
        .vastleggen_verwijzend(
            "fictieve_dienst",
            "fictieve_vergunningwet#4#leges_betaald",
            &map(&json!({"bedrag": 5000})),
            Some(&besluit),
            dag("2026-02-09"),
        )
        .unwrap();
    let e = spel
        .vastleggen_verwijzend(
            "fictieve_dienst",
            SENT,
            &velden,
            Some(betaling["id"].as_str().unwrap()),
            dag("2026-02-09"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("makes a decision known"), "{e}");
}
