//! The game one visitor plays: one handler per step of [`crate::kern`],
//! apart from how the app reaches them ([`crate::wasm`] in the browser, the
//! tests natively).
//!
//! The app plays one case at a time (a casus: a cell and the article that
//! establishes the application). [`Spel::casussen`] lists them,
//! [`Spel::kiezen`] chooses one and starts its cell ([`Corpus::cell`]: an
//! empty chronicle, or what the case's cell recorded before the first
//! scenario), and every other step works on the chosen case; without a
//! choice it says so. The state is the visitor's own: every browser tab
//! holds a [`Spel`] of its own, so one visitor's reset leaves another's game
//! alone. [`Spel::reset`] starts the chronicle of the chosen case anew.

use chrono::NaiveDate;
use regelrecht_cel::{Error, Gram};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::bundel::BronStand;
use crate::kern::{self, Cellen, Corpus};

/// The chosen case, its cell and the other cells of its sphere.
struct Gekozen {
    casus: usize,
    cell: Cellen,
}

/// The cases to choose from, and the one chosen with its cells.
pub struct Spel {
    casussen: Vec<Corpus>,
    /// What became of each source the page gave ([`crate::bundel::met_bronnen`]).
    bronnen: Vec<BronStand>,
    gekozen: Option<Gekozen>,
}

/// What can go wrong in a step, as the app shows it. What the cell or the
/// engine refuses is the visitor's to correct, as is a step before a case is
/// chosen, or a case that does not exist.
#[derive(Debug)]
pub enum Fout {
    Cel(Error),
    /// No case is chosen yet.
    GeenCasus,
    /// There is no case by that id.
    OnbekendeCasus(String),
    /// An answer that cannot be serialized, or a request or a saved game
    /// that cannot be read.
    Intern(String),
}

impl From<Error> for Fout {
    fn from(e: Error) -> Self {
        Self::Cel(e)
    }
}

impl std::fmt::Display for Fout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cel(e) => write!(f, "{e}"),
            Self::GeenCasus => write!(f, "no case is chosen: choose one first"),
            Self::OnbekendeCasus(id) => write!(f, "there is no case '{id}'"),
            Self::Intern(e) => write!(f, "{e}"),
        }
    }
}

pub type Antwoord = std::result::Result<Value, Fout>;

fn ok(value: impl Serialize) -> Antwoord {
    serde_json::to_value(value).map_err(|e| Fout::Intern(e.to_string()))
}

/// A game as [`Spel::bewaren`] gives it and [`Spel::herstellen`] takes it
/// back: the chosen case and the chronicle of each of its cells.
#[derive(Serialize, Deserialize)]
struct Bewaard {
    casus: String,
    kronieken: Vec<Vec<Gram>>,
}

impl Spel {
    /// The app with `casussen` to choose from, and none chosen yet.
    pub fn new(casussen: Vec<Corpus>) -> Self {
        Self::met_bronnen(casussen, Vec::new())
    }

    /// The app with `casussen`, some of them from sources, and what became
    /// of each source, to show next to the cases.
    pub fn met_bronnen(casussen: Vec<Corpus>, bronnen: Vec<BronStand>) -> Self {
        Self {
            casussen,
            bronnen,
            gekozen: None,
        }
    }

    /// Run `stap` on the chosen case and its cell.
    fn met<T>(
        &mut self,
        stap: impl FnOnce(&Corpus, &mut Cellen) -> kern::Result<T>,
    ) -> Result<T, Fout> {
        let spel = self.gekozen.as_mut().ok_or(Fout::GeenCasus)?;
        let corpus = &self.casussen[spel.casus];
        Ok(stap(corpus, &mut spel.cell)?)
    }

    fn positie(&self, id: &str) -> Result<usize, Fout> {
        self.casussen
            .iter()
            .position(|c| c.casus.id == id)
            .ok_or_else(|| Fout::OnbekendeCasus(id.to_string()))
    }

