//! Every scenario of every case of this crate, played as the guide plays it:
//! step by step, with exactly the values its step in the case YAML gives
//! (`invoer`, over the case's `voorbeeld`), and checked against what the
//! step expects (`uitkomst`). Each form a step fills must get a value for
//! every field the law asks of it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use chrono::NaiveDate;
use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde_json::{json, Map, Value};

fn spel() -> Spel {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    Spel::new(Corpus::alle(&dir.join("corpus"), &dir.join("corpus/casussen")).unwrap())
}

fn map(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn dag(v: &Value) -> NaiveDate {
    v.as_str().unwrap().parse().unwrap()
}

/// `basis` with `invoer` over it.
fn met(basis: &Value, invoer: &Value) -> Map<String, Value> {
    let mut uit = map(basis);
    uit.extend(map(invoer));
    uit
}

/// The names of the fields in `velden` (a form as `vormen` gives it).
fn namen(velden: &Value) -> Vec<String> {
    velden
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["naam"].as_str().unwrap().to_string())
        .collect()
}

/// That `gegeven` has a value for every field of `velden`.
fn alles_gevuld(velden: &Value, gegeven: &Map<String, Value>, waar: &str) {
    for naam in namen(velden) {
        assert!(gegeven.contains_key(&naam), "{waar}: no value for '{naam}'");
    }
}

/// The fields of the form `velden` that the submitter fills against the
/// decision `id` of the case's own cell: not those the cell takes from that
/// decision (its outputs and fields, such as the term of Awb 6:8), as the
/// form on the page leaves them out.
fn tegen(spel: &mut Spel, velden: &Value, id: &str) -> Value {
    let kroniek = spel.kroniek().unwrap();
    let besluit = kroniek["grammen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == json!(id))
        .unwrap()
        .clone();
    let in_besluit = |naam: &str| {
        besluit["outputs"].get(naam).is_some() || besluit["fields"].get(naam).is_some()
    };
    Value::Array(
        velden
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| !in_besluit(v["naam"].as_str().unwrap()))
            .cloned()
            .collect(),
    )
}

/// The refusal a step expects (`uitkomst: geweigerd`), by kind, as the guide
/// recognises it (frontend/src/scenarioGuides.js): a decision that misses a
/// judgement, a decision on objection against a decision already replaced.
fn geweigerd(e: &impl std::fmt::Display, soort: &str, waar: &str) {
    let melding = e.to_string();
    let delen: &[&str] = match soort {
        "besluit" => &["no judgement was given", "asks '"],
        _ => &["already replaces"],
    };
    assert!(
        delen.iter().any(|d| melding.contains(d)),
        "{waar}: refused for another reason: {melding}"
    );
}

/// What a scenario has done so far, to find what a step acts on.
#[derive(Default)]
struct Stand {
    /// Per application in order: its id and the first decision on it.
    aanvragen: Vec<(String, Option<String>)>,
    /// Per objection in order: its id and whether it was decided.
    bezwaren: Vec<(String, bool)>,
    /// The decisions on objection, in order.
    beslissingen: Vec<String>,
    beroepen: Vec<String>,
    uitspraken: Vec<String>,
    ontvangsten: Vec<String>,
}

/// The step `n` (counted from 1) of `lijst`, or else the one `standaard`
/// picks.
fn kies<T: Clone>(lijst: &[T], op: &Value, standaard: impl Fn(&[T]) -> Option<usize>) -> T {
    let i = match op.as_u64() {
        Some(n) => n as usize - 1,
        None => standaard(lijst).expect("nothing to act on"),
    };
    lijst[i].clone()
}

/// That the decision `besluit` has each output the step expects
/// (`uitvoer`), with that value.
fn uitvoer_klopt(stap: &Value, besluit: &Value, waar: &str) {
    for (naam, verwacht) in map(&stap["uitvoer"]) {
        assert_eq!(
            besluit["outputs"][&naam], verwacht,
            "{waar}: output '{naam}'"
        );
    }
}

/// The outcome of a decision on objection as a step writes it.
fn op_bezwaar(beslissing: &Value) -> &'static str {
    let uit = &beslissing["outputs"];
    if uit["bezwaar_ontvankelijk"] == json!(false) {
        "niet_ontvankelijk"
    } else if uit["bestreden_besluit_herroepen"] == json!(true) {
        "gegrond"
    } else {
        "ongegrond"
    }
}

