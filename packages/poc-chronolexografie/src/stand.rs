//! What a chronicle says about the process, read from its records and the
//! law alone: which record is a submission, a decision or a recorded fact
//! ([`Rol`]), which records a later one revoked, and per submission what
//! answers it now ([`Antwoord`]). The wetsprocessysteem and its screens take
//! this from here rather than guess it from the kind of a record.
//!
//! A record is revoked as the law says it is: a decision or a fact whose
//! article gives it an effect (`effects`, `revokes`, schema v0.8.14 and
//! v0.8.15) revokes the record that effect names, when its `when` holds. A
//! record that is itself revoked revokes nothing, so revoking a revocation
//! revives what it revoked. The engine plays a projection by the same rule;
//! this reads it for the chronicle as a whole, for no reduction.
//!
//! What answers a submission: its last decision, unless that decision is
//! revoked. Then either a record took another decision in its place (the
//! new decision of Awb 7:11, second paragraph, held as an output of type
//! `decision`), or nothing did ([`Antwoord::Vervallen`]). A submission whose
//! decision a recorded fact revoked, such as the receipt of a judgment that
//! annulled it, waits for a decision again, after that fact
//! (`Cell::decide_after`). One whose decision another decision revoked, such
//! as a withdrawal taken ex officio, does not: that decision is the
//! authority's answer.

use std::collections::BTreeMap;

use regelrecht_cel::{EXECUTOGRAM, Error, Gram, decision};
use regelrecht_engine::{LawExecutionService, ParameterType};
use regelrecht_law_model::ArticleBasedLaw;
use serde::Serialize;
use serde_json::Value;

use crate::kern::{Result, in_de_plaats};

/// What a record is in the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rol {
    /// A submission: of the kind its article establishes (`AANVRAAG`,
    /// `BEZWAAR`, `BEROEP`).
    Indiening,
    /// A decision, such as a `BESCHIKKING` or a court's `UITSPRAAK`: any
    /// record that is neither a fact nor of the kind of submission its
    /// article establishes. The cell recorded no other.
    Besluit,
    /// A fact a cell recorded (an executogram), such as an advice or the
    /// receipt of a copy of a judgment.
    Vastlegging,
}

/// What answers a submission now.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum Antwoord {
    /// No decision was taken on it yet.
    Open,
    /// Its last decision stands.
    Besloten { besluit: String },
    /// Its last decision was revoked, and the record `door` took another in
    /// its place: `nieuw`, the value of its output `uitvoer`, with the day
    /// `door` was recorded on. The new decision is no record of its own; it
    /// falls with `door`.
    InDePlaats {
        besluit: String,
        door: String,
        uitvoer: String,
        nieuw: Value,
    },
    /// Its last decision was revoked by the record `door`, and nothing took
    /// its place. With `na` the submission waits for a decision again, to be
    /// taken after that record (`Cell::decide_after`): `door` is a recorded
    /// fact that refers to the decision it revoked. Without it, `door` is a
    /// decision, such as a withdrawal, and the submission waits for nothing.
    Vervallen {
        besluit: String,
        door: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        na: Option<String>,
    },
}

impl Antwoord {
    /// The ids of the records the answer names.
    fn ids(&self) -> impl Iterator<Item = &str> {
        let (besluit, door, na) = match self {
            Self::Open => (None, None, None),
            Self::Besloten { besluit } => (Some(besluit), None, None),
            Self::InDePlaats { besluit, door, .. } => (Some(besluit), Some(door), None),
            Self::Vervallen { besluit, door, na } => (Some(besluit), Some(door), na.as_ref()),
        };
        [besluit, door, na]
            .into_iter()
            .flatten()
            .map(String::as_str)
    }

    /// Whether the submission waits for a decision, and after which record:
    /// `Some(None)` for one not decided on yet, `Some(Some(na))` for one whose
    /// decision was revoked, `None` otherwise.
    pub fn wacht(&self) -> Option<Option<&str>> {
        match self {
            Self::Open => Some(None),
            Self::Vervallen { na: Some(na), .. } => Some(Some(na)),
            Self::Besloten { .. } | Self::InDePlaats { .. } | Self::Vervallen { na: None, .. } => {
                None
            }
        }
    }
}

