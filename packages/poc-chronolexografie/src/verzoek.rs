//! Requests of the case's cell to another cell, and the answers back.
//!
//! When an application comes in, the case's cell runs the articles of its
//! own policy that the case file names under `bij_aanvraag`
//! ([`crate::casus::Opvolging`]). An article whose one boolean output says
//! so has the cell record the fact of that article it establishes, such as
//! the municipality's request to the GGD for an examination. The cell the
//! article names for a fact with the same parameters records its receipt in
//! the same step, by the delivery every executogram goes through
//! ([`crate::kern::leg_vast`]): the same mechanism that brings the GGD's
//! advice to the municipality.
//!
//! The other party answers with the fact the case file names
//! (`antwoord`). An answer belongs to a request when it holds the same values
//! in the fields both have and is of a day on or after the request
//! ([`hoort_bij`]). Which requests are still open, and which answer came
//! back, [`verzoeken`] reads from the chronicles; nothing else is kept.

use chrono::NaiveDate;
use regelrecht_cel::{Cell, Error, Gram, shape};
use regelrecht_engine::{Authority, LawExecutionService};
use regelrecht_law_model::stored;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::casus::{Aanvraaggegeven, Opvolging, Verzoekantwoord};
use crate::kern::{Cellen, Corpus, Result, leg_vast};

/// A request the case's cell made, as the chronicles hold it now.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Verzoek {
    /// The article that made it, `<regelwerk>#<article>`.
    pub artikel: String,
    /// The cell it went to.
    pub aan: String,
    /// The request in the chronicle of the case's cell.
    pub verzoek: Gram,
    /// Its receipt in the chronicle of `aan`.
    pub ontvangen: Option<Gram>,
    /// The fact by which `aan` answers it, `<regelwerk>#<article>#<fact>`.
    pub antwoordfeit: Option<String>,
    /// The answer in the chronicle of `aan`, once it came.
    pub antwoord: Option<Gram>,
    /// The receipt of that answer in the chronicle of the case's cell.
    pub antwoord_ontvangen: Option<Gram>,
}

impl Verzoek {
    /// Whether no answer came yet.
    pub fn open(&self) -> bool {
        self.antwoord.is_none()
    }
}

/// Run the articles of `bij_aanvraag` for the application `aanvraag` of the
/// case, received on `on`. Each article that says so has the case's cell
/// record its request, and the cell it goes to its receipt. All of it lands,
/// or the step is refused.
///
/// # Errors
///
/// Refused when an article of `bij_aanvraag` is not in force on `on`, has
/// no single boolean output, requires no single fact the case's cell
/// establishes, or cannot run, and whatever the cells refuse to record.
pub(crate) fn na_aanvraag(
    service: &LawExecutionService,
    corpus: &Corpus,
    cellen: &mut Cellen,
    aanvraag: &Gram,
    on: NaiveDate,
) -> Result<()> {
    for opvolging in &corpus.casus.bij_aanvraag {
        let invoer = invoer(opvolging, aanvraag, on);
        let (bron, feit) = vorm(service, opvolging, cellen.eigen.authority(), on)?;
        let result = cellen
            .eigen
            .consult(service, &cellen.sfeer(), &bron, &invoer, on)
            .map_err(|e| Error::Refused(format!("{} does not run: {e}", opvolging.artikel)))?;
        let output = bron.rsplit_once('#').map_or(bron.as_str(), |(_, o)| o);
        let doen = result
            .outputs
            .get(output)
            .map(stored::to_stored)
            .transpose()?
            .is_some_and(|v| v == Value::Bool(true));
        if !doen {
            continue;
        }
        let velden = velden_van(service, &feit, &invoer, on)?;
        let eigen = cellen.eigen.recording_actor().to_string();
        // The request and its receipt land together or not at all.
        let mut proef = cellen.sandbox();
        leg_vast(service, &mut proef, &eigen, &feit, &velden, None, on)?;
        leg_vast(service, cellen, &eigen, &feit, &velden, None, on)?;
    }
    Ok(())
}

/// What the article of `opvolging` runs on: the fields of the application,
/// and per parameter of the case file what the system knows of it.
fn invoer(opvolging: &Opvolging, aanvraag: &Gram, on: NaiveDate) -> Map<String, Value> {
    let mut invoer: Map<String, Value> = aanvraag
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (parameter, gegeven) in &opvolging.parameters {
        let waarde = match gegeven {
            Aanvraaggegeven::Datum => json!(on.to_string()),
            Aanvraaggegeven::Aanvraag => json!(aanvraag.id),
        };
        invoer.insert(parameter.clone(), waarde);
    }
    invoer
}

