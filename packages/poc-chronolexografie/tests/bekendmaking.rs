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

#[test]
fn the_authority_cannot_give_what_the_fact_holds_with_the_decision() {
    let mut spel = spel();
    let aanvraag = spel
        .indienen(
            "fictieve_vergunningwet#1",
            None,
            &map(&json!({"aanvrager": "D. Voorbeeld", "meerderjarig": true})),
            dag("2026-02-02"),
        )
        .unwrap();
    let e = spel
        .beslissen(
            aanvraag["id"].as_str().unwrap(),
            None,
            &map(&json!({"bekendmaking_datum": "2026-02-02"})),
            dag("2026-02-02"),
        )
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "refused: 'bekendmaking_datum' of the decision of fictieve_vergunningwet#1 comes from the fact \
         fictieve_vergunningwet#6#besluit_verzonden, which the authority records after the decision"
    );
}

#[test]
fn a_failed_delivery_the_fact_may_leave_out() {
    let mut spel = spel();
    let besluit = verleend(&mut spel, "2026-02-02");
    // Sent, and the delivery failed: the service sends it again.
    spel.vastleggen_verwijzend(
        "fictieve_dienst",
        SENT,
        &map(&json!({"bekendmaking_datum": "2026-02-09", "bezorging_mislukt": true})),
        Some(&besluit),
        dag("2026-02-09"),
    )
    .unwrap();
    let na = spel.bekendmaking(&besluit, dag("2026-02-09")).unwrap();
    assert_eq!(na["bekendgemaakt"], json!(true));
    assert_eq!(na["uitvoer"]["opnieuw_verzenden"], json!(true));
    // Sent again a week later, delivery not known: the latest record
    // counts, and the field it leaves out is absent.
    spel.vastleggen_verwijzend(
        "fictieve_dienst",
        SENT,
        &map(&json!({"bekendmaking_datum": "2026-02-16"})),
        Some(&besluit),
        dag("2026-02-16"),
    )
    .unwrap();
    let opnieuw = spel.bekendmaking(&besluit, dag("2026-02-16")).unwrap();
    assert_eq!(opnieuw["uitvoer"]["termijn_einde"], json!("2026-03-30"));
    assert_eq!(opnieuw["uitvoer"]["opnieuw_verzenden"], json!(false));
}

/// Article 7, added to a copy of the fixture: a decision that asks the day
/// of its own announcement when it is taken, where the fact of article 6
/// holds that day only after it. A modelling error in article 7, which the
/// cel refuses when article 7 is decided, and only then.
const ARTIKEL_7: &str = r"
  - number: '7'
    url: https://example.invalid/fictieve_vergunningwet#7
    text: >-
      FICTIEF. Op aanvraag verklaart de dienst iets, en het besluit noemt de
      dag waarop het is bekendgemaakt.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: fictieve_vergunningwet#7
            parameters:
              - bekendmaking_datum
          - type: BESCHIKKING
            outputs:
              - verklaard
        parameters:
          - name: bekendmaking_datum
            type: date
            description: De dag waarop het besluit is bekendgemaakt.
        output:
          - name: verklaard
            type: boolean
        actions:
          - output: verklaard
            value: true
";

/// Copy the directory `from` into `to`, file by file.
fn kopieer(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let doel = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            kopieer(&entry.path(), &doel);
        } else {
            std::fs::copy(entry.path(), doel).unwrap();
        }
    }
}

#[test]
fn a_decision_whose_shape_collides_with_the_fact_leaves_the_others_usable() {
    let bron = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/procedure-zonder-awb");
    let stempel = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pad = std::env::temp_dir().join(format!("bekendmaking-{}-{stempel}", std::process::id()));
    kopieer(&bron, &pad);
    let wet = pad.join("regulation/nl/wet/fictieve_vergunningwet/2026-01-01.yaml");
    let tekst = std::fs::read_to_string(&wet).unwrap();
    std::fs::write(&wet, format!("{}\n{ARTIKEL_7}", tekst.trim_end())).unwrap();
    let mut spel = Spel::new(Corpus::alle(&pad, &pad.join("casussen")).unwrap());
    spel.kiezen("fictieve_vergunning").unwrap();

    // The forms load; only the colliding decision carries its refusal.
    let vormen = spel.vormen(dag("2026-02-02")).unwrap();
    let fout = vormen["besluiten"]["fictieve_vergunningwet#7"]["fout"]
        .as_str()
        .unwrap();
    assert!(fout.contains("'bekendmaking_datum'"), "{fout}");
    assert!(fout.contains("fictieve_vergunningwet#7"), "{fout}");
    assert_eq!(
        vormen["besluiten"]["fictieve_vergunningwet#1"]["bekendmaking"]["feiten"][0]["feit"],
        json!(SENT)
    );

    // Article 1 still decides and is made known.
    let besluit = verleend(&mut spel, "2026-02-02");
    spel.vastleggen_verwijzend(
        "fictieve_dienst",
        SENT,
        &map(&json!({"bekendmaking_datum": "2026-02-09"})),
        Some(&besluit),
        dag("2026-02-09"),
    )
    .unwrap();
    let na = spel.bekendmaking(&besluit, dag("2026-02-09")).unwrap();
    assert_eq!(na["uitvoer"]["termijn_einde"], json!("2026-03-23"));

    // Article 7 is refused when it is decided, with the same reason.
    let aanvraag = spel
        .indienen(
            "fictieve_vergunningwet#7",
            None,
            &map(&json!({"bekendmaking_datum": "2026-02-10"})),
            dag("2026-02-10"),
        )
        .unwrap();
    let e = spel
        .beslissen(
            aanvraag["id"].as_str().unwrap(),
            None,
            &Map::new(),
            dag("2026-02-10"),
        )
        .unwrap_err();
    assert!(e.to_string().contains("'bekendmaking_datum'"), "{e}");
    std::fs::remove_dir_all(&pad).unwrap();
}