/// What one chronicle says about the process ([`stand`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Stand {
    /// Per record, by id: what it is.
    pub rollen: BTreeMap<String, Rol>,
    /// Per revoked record, by id: the last record that revoked it.
    pub herroepen: BTreeMap<String, String>,
    /// Per submission, by id: what answers it now.
    pub antwoorden: BTreeMap<String, Antwoord>,
}

/// A submission that waits for a decision ([`Stand::open`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Open<'g, 's> {
    /// The submission.
    pub stuk: &'g Gram,
    /// The record the decision is to be taken after, when an earlier decision
    /// on it was revoked.
    pub na: Option<&'s str>,
}

impl Stand {
    /// What of this standing a viewer gets who may see only the records
    /// `zichtbaar` holds, such as a citizen their own dossier: every entry
    /// about such a record, and of those only the ones that name no record
    /// outside it. A revocation by a record the viewer may not see, and an
    /// answer that names one, is left out whole, so no id of a record
    /// outside reaches the viewer. For a dossier that drops nothing today:
    /// a record that revokes or answers one in it links to it, and so is in
    /// the dossier too. A change to those links would make the screen show
    /// such a decision as standing, not leak the id.
    pub fn binnen(self, zichtbaar: impl Fn(&str) -> bool) -> Self {
        Self {
            rollen: self
                .rollen
                .into_iter()
                .filter(|(id, _)| zichtbaar(id))
                .collect(),
            herroepen: self
                .herroepen
                .into_iter()
                .filter(|(id, door)| zichtbaar(id) && zichtbaar(door))
                .collect(),
            antwoorden: self
                .antwoorden
                .into_iter()
                .filter(|(id, a)| zichtbaar(id) && a.ids().all(&zichtbaar))
                .collect(),
        }
    }

    /// Whether the record `id` is of the role `rol`.
    pub fn is(&self, id: &str, rol: Rol) -> bool {
        self.rollen.get(id) == Some(&rol)
    }

    /// The submissions among `grams` that wait for a decision, in chronicle
    /// order: those not decided on yet, and those whose last decision was
    /// revoked with nothing in its place.
    pub fn open<'g, 's>(&'s self, grams: &'g [Gram]) -> Vec<Open<'g, 's>> {
        grams
            .iter()
            .filter_map(|g| {
                let na = self.antwoorden.get(&g.id)?.wacht()?;
                Some(Open { stuk: g, na })
            })
            .collect()
    }
}

/// The version of the law `gram` is under, on the day it was recorded, with
/// the article's number.
fn law_of<'s, 'g>(
    service: &'s LawExecutionService,
    gram: &'g Gram,
) -> Result<(Option<&'s ArticleBasedLaw>, &'g str)> {
    let (law_id, article) = gram
        .basis
        .split_once('#')
        .ok_or_else(|| Error::Setup(format!("'{}' has no basis", gram.id)))?;
    let law = service
        .resolver()
        .get_law_for_date(law_id, Some(gram.recorded_on));
    Ok((law, article))
}

/// What `gram` is in the process ([`Rol`]), by its kind and the article it is
/// under: an executogram is a recorded fact, a record of the kind of
/// submission its article establishes a submission, and any other record a
/// decision.
///
/// # Errors
///
/// When `gram` names no article in its basis.
pub fn rol(service: &LawExecutionService, gram: &Gram) -> Result<Rol> {
    if gram.kind == EXECUTOGRAM {
        return Ok(Rol::Vastlegging);
    }
    let (law, article) = law_of(service, gram)?;
    let submission = law
        .and_then(|l| l.find_article_by_number(article))
        .and_then(|a| a.get_submission())
        .and_then(|s| s.submission_kind());
    Ok(if submission == Some(gram.kind.as_str()) {
        Rol::Indiening
    } else {
        Rol::Besluit
    })
}

