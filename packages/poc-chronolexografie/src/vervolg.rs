//! What follows a grant: the acts the case's cell records about what it
//! granted, one after the other, such as ordering a parking card from the
//! printer, receiving it and handing it out (`na_toekenning` in the case
//! file, [`NaToekenning`]).
//!
//! The law says which facts there are and what each holds; a reduction of
//! the case's cell plays one aggregate per decision and keeps, in its state,
//! the record that granted and the day of each act. The case names that
//! aggregate, those properties and the order of the acts. Nothing here names
//! an article or a property of a particular law: a case without
//! `na_toekenning` has no acts after a grant, and records its facts as
//! before.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_cel::{Error, reading, shape};
use regelrecht_law_model::stored;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kern::{Cellen, Corpus, Result};

/// What follows a grant in a case: the aggregate that keeps it, and the acts
/// in their order.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NaToekenning {
    /// The article of a reduction of the case's cell that plays one
    /// aggregate per decision, `<reduction>#<article>`.
    pub aggregaat: String,
    /// The property of that aggregate's state that holds the id of the
    /// record that granted, empty while none does: the decision on the
    /// application, or a decision on objection that put a grant in its
    /// place. Every act refers to that record.
    pub toekenning: String,
    /// What a citizen reads of a grant before the first act.
    pub stand: String,
    /// The acts, in the order they come.
    pub handelingen: Vec<Vervolghandeling>,
}

/// One act after a grant.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vervolghandeling {
    /// The fact the case's cell records, `<regelwerk>#<article>#<fact>`.
    pub feit: String,
    /// The property of the aggregate's state that holds the day of the act,
    /// empty until it is recorded.
    pub gedaan: String,
    /// What the button that records it says.
    pub knop: String,
    /// What a citizen reads once it is done; `{datum}` stands for its day.
    pub stand: String,
}

/// One grant and how far its acts are.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stand {
    /// The root of the aggregate: the decision on the application.
    pub besluit: String,
    /// The record that granted, which every act refers to.
    pub toekenning: String,
    /// Per act, in order, the day it was done, if it was.
    pub stappen: Vec<Stap>,
    /// The fact of the next act; none once every act is done.
    pub volgende: Option<String>,
}

/// One act of a [`Stand`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stap {
    pub feit: String,
    pub gedaan: Option<NaiveDate>,
}

/// Every grant in the chronicle of the case's cell on `on`, with how far its
/// acts are: per aggregate whose state names a record that granted. Empty
/// for a case without `na_toekenning`.
///
/// # Errors
///
/// When the aggregate does not play, or its state holds a day that is no
/// date.
pub fn standen(corpus: &Corpus, cellen: &Cellen, on: NaiveDate) -> Result<Vec<Stand>> {
    let Some(na) = &corpus.casus.na_toekenning else {
        return Ok(Vec::new());
    };
    let grams: Vec<_> = cellen
        .eigen
        .grams()
        .iter()
        .filter(|g| g.recorded_on <= on)
        .cloned()
        .collect();
    let records = reading::chronicle_records(&grams)?;
    let service = corpus.service()?;
    let rows =
        service.projection_rows(&na.aggregaat, &records, &BTreeMap::new(), &on.to_string())?;
    let Value::Array(rows) = stored::to_stored(&rows)? else {
        return Err(Error::Setup(format!(
            "the aggregate {} gives no rows",
            na.aggregaat
        )));
    };
    rows.iter()
        .filter_map(|row| stand(na, row).transpose())
        .collect()
}

