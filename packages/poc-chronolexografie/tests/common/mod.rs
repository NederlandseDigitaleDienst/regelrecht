//! The steps of the Awb procedure as these tests name them, each played
//! through the generic steps of the game ([`Spel::indienen`],
//! [`Spel::beslissen`], [`Spel::voor_beslissen`], [`Spel::voorvertoning`]),
//! which name none. The tests are about cases under the Awb, so they may
//! speak of an objection; the harness does not. The articles come from what
//! the game derived from the law (`indieningen` of the case), by the kind of
//! submission.

// Each test file uses its own part of these.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::NaiveDate;
use regelrecht_poc_chronolexografie::spel::{Antwoord, Spel};
use serde_json::{Map, Value, json};

/// `bevindingen` for a decision on an objection, with what the authority
/// judges on admissibility (Awb 7:11) where `bevindingen` says nothing: the
/// objector is an interested party (1:2, 7:1 and 8:1), and was at fault for
/// a late objection, so 6:11 does not save it.
pub fn ontvankelijk(bevindingen: &Map<String, Value>) -> Map<String, Value> {
    let mut uit = bevindingen.clone();
    uit.entry("indiener_bezwaarschrift_is_belanghebbende")
        .or_insert(json!(true));
    uit.entry("indiener_bezwaarschrift_niet_in_verzuim")
        .or_insert(json!(false));
    uit
}