/// Whether `gram` is a decision taken on what it refers to, of the legal
/// character its article gives a decision: a decision whose reasoning the
/// cell can replay. An executogram of a fact, such as the receipt of a
/// judgment, refers to a decision and is none; a decision taken ex officio
/// on nothing is a decision ([`rol`]) but not one taken on a record.
///
/// # Errors
///
/// When `gram` names no article in its basis, or the decision's article does
/// not resolve.
pub fn is_besluit(service: &LawExecutionService, gram: &Gram) -> Result<bool> {
    if gram.refers_to.is_none() || gram.kind == EXECUTOGRAM {
        return Ok(false);
    }
    let (law_id, article) = gram
        .basis
        .split_once('#')
        .ok_or_else(|| Error::Setup(format!("'{}' has no basis", gram.id)))?;
    Ok(decision::derive(service, law_id, article, gram.recorded_on)?.kind == gram.kind)
}

/// The ids of the records the effects of `gram` revoke, as its article gives
/// them and when their `when` holds: a decision revokes the record a field
/// of it names, a fact the record it refers to. An effect whose condition or
/// target the record does not hold revokes nothing here; the cell refused
/// such a record when it was recorded.
fn revokes<'g>(service: &LawExecutionService, gram: &'g Gram, rol: Rol) -> Result<Vec<&'g str>> {
    let (law, article) = law_of(service, gram)?;
    let Some(article) = law.and_then(|l| l.find_article_by_number(article)) else {
        return Ok(Vec::new());
    };
    let holds = |when: &Option<String>, values: &BTreeMap<String, Value>| {
        when.as_ref()
            .is_none_or(|w| values.get(w) == Some(&Value::Bool(true)))
    };
    let targets = match rol {
        Rol::Indiening => Vec::new(),
        Rol::Vastlegging => {
            let effects = gram
                .fact
                .as_deref()
                .and_then(|f| article.required_fact(f))
                .map(|r| r.effects())
                .unwrap_or_default();
            effects
                .iter()
                .filter(|e| holds(&e.when, &gram.fields))
                .filter_map(|_| gram.refers_to.as_deref())
                .collect()
        }
        Rol::Besluit => {
            let effects = article
                .get_decision()
                .filter(|d| d.legal_character() == Some(gram.kind.as_str()))
                .map(|d| d.effects())
                .unwrap_or_default();
            effects
                .iter()
                .filter(|e| holds(&e.when, &gram.outputs))
                .filter_map(|e| gram.fields.get(&e.parameter).and_then(Value::as_str))
                .collect()
        }
    };
    Ok(targets)
}

/// The decision `door` took in the place of the one it revoked: per output
/// of type `decision` a decision holds, the first, or for a fact the first
/// field of type `decision` it holds, such as a court's ruling put in the
/// place of the annulled decision (Awb 8:72, third paragraph, under b). The
/// name of the output or field, and the value with the day `door` was
/// recorded on.
fn in_plaats_van(
    service: &LawExecutionService,
    door: &Gram,
    rol: Rol,
) -> Result<Option<(String, Value)>> {
    if rol == Rol::Besluit {
        return Ok(in_de_plaats(service, door)?.into_iter().next());
    }
    let (law, article) = law_of(service, door)?;
    let Some(article) = law.and_then(|l| l.find_article_by_number(article)) else {
        return Ok(None);
    };
    let Some(fact) = door.fact.as_deref().and_then(|f| article.required_fact(f)) else {
        return Ok(None);
    };
    let decision_typed = |name: &str| {
        article
            .get_parameters()
            .iter()
            .any(|p| p.name == name && p.param_type == ParameterType::Decision)
    };
    let found = fact
        .parameters
        .iter()
        .flatten()
        .filter(|name| decision_typed(name))
        .find_map(|name| match door.fields.get(name) {
            Some(Value::Object(waarde)) => {
                let mut genomen = waarde.clone();
                genomen.insert("recorded_on".into(), serde_json::json!(door.recorded_on));
                Some((name.clone(), Value::Object(genomen)))
            }
            _ => None,
        });
    Ok(found)
}