/// The [`Stand`] of the aggregate `row`, or none when no record in it
/// grants.
fn stand(na: &NaToekenning, row: &Value) -> Result<Option<Stand>> {
    let Some(toekenning) = row.get(&na.toekenning).and_then(Value::as_str) else {
        return Ok(None);
    };
    let besluit = row
        .get(regelrecht_engine::projection::ROOT_ID)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Setup(format!("a row of {} has no id", na.aggregaat)))?;
    let stappen = na
        .handelingen
        .iter()
        .map(|h| {
            let gedaan = match row.get(&h.gedaan) {
                None | Some(Value::Null) => None,
                Some(Value::String(dag)) => Some(dag.parse().map_err(|e| {
                    Error::Setup(format!(
                        "'{}' of {} holds '{dag}': {e}",
                        h.gedaan, na.aggregaat
                    ))
                })?),
                Some(other) => {
                    return Err(Error::Setup(format!(
                        "'{}' of {} holds {other}, which is no date",
                        h.gedaan, na.aggregaat
                    )));
                }
            };
            Ok(Stap {
                feit: h.feit.clone(),
                gedaan,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let volgende = stappen
        .iter()
        .find(|s| s.gedaan.is_none())
        .map(|s| s.feit.clone());
    Ok(Some(Stand {
        besluit: besluit.to_string(),
        toekenning: toekenning.to_string(),
        stappen,
        volgende,
    }))
}

/// Refuse to record `feit` about `refers_to` on `on` unless it is the next
/// act after a grant: an act of `na_toekenning` refers to the record that
/// granted, and only when every act before it is done and it is not done
/// itself. A fact that is no such act passes.
///
/// # Errors
///
/// [`Error::Refused`] when `feit` is an act that refers to no record, to a
/// record that grants nothing (a refusal, or a grant revoked on objection or
/// annulled by the court), or that is not next; and what [`standen`] fails
/// on.
pub fn toets(
    corpus: &Corpus,
    cellen: &Cellen,
    feit: &str,
    refers_to: Option<&str>,
    on: NaiveDate,
) -> Result<()> {
    let Some(na) = &corpus.casus.na_toekenning else {
        return Ok(());
    };
    if !na.handelingen.iter().any(|h| h.feit == feit) {
        return Ok(());
    }
    let Some(naar) = refers_to else {
        return Err(Error::Refused(format!(
            "{feit} follows a grant and refers to the record that granted"
        )));
    };
    let standen = standen(corpus, cellen, on)?;
    let Some(stand) = standen.iter().find(|s| s.toekenning == naar) else {
        return Err(Error::Refused(format!(
            "'{naar}' grants nothing that {feit} can follow: it is no grant, or the grant was \
             revoked or annulled"
        )));
    };
    match &stand.volgende {
        Some(volgende) if volgende == feit => Ok(()),
        Some(volgende) => {
            let gedaan = stand
                .stappen
                .iter()
                .any(|s| s.feit == feit && s.gedaan.is_some());
            Err(Error::Refused(if gedaan {
                format!("{feit} is already recorded for '{naar}'")
            } else {
                format!("{feit} comes after {volgende}, which is not recorded for '{naar}' yet")
            }))
        }
        None => Err(Error::Refused(format!(
            "{feit} is already recorded for '{naar}'"
        ))),
    }
}

/// Check what the case says follows a grant against the law in force on the
/// case's first day: the aggregate is a projection whose state holds the
/// property of the grant and of every act, and every act is a fact the law
/// requires. A case without `na_toekenning` passes.
///
/// # Errors
///
/// [`Error::Setup`] naming what does not hold.
pub(crate) fn controleer(corpus: &Corpus) -> Result<()> {
    let Some(na) = &corpus.casus.na_toekenning else {
        return Ok(());
    };
    let fout = |m: String| Error::Setup(format!("case '{}', na_toekenning: {m}", corpus.casus.id));
    let service = corpus.service()?;
    let on = corpus.casus.speeldatum;
    let (law, nummer) = na
        .aggregaat
        .split_once('#')
        .ok_or_else(|| fout(format!("'{}' is no <reduction>#<article>", na.aggregaat)))?;
    let projection = service
        .resolver()
        .get_law_for_date(law, Some(on))
        .and_then(|l| l.find_article_by_number(nummer))
        .and_then(|a| a.machine_readable.as_ref()?.projection.as_ref())
        .ok_or_else(|| fout(format!("{} plays no aggregate on {on}", na.aggregaat)))?;
    let eigenschappen =
        std::iter::once(&na.toekenning).chain(na.handelingen.iter().map(|h| &h.gedaan));
    for naam in eigenschappen {
        if !projection.state.iter().any(|p| &p.name == naam) {
            return Err(fout(format!(
                "the state of {} has no '{naam}'",
                na.aggregaat
            )));
        }
    }
    for h in &na.handelingen {
        shape::fact_fields(&service, &h.feit, on, None)
            .map_err(|e| fout(format!("{}: {e}", h.feit)))?;
    }
    Ok(())
}
