//! The player: one scenario of a case, played as the guide plays it, step by
//! step, with the values of its step (`invoer`) over the case's `voorbeeld`,
//! and checked against what the step expects (`uitkomst`, `uitvoer`).
//!
//! The player does not stop at the first step that goes wrong: it writes
//! what happened in a [`Verslag`], and [`Verslag::fouten`] says per step what
//! differed. A step as text (a line of the guide, not an action) is noted
//! and skipped. A field the form asks and the step leaves out is noted in
//! `leeg_gelaten`; on a form of a submission it is sent empty (`null`), as
//! the page sends it.
//!
//! The report is the shape the documentation describes
//! (`docs/src/content/docs/components/poc-portal.md`), so a source
//! that measured its scenarios with a runner of its own keeps its reports.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde::Serialize;
use serde_json::{Map, Value, json};

/// What one scenario came to.
#[derive(Serialize)]
pub struct Verslag {
    pub casus: String,
    pub scenario: String,
    pub index: usize,
    /// How many steps went wrong.
    pub fouten: usize,
    pub stappen: Vec<Value>,
    /// Per step, what was added to the chronicles, with `waarom` for a
    /// decision and `wat_volgt` for a submission; only when asked for.
    pub metingen: Vec<Value>,
    /// The ids of the grams the chronicles held before the first step (the
    /// case's `begin`): the world every scenario starts from.
    #[serde(skip)]
    pub vooraf: HashSet<Value>,
}

impl Verslag {
    /// Per step that went wrong: where, and what differed.
    pub fn fouten(&self) -> Vec<String> {
        self.stappen
            .iter()
            .filter_map(|s| {
                let fout = s.get("fout")?.as_str()?;
                Some(format!(
                    "{}, '{}', step {}: {fout}",
                    self.casus, self.scenario, s["nr"]
                ))
            })
            .collect()
    }

    /// The steps that are actions, not text.
    pub fn handelingen(&self) -> impl Iterator<Item = &Value> {
        self.stappen.iter().filter(|s| s["soort"] != "tekst")
    }
}

/// What a scenario has done so far, to find what a step acts on.
#[derive(Default)]
struct Stand {
    /// Per application in order: its id and the first decision on it.
    aanvragen: Vec<(String, Option<String>)>,
    /// The objections in order.
    bezwaren: Vec<String>,
    /// The decisions on objection, in order.
    beslissingen: Vec<String>,
    beroepen: Vec<String>,
    uitspraken: Vec<String>,
    ontvangsten: Vec<String>,
}

/// Plays one scenario on one game.
struct Speler<'a> {
    spel: &'a mut Spel,
    voorbeeld: Value,
    /// The output of a decision that says the application is granted.
    uitkomst: String,
    stand: Stand,
    /// Per kind of step, how many came so far: without `op`, the n-th
    /// decision acts on the n-th application, as the guide counts.
    geteld: HashMap<String, usize>,
}

/// The scenario `index` of the case `casus_id`, played on `spel` from the
/// start of that case. With `metingen`, the report holds what each step
/// added to the chronicles.
///
/// # Errors
///
/// When the case cannot be chosen or has no such scenario.
pub fn speel(
    spel: &mut Spel,
    casus_id: &str,
    index: usize,
    metingen: bool,
) -> Result<Verslag, String> {
    spel.kiezen(casus_id).map_err(|e| e.to_string())?;
    let casus = spel.casus().map_err(|e| e.to_string())?;
    let scenario = casus["scenarios"]
        .get(index)
        .ok_or_else(|| format!("{casus_id} has no scenario {index}"))?;
    let mut gezien = gram_ids(spel)?;
    let mut verslag = Verslag {
        casus: casus_id.to_string(),
        scenario: scenario["naam"].as_str().unwrap_or_default().to_string(),
        index,
        fouten: 0,
        stappen: Vec::new(),
        metingen: Vec::new(),
        vooraf: gezien.clone(),
    };
    let mut datum = dag(&casus["speeldatum"])?;
    let mut speler = Speler {
        spel,
        voorbeeld: casus["voorbeeld"].clone(),
        uitkomst: casus["uitkomst"].as_str().unwrap_or_default().to_string(),
        stand: Stand::default(),
        geteld: HashMap::new(),
    };
    let stappen = scenario["stappen"].as_array().cloned().unwrap_or_default();
    for (i, stap) in stappen.iter().enumerate() {
        let nr = i + 1;
        if let Some(tekst) = stap.as_str() {
            verslag
                .stappen
                .push(json!({"nr": nr, "soort": "tekst", "tekst": tekst}));
            continue;
        }
        if let Some(d) = stap.get("datum") {
            datum = dag(d)?;
        }
        let mut rij = json!({
            "nr": nr,
            "soort": stap["stap"],
            "datum": datum.to_string(),
            "verhaal": stap.get("verhaal"),
            "verwacht": stap.get("uitkomst"),
        });
        if let Err(fout) = speler.stap(stap, datum, &mut rij) {
            rij["fout"] = json!(fout);
        }
        if let Some(fout) = uitvoer_wijkt_af(&rij) {
            rij["fout"] = json!(fout);
        }
        if rij.get("fout").is_some() {
            verslag.fouten += 1;
        }
        if metingen {
            meet(speler.spel, &mut gezien, &rij, &mut verslag.metingen)?;
        }
        verslag.stappen.push(rij);
    }
    Ok(verslag)
}