    /// The cases to choose from, which is chosen, and what became of each
    /// source.
    pub fn casussen(&self) -> Antwoord {
        let gekozen = self
            .gekozen
            .as_ref()
            .map(|g| self.casussen[g.casus].casus.id.clone());
        let kaarten: Vec<Value> = self.casussen.iter().map(Corpus::kaart).collect();
        ok(json!({ "casussen": kaarten, "gekozen": gekozen, "bronnen": self.bronnen }))
    }

    /// The chosen case, as the app shows it.
    pub fn casus(&mut self) -> Antwoord {
        ok(self.met(|c, _| Ok(c.beschrijving()))?)
    }

    /// Choose a case: its cell starts anew ([`Corpus::cell`]).
    pub fn kiezen(&mut self, id: &str) -> Antwoord {
        let i = self.positie(id)?;
        let corpus = &self.casussen[i];
        let cell = corpus.cell()?;
        let beschrijving = corpus.beschrijving();
        self.gekozen = Some(Gekozen { casus: i, cell });
        ok(beschrijving)
    }

    pub fn vormen(&mut self, datum: NaiveDate) -> Antwoord {
        ok(self.met(|c, cell| kern::vormen(c, cell, datum))?)
    }

    pub fn kroniek(&mut self) -> Antwoord {
        ok(self.met(|c, cell| {
            // The other cells of the sphere, each with its own chronicle
            // (RFC-052), under the names the case file gives them.
            let andere: Vec<Value> = cell
                .andere
                .iter()
                .zip(&c.casus.andere_cellen)
                .map(|(andere, a)| {
                    json!({
                        "cel": andere.recording_actor(),
                        "cel_naam": a.cel_naam,
                        "grammen": andere.grams(),
                    })
                })
                .collect();
            Ok(json!({
                "cel": cell.recording_actor(),
                "grammen": cell.grams(),
                "andere": andere,
            }))
        })?)
    }

    pub fn aanvragen(&mut self, velden: &Map<String, Value>, datum: NaiveDate) -> Antwoord {
        ok(self.met(|c, cell| kern::aanvragen(c, cell, velden, datum))?)
    }

