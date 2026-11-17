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

#[path = "../common/mod.rs"]
mod common;
mod speler;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use regelrecht_poc_chronolexografie::bundel::{Bron, BronStand, met_bronnen};
use regelrecht_poc_chronolexografie::inlogstap::WACHTWOORD;
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
    spel_in("corpus")
}

/// A fictional procedure that is not the Awb's: no objection, no court, a
/// second application that refers to the decision on the first, and a
/// decision taken ex officio (`tests/procedure-zonder-awb`).
const ZONDER_AWB: &str = "tests/procedure-zonder-awb";

/// The game on the corpus at `root` (relative to this crate), with its
/// cases under `casussen/`.
fn spel_in(root: &str) -> Spel {
    let dir = crate_dir().join(root);
    Spel::new(Corpus::alle(&dir, &dir.join("casussen")).unwrap())
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
fn aantal(spel: &mut Spel, casus_id: &str) -> Result<usize, String> {
    spel.kiezen(casus_id).map_err(|e| e.to_string())?;
    let casus = spel.casus().map_err(|e| e.to_string())?;
    Ok(casus["scenarios"].as_array().map_or(0, Vec::len))
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
        let n = match aantal(spel, casus_id) {
            Ok(n) => n,
            Err(e) => {
                fouten.push(format!("{casus_id}: {e}"));
                continue;
            }
        };
        for i in 0..n {
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
    let Some(dag) = casus["speeldatum"].as_str().and_then(|d| d.parse().ok()) else {
        uit["vormen_fout"] = json!("the case has no playing date");
        return uit;
    };
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
    assert_eq!(laden["casussen"][0]["scenarios"], json!(4));
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

/// The test source of the login step in `tests/bron-inlogstap`, by its
/// manifest.
fn inlogbron() -> BTreeMap<String, String> {
    bestanden(&crate_dir().join("tests/bron-inlogstap/bron.yaml"))
}

#[test]
fn every_scenario_of_a_case_with_a_login_step_plays() {
    let (mut spel, stand) = spel_met_bron(inlogbron());
    assert_eq!(stand.casussen, ["voorbeeld_inlogstap"], "{stand:?}");
    let fouten = speel_bron(&mut spel, &stand, None);
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));
}

/// The authentications in the chronicle of the accredited service of the
/// login test source.
fn authenticaties(spel: &mut Spel) -> usize {
    let kroniek = spel.kroniek().unwrap();
    kroniek["andere"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["cel"] == "voorbeeld_erkende_dienst")
        .flat_map(|c| c["grammen"].as_array().unwrap())
        .filter(|g| g["fact"] == "authenticatie_verricht")
        .count()
}

const DIENST_1: &str = "urn:etoegang:DV:00000009999999997000:services:1";
const DIENST_2: &str = "urn:etoegang:DV:00000009999999997000:services:2";

#[test]
fn a_login_records_the_authentication_and_a_refused_one_records_nothing() {
    let (mut spel, _) = spel_met_bron(inlogbron());
    spel.kiezen("voorbeeld_inlogstap").unwrap();
    let dag = "2026-10-13".parse().unwrap();
    let vooraf = authenticaties(&mut spel);

    let fout = spel.inloggen("bakker", "verkeerd", None, dag).unwrap();
    assert_eq!(
        fout,
        json!({"uitkomst": "geweigerd", "gebruikersnaam": "bakker", "reden": "wachtwoord"})
    );
    // Authorized for another service only, and no means of login at all.
    for naam in ["kapper", "niemand"] {
        let geweigerd = spel.inloggen(naam, WACHTWOORD, None, dag).unwrap();
        assert_eq!(geweigerd["reden"], "geen_machtiging", "{geweigerd}");
    }
    assert_eq!(authenticaties(&mut spel), vooraf);

    let bakker = spel.inloggen("bakker", WACHTWOORD, None, dag).unwrap();
    assert_eq!(bakker["uitkomst"], "ingelogd", "{bakker}");
    assert_eq!(
        bakker["claims"],
        json!({
            "kvk_nummer": "99970011",
            "pseudoniem": "ps-b4k001",
            "dienst": DIENST_1,
            "betrouwbaarheidsniveau": "substantieel",
            "datum_authenticatie": "2026-10-13",
        })
    );
    assert_eq!(authenticaties(&mut spel), vooraf + 1);

    // The lower of the means (hoog) and the authorization (substantieel).
    let bloemist = spel.inloggen("bloemist", WACHTWOORD, None, dag).unwrap();
    assert_eq!(bloemist["claims"]["betrouwbaarheidsniveau"], "substantieel");
    // For the service it is authorized for, the hairdresser gets in.
    let kapper = spel
        .inloggen("kapper", WACHTWOORD, Some(DIENST_2), dag)
        .unwrap();
    assert_eq!(kapper["claims"]["kvk_nummer"], "99970022", "{kapper}");
}

#[test]
fn a_login_after_an_authorization_that_was_not_revoked_gets_in_and_fails_the_step() {
    let mut bestanden = inlogbron();
    let pad = "casussen/voorbeeld_inlogstap.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    // The scenario of the revocation, without the revocation.
    casus["scenarios"][2]["stappen"]
        .as_array_mut()
        .unwrap()
        .remove(2);
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    let (mut spel, stand) = spel_met_bron(bestanden);
    let fouten = speel_bron(&mut spel, &stand, None);
    assert_eq!(
        fouten,
        [
            "voorbeeld_inlogstap, 'Na intrekking komt de bloemist niet meer binnen', step 3: \
             expected geweigerd, got ingelogd"
        ]
    );
}

#[test]
fn a_step_after_a_login_that_names_the_login_field_itself_is_wrong() {
    let mut bestanden = inlogbron();
    let pad = "casussen/voorbeeld_inlogstap.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    casus["scenarios"][0]["stappen"][1]["invoer"] = json!({"kvk_nummer": "99970022"});
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    let (mut spel, stand) = spel_met_bron(bestanden);
    let fouten = speel_bron(&mut spel, &stand, None);
    assert!(
        fouten.iter().any(|f| f.contains(
            "step 2: the step fills 'kvk_nummer', which the session of the login before it fills"
        )),
        "{fouten:?}"
    );
}

#[test]
fn the_form_of_a_case_with_a_login_step_offers_the_step_and_no_choice() {
    let (mut spel, _) = spel_met_bron(inlogbron());
    spel.kiezen("voorbeeld_inlogstap").unwrap();
    let vormen = spel.vormen("2026-10-13".parse().unwrap()).unwrap();
    let inloggen = &vormen["indieningen"]["voorbeeld_vergunning_inlogstap#1"]["inloggen"];
    assert_eq!(inloggen["veld"], "kvk_nummer");
    assert!(inloggen.get("keuzes").is_none(), "{inloggen}");
    assert_eq!(
        inloggen["inlogstap"],
        json!({
            "cel": "voorbeeld_erkende_dienst",
            "cel_naam": "Erkende dienst voor bedrijfs- en organisatiemiddelen",
            "dienst": DIENST_1,
            "wachtwoord": WACHTWOORD,
            "gebruikersnamen": ["bakker", "kapper", "bloemist"],
        })
    );
}

#[test]
fn a_login_step_in_a_case_without_one_does_not_load() {
    let mut bestanden = testbron();
    let pad = "casussen/voorbeeld_subsidie.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    casus["scenarios"][0]["stappen"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({"stap": "inloggen", "invoer": {"gebruikersnaam": "bakker"}}),
        );
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    let (_, stand) = spel_met_bron(bestanden);
    assert!(
        stand
            .fouten
            .iter()
            .any(|f| f.contains("the case has no login step")),
        "{stand:?}"
    );
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

#[test]
fn an_expectation_on_a_step_that_checks_none_fails() {
    let mut bestanden = testbron();
    let pad = "casussen/voorbeeld_subsidie.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    casus["scenarios"][2]["stappen"][3]["uitkomst"] = json!("gegrond");
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    let (mut spel, stand) = spel_met_bron(bestanden);
    let fouten = speel_bron(&mut spel, &stand, None);
    assert_eq!(
        fouten,
        ["voorbeeld_subsidie, 'Bezwaar: gegrond', step 4: a step 'bezwaar' checks no 'uitkomst'"]
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
    let n = aantal(&mut spel, casus_id).unwrap();
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

/// The harness plays a procedure it has no name for, from its YAML alone:
/// the law establishes two applications and no objection, and a decision
/// the cell takes ex officio. Nothing in the harness asks for an objection
/// or an appeal.
#[test]
fn every_scenario_of_a_procedure_without_the_awb_plays_with_its_own_values() {
    let mut spel = spel_in(ZONDER_AWB);
    let casus = spel.kiezen("fictieve_vergunning").unwrap();
    let soorten: Vec<(&str, &str)> = casus["indieningen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["artikel"].as_str().unwrap(), i["soort"].as_str().unwrap()))
        .collect();
    assert_eq!(
        soorten,
        [
            ("fictieve_vergunningwet#1", "AANVRAAG"),
            ("fictieve_vergunningwet#2", "AANVRAAG"),
        ]
    );
    assert_eq!(
        casus["ambtshalve"],
        json!([{ "artikel": "fictieve_vergunningwet#3", "cel": "fictieve_dienst", "cel_naam": "Fictieve dienst" }])
    );
    let n = aantal(&mut spel, "fictieve_vergunning").unwrap();
    assert!(n > 0);
    for i in 0..n {
        let verslag = speler::speel(&mut spel, "fictieve_vergunning", i, false).unwrap();
        let fouten = verslag.fouten();
        assert!(fouten.is_empty(), "{}", fouten.join("\n"));
        streng(&verslag);
    }
    // Each record a generic step makes holds the regulation versions it ran
    // under (`law_versions`): the application, the decision on it, and the
    // decision taken ex officio.
    spel.kiezen("fictieve_vergunning").unwrap();
    let dag0 = "2026-02-02".parse().unwrap();
    let velden = json!({ "aanvrager": "C. Voorbeeld", "meerderjarig": true });
    let aanvraag = spel
        .indienen(
            "fictieve_vergunningwet#1",
            None,
            velden.as_object().unwrap(),
            dag0,
        )
        .unwrap();
    let id = |g: &Value| g["id"].as_str().unwrap().to_string();
    let besluit = spel
        .beslissen(&id(&aanvraag), None, &serde_json::Map::new(), dag0)
        .unwrap()["besluit"]
        .clone();
    let intrekking = spel
        .ambtshalve(
            "fictieve_vergunningwet#3",
            Some(&id(&besluit)),
            &serde_json::Map::new(),
            dag0,
        )
        .unwrap();
    for gram in [&aanvraag, &besluit, &intrekking] {
        let gelezen: Vec<&str> = gram["law_versions"]
            .as_array()
            .unwrap_or_else(|| panic!("no law_versions on {gram}"))
            .iter()
            .filter_map(|r| r["law_id"].as_str())
            .collect();
        assert!(gelezen.contains(&"fictieve_vergunningwet"), "{gram}");
    }
    // The refusal the second scenario expects: article 2 is taken on an
    // application, so no cell takes it ex officio.
    let e = spel
        .ambtshalve(
            "fictieve_vergunningwet#2",
            None,
            &serde_json::Map::new(),
            "2026-03-02".parse().unwrap(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("ex officio"), "{e}");
}

/// The case's own cell records a fact while it handles the case: a payment
/// on its decision, and a refund on that payment, each referring to the
/// record it is about. The form offers it only the facts the law gives it:
/// not a fact of another authority, and not a receipt, which arrives on its
/// own.
#[test]
fn the_case_s_own_cell_records_a_fact_about_its_decision() {
    let mut spel = spel_in(ZONDER_AWB);
    let casus = spel.kiezen("fictieve_vergunning").unwrap();
    let index = casus["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .position(|s| s["naam"] == "Verleend, leges betaald en terugbetaald")
        .unwrap();
    let verslag = speler::speel(&mut spel, "fictieve_vergunning", index, false).unwrap();
    let fouten = verslag.fouten();
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));
    streng(&verslag);

    let kroniek = spel.kroniek().unwrap();
    let grammen: Vec<&Value> = speler::grammen(&kroniek).collect();
    let feit = |naam: &str| {
        *grammen
            .iter()
            .find(|g| g["fact"] == naam)
            .unwrap_or_else(|| panic!("no {naam} in {kroniek}"))
    };
    let besluit = grammen.iter().find(|g| g["kind"] == "BESCHIKKING").unwrap();
    let betaling = feit("leges_betaald");
    let terug = feit("leges_terugbetaald");
    assert_eq!(betaling["refers_to"], besluit["id"]);
    assert_eq!(betaling["recording_actor"], "fictieve_dienst");
    assert_eq!(terug["refers_to"], betaling["id"]);
    let aanvraag = grammen.iter().find(|g| g["kind"] == "AANVRAAG").unwrap();

    let dag = "2026-03-02".parse().unwrap();
    let vormen = spel.vormen(dag).unwrap();
    let eigen: Vec<&str> = vormen["eigen"]["handelingen"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["feit"].as_str().unwrap())
        .collect();
    assert_eq!(
        eigen,
        [
            "fictieve_vergunningwet#4#leges_betaald",
            "fictieve_vergunningwet#4#leges_terugbetaald",
        ]
    );
    let bedrag = json!({ "bedrag": 100 });
    let bedrag = bedrag.as_object().unwrap();
    for (feit, reden) in [
        (
            "fictieve_vergunningwet#5#betaling_gemeld",
            "records no fact",
        ),
        (
            "fictieve_vergunningwet#5#melding_ontvangen",
            "records no fact",
        ),
    ] {
        let e = spel
            .vastleggen("fictieve_dienst", feit, bedrag, dag)
            .unwrap_err();
        assert!(e.to_string().contains(reden), "{feit}: {e}");
    }
    let e = spel
        .vastleggen_verwijzend(
            "fictieve_dienst",
            "fictieve_vergunningwet#4#leges_betaald",
            bedrag,
            aanvraag["id"].as_str(),
            dag,
        )
        .unwrap_err();
    assert!(e.to_string().contains("is a submission"), "{e}");
    let e = spel
        .vastleggen_verwijzend(
            "fictieve_dienst",
            "fictieve_vergunningwet#4#leges_betaald",
            bedrag,
            Some("onbekend"),
            dag,
        )
        .unwrap_err();
    assert!(
        e.to_string().contains("is no record in the chronicle"),
        "{e}"
    );
}

/// The fictional procedure of `ZONDER_AWB` as a source, with the expected
/// answer of the first `antwoorden` in its fees scenario set to `waarde`.
fn leges_met_antwoord(waarde: Value) -> BTreeMap<String, String> {
    leges_met(|a| a["verwacht"] = waarde)
}

/// The fictional procedure of `ZONDER_AWB` as a source, with `wijzig` applied
/// to the first answer its fees scenario expects.
fn leges_met(wijzig: impl FnOnce(&mut Value)) -> BTreeMap<String, String> {
    leges_stappen(|stappen| {
        let stap = stappen
            .iter_mut()
            .find(|s| s.get("antwoorden").is_some())
            .unwrap();
        wijzig(&mut stap["antwoorden"][0]);
    })
}

/// The fictional procedure of `ZONDER_AWB` as a source, with `wijzig`
/// applied to the steps of its fees scenario.
fn leges_stappen(wijzig: impl FnOnce(&mut Vec<Value>)) -> BTreeMap<String, String> {
    let mut bestanden = bestanden(&crate_dir().join(ZONDER_AWB));
    let pad = "casussen/fictieve_vergunning.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    let stappen = casus["scenarios"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["naam"] == "Verleend, leges betaald en terugbetaald")
        .unwrap()["stappen"]
        .as_array_mut()
        .unwrap();
    wijzig(stappen);
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    bestanden
}

/// A step checks an answer of a cell, such as the fees it received and did
/// not refund: the report holds what the cell answered, and a different
/// answer fails with the step and both values.
#[test]
fn a_step_checks_an_answer_of_a_cell() {
    let (mut spel, stand) = spel_met_bron(leges_met_antwoord(json!(5000)));
    let uit = tempdir();
    let fouten = speel_bron(&mut spel, &stand, Some(&uit));
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));
    let verslag: Value = serde_json::from_str(
        &std::fs::read_to_string(uit.join("scenario-fictieve_vergunning-02.json")).unwrap(),
    )
    .unwrap();
    let antwoorden: Vec<&Value> = verslag["stappen"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s.get("antwoorden"))
        .collect();
    assert_eq!(
        antwoorden,
        [
            &json!([{"bron": "fictief_beleid_leges#1#betaalde_leges", "cel": "fictieve_dienst",
                     "verwacht": 5000, "kreeg": 5000, "klopt": true}]),
            &json!([{"bron": "fictief_beleid_leges#1#betaalde_leges", "cel": "fictieve_dienst",
                     "verwacht": 0, "kreeg": 0, "klopt": true}]),
            &json!([{"bron": "fictief_beleid_leges#1#betaalde_leges", "cel": "fictieve_dienst",
                     "verwacht": 0, "kreeg": 0, "klopt": true}]),
        ]
    );
    // The check step (toetsen) did nothing but check its answers.
    assert_eq!(verslag["stappen"][4]["soort"], json!("toetsen"));
    std::fs::remove_dir_all(&uit).unwrap();

    let (mut spel, stand) = spel_met_bron(leges_met_antwoord(json!(4000)));
    let fouten = speel_bron(&mut spel, &stand, None);
    assert_eq!(
        fouten,
        [
            "fictieve_vergunning, 'Verleend, leges betaald en terugbetaald', step 3: answer \
          'fictief_beleid_leges#1#betaalde_leges' of 'fictieve_dienst': expected 4000, got 5000"
        ]
    );
}

/// An answer the cell refuses is no answer: the report holds the reason, and
/// the step is wrong.
#[test]
fn a_refused_answer_makes_the_step_wrong() {
    let bron = "reductie_leges#betaalde_leges#betaalde_leges";
    let (mut spel, stand) = spel_met_bron(leges_met(|a| a["bron"] = json!(bron)));
    let uit = tempdir();
    let fouten = speel_bron(&mut spel, &stand, Some(&uit));
    assert_eq!(fouten.len(), 1, "{fouten:?}");
    assert!(
        fouten[0].contains(&format!(
            "step 3: answer '{bron}' of 'fictieve_dienst': expected 5000, got null ("
        )),
        "{}",
        fouten[0]
    );
    assert!(fouten[0].contains("RFC-056"), "{}", fouten[0]);
    let verslag: Value = serde_json::from_str(
        &std::fs::read_to_string(uit.join("scenario-fictieve_vergunning-02.json")).unwrap(),
    )
    .unwrap();
    let rij = &verslag["stappen"][2]["antwoorden"][0];
    assert_eq!(
        (&rij["kreeg"], &rij["klopt"]),
        (&Value::Null, &json!(false))
    );
    assert!(rij["fout"].as_str().unwrap().contains("RFC-056"), "{rij}");
    std::fs::remove_dir_all(&uit).unwrap();
}

/// An answer comes from a law or the cell's own policy, as `Cell::consult`
/// computes it; a reduction is no ground (RFC-056), and the step fails.
#[test]
fn an_answer_straight_from_a_reduction_is_refused() {
    let mut spel = spel_in(ZONDER_AWB);
    spel.kiezen("fictieve_vergunning").unwrap();
    let e = spel
        .antwoord(
            None,
            "reductie_leges#betaalde_leges#betaalde_leges",
            &serde_json::Map::new(),
            "2026-03-02".parse().unwrap(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("RFC-056"), "{e}");
    let e = spel
        .antwoord(
            Some("elders"),
            "fictief_beleid_leges#1#betaalde_leges",
            &serde_json::Map::new(),
            "2026-03-02".parse().unwrap(),
        )
        .unwrap_err();
    assert!(e.to_string().contains("no other cell"), "{e}");
}

/// A check step (toetsen) does nothing; on its day it checks the answers it
/// expects, and a different answer fails on that step.
#[test]
fn a_check_step_fails_on_a_different_answer() {
    let (mut spel, stand) = spel_met_bron(leges_stappen(|stappen| {
        let toets = stappen.last_mut().unwrap();
        assert_eq!(toets["stap"], json!("toetsen"));
        toets["antwoorden"][0]["verwacht"] = json!(4000);
    }));
    let fouten = speel_bron(&mut spel, &stand, None);
    assert_eq!(
        fouten,
        [
            "fictieve_vergunning, 'Verleend, leges betaald en terugbetaald', step 5: answer \
          'fictief_beleid_leges#1#betaalde_leges' of 'fictieve_dienst': expected 4000, got 0"
        ]
    );
}

/// A parameter of an answer names a record by the kenmerk of a step. A
/// kenmerk no step gives, or one a later step gives, is an error of the case
/// when it loads, not an empty parameter when it plays.
#[test]
fn an_answer_names_a_record_by_the_kenmerk_of_a_step_made_by_then() {
    for (kenmerk, melding) in [
        (
            "onbekend",
            "parameters name 'onbekend', which is the kenmerk of no step",
        ),
        (
            "betaling",
            "parameters name 'betaling', which is the kenmerk of a later step",
        ),
    ] {
        let (mut spel, stand) = spel_met_bron(leges_stappen(|stappen| {
            // The decision, before the payment that gives `betaling`.
            let besluit = stappen
                .iter_mut()
                .find(|s| s["stap"] == json!("beslissen"))
                .unwrap();
            besluit["antwoorden"] = json!([{
                "bron": "fictief_beleid_leges#1#betaalde_leges",
                "invoer": {"betaling": {"kenmerk": kenmerk}},
                "verwacht": 0,
            }]);
        }));
        let fouten = speel_bron(&mut spel, &stand, None);
        assert!(
            fouten.iter().any(|f| f.contains(melding)),
            "{kenmerk}: {fouten:?}"
        );
    }
}

/// The fictional procedure of `ZONDER_AWB` as a source, with `wijzig`
/// applied to the steps of its scenario that makes a decision known, and
/// `artikel` added to its law.
fn bekendmaking_stappen(
    artikel: &str,
    wijzig: impl FnOnce(&mut Vec<Value>),
) -> BTreeMap<String, String> {
    let mut bestanden = bestanden(&crate_dir().join(ZONDER_AWB));
    let wet = "regulation/nl/wet/fictieve_vergunningwet/2026-01-01.yaml";
    let tekst = format!("{}\n{artikel}", bestanden[wet].trim_end());
    bestanden.insert(wet.into(), tekst);
    let pad = "casussen/fictieve_vergunning.yaml";
    let mut casus: Value = serde_yaml_ng::from_str(&bestanden[pad]).unwrap();
    let stappen = casus["scenarios"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["naam"] == "Verleend en daarna bekendgemaakt")
        .unwrap()["stappen"]
        .as_array_mut()
        .unwrap();
    wijzig(stappen);
    bestanden.insert(pad.into(), serde_yaml_ng::to_string(&casus).unwrap());
    bestanden
}

/// A step that records the fact making a decision known checks, under
/// `uitvoer`, what the announcement then gives: the decision step cannot,
/// as the fact does not exist yet when the decision is taken.
#[test]
fn a_step_that_makes_a_decision_known_checks_what_the_announcement_gives() {
    let met = |termijn: &str| {
        bekendmaking_stappen("", |stappen| {
            let stap = stappen.last_mut().unwrap();
            assert_eq!(stap["stap"], json!("vastleggen"));
            stap["uitvoer"] = json!({"termijn_einde": termijn});
        })
    };
    let (mut spel, stand) = spel_met_bron(met("2026-03-23"));
    let fouten = speel_bron(&mut spel, &stand, None);
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));
    let (mut spel, stand) = spel_met_bron(met("2026-03-24"));
    assert_eq!(
        speel_bron(&mut spel, &stand, None),
        [
            "fictieve_vergunning, 'Verleend en daarna bekendgemaakt', step 3: output \
             'termijn_einde': expected \"2026-03-24\", got \"2026-03-23\""
        ]
    );
}

/// Article 8, added to the fictional law: a request against a decision,
/// which takes the period that runs from its announcement.
const VERZOEK_TEGEN_BESLUIT: &str = r"
  - number: '8'
    url: https://example.invalid/fictieve_vergunningwet#8
    text: >-
      FICTIEF. De houder van een vergunning kan de dienst om uitleg vragen.
      De dienst noemt daarbij of de termijn van artikel 6 bekend is.
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: fictieve_vergunningwet#8
            parameters:
              - uit_te_leggen_besluit
              - termijn_einde
          - type: BESCHIKKING
            outputs:
              - termijn_bekend
        parameters:
          - name: uit_te_leggen_besluit
            type: decision
          - name: termijn_einde
            type: date
            nullable: true
        output:
          - name: termijn_bekend
            type: boolean
        actions:
          - output: termijn_bekend
            value:
              operation: NOT
              value:
                operation: EQUALS
                subject: $termijn_einde
                value: null
";

/// A submission against a decision that is made known by a fact takes the
/// period from the announcement, after the fact is recorded: the player
/// leaves the field to the cell, as the form does, and does not supply it
/// empty.
#[test]
fn a_submission_after_the_announcement_takes_the_period_from_it() {
    let (mut spel, stand) = spel_met_bron(bekendmaking_stappen(VERZOEK_TEGEN_BESLUIT, |stappen| {
        stappen.push(json!({
            "stap": "indienen",
            "artikel": "fictieve_vergunningwet#8",
            "verwijst_naar": "vergunning",
            "kenmerk": "verzoek",
            "datum": "2026-02-12",
        }));
        stappen.push(json!({
            "stap": "beslissen",
            "verwijst_naar": "verzoek",
            "datum": "2026-02-12",
            "uitvoer": {"termijn_bekend": true},
        }));
    }));
    let fouten = speel_bron(&mut spel, &stand, None);
    assert!(fouten.is_empty(), "{}", fouten.join("\n"));
}