impl Speler<'_> {
    /// Play `stap` on `datum`, and note in `rij` what came of it.
    fn stap(&mut self, stap: &Value, datum: NaiveDate, rij: &mut Value) -> Result<(), String> {
        let soort = stap["stap"].as_str().ok_or("a step without 'stap'")?;
        let n = *self
            .geteld
            .entry(soort.to_string())
            .and_modify(|n| *n += 1)
            .or_insert(1);
        let doel = match stap["op"].as_u64() {
            Some(op) => usize::try_from(op)
                .ok()
                .and_then(|op| op.checked_sub(1))
                .ok_or_else(|| format!("'op: {op}' counts from 1"))?,
            None => n - 1,
        };
        let vormen = self.spel.vormen(datum).map_err(|e| e.to_string())?;
        match soort {
            "aanvraag" => self.aanvraag(stap, &vormen, datum, rij),
            "besluit" => self.besluit(stap, &vormen, doel, datum, rij),
            "bezwaar" => self.bezwaar(stap, &vormen, datum, rij),
            "beslissing_op_bezwaar" => self.beslissing_op_bezwaar(stap, doel, datum, rij),
            "vastleggen" => self.vastleggen(stap, &vormen, datum, rij),
            "beroep" => self.beroep(stap, &vormen, datum, rij),
            "uitspraak" => self.uitspraak(stap, datum),
            "toezenden" => self.toezenden(stap, datum),
            "opnieuw_beslissen" => self.opnieuw_beslissen(stap, datum, rij),
            andere => Err(format!("unknown step '{andere}'")),
        }
    }

    fn aanvraag(
        &mut self,
        stap: &Value,
        vormen: &Value,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let mut velden = met(&self.voorbeeld["aanvraag"], &stap["invoer"]);
        rij["leeg_gelaten"] = json!(vul_leeg(&vormen["aanvraag"]["velden"], &mut velden));
        let gram = self
            .spel
            .aanvragen(&velden, datum)
            .map_err(|e| e.to_string())?;
        let id = id(&gram)?;
        // What the receipt computes is not in the gram but in `wat_volgt`.
        if stap.get("uitvoer").is_some() {
            let volgt = self.spel.wat_volgt(&id).map_err(|e| e.to_string())?;
            rij["uitvoer"] = json!(uitvoer(stap, &volgt));
        }
        self.stand.aanvragen.push((id, None));
        Ok(())
    }

    fn besluit(
        &mut self,
        stap: &Value,
        vormen: &Value,
        doel: usize,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let op = self
            .stand
            .aanvragen
            .get(doel)
            .map(|a| a.0.clone())
            .ok_or("no application to decide on")?;
        let bevindingen = met(&self.voorbeeld["bevindingen"], &stap["invoer"]);
        // Not filled with null: the page leaves an empty finding out, and an
        // open term with a default then takes its default.
        rij["leeg_gelaten"] = json!(
            namen(&vormen["besluit"]["bevindingen"])
                .into_iter()
                .filter(|n| !bevindingen.contains_key(n))
                .collect::<Vec<_>>()
        );
        let genomen = self.spel.besluiten(&op, &bevindingen, datum);
        let Some(genomen) = geweigerd_of_genomen(genomen, stap, "besluit", rij)? else {
            return Ok(());
        };
        let besluit = &genomen["besluit"];
        self.stand.aanvragen[doel].1 = Some(id(besluit)?);
        rij["uitvoer"] = json!(uitvoer(stap, &besluit["outputs"]));
        let kreeg = if besluit["outputs"][&self.uitkomst] == json!(true) {
            "toegewezen"
        } else {
            "afgewezen"
        };
        verwacht(stap, kreeg, rij)
    }

    fn bezwaar(
        &mut self,
        stap: &Value,
        vormen: &Value,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let (_, besluit) = kies(&self.stand.aanvragen, &stap["op"])?;
        let besluit = besluit.ok_or("no decision to object to")?;
        let mut velden = met(&self.voorbeeld["bezwaar"], &stap["invoer"]);
        dagtekening(&mut velden, datum);
        let gevraagd = tegen(self.spel, &vormen["bezwaar"]["velden"], &besluit)?;
        rij["leeg_gelaten"] = json!(vul_leeg(&gevraagd, &mut velden));
        let gram = self
            .spel
            .bezwaren(&besluit, &velden, datum)
            .map_err(|e| e.to_string())?;
        self.stand.bezwaren.push(id(&gram)?);
        Ok(())
    }

    fn beslissing_op_bezwaar(
        &mut self,
        stap: &Value,
        doel: usize,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let op = self
            .stand
            .bezwaren
            .get(doel)
            .cloned()
            .ok_or("no objection to decide on")?;
        let gegeven = met(&self.voorbeeld["beslissing_op_bezwaar"], &stap["invoer"]);
        let genomen = self
            .spel
            .toetsen_en_beslissen_op_bezwaar(&op, &gegeven, datum);
        let Some(genomen) = geweigerd_of_genomen(genomen, stap, "bezwaar", rij)? else {
            return Ok(());
        };
        self.stand.beslissingen.push(id(&genomen["beslissing"])?);
        if stap.get("uitvoer").is_some() {
            // The outputs of the decision on objection itself and those of
            // the new decision it holds, checked together.
            let mut samen = map(&genomen["beslissing"]["outputs"]);
            samen.extend(map(&genomen["nieuw_besluit"]["outputs"]));
            rij["uitvoer"] = json!(uitvoer(stap, &Value::Object(samen)));
        }
        verwacht(stap, op_bezwaar(&genomen["beslissing"]), rij)
    }

    fn vastleggen(
        &mut self,
        stap: &Value,
        vormen: &Value,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let cel = stap["cel"].as_str().ok_or("a step without 'cel'")?;
        let feit = stap["feit"].as_str().ok_or("a step without 'feit'")?;
        let handeling = vormen["andere_cellen"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["cel"] == json!(cel))
            .and_then(|c| {
                c["handelingen"]
                    .as_array()?
                    .iter()
                    .find(|h| h["feit"] == json!(feit))
            })
            .ok_or_else(|| format!("{cel} records no {feit}"))?;
        let mut velden = map(&stap["invoer"]);
        rij["leeg_gelaten"] = json!(vul_leeg(&handeling["velden"], &mut velden));
        let verwijst_naar = stap.get("verwijst_naar").and_then(Value::as_str);
        self.spel
            .vastleggen_verwijzend(cel, feit, &velden, verwijst_naar, datum)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn beroep(
        &mut self,
        stap: &Value,
        vormen: &Value,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let besluit = kies(&self.stand.beslissingen, &stap["op"])?;
        let mut velden = met(&self.voorbeeld["beroep"], &stap["invoer"]);
        dagtekening(&mut velden, datum);
        let gevraagd = tegen(self.spel, &vormen["beroep"]["velden"], &besluit)?;
        rij["leeg_gelaten"] = json!(vul_leeg(&gevraagd, &mut velden));
        let gram = self
            .spel
            .beroep_instellen(&besluit, &velden, datum)
            .map_err(|e| e.to_string())?;
        self.stand.beroepen.push(id(&gram)?);
        Ok(())
    }

    fn uitspraak(&mut self, stap: &Value, datum: NaiveDate) -> Result<(), String> {
        let beroep = kies(&self.stand.beroepen, &stap["op"])?;
        let bevindingen = met(&self.voorbeeld["uitspraak"], &stap["invoer"]);
        let gedaan = self
            .spel
            .uitspraak_doen(&beroep, &bevindingen, datum)
            .map_err(|e| e.to_string())?;
        self.stand.uitspraken.push(id(&gedaan["uitspraak"])?);
        Ok(())
    }

    fn toezenden(&mut self, stap: &Value, datum: NaiveDate) -> Result<(), String> {
        let uitspraak = kies(&self.stand.uitspraken, &stap["op"])?;
        let gram = self
            .spel
            .toezenden(&uitspraak, datum)
            .map_err(|e| e.to_string())?;
        self.stand.ontvangsten.push(id(&gram)?);
        Ok(())
    }

    fn opnieuw_beslissen(
        &mut self,
        stap: &Value,
        datum: NaiveDate,
        rij: &mut Value,
    ) -> Result<(), String> {
        let ontvangst = kies(&self.stand.ontvangsten, &stap["op"])?;
        self.spel
            .heroverweging_na_uitspraak(&ontvangst)
            .map_err(|e| e.to_string())?;
        let bevindingen = met(&self.voorbeeld["beslissing_op_bezwaar"], &stap["invoer"]);
        let genomen = self
            .spel
            .opnieuw_beslissen_op_bezwaar(&ontvangst, &bevindingen, datum)
            .map_err(|e| e.to_string())?;
        verwacht(stap, op_bezwaar(&genomen["beslissing"]), rij)
    }
}