fn speel(casus_id: &str, index: usize) {
    let mut spel = spel();
    spel.kiezen(casus_id).unwrap();
    let casus = spel.casus().unwrap();
    let scenario = &casus["scenarios"][index];
    let naam = scenario["naam"].as_str().unwrap().to_string();
    let voorbeeld = &casus["voorbeeld"];
    let uitkomst = casus["uitkomst"].as_str().unwrap();
    let mut datum = dag(&casus["speeldatum"]);
    let mut stand = Stand::default();
    // Per kind, how many steps of it came so far: without `op`, the n-th
    // decision step acts on the n-th application, as the guide counts.
    let mut geteld: std::collections::HashMap<String, usize> = Default::default();
    for (i, stap) in scenario["stappen"].as_array().unwrap().iter().enumerate() {
        let waar = format!("{casus_id}, '{naam}', step {}", i + 1);
        assert!(
            stap.is_object(),
            "{waar}: a step of this crate is an action"
        );
        if let Some(d) = stap.get("datum") {
            datum = dag(d);
        }
        let invoer = &stap["invoer"];
        let verwacht = stap["uitkomst"].as_str();
        let vormen = spel.vormen(datum).unwrap();
        let soort = stap["stap"].as_str().unwrap();
        let n = *geteld
            .entry(soort.to_string())
            .and_modify(|n| *n += 1)
            .or_insert(1);
        let doel = stap["op"].as_u64().map_or(n, |op| op as usize) - 1;
        match soort {
            "aanvraag" => {
                let velden = met(&voorbeeld["aanvraag"], invoer);
                alles_gevuld(&vormen["aanvraag"]["velden"], &velden, &waar);
                let gram = spel.aanvragen(&velden, datum).unwrap();
                stand
                    .aanvragen
                    .push((gram["id"].as_str().unwrap().to_string(), None));
            }
            "besluit" => {
                let op = stand.aanvragen[doel].0.clone();
                let bevindingen = met(&voorbeeld["bevindingen"], invoer);
                alles_gevuld(&vormen["besluit"]["bevindingen"], &bevindingen, &waar);
                let genomen = match stap["artikel"].as_str() {
                    Some(basis) => spel.besluiten_onder(&op, basis, &bevindingen, datum),
                    None => spel.besluiten(&op, &bevindingen, datum),
                };
                if verwacht == Some("geweigerd") {
                    let e = genomen
                        .err()
                        .unwrap_or_else(|| panic!("{waar}: the cell decides"));
                    geweigerd(&e, soort, &waar);
                    continue;
                }
                let genomen = genomen.unwrap_or_else(|e| panic!("{waar}: {e}"));
                stand.aanvragen[doel].1 =
                    Some(genomen["besluit"]["id"].as_str().unwrap().to_string());
                uitvoer_klopt(stap, &genomen["besluit"], &waar);
                let uitkomst = casus["besluiten"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|b| b["basis"] == genomen["besluit"]["basis"])
                    .map_or(uitkomst, |b| b["uitkomst"].as_str().unwrap());
                let toegewezen = genomen["besluit"]["outputs"][uitkomst] == json!(true);
                if let Some(v) = verwacht {
                    let kreeg = if toegewezen {
                        "toegewezen"
                    } else {
                        "afgewezen"
                    };
                    assert_eq!(kreeg, v, "{waar}");
                }
            }
            "bezwaar" => {
                let (_, besluit) = kies(&stand.aanvragen, &stap["op"], |l| l.len().checked_sub(1));
                let besluit = besluit.unwrap();
                let mut velden = met(&voorbeeld["bezwaar"], invoer);
                velden
                    .entry("dagtekening_bezwaar_of_beroepschrift")
                    .or_insert(json!(datum));
                let gevraagd = tegen(&mut spel, &vormen["bezwaar"]["velden"], &besluit);
                alles_gevuld(&gevraagd, &velden, &waar);
                let gram = spel.bezwaren(&besluit, &velden, datum).unwrap();
                stand
                    .bezwaren
                    .push((gram["id"].as_str().unwrap().to_string(), false));
            }
            "beslissing_op_bezwaar" => {
                let op = stand.bezwaren[doel].0.clone();
                spel.heroverweging(&op, datum).unwrap();
                let genomen = spel.beslissen_op_bezwaar(&op, &map(invoer), datum);
                if verwacht == Some("geweigerd") {
                    let e = genomen
                        .err()
                        .unwrap_or_else(|| panic!("{waar}: the cell decides"));
                    geweigerd(&e, soort, &waar);
                    continue;
                }
                let genomen = genomen.unwrap_or_else(|e| panic!("{waar}: {e}"));
                stand.bezwaren[doel].1 = true;
                stand
                    .beslissingen
                    .push(genomen["beslissing"]["id"].as_str().unwrap().to_string());
                if stap.get("uitvoer").is_some() {
                    assert!(
                        genomen["nieuw_besluit"].is_object(),
                        "{waar}: no new decision to check the outputs of"
                    );
                    uitvoer_klopt(stap, &genomen["nieuw_besluit"], &waar);
                }
                if let Some(v) = verwacht {
                    assert_eq!(op_bezwaar(&genomen["beslissing"]), v, "{waar}");
                }
            }
            "vastleggen" => {
                let cel = stap["cel"].as_str().unwrap();
                let feit = stap["feit"].as_str().unwrap();
                let handeling = vormen["andere_cellen"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["cel"] == json!(cel))
                    .and_then(|c| {
                        c["handelingen"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|h| h["feit"] == json!(feit))
                            .cloned()
                    })
                    .unwrap_or_else(|| panic!("{waar}: {cel} records no {feit}"));
                let velden = map(invoer);
                alles_gevuld(&handeling["velden"], &velden, &waar);
                spel.vastleggen(cel, feit, &velden, datum).unwrap();
            }
            "beroep" => {
                let besluit = kies(&stand.beslissingen, &stap["op"], |l| l.len().checked_sub(1));
                let mut velden = met(&voorbeeld["beroep"], invoer);
                velden
                    .entry("dagtekening_bezwaar_of_beroepschrift")
                    .or_insert(json!(datum));
                let gevraagd = tegen(&mut spel, &vormen["beroep"]["velden"], &besluit);
                alles_gevuld(&gevraagd, &velden, &waar);
                let gram = spel.beroep_instellen(&besluit, &velden, datum).unwrap();
                stand
                    .beroepen
                    .push(gram["id"].as_str().unwrap().to_string());
            }
            "uitspraak" => {
                let beroep = kies(&stand.beroepen, &stap["op"], |l| l.len().checked_sub(1));
                let bevindingen = met(&voorbeeld["uitspraak"], invoer);
                let gedaan = spel.uitspraak_doen(&beroep, &bevindingen, datum).unwrap();
                stand
                    .uitspraken
                    .push(gedaan["uitspraak"]["id"].as_str().unwrap().to_string());
            }
            "toezenden" => {
                let uitspraak = kies(&stand.uitspraken, &stap["op"], |l| l.len().checked_sub(1));
                let gram = spel.toezenden(&uitspraak, datum).unwrap();
                stand
                    .ontvangsten
                    .push(gram["id"].as_str().unwrap().to_string());
            }
            "opnieuw_beslissen" => {
                let ontvangst = kies(&stand.ontvangsten, &stap["op"], |l| l.len().checked_sub(1));
                spel.heroverweging_na_uitspraak(&ontvangst).unwrap();
                let genomen = spel
                    .opnieuw_beslissen_op_bezwaar(&ontvangst, &map(invoer), datum)
                    .unwrap_or_else(|e| panic!("{waar}: {e}"));
                if let Some(v) = verwacht {
                    assert_eq!(op_bezwaar(&genomen["beslissing"]), v, "{waar}");
                }
            }
            "tijd_verstrijkt" => {
                assert!(
                    !vormen["tijd"].as_array().unwrap().is_empty(),
                    "{waar}: no fact of this case arises as time passes"
                );
                let vastgelegd = spel
                    .tijd_verstrijkt(datum)
                    .unwrap_or_else(|e| panic!("{waar}: {e}"));
                for (naam, verwacht) in map(&stap["uitvoer"]) {
                    let kreeg: Vec<Value> = vastgelegd
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|g| g["fields"][&naam].clone())
                        .collect();
                    assert_eq!(Value::Array(kreeg), verwacht, "{waar}: field '{naam}'");
                }
            }
            soort => panic!("{waar}: unknown step '{soort}'"),
        }
    }
}

/// One test per case; each plays all of its scenarios.
fn alle(casus_id: &str) {
    let mut spel = spel();
    spel.kiezen(casus_id).unwrap();
    let n = spel.casus().unwrap()["scenarios"].as_array().unwrap().len();
    assert!(n > 0, "{casus_id} has no scenarios");
    for i in 0..n {
        speel(casus_id, i);
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
