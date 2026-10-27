//! The game in the browser: [`Spel`] behind wasm-bindgen, one [`Poc`] per
//! tab.
//!
//! The page gives the bundle it was built with ([`crate::bundel`]) and calls
//! one method per step. Every method answers in JSON text, as the HTTP API
//! did before the app moved into the browser, and a refusal is a thrown
//! `Error` with the cell's message. Structured arguments (`velden`,
//! `bevindingen`, a saved game) come in as JSON text too, so a value reaches
//! the cell exactly as the page wrote it, numbers and all.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

use crate::spel::{Antwoord, Spel};

#[wasm_bindgen]
pub struct Poc {
    spel: Spel,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// A panic aborts the WebAssembly instance, and the page only sees that it
/// stopped (api.js says so). The message goes to the console, so whoever
/// looks there sees why.
fn meld_panics() {
    static EENMAAL: std::sync::Once = std::sync::Once::new();
    EENMAAL.call_once(|| std::panic::set_hook(Box::new(|info| console_error(&info.to_string()))));
}

fn fout(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn antwoord(a: Antwoord) -> Result<String, JsError> {
    let value = a.map_err(fout)?;
    serde_json::to_string(&value).map_err(fout)
}

fn dag(datum: &str) -> Result<NaiveDate, JsError> {
    datum
        .parse()
        .map_err(|e| fout(format!("'{datum}' is no date (YYYY-MM-DD): {e}")))
}

/// The JSON object `json` given as `naam`.
fn kaart(naam: &str, json: &str) -> Result<Map<String, Value>, JsError> {
    serde_json::from_str(json).map_err(|e| fout(format!("{naam}: {e}")))
}

#[wasm_bindgen]
impl Poc {
    /// The game on `bundel`, the JSON object `{ "<path>": "<text>" }` the
    /// page was built with. A bundle that misses a file a case needs, or a
    /// case the law does not support, fails here, before anything is shown.
    ///
    /// `bronnen`, if given, is the JSON list of the sources the page fetched
    /// ([`crate::bundel::Bron`]). A source that is unavailable or broken
    /// does not fail here: `casussen` says what became of it.
    #[wasm_bindgen(constructor)]
    pub fn new(bundel: &str, bronnen: Option<String>) -> Result<Poc, JsError> {
        meld_panics();
        let files: BTreeMap<String, String> =
            serde_json::from_str(bundel).map_err(|e| fout(format!("the bundle: {e}")))?;
        let bronnen: Vec<crate::bundel::Bron> = match bronnen.as_deref() {
            Some(json) => {
                serde_json::from_str(json).map_err(|e| fout(format!("the sources: {e}")))?
            }
            None => Vec::new(),
        };
        let (casussen, standen) = crate::bundel::met_bronnen(files, bronnen).map_err(fout)?;
        Ok(Poc {
            spel: Spel::met_bronnen(casussen, standen),
        })
    }

    pub fn casussen(&self) -> Result<String, JsError> {
        antwoord(self.spel.casussen())
    }

    pub fn casus(&mut self) -> Result<String, JsError> {
        antwoord(self.spel.casus())
    }

    pub fn kies(&mut self, casus: &str) -> Result<String, JsError> {
        antwoord(self.spel.kiezen(casus))
    }

    pub fn vormen(&mut self, datum: &str) -> Result<String, JsError> {
        antwoord(self.spel.vormen(dag(datum)?))
    }

    pub fn kroniek(&mut self) -> Result<String, JsError> {
        antwoord(self.spel.kroniek())
    }

    pub fn aanvragen(&mut self, velden: &str, datum: &str) -> Result<String, JsError> {
        antwoord(self.spel.aanvragen(&kaart("velden", velden)?, dag(datum)?))
    }

    pub fn vastleggen(
        &mut self,
        cel: &str,
        feit: &str,
        velden: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(
            self.spel
                .vastleggen(cel, feit, &kaart("velden", velden)?, dag(datum)?),
        )
    }

    pub fn besluiten(
        &mut self,
        op: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(
            self.spel
                .besluiten(op, &kaart("bevindingen", bevindingen)?, dag(datum)?),
        )
    }

    #[wasm_bindgen(js_name = voorvertoningBesluit)]
    pub fn voorvertoning_besluit(
        &mut self,
        op: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(self.spel.voorvertoning_besluit(
            op,
            &kaart("bevindingen", bevindingen)?,
            dag(datum)?,
        ))
    }

    pub fn waarom(&mut self, id: &str) -> Result<String, JsError> {
        antwoord(self.spel.waarom(id))
    }

    #[wasm_bindgen(js_name = hoeBeantwoord)]
    pub fn hoe_beantwoord(&mut self, id: &str, naam: &str) -> Result<String, JsError> {
        antwoord(self.spel.hoe_beantwoord(id, naam))
    }

    pub fn bezwaren(
        &mut self,
        besluit: &str,
        velden: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(
            self.spel
                .bezwaren(besluit, &kaart("velden", velden)?, dag(datum)?),
        )
    }

    pub fn heroverweging(&mut self, bezwaar: &str, datum: &str) -> Result<String, JsError> {
        antwoord(self.spel.heroverweging(bezwaar, dag(datum)?))
    }

    #[wasm_bindgen(js_name = beslissenOpBezwaar)]
    pub fn beslissen_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(self.spel.beslissen_op_bezwaar(
            op,
            &kaart("bevindingen", bevindingen)?,
            dag(datum)?,
        ))
    }

    #[wasm_bindgen(js_name = voorvertoningOpBezwaar)]
    pub fn voorvertoning_op_bezwaar(
        &mut self,
        op: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(self.spel.voorvertoning_op_bezwaar(
            op,
            &kaart("bevindingen", bevindingen)?,
            dag(datum)?,
        ))
    }

    #[wasm_bindgen(js_name = beroepInstellen)]
    pub fn beroep_instellen(
        &mut self,
        besluit: &str,
        velden: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(
            self.spel
                .beroep_instellen(besluit, &kaart("velden", velden)?, dag(datum)?),
        )
    }

    #[wasm_bindgen(js_name = uitspraakDoen)]
    pub fn uitspraak_doen(
        &mut self,
        op: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(
            self.spel
                .uitspraak_doen(op, &kaart("bevindingen", bevindingen)?, dag(datum)?),
        )
    }

    #[wasm_bindgen(js_name = voorvertoningUitspraak)]
    pub fn voorvertoning_uitspraak(
        &mut self,
        op: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(self.spel.voorvertoning_uitspraak(
            op,
            &kaart("bevindingen", bevindingen)?,
            dag(datum)?,
        ))
    }

    pub fn toezenden(&mut self, uitspraak: &str, datum: &str) -> Result<String, JsError> {
        antwoord(self.spel.toezenden(uitspraak, dag(datum)?))
    }

    #[wasm_bindgen(js_name = waaromUitspraak)]
    pub fn waarom_uitspraak(&mut self, id: &str) -> Result<String, JsError> {
        antwoord(self.spel.waarom_uitspraak(id))
    }

    #[wasm_bindgen(js_name = heroverwegingNaUitspraak)]
    pub fn heroverweging_na_uitspraak(&mut self, ontvangst: &str) -> Result<String, JsError> {
        antwoord(self.spel.heroverweging_na_uitspraak(ontvangst))
    }

    #[wasm_bindgen(js_name = opnieuwBeslissenOpBezwaar)]
    pub fn opnieuw_beslissen_op_bezwaar(
        &mut self,
        ontvangst: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(self.spel.opnieuw_beslissen_op_bezwaar(
            ontvangst,
            &kaart("bevindingen", bevindingen)?,
            dag(datum)?,
        ))
    }

    #[wasm_bindgen(js_name = voorvertoningNaUitspraak)]
    pub fn voorvertoning_na_uitspraak(
        &mut self,
        ontvangst: &str,
        bevindingen: &str,
        datum: &str,
    ) -> Result<String, JsError> {
        antwoord(self.spel.voorvertoning_na_uitspraak(
            ontvangst,
            &kaart("bevindingen", bevindingen)?,
            dag(datum)?,
        ))
    }

    pub fn reducties(&mut self, datum: &str) -> Result<String, JsError> {
        antwoord(self.spel.reducties(dag(datum)?))
    }

    pub fn keten(&mut self, datum: &str) -> Result<String, JsError> {
        antwoord(self.spel.keten(dag(datum)?))
    }

    pub fn logboek(&mut self) -> Result<String, JsError> {
        antwoord(self.spel.logboek())
    }

    pub fn reset(&mut self) -> Result<String, JsError> {
        antwoord(self.spel.reset())
    }

    pub fn bewaren(&mut self) -> Result<String, JsError> {
        antwoord(self.spel.bewaren())
    }

    pub fn herstellen(&mut self, bewaard: &str) -> Result<String, JsError> {
        let bewaard: Value =
            serde_json::from_str(bewaard).map_err(|e| fout(format!("the saved game: {e}")))?;
        antwoord(self.spel.herstellen(bewaard))
    }
}