fn map(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn dag(v: &Value) -> Result<NaiveDate, String> {
    v.as_str()
        .and_then(|d| d.parse().ok())
        .ok_or_else(|| format!("{v} is no date"))
}

fn id(gram: &Value) -> Result<String, String> {
    gram["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "a record without an id".to_string())
}

/// `basis` with `invoer` over it.
fn met(basis: &Value, invoer: &Value) -> Map<String, Value> {
    let mut uit = map(basis);
    uit.extend(map(invoer));
    uit
}

/// The names of the fields in `velden` (a form as `vormen` gives it).
pub fn namen(velden: &Value) -> Vec<String> {
    velden
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["naam"].as_str().map(str::to_string))
        .collect()
}

/// The fields of `velden` that `gegeven` leaves out, filled with `null` as
/// the page sends an empty field.
fn vul_leeg(velden: &Value, gegeven: &mut Map<String, Value>) -> Vec<String> {
    let leeg: Vec<String> = namen(velden)
        .into_iter()
        .filter(|naam| !gegeven.contains_key(naam))
        .collect();
    for naam in &leeg {
        gegeven.insert(naam.clone(), Value::Null);
    }
    leeg
}

/// The date of a notice of objection or appeal: in a scenario the day of the
/// step, as the page fills it, unless the step gives one.
fn dagtekening(velden: &mut Map<String, Value>, datum: NaiveDate) {
    velden
        .entry("dagtekening_bezwaar_of_beroepschrift")
        .or_insert(json!(datum));
}

/// The fields of the form `velden` that the submitter fills against the
/// decision `id` of the case's own cell: not those the cell takes from that
/// decision (its outputs and fields, such as the term of Awb 6:8), as the
/// form on the page leaves them out.
fn tegen(spel: &mut Spel, velden: &Value, id: &str) -> Result<Value, String> {
    let kroniek = spel.kroniek().map_err(|e| e.to_string())?;
    let besluit = kroniek["grammen"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|g| g["id"] == json!(id))
        .ok_or_else(|| format!("no decision '{id}' in the chronicle"))?;
    let in_besluit = |naam: &str| {
        besluit["outputs"].get(naam).is_some() || besluit["fields"].get(naam).is_some()
    };
    Ok(Value::Array(
        velden
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| !v["naam"].as_str().is_some_and(in_besluit))
            .cloned()
            .collect(),
    ))
}