    /// The other cell `cel` of the sphere records the fact `feit`
    /// (`<regulation>#<article>#<fact>`).
    pub fn vastleggen(
        &mut self,
        cel: &str,
        feit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::vastleggen(c, cell, cel, feit, velden, datum))?)
    }

    /// Decide on the application `op`, with the deciding authority's
    /// findings for this case (RFC-050).
    pub fn besluiten(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::besluiten(c, cell, op, bevindingen, datum))?)
    }

    pub fn waarom(&mut self, id: &str) -> Antwoord {
        ok(self.met(|c, cell| kern::waarom(c, cell, id))?)
    }

    pub fn hoe_beantwoord(&mut self, id: &str, naam: &str) -> Antwoord {
        ok(self.met(|c, cell| kern::hoe_beantwoord(c, cell, id, naam))?)
    }

    pub fn bezwaren(
        &mut self,
        besluit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::bezwaren(c, cell, besluit, velden, datum))?)
    }

    pub fn heroverweging(&mut self, id: &str, datum: NaiveDate) -> Antwoord {
        ok(self.met(|c, cell| kern::heroverweging(c, cell, id, datum))?)
    }

    pub fn beslissen_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::beslissen_op_bezwaar(c, cell, op, bevindingen, datum))?)
    }

    /// A preview ([`kern::voorvertoning_besluit`]): `{ voorlopig }` or, when
    /// the cell refuses (a finding not given yet), `{ geweigerd }`, and in
    /// both the terms the authority judges because a policy leaves them to
    /// it for this case (`stil`). Any other error fails as in any step.
    pub fn voorvertoning_besluit(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::voorvertoning_besluit(c, cell, op, bevindingen, datum))?)
    }

    pub fn voorvertoning_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::voorvertoning_op_bezwaar(c, cell, op, bevindingen, datum))?)
    }

    /// The applicant appeals against the decision on objection `besluit`
    /// to the court of the case (RFC-053, [`kern::beroep_instellen`]).
    pub fn beroep_instellen(
        &mut self,
        besluit: &str,
        velden: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::beroep_instellen(c, cell, besluit, velden, datum))?)
    }

    /// The court gives judgment on the appeal `op` with its findings.
    pub fn uitspraak_doen(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::uitspraak_doen(c, cell, op, bevindingen, datum))?)
    }

    /// A preview of a judgment, as [`Spel::voorvertoning_besluit`].
    pub fn voorvertoning_uitspraak(
        &mut self,
        op: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| kern::voorvertoning_uitspraak(c, cell, op, bevindingen, datum))?)
    }

    /// The court sends a copy of its judgment `uitspraak` to the case's
    /// cell, which records the receipt.
    pub fn toezenden(&mut self, uitspraak: &str, datum: NaiveDate) -> Antwoord {
        ok(self.met(|c, cell| kern::toezenden(c, cell, uitspraak, datum))?)
    }

    pub fn waarom_uitspraak(&mut self, id: &str) -> Antwoord {
        ok(self.met(|c, cell| kern::waarom_uitspraak(c, cell, id))?)
    }

    /// What the authority finds anew after the judgment received in
    /// `ontvangst`.
    pub fn heroverweging_na_uitspraak(&mut self, ontvangst: &str) -> Antwoord {
        ok(self.met(|c, cell| kern::heroverweging_na_uitspraak(c, cell, ontvangst))?)
    }

    /// The authority decides on the objection again after the judgment
    /// received in `ontvangst`.
    pub fn opnieuw_beslissen_op_bezwaar(
        &mut self,
        ontvangst: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| {
            kern::opnieuw_beslissen_op_bezwaar(c, cell, ontvangst, bevindingen, datum)
        })?)
    }

    /// A preview of deciding on the objection again, as
    /// [`Spel::voorvertoning_besluit`].
    pub fn voorvertoning_na_uitspraak(
        &mut self,
        ontvangst: &str,
        bevindingen: &Map<String, Value>,
        datum: NaiveDate,
    ) -> Antwoord {
        ok(self.met(|c, cell| {
            kern::voorvertoning_na_uitspraak(c, cell, ontvangst, bevindingen, datum)
        })?)
    }

    pub fn reducties(&mut self, datum: NaiveDate) -> Antwoord {
        ok(self.met(|c, cell| kern::reducties(c, cell, datum))?)
    }

    pub fn logboek(&mut self) -> Antwoord {
        ok(self.met(|c, cell| Ok(json!({ "handelingen": kern::logboek(c, cell)? })))?)
    }

    /// Start the chronicle of the chosen case anew, from what its cells
    /// know before the first scenario.
    pub fn reset(&mut self) -> Antwoord {
        let grammen = self.met(|c, cell| {
            *cell = c.cell()?;
            Ok(cell.grams().to_vec())
        })?;
        ok(json!({ "grammen": grammen }))
    }

    /// The game so far, to keep across a reload: the chosen case and the
    /// chronicle of each of its cells.
    pub fn bewaren(&mut self) -> Antwoord {
        let bewaard = self.met(|c, cell| {
            Ok(Bewaard {
                casus: c.casus.id.clone(),
                kronieken: cell.kronieken(),
            })
        })?;
        ok(bewaard)
    }

    /// Play on with a game [`Spel::bewaren`] gave: its case is chosen, with
    /// the chronicles it held ([`Corpus::herstellen`]). A game that does not
    /// read, or whose chronicles the cells refuse, is an error, and the
    /// choice stays as it was.
    pub fn herstellen(&mut self, bewaard: Value) -> Antwoord {
        let bewaard: Bewaard = serde_json::from_value(bewaard)
            .map_err(|e| Fout::Intern(format!("the saved game does not read: {e}")))?;
        let i = self.positie(&bewaard.casus)?;
        let corpus = &self.casussen[i];
        let cell = corpus.herstellen(bewaard.kronieken)?;
        let beschrijving = corpus.beschrijving();
        self.gekozen = Some(Gekozen { casus: i, cell });
        ok(beschrijving)
    }
}