/// The submission of kind `soort` the case `casus` describes, as the game
/// derived it from the law: its article, the decision it asks for, the cell
/// that decides on it and the fact of the receipt of a copy of that
/// decision.
pub fn indiening_van(casus: &Value, soort: &str) -> Value {
    casus["indieningen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["soort"] == json!(soort))
        .cloned()
        .unwrap_or_else(|| panic!("no submission {soort} in {}", casus["id"]))
}

/// The article that establishes the submission of kind `soort` in the
/// chosen case.
pub fn artikel_van(spel: &mut Spel, soort: &str) -> String {
    let casus = spel.casus().unwrap();
    casus["indieningen"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["soort"] == json!(soort))
        .unwrap_or_else(|| panic!("no submission {soort} in {}", casus["id"]))["artikel"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The forms on `datum` under the names of the Awb: per submission the
/// form of its kind (`aanvraag`, `bezwaar`, `beroep`), and the form of the
/// decision it asks for (`besluit`, `beslissing_op_bezwaar`, `uitspraak`),
/// beside the forms per article the game gives.
pub fn awb_vormen(spel: &mut Spel, datum: NaiveDate) -> Value {
    let casus = spel.casus().unwrap();
    met_awb_namen(&casus, spel.vormen(datum).unwrap())
}

/// The forms `vormen` of the case `casus` with the names of the Awb steps
/// beside them ([`awb_vormen`]).
pub fn met_awb_namen(casus: &Value, vormen: Value) -> Value {
    let mut v = vormen;
    let artikel = casus["artikel"].as_str().unwrap_or_default();
    let besluit = casus["besluit"].as_str().unwrap_or_default();
    v["aanvraag"] = v["indieningen"][artikel].clone();
    v["besluit"] = v["besluiten"][besluit].clone();
    for (soort, indiening, beslissing) in [
        ("BEZWAAR", "bezwaar", "beslissing_op_bezwaar"),
        ("BEROEP", "beroep", "uitspraak"),
    ] {
        let gevonden = casus["indieningen"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|i| i["soort"] == json!(soort))
            .cloned();
        if let Some(i) = gevonden {
            v[indiening] = v["indieningen"][i["artikel"].as_str().unwrap_or_default()].clone();
            v[beslissing] = v["besluiten"][i["besluit"].as_str().unwrap_or_default()].clone();
        }
    }
    v
}

/// A preview with its outcome as `f` gives it.
fn als(v: Value, f: fn(Value) -> Value) -> Value {
    let mut v = v;
    if let Some(voorlopig) = v.get_mut("voorlopig") {
        *voorlopig = f(voorlopig.take());
    }
    v
}

/// A decision on an objection under the names of the Awb.
fn op_bezwaar(b: Value) -> Value {
    json!({
        "beslissing": b["besluit"],
        "nieuw_besluit": b["in_de_plaats"].get("nieuw_besluit").cloned().unwrap_or(Value::Null),
        "waarom": b["waarom"],
    })
}

/// A judgment under the names of the Awb.
fn uitspraak(b: Value) -> Value {
    json!({
        "uitspraak": b["besluit"],
        "niet_gevraagd": b["niet_gevraagd"],
        "waarom": b["waarom"],
    })
}

/// The record `id` in any chronicle of the case.
fn gram(spel: &mut Spel, id: &str) -> Value {
    let kroniek = spel.kroniek().unwrap();
    std::iter::once(&kroniek)
        .chain(kroniek["andere"].as_array().unwrap())
        .flat_map(|k| k["grammen"].as_array().unwrap())
        .find(|g| g["id"] == json!(id))
        .cloned()
        .unwrap_or_else(|| panic!("no record '{id}'"))
}

/// The receipt `ontvangst` of a judgment and what it leads back to by
/// `refers_to`: the decision on objection, the objection, and the decision
/// it contested.
fn na_uitspraak(spel: &mut Spel, ontvangst: &str) -> [Value; 4] {
    let receipt = gram(spel, ontvangst);
    let mut volgende = |g: &Value| gram(spel, g["refers_to"].as_str().unwrap());
    let beslissing = volgende(&receipt);
    let bezwaar = volgende(&beslissing);
    let bestreden = volgende(&bezwaar);
    [receipt, beslissing, bezwaar, bestreden]
}

pub trait Awb {
    fn aanvragen(&mut self, velden: &Map<String, Value>, datum: NaiveDate) -> Antwoord;
    fn besluiten(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn voorvertoning_besluit(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn bezwaren(
        &mut self,
        besluit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn heroverweging(
        &mut self,
        bezwaar: &str,
        ontvankelijkheid: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn beslissen_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn voorvertoning_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn beroep_instellen(
        &mut self,
        besluit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn uitspraak_doen(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn voorvertoning_uitspraak(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn waarom_uitspraak(&mut self, id: &str) -> Antwoord;
    fn toetsen_en_beslissen_op_bezwaar(
        &mut self,
        op: &str,
        gegeven: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn heroverweging_na_uitspraak(
        &mut self,
        ontvangst: &str,
        ontvankelijkheid: &Map<String, Value>,
    ) -> Antwoord;
    fn opnieuw_beslissen_op_bezwaar(
        &mut self,
        ontvangst: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
    fn voorvertoning_na_uitspraak(
        &mut self,
        ontvangst: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord;
}

impl Awb for Spel {
    fn aanvragen(&mut self, velden: &Map<String, Value>, datum: NaiveDate) -> Antwoord {
        let artikel = self.casus()?["artikel"].as_str().unwrap().to_string();
        self.indienen(&artikel, None, velden, datum)
    }

    fn besluiten(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        self.beslissen(op, None, bevindingen, datum)
    }

    fn voorvertoning_besluit(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        self.voorvertoning(op, None, bevindingen, datum)
    }

    fn bezwaren(
        &mut self,
        besluit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        let artikel = artikel_van(self, "BEZWAAR");
        self.indienen(&artikel, Some(besluit), velden, datum)
    }

    fn heroverweging(
        &mut self,
        bezwaar: &str,
        ontvankelijkheid: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        let v = self.voor_beslissen(bezwaar, None, ontvankelijkheid, datum)?;
        Ok(json!({
            "bezwaar": bezwaar,
            "bestreden_besluit": v["verwezen"],
            "ontvankelijk": v["uitvoer"]["bezwaar_ontvankelijk"] == json!(true),
            "tijdig": v["velden"]["bezwaar_of_beroepschrift_tijdig_ingediend"],
            "uitvoer": v["uitvoer"],
            "bevindingen": v["bevindingen"],
        }))
    }

    fn beslissen_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        self.beslissen(op, None, bevindingen, datum).map(op_bezwaar)
    }

    fn voorvertoning_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        Ok(als(
            self.voorvertoning(op, None, bevindingen, datum)?,
            op_bezwaar,
        ))
    }

    fn beroep_instellen(
        &mut self,
        besluit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        let artikel = artikel_van(self, "BEROEP");
        self.indienen(&artikel, Some(besluit), velden, datum)
    }

    fn uitspraak_doen(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        self.beslissen(op, None, bevindingen, datum).map(uitspraak)
    }

    fn voorvertoning_uitspraak(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        Ok(als(
            self.voorvertoning(op, None, bevindingen, datum)?,
            uitspraak,
        ))
    }

    fn waarom_uitspraak(&mut self, id: &str) -> Antwoord {
        self.waarom(id)
    }

    fn toetsen_en_beslissen_op_bezwaar(
        &mut self,
        op: &str,
        gegeven: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        let genomen = self.toetsen_en_beslissen(op, None, gegeven, datum)?;
        let mut uit = op_bezwaar(genomen.clone());
        uit["toets"] = genomen["toets"].clone();
        Ok(uit)
    }

    fn heroverweging_na_uitspraak(
        &mut self,
        ontvangst: &str,
        ontvankelijkheid: &Map<String, Value>,
    ) -> Antwoord {
        let [receipt, beslissing, bezwaar, bestreden] = na_uitspraak(self, ontvangst);
        let bezwaar = bezwaar["id"].as_str().unwrap().to_string();
        let dag = receipt["recorded_on"].as_str().unwrap().parse().unwrap();
        let v = self.voor_beslissen(&bezwaar, Some(ontvangst), ontvankelijkheid, dag)?;
        Ok(json!({
            "ontvangst": receipt["id"],
            "uitspraak": receipt["fields"],
            "beslissing": beslissing["id"],
            "bezwaar": bezwaar,
            "bestreden_besluit": bestreden["id"],
            "bevindingen": v["bevindingen"],
        }))
    }

    fn opnieuw_beslissen_op_bezwaar(
        &mut self,
        ontvangst: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        let bezwaar = na_uitspraak(self, ontvangst)[2]["id"]
            .as_str()
            .unwrap()
            .to_string();
        self.beslissen(&bezwaar, Some(ontvangst), bevindingen, datum)
            .map(op_bezwaar)
    }

    fn voorvertoning_na_uitspraak(
        &mut self,
        ontvangst: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        let bezwaar = na_uitspraak(self, ontvangst)[2]["id"]
            .as_str()
            .unwrap()
            .to_string();
        Ok(als(
            self.voorvertoning(&bezwaar, Some(ontvangst), bevindingen, datum)?,
            op_bezwaar,
        ))
    }
}
