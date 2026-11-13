//! Every scenario of a case, played as the guide plays it ([`speler`]): step
//! by step, with the values its step gives (`invoer`, over the case's
//! `voorbeeld`), and checked against what the step expects (`uitkomst`,
//! `uitvoer`).
//!
//! Without an environment variable this plays the cases of this crate, one
//! test per case, and holds them to more than a source: every step is an
//! action, every form a step fills gets a value for every field, and every
//! chronicle speaks of one applicant.
//!
//! With `POC_BRON` set to the manifest or folder of a source (as
//! `POC_BRON_<NAME>` for the portal), [`the_scenarios_of_the_source_in_poc_bron_play`]
//! plays every scenario of every case of that source. With `POC_SCENARIOS_UIT`
//! set to a folder as well, it writes `laden.json` and one JSON per scenario
//! there. `just poc-chronolexografie-scenarios <manifest> [folder]` runs it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod speler;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use regelrecht_poc_chronolexografie::bundel::{Bron, BronStand, met_bronnen};
use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde_json::{Value, json};
use speler::Verslag;

/// The manifest or folder of the source to play.
const POC_BRON: &str = "POC_BRON";

/// The folder for the reports.
const POC_SCENARIOS_UIT: &str = "POC_SCENARIOS_UIT";

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The repository root: a relative path in `POC_BRON` counts from there, as
/// for `just`.
fn repo() -> PathBuf {
    crate_dir().join("../..")
}

fn spel() -> Spel {
    let dir = crate_dir();
    Spel::new(Corpus::alle(&dir.join("corpus"), &dir.join("corpus/casussen")).unwrap())
}

/// The game on this crate's corpus with the source of `bestanden` (its
/// files by their path in the source), and what became of that source.
fn spel_met_bron(bestanden: BTreeMap<String, String>) -> (Spel, BronStand) {
    let mut files = BTreeMap::new();
    for (pad, tekst) in regelrecht_poc_portal::bron::uit_map(&crate_dir().join("corpus")).unwrap() {
        files.insert(format!("packages/poc-chronolexografie/corpus/{pad}"), tekst);
    }
    let bron = Bron {
        naam: "extern".into(),
        versie: Some("lokaal".into()),
        bestanden,
        ..Bron::default()
    };
    let (alle, standen) = met_bronnen(files, vec![bron]).unwrap();
    let stand = standen[0].clone();
    (Spel::met_bronnen(alle, standen), stand)
}

/// The files of the source at `pad`, a manifest or a folder.
fn bestanden(pad: &Path) -> BTreeMap<String, String> {
    regelrecht_poc_portal::bron::lokaal(pad).unwrap_or_else(|e| panic!("{e}"))
}

/// The number of scenarios of the case `casus_id`.
fn aantal(spel: &mut Spel, casus_id: &str) -> usize {
    spel.kiezen(casus_id).unwrap();
    spel.casus().unwrap()["scenarios"]
        .as_array()
        .map_or(0, Vec::len)
}

/// Every scenario of every case of the source `stand`, played on `spel`.
/// With `uit`, the reports go there: `laden.json` and per scenario
/// `scenario-<case>-<nn>.json`. The answer is what went wrong, per step.
fn speel_bron(spel: &mut Spel, stand: &BronStand, uit: Option<&Path>) -> Vec<String> {
    let mut fouten = Vec::new();
    if let Some(fout) = &stand.fout {
        fouten.push(format!("the source does not load: {fout}"));
    }
    fouten.extend(
        stand
            .fouten
            .iter()
            .map(|f| format!("a case does not load: {f}")),
    );
    if stand.fout.is_none() && stand.casussen.is_empty() {
        fouten.push("the source has no case".into());
    }
    let mut laden = Vec::new();
    let mut verslagen = Vec::new();
    for casus_id in &stand.casussen {
        laden.push(laden_van(spel, casus_id));
        for i in 0..aantal(spel, casus_id) {
            match speler::speel(spel, casus_id, i, uit.is_some()) {
                Ok(verslag) => {
                    fouten.extend(verslag.fouten());
                    verslagen.push(verslag);
                }
                Err(e) => fouten.push(format!("{casus_id}, scenario {i}: {e}")),
            }
        }
    }
    eprintln!("{}", samenvatting(&verslagen));
    if let Some(uit) = uit {
        schrijf(uit, &json!({"bron": stand, "casussen": laden}), &verslagen);
    }
    fouten
}