/// What the chronicle `grams` of one cell says about the process ([`Stand`]),
/// under the law in `service`.
///
/// # Errors
///
/// When a record names no article in its basis.
pub fn stand(service: &LawExecutionService, grams: &[Gram]) -> Result<Stand> {
    let mut rollen = BTreeMap::new();
    for g in grams {
        rollen.insert(g.id.clone(), rol(service, g)?);
    }
    let rol_van = |g: &Gram| rollen.get(&g.id).copied().unwrap_or(Rol::Besluit);
    // Per record its position, and per submission its decisions in order.
    let positie: BTreeMap<&str, usize> = grams
        .iter()
        .enumerate()
        .map(|(i, g)| (g.id.as_str(), i))
        .collect();
    let mut besluiten_op: BTreeMap<&str, Vec<&Gram>> = BTreeMap::new();
    for g in grams.iter().filter(|g| rol_van(g) == Rol::Besluit) {
        if let Some(op) = g.refers_to.as_deref() {
            besluiten_op.entry(op).or_default().push(g);
        }
    }

    // From the last record back: a record already revoked revokes nothing.
    let mut herroepen: BTreeMap<String, String> = BTreeMap::new();
    for (i, gram) in grams.iter().enumerate().rev() {
        if herroepen.contains_key(&gram.id) {
            continue;
        }
        for target in revokes(service, gram, rol_van(gram))? {
            if positie.get(target).is_some_and(|p| *p < i) {
                herroepen
                    .entry(target.to_string())
                    .or_insert_with(|| gram.id.clone());
            }
        }
    }

    let mut antwoorden = BTreeMap::new();
    for stuk in grams.iter().filter(|g| rol_van(g) == Rol::Indiening) {
        let laatste = besluiten_op
            .get(stuk.id.as_str())
            .and_then(|b| b.last().copied());
        let antwoord = match laatste {
            None => Antwoord::Open,
            Some(besluit) => match herroepen.get(&besluit.id) {
                None => Antwoord::Besloten {
                    besluit: besluit.id.clone(),
                },
                Some(door) => {
                    let door = positie
                        .get(door.as_str())
                        .and_then(|p| grams.get(*p))
                        .ok_or_else(|| {
                            Error::Setup(format!(
                                "'{door}' revoked a record and is not in the chronicle"
                            ))
                        })?;
                    na_herroeping(service, besluit, door, rol_van(door))?
                }
            },
        };
        antwoorden.insert(stuk.id.clone(), antwoord);
    }
    Ok(Stand {
        rollen,
        herroepen,
        antwoorden,
    })
}

/// What answers a submission whose last decision `besluit` the record
/// `door` revoked: the decision `door` took in its place, or else nothing.
/// Then a new decision follows `door` when `door` is a recorded fact that
/// refers to `besluit`, the record `Cell::decide_after` accepts. A decision
/// that revoked `besluit` without another in its place, such as a
/// withdrawal, is itself the answer: nothing waits. Only decisions refer to
/// a submission (`kern::verwezen` refuses a fact that does), so the last
/// decision here is the last record the cell finds on it.
fn na_herroeping(
    service: &LawExecutionService,
    besluit: &Gram,
    door: &Gram,
    rol: Rol,
) -> Result<Antwoord> {
    Ok(match in_plaats_van(service, door, rol)? {
        Some((uitvoer, nieuw)) => Antwoord::InDePlaats {
            besluit: besluit.id.clone(),
            door: door.id.clone(),
            uitvoer,
            nieuw,
        },
        None => Antwoord::Vervallen {
            besluit: besluit.id.clone(),
            door: door.id.clone(),
            na: (rol == Rol::Vastlegging && door.refers_to.as_deref() == Some(besluit.id.as_str()))
                .then(|| door.id.clone()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewer_gets_no_entry_that_names_a_record_outside_what_they_may_see() {
        let stand = Stand {
            rollen: BTreeMap::from([
                ("a".into(), Rol::Indiening),
                ("b".into(), Rol::Besluit),
                ("x".into(), Rol::Vastlegging),
            ]),
            herroepen: BTreeMap::from([("b".into(), "x".into())]),
            antwoorden: BTreeMap::from([
                (
                    "a".into(),
                    Antwoord::Vervallen {
                        besluit: "b".into(),
                        door: "x".into(),
                        na: Some("x".into()),
                    },
                ),
                ("c".into(), Antwoord::Open),
            ]),
        };
        let mijn = ["a", "b"];
        let gezien = stand.binnen(|id| mijn.contains(&id));
        assert_eq!(gezien.rollen.keys().collect::<Vec<_>>(), ["a", "b"]);
        assert!(gezien.herroepen.is_empty());
        assert!(gezien.antwoorden.is_empty());
    }
}
