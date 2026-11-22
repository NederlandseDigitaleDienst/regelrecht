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
//!
//! An act may be the system's ([`Vervolghandeling::door_het_systeem`]):
//! after every step, [`automatisch`] records each such act that is next, by
//! the same check a person's act passes ([`toets`]).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_cel::{Error, reading, shape};
use regelrecht_law_model::stored;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::casus::Opvolging;
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
    /// What a citizen reads of a grant that no longer counts because a new
    /// one took its place, such as a card replaced by a duplicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vervallen: Option<String>,
    /// What a citizen can apply for about a grant once every act is done,
    /// such as a duplicate of a card that is lost. Each is a submission the
    /// case establishes, which refers to the record that granted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aanvragen: Vec<Vervolgaanvraag>,
}

/// An application about a grant ([`NaToekenning::aanvragen`]).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Vervolgaanvraag {
    /// The article that establishes it, `<regelwerk>#<article>`.
    pub artikel: String,
    /// What the way in to it says.
    pub knop: String,
    /// The title of its form.
    pub titel: String,
    /// What the form says before the fields.
    pub uitleg: String,
    /// The output of the decision on it that says whether it grants what
    /// was asked, as `uitkomst` does for the case's own application.
    pub uitkomst: String,
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
    /// What the button that records it says, for an act a person records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knop: Option<String>,
    /// For an act no person needs to do, such as ordering the card: the
    /// article of the cell's own policy that has the system record it, in
    /// the form `bij_aanvraag` takes ([`crate::verzoek::opvolgen`]). Its
    /// `submission` parameter gets the record that granted. Without it a
    /// person records the act.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door_het_systeem: Option<Opvolging>,
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
    for a in &na.aanvragen {
        if corpus.afleiding.indiening(&a.artikel).is_none() {
            return Err(fout(format!(
                "{} establishes no submission of this case",
                a.artikel
            )));
        }
    }
    for h in &na.handelingen {
        shape::fact_fields(&service, &h.feit, on, None)
            .map_err(|e| fout(format!("{}: {e}", h.feit)))?;
        match (&h.door_het_systeem, &h.knop) {
            (Some(_), Some(_)) => {
                return Err(fout(format!(
                    "{}: an act is the system's or a person's, so it has `door_het_systeem` or \
                     `knop`, not both",
                    h.feit
                )));
            }
            (Some(o), None) => {
                if o.antwoord.is_some() {
                    return Err(fout(format!(
                        "{}: an act after a grant awaits no answer",
                        h.feit
                    )));
                }
                let eigen = corpus_eigen_authority(corpus)?;
                let feit = crate::verzoek::feit_van(&service, o, &eigen, on)
                    .map_err(|e| fout(e.to_string()))?;
                if feit.as_deref() != Some(h.feit.as_str()) {
                    return Err(fout(format!(
                        "{} records {}, not {}",
                        o.artikel,
                        feit.as_deref().unwrap_or("nothing"),
                        h.feit
                    )));
                }
            }
            (None, None) => {
                return Err(fout(format!(
                    "{}: an act a person records needs a `knop`",
                    h.feit
                )));
            }
            (None, Some(_)) => {}
        }
    }
    Ok(())
}

/// Record every act after a grant that is the system's and next
/// ([`Vervolghandeling::door_het_systeem`]), by the article that says the
/// system takes it, after whatever step brought the grant about: a decision
/// on an application or on an objection, a new decision after an
/// annulment, or a receipt that makes a grant count again. Each passes
/// [`toets`], so it comes in its turn and once. One act may make the next
/// one the system's to take, so this runs in rounds until a round records
/// nothing; there are never more rounds than acts, since every round
/// advances a grant by one act or ends. The ids of the records it made. It
/// runs on a copy first, so all of it lands or none.
///
/// Only the grants the step changed take part: those whose [`Stand`] differs
/// from what it was before the step (`voor`, from [`standen`]). A step about
/// something else brings about no act of the system, also not one that some
/// other grant still has pending.
///
/// # Errors
///
/// What the article or the cells refuse, and [`Error::Setup`] when the acts
/// do not settle within that many rounds.
pub fn automatisch(
    corpus: &Corpus,
    cellen: &mut Cellen,
    voor: &[Stand],
    on: NaiveDate,
) -> Result<Vec<String>> {
    let Some(na) = &corpus.casus.na_toekenning else {
        return Ok(Vec::new());
    };
    if na.handelingen.iter().all(|h| h.door_het_systeem.is_none()) {
        return Ok(Vec::new());
    }
    let veranderd: BTreeSet<String> = standen(corpus, cellen, on)?
        .into_iter()
        .filter(|s| !voor.contains(s))
        .map(|s| s.toekenning)
        .collect();
    if veranderd.is_empty() {
        return Ok(Vec::new());
    }
    automatisch_in(corpus, na, &mut cellen.sandbox(), &veranderd, on)?;
    automatisch_in(corpus, na, cellen, &veranderd, on)
}