/// What `laden.json` says of the case `casus_id`: whether it could be
/// chosen, its forms on its playing date, and how many scenarios it has.
fn laden_van(spel: &mut Spel, casus_id: &str) -> Value {
    let gekozen = spel.kiezen(casus_id).map_err(|e| e.to_string());
    let mut uit = json!({
        "casus": casus_id,
        "gekozen": gekozen.is_ok(),
        "kies_fout": gekozen.as_ref().err(),
    });
    if gekozen.is_err() {
        return uit;
    }
    let casus = spel.casus().unwrap();
    let dag = casus["speeldatum"].as_str().unwrap().parse().unwrap();
    match spel.vormen(dag) {
        Ok(v) => {
            let andere: Vec<Value> = v["andere_cellen"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    let feiten: Vec<&Value> = c["handelingen"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|h| &h["feit"])
                        .collect();
                    json!({"cel": c["cel"], "handelingen": feiten})
                })
                .collect();
            uit["vormen"] = json!({
                "aanvraag_velden": speler::namen(&v["aanvraag"]["velden"]),
                "aanvraag_velden_volledig": v["aanvraag"]["velden"],
                "besluit_bevindingen": speler::namen(&v["besluit"]["bevindingen"]),
                "besluit_bevindingen_volledig": v["besluit"]["bevindingen"],
                "besluit_invoer_n": v["besluit"]["invoer"].as_array().map(Vec::len),
                "besluit_invoer": v["besluit"]["invoer"],
                "andere_cellen": andere,
            });
        }
        Err(e) => uit["vormen_fout"] = json!(e.to_string()),
    }
    uit["scenarios"] = json!(casus["scenarios"].as_array().map_or(0, Vec::len));
    uit
}

/// One line on what was played and checked.
fn samenvatting(verslagen: &[Verslag]) -> String {
    let stappen: Vec<&Value> = verslagen.iter().flat_map(Verslag::handelingen).collect();
    let uitkomsten = stappen.iter().filter(|s| s["verwacht"].is_string()).count();
    let uitvoer: usize = stappen
        .iter()
        .map(|s| s["uitvoer"].as_array().map_or(0, Vec::len))
        .sum();
    let fout: usize = verslagen.iter().map(|v| v.fouten).sum();
    format!(
        "{} scenarios, {} steps, {uitkomsten} expected outcomes, {uitvoer} expected outputs, {fout} steps wrong",
        verslagen.len(),
        stappen.len()
    )
}

/// Write `laden` and every report to the folder `uit`.
fn schrijf(uit: &Path, laden: &Value, verslagen: &[Verslag]) {
    std::fs::create_dir_all(uit).unwrap();
    std::fs::write(uit.join("laden.json"), pretty(laden)).unwrap();
    for v in verslagen {
        let naam = format!("scenario-{}-{:02}.json", v.casus, v.index);
        std::fs::write(uit.join(naam), pretty(v)).unwrap();
    }
}

fn pretty(v: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(v).unwrap()
}

/// The path in the environment variable `naam`, if it is set and not
/// empty (`just` passes an argument left out as empty).
fn env_pad(naam: &str) -> Option<PathBuf> {
    std::env::var_os(naam)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// With `POC_BRON` set, every scenario of the source it names plays, with
/// what each step expects. The reports go to `POC_SCENARIOS_UIT` when that
/// is set, before the test fails, so every report is on disk.
#[test]
fn the_scenarios_of_the_source_in_poc_bron_play() {
    let Some(pad) = env_pad(POC_BRON) else {
        return;
    };
    let (mut spel, stand) = spel_met_bron(bestanden(&repo().join(pad)));
    let uit = env_pad(POC_SCENARIOS_UIT).map(|p| repo().join(p));
    let fouten = speel_bron(&mut spel, &stand, uit.as_deref());
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));
}

/// The test source in `tests/bron-scenarios`, by its manifest.
fn testbron() -> BTreeMap<String, String> {
    bestanden(&crate_dir().join("tests/bron-scenarios/bron.yaml"))
}

