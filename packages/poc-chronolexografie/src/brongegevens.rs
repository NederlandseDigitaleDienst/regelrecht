//! What the case's cell hands out to an applicant about themself: the
//! answers of its catalogue (`cel.yaml`, `answers`, RFC-048) that need
//! nothing but the field the login fills, each computed for the applicant.
//!
//! Which answers go out, the case says: only those it names under
//! `uitgeven` ([`crate::casus::Casus::uitgeven`]), and without that list
//! none. A named answer goes out only while the reduction it comes from
//! takes the login field as its one required parameter. One that needs
//! another parameter, such as how many permits a company holds by its KvK
//! number, or a date or a decision, is about something else than the
//! applicant; the server refuses to start with such a name ([`ongeschikt`]).
//! That a named answer is about the applicant still rests on its reduction
//! reading only the records of the person its parameter names, which is
//! why the case names each answer and nothing goes out by default.
//!
//! The applicant's app replays the records it got on a chronicle of its own
//! and computes the answers there with the same function, to compare them
//! with the signed ones.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_cel::AnswerParameter;
use regelrecht_engine::{AnswerProvenance, LawExecutionService, RegulationRead};
use regelrecht_law_model::stored;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::kern::{Cellen, Corpus, Result, herkomst};

/// What the case's cell hands out to one applicant on one day
/// ([`brongegevens`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Brongegevens {
    /// Per answer `{waarde, parameters, herkomst}`: the value, `null` when
    /// the cell is silent; what it was asked with (the login field and its
    /// value), so that whoever got it can ask the same again; and where it
    /// comes from, as [`crate::kern::voorinvullen`] gives it.
    pub antwoorden: Map<String, Value>,
    /// The versions of the rulework the answers were computed on, each once.
    pub regelwerken: Vec<RegulationRead>,
    /// The facts that come only after an application (`bij_aanvraag` of the
    /// case), each `{feit, taak}`: what the applicant cannot have yet, so
    /// that an app shows it as still to come and not as missing.
    pub bij_aanvraag: Vec<Value>,
}

/// Where an answer comes from, as [`herkomst`] gives it, without how far
/// the cell read its chronicle ([`brongegevens`]).
fn herkomst_zonder_kroniek(answer: &AnswerProvenance) -> Value {
    let mut uit = herkomst(answer);
    if let Some(velden) = uit.as_object_mut() {
        velden.remove("gelezen");
        velden.remove("tot_en_met");
    }
    uit
}

/// Whether an answer that takes `parameters` can be asked with the login
/// field `inlogveld` alone: it takes that field, and needs nothing else.
fn alleen_inlogveld(parameters: &[AnswerParameter], inlogveld: &str) -> bool {
    parameters.iter().any(|p| p.name == inlogveld)
        && parameters
            .iter()
            .all(|p| p.name == inlogveld || !p.required)
}

/// Why the answer `naam` cannot go out on `on`, or `None` when it can:
/// it is no answer of the cell, its reduction is not in force, or it needs
/// more than the login field `inlogveld`.
fn bezwaar(
    service: &LawExecutionService,
    cellen: &Cellen,
    naam: &str,
    inlogveld: &str,
    on: NaiveDate,
) -> Result<Option<String>> {
    if !cellen.eigen.answers().contains_key(naam) {
        return Ok(Some(format!(
            "'{naam}' is no answer of the cell '{}'",
            cellen.eigen.recording_actor()
        )));
    }
    let Some(parameters) = cellen.eigen.answer_parameters(service, naam, on)? else {
        return Ok(Some(format!(
            "the reduction of '{naam}' is not in force on {on}"
        )));
    };
    if alleen_inlogveld(&parameters, inlogveld) {
        return Ok(None);
    }
    let namen: Vec<&str> = parameters
        .iter()
        .filter(|p| p.required)
        .map(|p| p.name.as_str())
        .collect();
    Ok(Some(format!(
        "'{naam}' takes {} and not the login field '{inlogveld}' alone",
        namen.join(", ")
    )))
}