/// The output of the article of `opvolging` that says whether the case's
/// cell acts (`<regelwerk>#<article>#<output>`), and the fact it then
/// records: the one fact of the article that `eigen` establishes.
fn vorm(
    service: &LawExecutionService,
    opvolging: &Opvolging,
    eigen: &Authority,
    on: NaiveDate,
) -> Result<(String, String)> {
    let (law, nummer) = opvolging.artikel.split_once('#').ok_or_else(|| {
        Error::Setup(format!(
            "bij_aanvraag: '{}' names no article",
            opvolging.artikel
        ))
    })?;
    let article = artikel_op(service, law, nummer, on).ok_or_else(|| {
        Error::Refused(format!(
            "bij_aanvraag: {} is not in force on {on}",
            opvolging.artikel
        ))
    })?;
    let outputs = article.get_outputs();
    let [output] = outputs else {
        return Err(Error::Setup(format!(
            "bij_aanvraag: {} has {} outputs; it needs one, which says whether the cell acts",
            opvolging.artikel,
            outputs.len()
        )));
    };
    let feiten = shape::facts_established_by(law, article, eigen);
    let [feit] = feiten.as_slice() else {
        return Err(Error::Setup(format!(
            "bij_aanvraag: {} requires {} facts the case's cell establishes; it needs one",
            opvolging.artikel,
            feiten.len()
        )));
    };
    Ok((
        format!("{}#{}", opvolging.artikel, output.name),
        feit.clone(),
    ))
}

/// The article `nummer` of the regelwerk `law` in force on `on`.
fn artikel_op<'s>(
    service: &'s LawExecutionService,
    law: &str,
    nummer: &str,
    on: NaiveDate,
) -> Option<&'s regelrecht_law_model::Article> {
    service
        .resolver()
        .get_law_for_date(law, Some(on))
        .and_then(|l| l.find_article_by_number(nummer))
}

/// The fields of an executogram of `feit`, from `invoer`.
fn velden_van(
    service: &LawExecutionService,
    feit: &str,
    invoer: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Map<String, Value>> {
    Ok(shape::fact_fields(service, feit, on, None)?
        .iter()
        .filter_map(|f| invoer.get(&f.name).map(|v| (f.name.clone(), v.clone())))
        .collect())
}

/// The facts in which `cell` records that it received a request of the
/// case's cell: by hand it records none of them, since a request arrives
/// with its step.
pub(crate) fn ontvangsten(
    service: &LawExecutionService,
    corpus: &Corpus,
    eigen: &Authority,
    cell: &Cell,
    on: NaiveDate,
) -> Vec<String> {
    corpus
        .casus
        .bij_aanvraag
        .iter()
        .filter_map(|o| vorm(service, o, eigen, on).ok())
        .filter_map(|(_, feit)| shape::receipt_of(service, &feit, on, cell.authority()).ok()?)
        .collect()
}

/// The facts of the case's cell that it records by its policy when an
/// application comes in: by hand it records none of them.
pub(crate) fn verzonden(
    service: &LawExecutionService,
    corpus: &Corpus,
    eigen: &Authority,
    on: NaiveDate,
) -> Vec<String> {
    corpus
        .casus
        .bij_aanvraag
        .iter()
        .filter_map(|o| vorm(service, o, eigen, on).ok())
        .map(|(_, feit)| feit)
        .collect()
}

/// Every request the case's cell made by `bij_aanvraag`, in the order of its
/// chronicle, with its receipt, its answer, and the receipt of the answer.
///
/// # Errors
///
/// When the corpus does not load.
pub fn verzoeken(corpus: &Corpus, cellen: &Cellen) -> Result<Vec<Verzoek>> {
    let service = corpus.service()?;
    let eigen = &cellen.eigen;
    let mut uit = Vec::new();
    for opvolging in &corpus.casus.bij_aanvraag {
        for verzoek in eigen.grams().iter().filter(|g| {
            g.basis == opvolging.artikel
                && vorm(&service, opvolging, eigen.authority(), g.recorded_on)
                    .is_ok_and(|(_, feit)| is_feit(g, &feit))
        }) {
            uit.push(volg(&service, opvolging, cellen, verzoek)?);
        }
    }
    Ok(uit)
}

/// The request `verzoek` with what became of it.
fn volg(
    service: &LawExecutionService,
    opvolging: &Opvolging,
    cellen: &Cellen,
    verzoek: &Gram,
) -> Result<Verzoek> {
    let on = verzoek.recorded_on;
    let feit = format!(
        "{}#{}",
        verzoek.basis,
        verzoek.fact.as_deref().unwrap_or_default()
    );
    let mut aan = None;
    for cell in &cellen.andere {
        if let Some(ontvangst) = shape::receipt_of(service, &feit, on, cell.authority())? {
            aan = Some((cell, ontvangst));
            break;
        }
    }
    let Some((cell, ontvangst)) = aan else {
        return Err(Error::Refused(format!(
            "no cell of this case receives {feit}"
        )));
    };
    let ontvangen = cell
        .grams()
        .iter()
        .find(|g| is_feit(g, &ontvangst) && g.fields == verzoek.fields)
        .cloned();
    let antwoord = opvolging.antwoord.as_ref().and_then(|a| {
        cell.grams()
            .iter()
            .find(|g| is_feit(g, &a.feit) && hoort_bij(opvolging, a, verzoek, g))
            .cloned()
    });
    let antwoord_ontvangen = match (&opvolging.antwoord, &antwoord) {
        (Some(a), Some(gram)) => {
            let ontvangst =
                shape::receipt_of(service, &a.feit, gram.recorded_on, cellen.eigen.authority())?;
            ontvangst.and_then(|o| {
                cellen
                    .eigen
                    .grams()
                    .iter()
                    .find(|g| is_feit(g, &o) && g.fields == gram.fields)
                    .cloned()
            })
        }
        _ => None,
    };
    Ok(Verzoek {
        artikel: opvolging.artikel.clone(),
        aan: cell.recording_actor().to_string(),
        verzoek: verzoek.clone(),
        ontvangen,
        antwoordfeit: opvolging.antwoord.as_ref().map(|a| a.feit.clone()),
        antwoord,
        antwoord_ontvangen,
    })
}

/// Whether `gram` is an executogram of `feit` (`<regelwerk>#<article>#<fact>`).
fn is_feit(gram: &Gram, feit: &str) -> bool {
    gram.fact
        .as_ref()
        .is_some_and(|f| format!("{}#{f}", gram.basis) == feit)
}

/// The field of a request that holds its day: the parameter the case file
/// fills with the day of the application.
fn dagveld(opvolging: &Opvolging) -> Option<&str> {
    opvolging
        .parameters
        .iter()
        .find(|(_, g)| **g == Aanvraaggegeven::Datum)
        .map(|(p, _)| p.as_str())
}

/// Whether the answer `antwoord` belongs to the request `verzoek`: the same
/// values in every field both hold, and its day (`a.datum`) on or after the
/// day of the request.
fn hoort_bij(opvolging: &Opvolging, a: &Verzoekantwoord, verzoek: &Gram, antwoord: &Gram) -> bool {
    let gedeeld = verzoek
        .fields
        .iter()
        .filter(|(k, _)| antwoord.fields.contains_key(*k))
        .all(|(k, v)| antwoord.fields.get(k) == Some(v));
    let dag = |g: &Gram, veld: &str| {
        g.fields
            .get(veld)
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<NaiveDate>().ok())
    };
    let op_tijd = match (
        dagveld(opvolging).and_then(|v| dag(verzoek, v)),
        dag(antwoord, &a.datum),
    ) {
        (Some(gevraagd), Some(beantwoord)) => beantwoord >= gevraagd,
        _ => false,
    };
    gedeeld && op_tijd
}