#[test]
fn every_scenario_of_a_source_plays_from_its_manifest() {
    let (mut spel, stand) = spel_met_bron(testbron());
    assert_eq!(stand.casussen, ["voorbeeld_subsidie"]);
    let uit = tempdir();
    let fouten = speel_bron(&mut spel, &stand, Some(&uit));
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));

    let laden: Value =
        serde_json::from_str(&std::fs::read_to_string(uit.join("laden.json")).unwrap()).unwrap();
    assert_eq!(laden["bron"]["casussen"], json!(["voorbeeld_subsidie"]));
    assert_eq!(laden["casussen"][0]["gekozen"], json!(true));
    assert_eq!(laden["casussen"][0]["scenarios"], json!(3));
    let bezwaar: Value = serde_json::from_str(
        &std::fs::read_to_string(uit.join("scenario-voorbeeld_subsidie-02.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(bezwaar["fouten"], json!(0));
    assert_eq!(bezwaar["stappen"][0]["soort"], json!("tekst"));
    let beslissing = &bezwaar["stappen"][4];
    assert_eq!(beslissing["kreeg"], json!("gegrond"));
    assert_eq!(
        beslissing["uitvoer"],
        json!([{"uitkomst": "subsidie_verleend", "verwacht": true, "kreeg": true, "klopt": true}])
    );
    // What each step added, with why for a decision.
    let besluit = bezwaar["metingen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["na_stap"]["soort"] == json!("besluit"))
        .unwrap();
    assert!(besluit["waarom"]["keten"].is_array(), "{besluit}");
    std::fs::remove_dir_all(&uit).unwrap();
}

/// The test source with the expected output of step 2 of its first scenario
/// set to `waarde`.
fn testbron_met_uitvoer(waarde: Value) -> BTreeMap<String, String> {
    let mut bestanden = testbron();
    let pad = "casussen/voorbeeld_subsidie.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    casus["scenarios"][0]["stappen"][1]["uitvoer"]["subsidie_verleend"] = waarde;
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    bestanden
}

#[test]
fn a_wrong_expected_output_fails_with_the_step_and_both_values() {
    let (mut spel, stand) = spel_met_bron(testbron_met_uitvoer(json!(false)));
    let fouten = speel_bron(&mut spel, &stand, None);
    assert_eq!(
        fouten,
        ["voorbeeld_subsidie, 'Geregistreerd: verleend', step 2: \
             output 'subsidie_verleend': expected false, got true"]
    );
}

#[test]
fn a_wrong_expected_outcome_fails_with_the_step_and_both_outcomes() {
    let mut bestanden = testbron();
    let pad = "casussen/voorbeeld_subsidie.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    casus["scenarios"][1]["stappen"][1]["uitkomst"] = json!("toegewezen");
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    let (mut spel, stand) = spel_met_bron(bestanden);
    let fouten = speel_bron(&mut spel, &stand, None);
    assert_eq!(
        fouten,
        [
            "voorbeeld_subsidie, 'Niet geregistreerd: afgewezen', step 2: expected toegewezen, got afgewezen"
        ]
    );
}

/// A fresh folder in the system's temporary directory, for the reports of
/// one test.
fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "poc-scenarios-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// That a scenario of this crate meets what a source need not: every step
/// is an action, and every form a step fills has a value for every field.
fn streng(verslag: &Verslag) {
    let waar = format!("{}, '{}'", verslag.casus, verslag.scenario);
    for stap in &verslag.stappen {
        assert_ne!(
            stap["soort"],
            json!("tekst"),
            "{waar}: a step of this crate is an action"
        );
        let leeg = stap["leeg_gelaten"].as_array().map_or(0, Vec::len);
        assert_eq!(
            leeg, 0,
            "{waar}, step {}: no value for {}",
            stap["nr"], stap["leeg_gelaten"]
        );
    }
}

/// That every chronicle of a played scenario speaks of one applicant: the
/// visitor reads them all, so an advice about someone else in a chronicle
/// looks like the advice the decision used, whatever the reduction read.
/// The applicant is known by the fields the application shares with what
/// another cell records: the BSN in the advice of the GGD, the KvK number in
/// the registration at the Chamber. The grams of `vooraf`, recorded before
/// the scenario (the case's `begin`), are the world every scenario starts
/// from, not part of it.
fn één_aanvrager(spel: &mut Spel, vooraf: &HashSet<Value>, waar: &str) {
    let casus = spel.casus().unwrap();
    let dag = casus["speeldatum"].as_str().unwrap().parse().unwrap();
    let vormen = spel.vormen(dag).unwrap();
    let aanvraag = casus["voorbeeld"]["aanvraag"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let sleutels: BTreeSet<String> = vormen["andere_cellen"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["handelingen"].as_array().unwrap())
        .flat_map(|h| speler::namen(&h["velden"]))
        .filter(|naam| aanvraag.contains_key(naam))
        .collect();
    let kronieken = speler::kronieken(spel).unwrap();
    let grammen: Vec<&Value> = kronieken
        .iter()
        .flat_map(|(_, k)| speler::grammen(k))
        .filter(|g| !vooraf.contains(&g["id"]))
        .collect();
    for sleutel in &sleutels {
        let mut waarden: Vec<&Value> = grammen
            .iter()
            .filter_map(|g| g["fields"].get(sleutel))
            .collect();
        waarden.sort_by_key(|v| v.to_string());
        waarden.dedup();
        assert!(
            waarden.len() <= 1,
            "{waar}: more than one applicant by '{sleutel}': {waarden:?}"
        );
    }
}

/// One test per case of this crate; each plays all of its scenarios.
fn alle(casus_id: &str) {
    let mut spel = spel();
    let n = aantal(&mut spel, casus_id);
    assert!(n > 0, "{casus_id} has no scenarios");
    for i in 0..n {
        let verslag = speler::speel(&mut spel, casus_id, i, false).unwrap();
        let fouten = verslag.fouten();
        assert!(fouten.is_empty(), "{}", fouten.join("\n"));
        streng(&verslag);
        let waar = format!("{casus_id}, '{}'", verslag.scenario);
        één_aanvrager(&mut spel, &verslag.vooraf, &waar);
    }
}

#[test]
fn every_scenario_of_vakantieverlof_plays_with_its_own_values() {
    alle("vakantieverlof");
}

#[test]
fn every_scenario_of_gehandicaptenparkeerkaart_plays_with_its_own_values() {
    alle("gehandicaptenparkeerkaart");
}

#[test]
fn every_scenario_of_bedrijfsparkeervergunning_plays_with_its_own_values() {
    alle("bedrijfsparkeervergunning");
}

#[test]
fn every_scenario_of_zorgtoeslag_plays_with_its_own_values() {
    alle("zorgtoeslag");
}