/// Per answer the case names under `uitgeven` that cannot go out on `on`,
/// why ([`bezwaar`]): what a server checks before it starts, so that a
/// mistake in the list stops it instead of leaving an answer out unseen.
/// `service` is the corpus's ([`Corpus::service`]).
///
/// # Errors
///
/// When a reduction the catalogue names is not loaded, is not the cell's,
/// or lacks its answer.
pub fn ongeschikt(
    corpus: &Corpus,
    service: &LawExecutionService,
    cellen: &Cellen,
    inlogveld: &str,
    on: NaiveDate,
) -> Result<Vec<String>> {
    let mut uit = Vec::new();
    for naam in &corpus.casus.uitgeven {
        if let Some(waarom) = bezwaar(service, cellen, naam, inlogveld, on)? {
            uit.push(waarom);
        }
    }
    Ok(uit)
}

/// The names of the answers the case's cell hands out on `on`: those the
/// case names under `uitgeven` that can be asked with the login field
/// `inlogveld` alone, in the order of that list. None without the list.
/// `service` is the corpus's ([`Corpus::service`]).
///
/// # Errors
///
/// As [`ongeschikt`].
pub fn uitgeefbaar(
    corpus: &Corpus,
    service: &LawExecutionService,
    cellen: &Cellen,
    inlogveld: &str,
    on: NaiveDate,
) -> Result<Vec<String>> {
    let mut uit = Vec::new();
    for naam in &corpus.casus.uitgeven {
        if bezwaar(service, cellen, naam, inlogveld, on)?.is_none() {
            uit.push(naam.clone());
        }
    }
    Ok(uit)
}

/// The answers [`uitgeefbaar`] names, each computed on `on` with `waarde` for
/// the login field `inlogveld`, as the case's cell knows things then.
/// Nothing is recorded.
///
/// The provenance of each answer names the cell, the answer and the
/// reduction with its article and version, and not how far the cell read
/// its chronicle ([`herkomst`] does): that chronicle holds everyone's
/// records, and its length and last record are about other people.
///
/// # Errors
///
/// When the corpus does not load, as [`uitgeefbaar`], and when the engine
/// refuses an answer or a value it gives cannot be stored.
pub fn brongegevens(
    corpus: &Corpus,
    cellen: &Cellen,
    inlogveld: &str,
    waarde: &str,
    on: NaiveDate,
) -> Result<Brongegevens> {
    let service = corpus.service()?;
    let namen = uitgeefbaar(corpus, &service, cellen, inlogveld, on)?;
    let gevraagd = json!({ inlogveld: waarde });
    let parameters = BTreeMap::from([(
        inlogveld.to_string(),
        stored::from_stored(&Value::String(waarde.to_string()))?,
    )]);
    let mut antwoorden = Map::new();
    let mut regelwerken: Vec<RegulationRead> = Vec::new();
    for naam in namen {
        let gegeven = cellen.eigen.give_answer(&service, &naam, &parameters, on)?;
        let antwoord = match gegeven {
            Some(g) => {
                for versie in g.regulations_read {
                    if !regelwerken.contains(&versie) {
                        regelwerken.push(versie);
                    }
                }
                json!({
                    "waarde": stored::to_stored(&g.value)?,
                    "parameters": gevraagd,
                    "herkomst": [herkomst_zonder_kroniek(&g.provenance)],
                })
            }
            None => json!({ "waarde": null, "parameters": gevraagd, "herkomst": [] }),
        };
        antwoorden.insert(naam, antwoord);
    }
    let bij_aanvraag = corpus
        .casus
        .bij_aanvraag
        .iter()
        .filter_map(|o| o.antwoord.as_ref())
        .map(|a| json!({ "feit": a.feit, "taak": a.taak }))
        .collect();
    Ok(Brongegevens {
        antwoorden,
        regelwerken,
        bij_aanvraag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameter(name: &str, required: bool) -> AnswerParameter {
        AnswerParameter {
            name: name.to_string(),
            required,
        }
    }

    #[test]
    fn an_answer_on_the_login_field_alone_is_handed_out() {
        assert!(alleen_inlogveld(&[parameter("bsn", true)], "bsn"));
        assert!(alleen_inlogveld(
            &[parameter("bsn", true), parameter("peildatum", false)],
            "bsn"
        ));
    }

    #[test]
    fn an_answer_that_needs_more_or_another_parameter_is_not() {
        assert!(!alleen_inlogveld(
            &[parameter("bsn", true), parameter("besluit_datum", true)],
            "bsn"
        ));
        assert!(!alleen_inlogveld(&[parameter("kvk_nummer", true)], "bsn"));
        assert!(!alleen_inlogveld(&[], "bsn"));
    }
}