/// The other cell `cel` answers its open request `verzoek_id` (the id of
/// its receipt in its own chronicle) with the fact the case file names for
/// it, on `on`. The fields the answer shares with the request come from the
/// request; `velden` gives the rest. The answer goes back to the case's cell
/// as every executogram does.
///
/// # Errors
///
/// Refused when `verzoek_id` is no receipt of an open request in the
/// chronicle of `cel`, when `velden` gives a shared field another value than
/// the request, when the answer's day lies before the day of the request,
/// and whatever the cells refuse to record.
pub fn beantwoorden(
    corpus: &Corpus,
    cellen: &mut Cellen,
    cel: &str,
    verzoek_id: &str,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    let service = corpus.service()?;
    let gevonden = verzoeken(corpus, cellen)?
        .into_iter()
        .find(|v| v.aan == cel && v.ontvangen.as_ref().is_some_and(|g| g.id == verzoek_id));
    let Some(verzoek) = gevonden else {
        return Err(Error::Refused(format!(
            "'{verzoek_id}' is no request that '{cel}' received"
        )));
    };
    if !verzoek.open() {
        return Err(Error::Refused(format!(
            "the request '{verzoek_id}' is answered already"
        )));
    }
    let opvolging = corpus
        .casus
        .bij_aanvraag
        .iter()
        .find(|o| o.artikel == verzoek.artikel)
        .and_then(|o| Some((o, o.antwoord.as_ref()?)));
    let Some((opvolging, a)) = opvolging else {
        return Err(Error::Refused(format!(
            "the case names no answer to a request of {}",
            verzoek.artikel
        )));
    };
    let mut alle = velden.clone();
    for veld in shape::fact_fields(&service, &a.feit, on, None)? {
        let Some(waarde) = verzoek.verzoek.fields.get(&veld.name) else {
            continue;
        };
        match alle.get(&veld.name) {
            Some(gegeven) if gegeven != waarde => {
                return Err(Error::Refused(format!(
                    "'{}' of the answer is {waarde} in the request, not {gegeven}",
                    veld.name
                )));
            }
            _ => {
                alle.insert(veld.name.clone(), waarde.clone());
            }
        }
    }
    let mut proef = cellen.sandbox();
    let gram = leg_vast(&service, &mut proef, cel, &a.feit, &alle, None, on)?;
    if !hoort_bij(opvolging, a, &verzoek.verzoek, &gram) {
        return Err(Error::Refused(format!(
            "the answer is of a day before the request of {}; '{}' is on or after it",
            verzoek.verzoek.recorded_on, a.datum
        )));
    }
    leg_vast(&service, cellen, cel, &a.feit, &alle, None, on)
}