/// [`automatisch`] for the grants in `veranderd`, on `cellen` itself.
fn automatisch_in(
    corpus: &Corpus,
    na: &NaToekenning,
    cellen: &mut Cellen,
    veranderd: &BTreeSet<String>,
    on: NaiveDate,
) -> Result<Vec<String>> {
    let service = corpus.service()?;
    // Every record of every cell before this runs: what the step itself
    // recorded, in any cell, is not the system's.
    let voor: BTreeSet<String> = std::iter::once(&cellen.eigen)
        .chain(&cellen.andere)
        .flat_map(|c| c.grams())
        .map(|g| g.id.clone())
        .collect();
    let mut ronde = || -> Result<usize> {
        let mut gedaan = 0;
        for stand in standen(corpus, cellen, on)?
            .into_iter()
            .filter(|s| veranderd.contains(&s.toekenning))
        {
            let Some(volgende) = &stand.volgende else {
                continue;
            };
            let Some(opvolging) = na
                .handelingen
                .iter()
                .find(|h| &h.feit == volgende)
                .and_then(|h| h.door_het_systeem.as_ref())
            else {
                continue;
            };
            let Some(aanleiding) = cellen
                .eigen
                .grams()
                .iter()
                .find(|g| g.id == stand.toekenning)
                .cloned()
            else {
                continue;
            };
            toets(corpus, cellen, volgende, Some(&stand.toekenning), on)?;
            let gedaan_feit = crate::verzoek::opvolgen(
                &service,
                cellen,
                opvolging,
                &aanleiding,
                Some(&stand.toekenning),
                on,
            )?;
            if gedaan_feit.is_some() {
                gedaan += 1;
            }
        }
        Ok(gedaan)
    };
    tot_rust(na.handelingen.len(), &mut ronde)?;
    Ok(std::iter::once(&cellen.eigen)
        .chain(&cellen.andere)
        .flat_map(|c| c.grams())
        .filter(|g| !voor.contains(&g.id))
        .map(|g| g.id.clone())
        .collect())
}

/// Run `ronde` until it does nothing, at most `rondes` times and once more
/// to see that it settled.
///
/// # Errors
///
/// What `ronde` fails on, and [`Error::Setup`] when it still acts after
/// `rondes` rounds.
fn tot_rust(rondes: usize, ronde: &mut dyn FnMut() -> Result<usize>) -> Result<()> {
    for _ in 0..=rondes {
        if ronde()? == 0 {
            return Ok(());
        }
    }
    Err(Error::Setup(format!(
        "the acts after a grant do not settle within {rondes} rounds"
    )))
}

/// The acts the system would take by itself if a step brought `cellen` to
/// where they are from `voor`, without recording anything: per act its fact,
/// the article that has the system take it, and the record it refers to.
/// For a preview of a decision.
///
/// # Errors
///
/// As [`automatisch`].
pub fn zou_automatisch(
    corpus: &Corpus,
    cellen: &Cellen,
    voor: &[Stand],
    on: NaiveDate,
) -> Result<Vec<Value>> {
    let mut proef = cellen.sandbox();
    let ids = automatisch(corpus, &mut proef, voor, on)?;
    Ok(proef
        .eigen
        .grams()
        .iter()
        .filter(|g| ids.contains(&g.id))
        .map(|g| {
            serde_json::json!({
                "feit": format!("{}#{}", g.basis, g.fact.as_deref().unwrap_or_default()),
                "artikel": g.basis,
                "verwijst_naar": g.refers_to,
            })
        })
        .collect())
}

/// The authority of the case's own cell, from the cell itself.
fn corpus_eigen_authority(corpus: &Corpus) -> Result<regelrecht_engine::Authority> {
    Ok(corpus.cell()?.eigen.authority().clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn rounds_end_when_a_round_does_nothing() {
        let mut gedaan = [1, 1, 0].into_iter();
        assert!(tot_rust(3, &mut || Ok(gedaan.next().unwrap_or(0))).is_ok());
    }

    #[test]
    fn acts_that_keep_coming_are_stopped() {
        let mut keer = 0;
        let e = tot_rust(2, &mut || {
            keer += 1;
            Ok(1)
        })
        .unwrap_err();
        assert_eq!(keer, 3);
        assert!(e.to_string().contains("do not settle"), "{e}");
    }
}