/// The element `op` (counted from 1) of `lijst`, or else the last.
fn kies<T: Clone>(lijst: &[T], op: &Value) -> Result<T, String> {
    let i = match op.as_u64() {
        Some(n) => usize::try_from(n).ok().and_then(|n| n.checked_sub(1)),
        None => lijst.len().checked_sub(1),
    };
    i.and_then(|i| lijst.get(i))
        .cloned()
        .ok_or_else(|| "nothing to act on".to_string())
}

/// The decision a step took, or `None` when the cell refused it as the step
/// expects (`uitkomst: geweigerd`), for the reason the guide recognises
/// for that `soort` (frontend/src/scenarioGuides.js): a decision that misses
/// a judgement, a decision on objection against a decision already
/// replaced.
fn geweigerd_of_genomen(
    genomen: Result<Value, impl std::fmt::Display>,
    stap: &Value,
    soort: &str,
    rij: &mut Value,
) -> Result<Option<Value>, String> {
    let weigering = stap["uitkomst"] == json!("geweigerd");
    match genomen {
        Ok(_) if weigering => Err("expected geweigerd, the cell decided".into()),
        Ok(genomen) => Ok(Some(genomen)),
        Err(e) if weigering => {
            let melding = e.to_string();
            let delen: &[&str] = match soort {
                "besluit" => &["no judgement was given", "asks '"],
                _ => &["already replaces"],
            };
            rij["geweigerd"] = json!(melding);
            if delen.iter().any(|d| melding.contains(d)) {
                Ok(None)
            } else {
                Err(format!("refused for another reason: {melding}"))
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Note what a step `kreeg`, and whether that is the `uitkomst` it expects.
fn verwacht(stap: &Value, kreeg: &str, rij: &mut Value) -> Result<(), String> {
    rij["kreeg"] = json!(kreeg);
    match stap["uitkomst"].as_str() {
        Some(v) if v != kreeg => Err(format!("expected {v}, got {kreeg}")),
        _ => Ok(()),
    }
}

/// Per output the step expects (`uitvoer`): the value in `outputs`, and
/// whether it is the one expected.
fn uitvoer(stap: &Value, outputs: &Value) -> Vec<Value> {
    map(&stap["uitvoer"])
        .into_iter()
        .map(|(naam, verwacht)| {
            let kreeg = outputs.get(&naam).cloned().unwrap_or(Value::Null);
            let klopt = kreeg == verwacht;
            json!({"uitkomst": naam, "verwacht": verwacht, "kreeg": kreeg, "klopt": klopt})
        })
        .collect()
}

/// Every output of the row that is not the one the step expects, as one
/// message.
fn uitvoer_wijkt_af(rij: &Value) -> Option<String> {
    let afwijkingen: Vec<String> = rij["uitvoer"]
        .as_array()?
        .iter()
        .filter(|u| u["klopt"] == json!(false))
        .map(|u| {
            format!(
                "output '{}': expected {}, got {}",
                u["uitkomst"].as_str().unwrap_or_default(),
                u["verwacht"],
                u["kreeg"]
            )
        })
        .collect();
    let eerder = rij["fout"].as_str().map(str::to_string);
    if afwijkingen.is_empty() {
        return None;
    }
    Some(
        eerder
            .into_iter()
            .chain(afwijkingen)
            .collect::<Vec<_>>()
            .join("; "),
    )
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

/// Every chronicle of `spel`, its own cell's first, each by the name of its
/// cell (`eigen` for the own one).
pub fn kronieken(spel: &mut Spel) -> Result<Vec<(String, Value)>, String> {
    let kroniek = spel.kroniek().map_err(|e| e.to_string())?;
    let andere: Vec<(String, Value)> = kroniek["andere"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| (k["cel"].as_str().unwrap_or("andere").to_string(), k.clone()))
        .collect();
    Ok(std::iter::once(("eigen".to_string(), kroniek))
        .chain(andere)
        .collect())
}

/// The grams of a chronicle.
pub fn grammen(kroniek: &Value) -> impl Iterator<Item = &Value> {
    kroniek["grammen"].as_array().into_iter().flatten()
}

/// The ids of the grams in every chronicle of `spel`.
pub fn gram_ids(spel: &mut Spel) -> Result<HashSet<Value>, String> {
    Ok(kronieken(spel)?
        .iter()
        .flat_map(|(_, k)| grammen(k).map(|g| g["id"].clone()).collect::<Vec<_>>())
        .collect())
}

/// What a chronicle gained since the grams of `gezien`, after the step
/// `na_stap`, with `waarom` for a decision and `wat_volgt` for a submission.
fn meet(
    spel: &mut Spel,
    gezien: &mut HashSet<Value>,
    na_stap: &Value,
    uit: &mut Vec<Value>,
) -> Result<(), String> {
    let mut nieuw = Vec::new();
    for (cel, kroniek) in kronieken(spel)? {
        for g in grammen(&kroniek) {
            if gezien.insert(g["id"].clone()) {
                nieuw.push((cel.clone(), g.clone()));
            }
        }
    }
    for (cel, g) in nieuw {
        let id = id(&g)?;
        let waarom = spel
            .waarom(&id)
            .ok()
            .or_else(|| spel.waarom_uitspraak(&id).ok());
        let volgt = spel.wat_volgt(&id).ok();
        uit.push(json!({
            "na_stap": na_stap,
            "cel": cel,
            "gram": g,
            "waarom": waarom.map(|w| json!({
                "keten": w["keten"],
                "spoor": w["spoor"],
                "ingevuld": w["ingevuld"],
                "antwoorden": w["antwoorden"],
            })),
            "wat_volgt": volgt,
        }));
    }
    Ok(())
}
