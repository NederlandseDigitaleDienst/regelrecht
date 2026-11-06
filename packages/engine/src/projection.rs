//! A reduction that plays a cell's chronicle forward (schema v0.8.14).
//!
//! A reduction with a `projection` reads the records of a cell's chronicle
//! that are in force. The engine plays the records the cell knew on the
//! calculation date in the order the cell recorded them, carries out what
//! the law says a record does with an earlier one, and gives the
//! reduction's inputs that read decisions or facts (`source.decisions`,
//! `source.facts`) only the records still in force, as rows. The
//! reduction's `execution` computes its answer from those rows. It keeps no
//! state of its own: what a record changes is in the record, and whether it
//! still counts is the engine's.
//!
//! An answer has two days (schema v0.8.15). It is about the calculation
//! date, the day it must hold on (`$referencedate` in the reduction), and it
//! is given as known on a day, by default the same one: a record the cell
//! came to know after that day is not played ([`ProjectionOptions`]). So a
//! question about last month can be answered with what was known then or
//! with what is known now.
//!
//! What a record does with an earlier record is law, not part of the
//! reduction: the article says that a record of it undoes or replaces
//! another record (`produces[].effects`, `requires[].effects`), optionally
//! only `when` an outcome is true and only `from` or `until` a day, under the
//! version of that law in force on the day the record was recorded. Any
//! record may have effects (an application, an objection, a decision, a
//! fact), on any earlier record, and an effect touches only the record it
//! names. The engine carries that out by rewinding. When an effect of a
//! record that counts undoes an earlier record that was applied, the engine
//! plays the chronicle again from the start and skips the undone record. A
//! record is skipped for as long as at least one record that undoes it
//! counts. A record that is itself revoked or replaced undoes nothing any
//! more, so a record it alone undid counts again and the engine rewinds once
//! more; a replacement that should keep an undoing in force declares that
//! effect itself. The chronicle is never changed. An undone record is gone
//! from the whole history (ex tunc), unless the effect holds only from or
//! until a day: then it is gone only from an answer about a day in that
//! window, such as a decision suspended for a while (Awb 8:81).
//!
//! A record another cell recorded, such as a court's judgment this cell
//! received a copy of, plays here from the day this cell received it
//! (`received_on`); its effects come from its own law on the day the other
//! cell recorded it.
//!
//! With `key`, the engine plays the records of one person: those that hold
//! the asked key under the key's name, and what refers to them. A record of
//! someone else that refers in counts for its effects, but is no row of the
//! reduction.
//!
//! The engine records every step it takes: each record applied, each record
//! skipped (with who undid it), each record of someone else that counted for
//! its effects only, and each rewind (with what caused it), and the records
//! in force at the end. The events come back with the answer and, when
//! tracing, as a `PROJECTION` node in the engine's trace.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_law_model::{Article, ArticleBasedLaw, EffectKind, Projection, RecordEffect};
use serde::{Deserialize, Serialize};

use crate::answer::{Call, Chronicle, ChronicleRead};
use crate::engine::ArticleResult;
use crate::error::{EngineError, Result};
use crate::service::LawExecutionService;
use crate::trace::{PathNode, TraceBuilder};
use crate::types::{ParameterType, PathNodeType, Value};

/// The kind of a record of a fact (RFC-051), as the cell writes it.
const EXECUTOGRAM: &str = "EXECUTOGRAM";

/// One record of a cell's chronicle, as the projection reads it: its id, its
/// kind (`AANVRAAG`, `BESCHIKKING`, `EXECUTOGRAM`, ...), the article it rests
/// on, the fact for an executogram, the day it was recorded, and what it
/// holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ChronicleRecord {
    pub id: String,
    pub kind: String,
    pub basis: String,
    pub fact: Option<String>,
    /// The day it was recorded, by the cell that recorded it: the version
    /// of its law in force that day gives its effects.
    pub recorded_on: NaiveDate,
    /// For a record of another cell, such as a judgment this cell received
    /// a copy of: the day this cell received it. It is known from then on;
    /// a record of the cell itself is known from the day it was recorded.
    pub received_on: Option<NaiveDate>,
    /// The record it refers to, such as the submission a decision was taken
    /// on, or the decision an objection contests.
    pub refers_to: Option<String>,
    pub fields: BTreeMap<String, Value>,
    pub outputs: BTreeMap<String, Value>,
}

impl ChronicleRecord {
    /// The day from which the cell knew the record.
    pub fn known_from(&self) -> NaiveDate {
        self.received_on.unwrap_or(self.recorded_on)
    }
}

/// What caused a rewind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewindCause {
    /// A record undoes an earlier one that was applied.
    Undoes,
    /// A record replaces an earlier one that was applied.
    Replaces,
    /// A record that undid an earlier one is itself undone or replaced now,
    /// and no other record undoes the earlier one any more: it counts again.
    Revives,
}

impl From<EffectKind> for RewindCause {
    fn from(kind: EffectKind) -> Self {
        match kind {
            EffectKind::Undoes => Self::Undoes,
            EffectKind::Replaces => Self::Replaces,
        }
    }
}

/// One step of playing the chronicle, in the order the engine took them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProjectionEvent {
    /// The record counts, so far.
    Applied { record: String },
    /// The record was skipped, because these records undo it.
    Skipped {
        record: String,
        undone_by: Vec<String>,
    },
    /// The engine starts again from the beginning: `record` changed whether
    /// `target` counts.
    Rewound {
        record: String,
        cause: RewindCause,
        target: String,
    },
    /// The record belongs to someone else, but what it does with a record
    /// of this partition holds here too: its effects hold and may make the
    /// engine rewind, while it is no row of the reduction. `reason` says why
    /// it is someone else's: its own key differs (`key differs: <field>`),
    /// or it refers only to such records (`refers to <id>`).
    EffectsOnly { record: String, reason: String },
}

/// The answer of a projection: the ids of the records in force (the rows
/// the reduction read), every event, and the result of the reduction's
/// `execution`.
#[derive(Debug, Clone)]
pub struct ProjectionOutcome {
    pub in_force: Vec<String>,
    pub events: Vec<ProjectionEvent>,
    pub result: ArticleResult,
}

/// How a projection is asked, beside the day it is about
/// ([`LawExecutionService::evaluate_projection_with`]).
#[derive(Default)]
pub struct ProjectionOptions<'a> {
    /// The day up to which records are known: a record the cell came to
    /// know later is not played. Without it, the calculation date.
    pub known_on: Option<&'a str>,
    /// Who answers the reduction's execution, such as the cell that runs
    /// it (RFC-048). Its chronicle is replaced by the records in force.
    pub call: Call<'a>,
    /// Whether the result carries the engine's trace.
    pub trace: bool,
}

impl LawExecutionService {
    /// Play `records`, in their order, through the projection of the
    /// reduction `law_id` that gives `output_names`, and evaluate those
    /// outputs with `parameters`; its inputs that read submissions,
    /// decisions or facts read the records in force. The answer is about
    /// `calculation_date`, as known on that day.
    pub fn evaluate_projection(
        &self,
        law_id: &str,
        output_names: &[&str],
        records: &[ChronicleRecord],
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: bool,
    ) -> Result<ProjectionOutcome> {
        self.evaluate_projection_with(
            law_id,
            output_names,
            records,
            parameters,
            calculation_date,
            ProjectionOptions {
                trace,
                ..ProjectionOptions::default()
            },
        )
    }

    /// [`Self::evaluate_projection`], as known on the day `options` names,
    /// with the call it gives the reduction's execution.
    ///
    /// Refuses a reduction article without a projection, an effect on a
    /// record that is not an earlier record, an effect whose `when` outcome
    /// the record does not hold as a boolean, and a replay that does not
    /// settle.
    pub fn evaluate_projection_with<'a>(
        &self,
        law_id: &str,
        output_names: &[&str],
        records: &[ChronicleRecord],
        parameters: BTreeMap<String, Value>,
        calculation_date: &'a str,
        options: ProjectionOptions<'a>,
    ) -> Result<ProjectionOutcome> {
        let day = parse_day(calculation_date)?;
        let known_on = match options.known_on {
            Some(known_on) => parse_day(known_on)?,
            None => day,
        };
        let Some(first) = output_names.first() else {
            return Err(EngineError::InvalidOperation("no output asked".to_string()));
        };
        let (projection, key_type) = {
            let law = self
                .resolver()
                .get_law_for_date(law_id, Some(day))
                .ok_or_else(|| EngineError::LawNotFound(law_id.to_string()))?;
            let article = law.find_article_by_output(first).ok_or_else(|| {
                EngineError::InvalidOperation(format!("'{law_id}' has no output '{first}'"))
            })?;
            if let Some(other) = output_names
                .iter()
                .find(|o| law.find_article_by_output(o).map(|a| &a.number) != Some(&article.number))
            {
                return Err(EngineError::InvalidOperation(format!(
                    "'{law_id}' answers '{first}' and '{other}' from different articles; a \
                     projection answers from one"
                )));
            }
            let projection = projection_of(law_id, article)?.clone();
            let key_type = projection.key.as_ref().and_then(|key| {
                article
                    .get_parameters()
                    .iter()
                    .find(|p| &p.name == key)
                    .map(|p| p.param_type)
            });
            (projection, key_type)
        };

        // What the cell came to know after the day it answers as of is not
        // played.
        let known: Vec<ChronicleRecord> = records
            .iter()
            .filter(|r| r.known_from() <= known_on)
            .cloned()
            .collect();
        let partition = self.partition(&projection, key_type.as_ref(), &known, &parameters)?;
        let played = self.play(law_id, &known, &partition, day)?;
        let in_force = InForce {
            records: played.in_force.iter().map(|&i| &known[i]).collect(),
        };
        // The reduction, and every reduction it calls, reads the records in
        // force and nothing else.
        let call = Call {
            chronicle: Some(&in_force),
            ..options.call
        };

        let mut result = if options.trace {
            self.evaluate_law_traced_played(
                law_id,
                output_names,
                parameters,
                call,
                calculation_date,
                TraceBuilder::new(),
                true,
            )?
        } else {
            self.evaluate_law_played(
                law_id,
                output_names,
                parameters,
                call,
                calculation_date,
                true,
            )?
        };
        let in_force: Vec<String> = in_force.records.iter().map(|r| r.id.clone()).collect();
        if let Some(root) = result.trace.as_mut() {
            root.children
                .insert(0, trace_node(&played.events, &in_force));
        }
        Ok(ProjectionOutcome {
            in_force,
            events: played.events,
            result,
        })
    }

    /// Play the chronicle, rewinding until no record changes whether an
    /// earlier record counts. Returns the positions of the records in force
    /// that are rows of the reduction, and every event. `valid_on` is the
    /// day the answer is about, which decides whether an effect limited by
    /// `from` or `until` holds.
    fn play(
        &self,
        law_id: &str,
        records: &[ChronicleRecord],
        partition: &Partition,
        valid_on: NaiveDate,
    ) -> Result<Played> {
        // undone_by[t]: the records with an effect on t that held when they
        // were last played, each with what it does to t. A record is skipped
        // while this is not empty.
        let mut undone_by: BTreeMap<&str, BTreeMap<&str, EffectKind>> = BTreeMap::new();
        let mut events = Vec::new();
        // Each rewind follows from a record changing whether an earlier one
        // counts, and effects only reach back, so the replays settle. A
        // chain of undoings (a revocation of a revocation of ...) needs a
        // number of replays quadratic in its length, because each replay
        // stops at the first change. The bound is well above that and turns
        // a bug into an error instead of an endless replay.
        let max_passes = records.len() * records.len() + 2;
        for _ in 0..max_passes {
            let pass_start = events.len();
            let mut applied: BTreeSet<&str> = BTreeSet::new();
            let mut in_force = Vec::new();
            let mut rewind = None;

            'records: for (i, record) in records.iter().enumerate() {
                if !partition.holds(&record.id) {
                    continue;
                }
                let undoers: Vec<String> = undone_by
                    .get(record.id.as_str())
                    .map(|u| u.keys().map(|id| id.to_string()).collect())
                    .unwrap_or_default();
                if !undoers.is_empty() {
                    events.push(ProjectionEvent::Skipped {
                        record: record.id.clone(),
                        undone_by: undoers,
                    });
                    // A record that is skipped, revoked or replaced, undoes
                    // nothing any more: a record it alone undid counts
                    // again. A replacer that should keep an undoing in
                    // force says so itself, with an effect of its own.
                    let mut revived = None;
                    for (target, by) in undone_by.iter_mut() {
                        if by.remove(record.id.as_str()).is_some()
                            && by.is_empty()
                            && revived.is_none()
                        {
                            revived = Some(target.to_string());
                        }
                    }
                    if let Some(target) = revived {
                        rewind = Some(ProjectionEvent::Rewound {
                            record: record.id.clone(),
                            cause: RewindCause::Revives,
                            target,
                        });
                        break 'records;
                    }
                    continue;
                }

                // An effect on a record of another partition is that
                // partition's business.
                let effects = self.holding_effects(record, &records[..i], valid_on)?;
                for (kind, target) in effects.into_iter().filter(|(_, t)| partition.holds(t)) {
                    undone_by
                        .entry(target)
                        .or_default()
                        .entry(record.id.as_str())
                        .or_insert(kind);
                    // A target that was applied had no undoer yet: one would
                    // have made the replay skip it.
                    if applied.contains(target) {
                        rewind = Some(ProjectionEvent::Rewound {
                            record: record.id.clone(),
                            cause: kind.into(),
                            target: target.to_string(),
                        });
                        break 'records;
                    }
                }
                applied.insert(record.id.as_str());
                // A record of someone else counts for its effects only: the
                // law says once what it undoes, whoever the question is
                // about, but it is no row of this reduction.
                match partition.foreign.get(&record.id) {
                    Some(reason) => events.push(ProjectionEvent::EffectsOnly {
                        record: record.id.clone(),
                        reason: reason.clone(),
                    }),
                    None => {
                        events.push(ProjectionEvent::Applied {
                            record: record.id.clone(),
                        });
                        in_force.push(i);
                    }
                }
            }

            // Who undoes a record is known only at the end of the replay: a
            // later record that turned out to be skipped no longer does.
            for event in &mut events[pass_start..] {
                if let ProjectionEvent::Skipped {
                    record,
                    undone_by: by,
                } = event
                {
                    if let Some(now) = undone_by.get(record.as_str()).filter(|n| !n.is_empty()) {
                        *by = now.keys().map(|id| id.to_string()).collect();
                    }
                }
            }
            match rewind {
                Some(event) => events.push(event),
                None => return Ok(Played { in_force, events }),
            }
        }
        Err(EngineError::InvalidOperation(format!(
            "the projection of '{law_id}' does not settle after {max_passes} replays"
        )))
    }

    /// The article `record` rests on, in the law of the day it was recorded.
    /// A record whose article cannot be found could undo another one unseen;
    /// that is refused rather than read as "no effects".
    fn article_of(&self, record: &ChronicleRecord) -> Result<&Article> {
        let unresolved = || {
            EngineError::InvalidOperation(format!(
                "record '{}' rests on '{}', which names no article in force on {}",
                record.id, record.basis, record.recorded_on
            ))
        };
        let (law_id, number) = record.basis.split_once('#').ok_or_else(unresolved)?;
        self.resolver()
            .get_law_for_date(law_id, Some(record.recorded_on))
            .and_then(|law| law.find_article_by_number(number))
            .ok_or_else(unresolved)
    }

    /// The records of the chronicle that belong to the key the reduction is
    /// asked for (`projection.key`), or all records when it names none.
    ///
    /// A record belongs when it holds the asked key under the key's name, as
    /// a field or an output. A record that refers to a record of the
    /// partition (`refers_to`, or a field an effect of its article names,
    /// such as `bestreden_besluit`) joins it too, transitively. It joins for
    /// its effects only when it is someone else's: when it holds a key of
    /// its own with another value, or holds none and refers only to records
    /// that are someone else's. Such a record still undoes or replaces what
    /// it names, because the law says once what a record does with another,
    /// whatever the question is keyed on; it is no row of the reduction.
    /// Following references through it keeps the effects right: a record
    /// that undoes it joins too, and what it undid counts again. Records only
    /// join through what refers to them later, never through what they refer
    /// to, so the partition stays the records reachable from the key. A
    /// record without a key therefore plays only when it refers in. The
    /// chronicle lists a record after what it refers to, which one pass in
    /// its order relies on.
    fn partition(
        &self,
        projection: &Projection,
        key_type: Option<&ParameterType>,
        records: &[ChronicleRecord],
        parameters: &BTreeMap<String, Value>,
    ) -> Result<Partition> {
        let Some(key_name) = &projection.key else {
            return Ok(Partition::default());
        };
        let key = parameters.get(key_name).ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "the projection plays the records of one '{key_name}', but none is passed"
            ))
        })?;
        // A key of another type than the parameter declares would match no
        // record and give the initial state without a word.
        if let Some(declared) = key_type.filter(|t| !fits_type(t, key)) {
            return Err(EngineError::InvalidOperation(format!(
                "'{key_name}' is passed as {key:?}, which is no {declared:?}"
            )));
        }
        let mut members: BTreeSet<String> = BTreeSet::new();
        let mut foreign: BTreeMap<String, String> = BTreeMap::new();
        for record in records {
            let own = record
                .fields
                .get(key_name)
                .or_else(|| record.outputs.get(key_name))
                // A key that holds null is no key.
                .filter(|value| !matches!(value, Value::Null));
            if own == Some(key) {
                members.insert(record.id.clone());
                continue;
            }
            let references = self.references(record)?;
            let Some(first) = references.iter().find(|id| members.contains(*id)) else {
                continue;
            };
            let own_record = references
                .iter()
                .any(|id| members.contains(id) && !foreign.contains_key(id));
            let reason = match own {
                Some(_) => Some(format!("key differs: {key_name}")),
                None if !own_record => Some(format!("refers to {first}")),
                None => None,
            };
            if let Some(reason) = reason {
                foreign.insert(record.id.clone(), reason);
            }
            members.insert(record.id.clone());
        }
        Ok(Partition {
            members: Some(members),
            foreign,
        })
    }

    /// The records `record` refers to: its `refers_to`, and the values of
    /// the fields that an effect of its article names, of any item or fact
    /// of it. That holds for every record under the article, so an
    /// objection that names the contested decision in such a field refers
    /// to it, as the decision on it does.
    fn references(&self, record: &ChronicleRecord) -> Result<Vec<String>> {
        let mut ids: Vec<String> = record.refers_to.iter().cloned().collect();
        let article = self.article_of(record)?;
        for effect in all_effects(article) {
            if let Some(Value::String(id)) = record.fields.get(&effect.parameter) {
                ids.push(id.clone());
            }
        }
        Ok(ids)
    }

    /// The effects the law gives `record` (schema v0.8.14, v0.8.15): those
    /// of the item of its article it is (a submission or a decision), or for
    /// an executogram those of the fact it records, under the law of the
    /// day it was recorded. A record of a kind its article does not
    /// produce, or of a fact it does not require, is refused, for the same
    /// reason as an unknown article.
    fn effects_of<'s>(&'s self, record: &ChronicleRecord) -> Result<&'s [RecordEffect]> {
        let article = self.article_of(record)?;
        let unknown = |what: String| {
            EngineError::InvalidOperation(format!(
                "record '{}' is {what} under '{}', which that article does not {}",
                record.id,
                record.basis,
                if record.kind == EXECUTOGRAM {
                    "require"
                } else {
                    "produce"
                }
            ))
        };
        if record.kind == EXECUTOGRAM {
            let fact = record.fact.as_deref();
            let requirement = article
                .get_requires()
                .iter()
                .find(|r| fact.is_some() && r.fact.as_deref() == fact)
                .ok_or_else(|| unknown(format!("the fact {}", fact.unwrap_or("(none)"))))?;
            return Ok(requirement.effects.as_deref().unwrap_or_default());
        }
        let submission = article
            .get_submission()
            .filter(|s| s.submission_kind() == Some(record.kind.as_str()));
        let decision = article
            .get_decision()
            .filter(|d| d.legal_character() == Some(record.kind.as_str()));
        submission
            .or(decision)
            .map(|item| item.effects())
            .ok_or_else(|| unknown(format!("a {}", record.kind)))
    }

    /// The effects of `record` that hold for an answer about `valid_on`,
    /// each with the earlier record it undoes.
    fn holding_effects<'r>(
        &self,
        record: &ChronicleRecord,
        earlier: &'r [ChronicleRecord],
        valid_on: NaiveDate,
    ) -> Result<Vec<(EffectKind, &'r str)>> {
        let mut holding = Vec::new();
        for effect in self.effects_of(record)? {
            if let Some(when) = &effect.when {
                match record.outputs.get(when) {
                    Some(Value::Bool(true)) => {}
                    Some(Value::Bool(false)) => continue,
                    _ => {
                        return Err(EngineError::InvalidOperation(format!(
                            "record '{}' ({}) has an effect that depends on '{when}', but holds \
                             no boolean under it",
                            record.id, record.basis
                        )))
                    }
                }
            }
            if !holds_on(record, effect, valid_on)? {
                continue;
            }
            let name = &effect.parameter;
            // A field that holds null names no record, such as the earlier
            // application of a first one: the effect undoes nothing.
            if matches!(record.fields.get(name), Some(Value::Null)) {
                continue;
            }
            let Some(Value::String(target)) = record.fields.get(name) else {
                return Err(EngineError::InvalidOperation(format!(
                    "record '{}' ({}) has an effect on '{name}', but holds no record id under it",
                    record.id, record.basis
                )));
            };
            let target = earlier.iter().find(|r| &r.id == target).ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "record '{}' ({}) has an effect on '{target}', which is no record \
                         recorded before it",
                    record.id, record.basis
                ))
            })?;
            holding.push((effect.kind, target.id.as_str()));
        }
        Ok(holding)
    }
}

/// Every effect an item or fact of `article` declares.
fn all_effects(article: &Article) -> impl Iterator<Item = &RecordEffect> {
    let produced = article.get_produced().into_iter().flat_map(|p| p.effects());
    let facts = article
        .get_requires()
        .iter()
        .flat_map(|r| r.effects.iter().flatten());
    produced.chain(facts)
}

/// Whether `effect` of `record` holds for an answer about `valid_on`: on or
/// after its `from`, and before its `until`, each a date the record holds.
/// A null date sets no bound. A date the record does not hold at all is
/// refused: read as no bound, it would undo the other record for every day.
fn holds_on(record: &ChronicleRecord, effect: &RecordEffect, valid_on: NaiveDate) -> Result<bool> {
    let day =
        |name: &String| -> Result<Option<NaiveDate>> {
            let value = record.fields.get(name).or_else(|| record.outputs.get(name));
            match value {
                None => Err(EngineError::InvalidOperation(format!(
                    "record '{}' ({}) has an effect that holds from or until '{name}', but holds \
                     no value under it",
                    record.id, record.basis
                ))),
                Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(Some)
                    .map_err(|e| {
                        EngineError::InvalidOperation(format!(
                            "record '{}' ({}) holds '{name}' as {s}, which is no date: {e}",
                            record.id, record.basis
                        ))
                    }),
                Some(other) => Err(EngineError::InvalidOperation(format!(
                    "record '{}' ({}) holds '{name}' as {other:?}, which is no date",
                    record.id, record.basis
                ))),
            }
        };
    let from = effect.from.as_ref().map(day).transpose()?.flatten();
    let until = effect.until.as_ref().map(day).transpose()?.flatten();
    Ok(from.is_none_or(|f| f <= valid_on) && until.is_none_or(|u| valid_on < u))
}

/// Which records a play reads: all of them (`members` is `None`), or those of
/// one key, with per record of someone else among them why it counts for
/// its effects only.
#[derive(Default)]
struct Partition {
    members: Option<BTreeSet<String>>,
    foreign: BTreeMap<String, String>,
}

impl Partition {
    fn holds(&self, id: &str) -> bool {
        self.members.as_ref().is_none_or(|m| m.contains(id))
    }
}

/// The records in force after the last replay, by position, and the events.
struct Played {
    in_force: Vec<usize>,
    events: Vec<ProjectionEvent>,
}

/// The records in force, as the chronicle the reduction reads: per input
/// that reads the decisions under an article or the recorded instances of a
/// fact, one row per record in force, in the order of the chronicle.
struct InForce<'r> {
    records: Vec<&'r ChronicleRecord>,
}

impl Chronicle for InForce<'_> {
    fn keeper(&self) -> &str {
        "the records in force"
    }

    fn rows(
        &self,
        service: &LawExecutionService,
        _law: &ArticleBasedLaw,
        read: ChronicleRead<'_>,
        _calculation_date: &str,
    ) -> Result<Option<Value>> {
        let mut rows = Vec::new();
        for record in &self.records {
            let reads = match read {
                ChronicleRead::Decisions(basis) => {
                    record.basis == basis
                        && record.kind != EXECUTOGRAM
                        && service
                            .article_of(record)?
                            .get_decision()
                            .and_then(|d| d.legal_character())
                            == Some(record.kind.as_str())
                }
                ChronicleRead::Submissions(basis) => {
                    record.basis == basis
                        && record.kind != EXECUTOGRAM
                        && service
                            .article_of(record)?
                            .get_submission()
                            .and_then(|s| s.submission_kind())
                            == Some(record.kind.as_str())
                }
                ChronicleRead::Facts(fact) => fact.rsplit_once('#').is_some_and(|(basis, name)| {
                    record.kind == EXECUTOGRAM
                        && record.basis == basis
                        && record.fact.as_deref() == Some(name)
                }),
            };
            if reads {
                rows.push(row(record));
            }
        }
        Ok(Some(Value::Array(rows)))
    }
}

/// A record as a row: its fields, then its outputs, then `id`, `basis`,
/// `type`, `recorded_on` and `refers_to` (null when it refers to none); a
/// later one wins over a field of the same name.
fn row(record: &ChronicleRecord) -> Value {
    let mut row = record.fields.clone();
    row.extend(record.outputs.clone());
    row.insert("id".to_string(), Value::String(record.id.clone()));
    row.insert("basis".to_string(), Value::String(record.basis.clone()));
    row.insert("type".to_string(), Value::String(record.kind.clone()));
    row.insert(
        "recorded_on".to_string(),
        Value::String(record.recorded_on.to_string()),
    );
    row.insert(
        "refers_to".to_string(),
        record.refers_to.clone().map_or(Value::Null, Value::String),
    );
    Value::Object(row)
}

/// Whether `value` is a value of `ty`, for the key passed to a projection:
/// boolean, number, string, amount (whole eurocents) or date.
fn fits_type(ty: &ParameterType, value: &Value) -> bool {
    match (ty, value) {
        (ParameterType::Boolean, Value::Bool(_))
        | (ParameterType::Number, Value::Int(_) | Value::Decimal(_))
        | (ParameterType::Amount, Value::Int(_))
        | (ParameterType::String, Value::String(_)) => true,
        (ParameterType::Date, Value::String(s)) => NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok(),
        _ => false,
    }
}

fn projection_of<'a>(law_id: &str, article: &'a Article) -> Result<&'a Projection> {
    article
        .machine_readable
        .as_ref()
        .and_then(|m| m.projection.as_ref())
        .ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "article {} of '{law_id}' has no projection",
                article.number
            ))
        })
}

/// Whether `article` reads the chronicle through a projection: a reduction
/// article only [`LawExecutionService::evaluate_projection`] answers.
pub fn has_projection(article: &Article) -> bool {
    article
        .machine_readable
        .as_ref()
        .is_some_and(|m| m.projection.is_some())
}

fn parse_day(date: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| EngineError::InvalidDate(format!("{date}: {e}")))
}

/// The events as a `PROJECTION` node of the engine trace: one child per
/// event, named after it, with a message saying what happened; the node's
/// result is the ids of the records in force.
fn trace_node(events: &[ProjectionEvent], in_force: &[String]) -> PathNode {
    let mut node = PathNode::new(PathNodeType::Operation, "PROJECTION").with_result(Value::Array(
        in_force.iter().cloned().map(Value::String).collect(),
    ));
    node.message = Some(format!("in force: {}", show_ids(in_force)));
    for event in events {
        let (name, message) = match event {
            ProjectionEvent::Applied { record } => ("APPLY", record.clone()),
            ProjectionEvent::Skipped { record, undone_by } => (
                "SKIP",
                format!("{record}: undone by {}", undone_by.join(", ")),
            ),
            ProjectionEvent::EffectsOnly { record, reason } => {
                ("EFFECTS_ONLY", format!("{record}: effects only, {reason}"))
            }
            ProjectionEvent::Rewound {
                record,
                cause,
                target,
            } => (
                "REWIND",
                format!(
                    "{record} {} {target}: play again from the start",
                    cause_word(*cause)
                ),
            ),
        };
        let mut child = PathNode::new(PathNodeType::Operation, name);
        child.message = Some(message);
        node.children.push(child);
    }
    node
}

/// Ids comma-separated, `-` for none.
pub fn show_ids(ids: &[String]) -> String {
    if ids.is_empty() {
        "-".to_string()
    } else {
        ids.join(", ")
    }
}

/// The word for a rewind's cause, as the trace shows it.
pub fn cause_word(cause: RewindCause) -> &'static str {
    match cause {
        RewindCause::Undoes => "undoes",
        RewindCause::Replaces => "replaces",
        RewindCause::Revives => "revives",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Article 1 grants (`verleend`), article 2 revokes `bestreden` when
    /// `herroepen`, article 3 replaces `bestreden`, article 4 requires the
    /// fact `ingeleverd`.
    const LAW: &str = r#"
$id: wet
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        produces:
          - {type: AANVRAAG, requests: 'wet#1'}
          - {type: BESCHIKKING, outputs: [verleend]}
        output: [{name: verleend, type: boolean}]
        actions: [{output: verleend, value: true}]
  - number: '2'
    text: t
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
            effects: [{undoes: bestreden, when: herroepen}]
        parameters: [{name: bestreden, type: string}]
        output: [{name: herroepen, type: boolean}]
        actions: [{output: herroepen, value: true}]
  - number: '3'
    text: t
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
            effects: [{replaces: bestreden}]
        parameters: [{name: bestreden, type: string}]
        output: [{name: verleend, type: boolean}]
        actions: [{output: verleend, value: false}]
  - number: '4'
    text: t
    machine_readable:
      requires:
        - fact: ingeleverd
          parameters: [op]
      execution:
        parameters: [{name: op, type: date}]
"#;

    /// Whether a grant or a replacement in force gives a card that was not
    /// handed in on a later day, and how many grants are in force.
    const REDUCTION: &str = r#"
$id: reductie
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      projection: {}
      execution:
        input:
          - {name: verleningen, type: array, source: {decisions: 'wet#1'}}
          - {name: vervangingen, type: array, source: {decisions: 'wet#3'}}
          - {name: inleveringen, type: array, source: {facts: 'wet#4#ingeleverd'}}
        output: [{name: heeft_kaart, type: boolean}, {name: aantal, type: number}]
        actions:
          - output: heeft_kaart
            value:
              operation: OR
              conditions:
                - operation: FOREACH
                  collection: $verleningen
                  as: g
                  filter: &na_inlevering
                    operation: AND
                    conditions:
                      - $g.verleend
                      # Recorded after the last card handed in: fewer days
                      # ago than the hand-in nearest to the calculation date.
                      - operation: OR
                        conditions:
                          - {operation: EQUALS, subject: &laatste {operation: FOREACH, collection: $inleveringen, as: i, body: {operation: DATE_DIFF, from: $i.recorded_on, to: $referencedate, in: days}, combine: MIN}, value: null}
                          - operation: LESS_THAN
                            subject: {operation: DATE_DIFF, from: $g.recorded_on, to: $referencedate, in: days}
                            value: *laatste
                  body: true
                  combine: OR
                - operation: FOREACH
                  collection: $vervangingen
                  as: g
                  filter: *na_inlevering
                  body: true
                  combine: OR
          - output: aantal
            value: {operation: FOREACH, collection: $verleningen, as: g, body: 1, combine: ADD}
"#;

    fn service() -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service.load_law(LAW).unwrap();
        service.load_law(REDUCTION).unwrap();
        service
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn record(
        id: &str,
        article: &str,
        fields: &[(&str, Value)],
        outputs: &[(&str, Value)],
    ) -> ChronicleRecord {
        ChronicleRecord {
            id: id.to_string(),
            kind: "BESCHIKKING".to_string(),
            basis: format!("wet#{article}"),
            fact: None,
            recorded_on: day("2025-03-01"),
            received_on: None,
            refers_to: None,
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            outputs: outputs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        }
    }

    fn grant(id: &str) -> ChronicleRecord {
        record(id, "1", &[], &[("verleend", Value::Bool(true))])
    }

    fn revoke(id: &str, target: &str, holds: bool) -> ChronicleRecord {
        record(
            id,
            "2",
            &[("bestreden", Value::String(target.to_string()))],
            &[("herroepen", Value::Bool(holds))],
        )
    }

    fn replace(id: &str, target: &str) -> ChronicleRecord {
        record(
            id,
            "3",
            &[("bestreden", Value::String(target.to_string()))],
            &[("verleend", Value::Bool(false))],
        )
    }

    /// A card handed in, recorded on `on`.
    fn handed_in(id: &str, on: &str) -> ChronicleRecord {
        ChronicleRecord {
            kind: "EXECUTOGRAM".to_string(),
            fact: Some("ingeleverd".to_string()),
            recorded_on: day(on),
            ..record(id, "4", &[("op", Value::String(on.to_string()))], &[])
        }
    }

    fn on(record: ChronicleRecord, date: &str) -> ChronicleRecord {
        ChronicleRecord {
            recorded_on: day(date),
            ..record
        }
    }

    fn play(records: &[ChronicleRecord]) -> Result<ProjectionOutcome> {
        play_on(records, "2025-03-31")
    }

    fn play_on(records: &[ChronicleRecord], date: &str) -> Result<ProjectionOutcome> {
        service().evaluate_projection(
            "reductie",
            &["heeft_kaart", "aantal"],
            records,
            BTreeMap::new(),
            date,
            false,
        )
    }

    fn skipped_in_last_replay(outcome: &ProjectionOutcome) -> Vec<&str> {
        let start = outcome
            .events
            .iter()
            .rposition(|e| matches!(e, ProjectionEvent::Rewound { .. }))
            .map_or(0, |i| i + 1);
        outcome.events[start..]
            .iter()
            .filter_map(|e| match e {
                ProjectionEvent::Skipped { record, .. } => Some(record.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_revoked_record_is_replayed_without_it() {
        let outcome = play(&[
            grant("a"),
            handed_in("i", "2025-03-02"),
            on(grant("b"), "2025-03-03"),
            on(revoke("h", "a", true), "2025-03-04"),
        ])
        .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(1));
        assert_eq!(outcome.in_force, ["i", "b", "h"]);
        assert_eq!(skipped_in_last_replay(&outcome), ["a"]);
        assert!(outcome.events.contains(&ProjectionEvent::Rewound {
            record: "h".to_string(),
            cause: RewindCause::Undoes,
            target: "a".to_string(),
        }));
    }

    #[test]
    fn an_effect_whose_outcome_is_false_does_not_rewind() {
        let outcome = play(&[grant("a"), revoke("h", "a", false)]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert!(!outcome
            .events
            .iter()
            .any(|e| matches!(e, ProjectionEvent::Rewound { .. })));
    }

    #[test]
    fn a_revoked_revocation_revives_its_target() {
        let outcome = play(&[
            grant("a"),
            revoke("h1", "a", true),
            revoke("h2", "h1", true),
        ])
        .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(skipped_in_last_replay(&outcome), ["h1"]);
        assert!(outcome.events.contains(&ProjectionEvent::Rewound {
            record: "h1".to_string(),
            cause: RewindCause::Revives,
            target: "a".to_string(),
        }));
    }

    #[test]
    fn a_record_stays_skipped_while_one_record_undoing_it_counts() {
        let outcome = play(&[
            grant("a"),
            revoke("h1", "a", true),
            revoke("h2", "a", true),
            revoke("h3", "h1", true),
        ])
        .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(skipped_in_last_replay(&outcome), ["a", "h1"]);
    }

    #[test]
    fn a_replacing_record_is_a_row_in_the_place_of_the_replaced_one() {
        let outcome = play(&[grant("a"), replace("n", "a")]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(0));
        assert_eq!(outcome.in_force, ["n"]);
        assert!(outcome.events.contains(&ProjectionEvent::Rewound {
            record: "n".to_string(),
            cause: RewindCause::Replaces,
            target: "a".to_string(),
        }));
    }

    #[test]
    fn a_replaced_replacement_undoes_nothing_any_more() {
        // n1 replaces a; n2 replaces n1, so n1 counts no more and neither
        // does its replacing of a: a counts again, beside n2.
        let outcome = play(&[grant("a"), replace("n1", "a"), replace("n2", "n1")]).unwrap();
        assert_eq!(skipped_in_last_replay(&outcome), ["n1"]);
        assert_eq!(outcome.in_force, ["a", "n2"]);
    }

    #[test]
    fn revoking_the_last_replacement_revives_the_one_it_replaced() {
        let outcome = play(&[
            grant("a"),
            replace("n1", "a"),
            replace("n2", "n1"),
            revoke("h", "n2", true),
        ])
        .unwrap();
        assert_eq!(skipped_in_last_replay(&outcome), ["a", "n2"]);
    }

    #[test]
    fn an_input_reads_the_records_in_force_of_its_article_kind_and_fact() {
        // A submission under article 1 is no decision row; a fact of another
        // name is no row of `ingeleverd`.
        let submission = ChronicleRecord {
            kind: "AANVRAAG".to_string(),
            ..record("s", "1", &[], &[])
        };
        let outcome = play(&[grant("a"), submission]).unwrap();
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(1));
        assert_eq!(outcome.in_force, ["a", "s"]);
        // A card handed in on a later day: no card any more.
        let outcome = play(&[grant("a"), handed_in("i", "2025-03-02")]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        // Handed in, then granted again on a later day.
        let outcome = play(&[
            grant("a"),
            handed_in("i", "2025-03-02"),
            on(grant("b"), "2025-03-03"),
        ])
        .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
    }

    #[test]
    fn a_row_holds_the_record_and_what_it_holds() {
        let mut decision = grant("a");
        decision.refers_to = Some("s".to_string());
        decision
            .fields
            .insert("wie".to_string(), Value::String("x".to_string()));
        let service = service();
        let law = service
            .resolver()
            .get_law_for_date("reductie", Some(day("2025-03-01")))
            .unwrap();
        let rows = InForce {
            records: vec![&decision],
        }
        .rows(
            &service,
            law,
            ChronicleRead::Decisions("wet#1"),
            "2025-03-01",
        )
        .unwrap()
        .unwrap();
        let expected = Value::Object(BTreeMap::from([
            ("wie".to_string(), Value::String("x".into())),
            ("verleend".to_string(), Value::Bool(true)),
            ("id".to_string(), Value::String("a".into())),
            ("basis".to_string(), Value::String("wet#1".into())),
            ("type".to_string(), Value::String("BESCHIKKING".into())),
            (
                "recorded_on".to_string(),
                Value::String("2025-03-01".into()),
            ),
            ("refers_to".to_string(), Value::String("s".into())),
        ]));
        assert_eq!(rows, Value::Array(vec![expected]));
    }

    #[test]
    fn a_fact_input_reads_the_executograms_of_its_article_and_fact() {
        let service = service();
        let law = service
            .resolver()
            .get_law_for_date("reductie", Some(day("2025-03-01")))
            .unwrap();
        let ingeleverd = handed_in("i", "2025-03-01");
        let other_fact = ChronicleRecord {
            fact: Some("anders".to_string()),
            ..handed_in("x", "2025-03-01")
        };
        let other_article = ChronicleRecord {
            basis: "wet#1".to_string(),
            ..handed_in("y", "2025-03-01")
        };
        let no_executogram = ChronicleRecord {
            kind: "BESCHIKKING".to_string(),
            ..handed_in("z", "2025-03-01")
        };
        let in_force = InForce {
            records: vec![&ingeleverd, &other_fact, &other_article, &no_executogram],
        };
        assert_eq!(in_force.keeper(), "the records in force");
        let rows = in_force
            .rows(
                &service,
                law,
                ChronicleRead::Facts("wet#4#ingeleverd"),
                "2025-03-01",
            )
            .unwrap()
            .unwrap();
        let Value::Array(rows) = rows else {
            panic!("{rows:?}")
        };
        let ids: Vec<&Value> = rows
            .iter()
            .map(|r| match r {
                Value::Object(row) => &row["id"],
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(ids, [&Value::String("i".into())]);
    }

    #[test]
    fn an_effect_needs_an_earlier_record_and_a_boolean_outcome() {
        for (records, why) in [
            (
                vec![revoke("h", "a", true), grant("a")],
                "no record recorded before it",
            ),
            (
                vec![
                    grant("a"),
                    record("h", "2", &[("bestreden", Value::String("a".into()))], &[]),
                ],
                "holds no boolean under it",
            ),
            (
                vec![
                    grant("a"),
                    record("h", "2", &[], &[("herroepen", Value::Bool(true))]),
                ],
                "holds no record id under it",
            ),
        ] {
            let err = play(&records).expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
    }

    #[test]
    fn a_long_chain_of_undoings_settles() {
        // A grant, then eight revocations, each revoking the one before:
        // the grant counts when the number of revocations is even.
        let mut records = vec![grant("a")];
        let mut previous = "a".to_string();
        for k in 1..=8 {
            let id = format!("h{k}");
            records.push(revoke(&id, &previous, true));
            previous = id;
        }
        let outcome = play(&records).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(skipped_in_last_replay(&outcome), ["h1", "h3", "h5", "h7"]);
    }

    #[test]
    fn a_skipped_record_names_only_the_undoers_that_count() {
        let outcome = play(&[
            grant("a"),
            revoke("h1", "a", true),
            revoke("h2", "a", true),
            revoke("h3", "h1", true),
        ])
        .unwrap();
        let start = outcome
            .events
            .iter()
            .rposition(|e| matches!(e, ProjectionEvent::Rewound { .. }))
            .map_or(0, |i| i + 1);
        assert_eq!(
            outcome.events[start],
            ProjectionEvent::Skipped {
                record: "a".to_string(),
                undone_by: vec!["h2".to_string()],
            }
        );
    }

    #[test]
    fn a_record_whose_article_cannot_be_found_is_refused() {
        let unknown = record("x", "9", &[], &[]);
        let err = play(&[grant("a"), unknown]).expect_err("no article 9");
        assert!(
            err.to_string().contains("names no article in force"),
            "{err}"
        );
    }

    #[test]
    fn one_version_of_the_reduction_answers_for_the_whole_chronicle() {
        // From 2025-06-01 the reduction counts no grants any more. The
        // version in force on the calculation date answers.
        let mut service = service();
        service
            .load_law(
                &REDUCTION
                    .replace("valid_from: '2025-01-01'", "valid_from: '2025-06-01'")
                    .replace(
                        "value: {operation: FOREACH, collection: $verleningen, as: g, body: 1, combine: ADD}",
                        "value: 0",
                    ),
            )
            .unwrap();
        let later = on(grant("b"), "2025-07-01");
        let aantal_on = |date: &str| {
            service
                .evaluate_projection(
                    "reductie",
                    &["aantal"],
                    &[grant("a"), later.clone()],
                    BTreeMap::new(),
                    date,
                    false,
                )
                .unwrap()
                .result
                .outputs["aantal"]
                .clone()
        };
        assert_eq!(aantal_on("2025-08-01"), Value::Int(0));
        // Before the second version, and before `b` was recorded: only `a`.
        assert_eq!(aantal_on("2025-05-01"), Value::Int(1));
    }

    #[test]
    fn a_record_recorded_after_the_calculation_date_is_not_played() {
        let later = on(revoke("h", "a", true), "2025-04-01");
        let outcome = play_on(&[grant("a"), later], "2025-03-31").unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(outcome.events.len(), 1);
    }

    #[test]
    fn a_record_of_a_kind_its_article_does_not_produce_is_refused() {
        let toets = ChronicleRecord {
            kind: "TOETS".to_string(),
            ..grant("t")
        };
        let err = play(&[toets]).expect_err("article 1 takes no TOETS");
        assert!(err.to_string().contains("does not produce"), "{err}");
    }

    /// Whether each record counts does not depend on what the reduction
    /// computes, so one pass from the last record back gives it: a record
    /// counts unless a record after it that counts has an effect on it that
    /// holds. Rewinding must arrive at the same records in force, over many
    /// generated chronicles.
    #[test]
    fn rewinding_counts_the_same_records_as_one_pass_back() {
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |bound: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % bound as u64) as usize
        };
        for _ in 0..500 {
            let n = 1 + next(12);
            let mut records = Vec::new();
            // (target, holds) per record; None for a grant.
            let mut effect_of: Vec<Option<(usize, bool)>> = Vec::new();
            for i in 0..n {
                let id = format!("r{i}");
                if i == 0 || next(3) == 0 {
                    records.push(grant(&id));
                    effect_of.push(None);
                    continue;
                }
                let target = next(i);
                let target_id = format!("r{target}");
                if next(2) == 0 {
                    let holds = next(4) != 0;
                    records.push(revoke(&id, &target_id, holds));
                    effect_of.push(Some((target, holds)));
                } else {
                    records.push(replace(&id, &target_id));
                    effect_of.push(Some((target, true)));
                }
            }
            let mut counts = vec![true; n];
            for i in (0..n).rev() {
                counts[i] = !(i + 1..n).any(|j| counts[j] && effect_of[j] == Some((i, true)));
            }
            let skipped: Vec<String> = (0..n)
                .filter(|&i| !counts[i])
                .map(|i| format!("r{i}"))
                .collect();
            let in_force: Vec<String> = (0..n)
                .filter(|&i| counts[i])
                .map(|i| format!("r{i}"))
                .collect();
            let outcome = play(&records).unwrap();
            assert_eq!(skipped_in_last_replay(&outcome), skipped, "{records:#?}");
            assert_eq!(outcome.in_force, in_force, "{records:#?}");
        }
    }

    #[test]
    fn articles_of_one_reduction_may_read_each_other_while_it_plays() {
        // Article 1 (the projection) reads `geldig` of article 2, which reads
        // `aantal` of article 1: the play is passed on, not refused.
        let looped = REDUCTION
            .replace("$id: reductie", "$id: reductie_lus")
            .replace(
                "          - {name: inleveringen, type: array, source: {facts: 'wet#4#ingeleverd'}}\n",
                "          - {name: inleveringen, type: array, source: {facts: 'wet#4#ingeleverd'}}\n          - {name: geldig, type: boolean, source: {output: geldig}}\n",
            )
            + "  - number: '2'\n    text: t\n    machine_readable:\n      execution:\n        input:\n          - {name: n, type: number, source: {output: aantal}}\n        output: [{name: geldig, type: boolean}]\n        actions:\n          - output: geldig\n            value: {operation: GREATER_THAN, subject: $n, value: 0}\n";
        let looped = looped.replace(
            "          - output: aantal\n",
            "          - output: aantal_geldig\n            value: $geldig\n          - output: aantal\n",
        ).replace(
            "output: [{name: heeft_kaart, type: boolean}, {name: aantal, type: number}]",
            "output: [{name: heeft_kaart, type: boolean}, {name: aantal, type: number}, {name: aantal_geldig, type: boolean}]",
        );
        let mut service = service();
        service.load_law(&looped).unwrap();
        let outcome = service
            .evaluate_projection(
                "reductie_lus",
                &["aantal_geldig"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .unwrap();
        assert_eq!(outcome.result.outputs["aantal_geldig"], Value::Bool(true));
    }

    #[test]
    fn a_reduction_with_a_projection_is_not_evaluated_directly() {
        let mut service = service();
        // Another reduction that takes the answer of this one as an input.
        service
            .load_law(
                r#"
$id: andere_reductie
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        input:
          - name: kaart
            type: boolean
            source: {regulation: reductie, output: heeft_kaart}
        output: [{name: x, type: boolean}]
        actions: [{output: x, value: $kaart}]
"#,
            )
            .unwrap();
        for (law, output) in [("reductie", "heeft_kaart"), ("andere_reductie", "x")] {
            let err = service
                .evaluate_law(law, &[output], BTreeMap::new(), "2025-03-01")
                .expect_err("not played");
            assert!(
                err.to_string().contains("with evaluate_projection"),
                "{law}: {err}"
            );
        }
    }

    #[test]
    fn an_empty_chronicle_has_no_records_in_force() {
        let outcome = play(&[]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(0));
        assert!(outcome.events.is_empty());
        assert!(outcome.in_force.is_empty());
    }

    #[test]
    fn the_trace_names_each_cause_of_a_rewind() {
        let messages = |records: &[ChronicleRecord]| -> Vec<String> {
            let outcome = service()
                .evaluate_projection(
                    "reductie",
                    &["heeft_kaart"],
                    records,
                    BTreeMap::new(),
                    "2025-03-01",
                    true,
                )
                .unwrap();
            let root = outcome.result.trace.expect("a trace");
            root.children[0]
                .children
                .iter()
                .filter(|n| n.name == "REWIND")
                .filter_map(|n| n.message.clone())
                .collect()
        };
        assert_eq!(
            messages(&[grant("a"), replace("n", "a")]),
            ["n replaces a: play again from the start"]
        );
        assert_eq!(
            messages(&[
                grant("a"),
                revoke("h1", "a", true),
                revoke("h2", "h1", true)
            ]),
            [
                "h1 undoes a: play again from the start",
                "h2 undoes h1: play again from the start",
                "h1 revives a: play again from the start",
            ]
        );
    }

    /// The reduction, playing only the records of one `wie`.
    fn keyed() -> LawExecutionService {
        let mut service = service();
        service
            .load_law(
                &REDUCTION
                    .replace("$id: reductie", "$id: reductie_per_wie")
                    .replace("      projection: {}", "      projection: {key: wie}")
                    .replace(
                        "      execution:\n        input:",
                        "      execution:\n        parameters: [{name: wie, type: string}]\n        input:",
                    ),
            )
            .unwrap();
        service
    }

    fn of(record: ChronicleRecord, wie: &str) -> ChronicleRecord {
        let mut record = record;
        record
            .fields
            .insert("wie".to_string(), Value::String(wie.to_string()));
        record
    }

    fn play_for(wie: &str, records: &[ChronicleRecord]) -> Result<ProjectionOutcome> {
        keyed().evaluate_projection(
            "reductie_per_wie",
            &["heeft_kaart", "aantal"],
            records,
            BTreeMap::from([("wie".to_string(), Value::String(wie.to_string()))]),
            "2025-03-31",
            false,
        )
    }

    /// The records the last replay applied, skipped or counted for their
    /// effects only, in order.
    fn records_in(outcome: &ProjectionOutcome) -> Vec<&str> {
        let start = outcome
            .events
            .iter()
            .rposition(|e| matches!(e, ProjectionEvent::Rewound { .. }))
            .map_or(0, |i| i + 1);
        outcome.events[start..]
            .iter()
            .filter_map(|e| match e {
                ProjectionEvent::Applied { record }
                | ProjectionEvent::Skipped { record, .. }
                | ProjectionEvent::EffectsOnly { record, .. } => Some(record.as_str()),
                ProjectionEvent::Rewound { .. } => None,
            })
            .collect()
    }

    #[test]
    fn a_key_plays_only_the_records_of_one_and_what_refers_to_them() {
        let records = [
            of(grant("a"), "x"),
            of(grant("b"), "y"),
            // Refers through `refers_to` and through an effect's field.
            ChronicleRecord {
                refers_to: Some("a".to_string()),
                kind: "AANVRAAG".to_string(),
                ..record("s", "1", &[], &[])
            },
            revoke("h", "a", true),
            revoke("h2", "h", true),
        ];
        let x = play_for("x", &records).unwrap();
        assert_eq!(records_in(&x), ["a", "s", "h", "h2"]);
        assert_eq!(x.result.outputs["heeft_kaart"], Value::Bool(true));
        let y = play_for("y", &records).unwrap();
        assert_eq!(records_in(&y), ["b"]);
    }

    #[test]
    fn a_record_with_another_key_that_refers_in_counts_for_its_effects_only() {
        // `n` replaces x's grant and carries y's key: for x it replaces the
        // grant all the same, but it is no row of x's reduction.
        let records = [of(grant("a"), "x"), of(replace("n", "a"), "y")];
        let x = play_for("x", &records).unwrap();
        assert!(x.events.contains(&ProjectionEvent::EffectsOnly {
            record: "n".to_string(),
            reason: "key differs: wie".to_string(),
        }));
        assert!(x.events.contains(&ProjectionEvent::Rewound {
            record: "n".to_string(),
            cause: RewindCause::Replaces,
            target: "a".to_string(),
        }));
        assert_eq!(skipped_in_last_replay(&x), ["a"]);
        assert!(x.in_force.is_empty());
        assert_eq!(x.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(x.result.outputs["aantal"], Value::Int(0));
        // For y, `n` is its own record, and replaces a record outside y's
        // partition: that effect is not y's business.
        let y = play_for("y", &records).unwrap();
        assert_eq!(records_in(&y), ["n"]);
        assert_eq!(y.in_force, ["n"]);
        assert!(!y
            .events
            .iter()
            .any(|e| matches!(e, ProjectionEvent::Rewound { .. })));
    }

    #[test]
    fn undoing_a_record_of_someone_else_revives_what_it_replaced() {
        // `n` (y) replaces x's grant; `h` revokes `n` and carries no key. It
        // refers only to a record of someone else, so it counts for x for its
        // effects only, and x's grant counts again.
        let records = [
            of(grant("a"), "x"),
            of(replace("n", "a"), "y"),
            revoke("h", "n", true),
        ];
        let x = play_for("x", &records).unwrap();
        assert_eq!(records_in(&x), ["a", "n", "h"]);
        assert!(x.events.contains(&ProjectionEvent::EffectsOnly {
            record: "h".to_string(),
            reason: "refers to n".to_string(),
        }));
        assert_eq!(x.in_force, ["a"]);
        assert_eq!(x.result.outputs["heeft_kaart"], Value::Bool(true));
        let y = play_for("y", &records).unwrap();
        assert_eq!(records_in(&y), ["n", "h"]);
        assert_eq!(y.in_force, ["h"]);
    }

    #[test]
    fn a_record_that_refers_to_one_of_its_own_records_is_its_own() {
        // `h` revokes `n` (y's) and also names x's grant `a` by `refers_to`:
        // it refers to a record of x, so it is a row of x's reduction.
        let mut h = revoke("h", "n", true);
        h.refers_to = Some("a".to_string());
        let records = [of(grant("a"), "x"), of(replace("n", "a"), "y"), h];
        let x = play_for("x", &records).unwrap();
        assert!(x.in_force.contains(&"h".to_string()));
        assert!(!x
            .events
            .iter()
            .any(|e| matches!(e, ProjectionEvent::EffectsOnly { record, .. } if record == "h")));
    }

    #[test]
    fn a_null_key_is_no_key() {
        let mut nameless = replace("n", "a");
        nameless.fields.insert("wie".to_string(), Value::Null);
        let x = play_for("x", &[of(grant("a"), "x"), nameless]).unwrap();
        assert_eq!(records_in(&x), ["a", "n"]);
        assert_eq!(x.in_force, ["n"]);
    }

    #[test]
    fn a_key_of_another_type_is_refused() {
        let err = keyed()
            .evaluate_projection(
                "reductie_per_wie",
                &["heeft_kaart"],
                &[],
                BTreeMap::from([("wie".to_string(), Value::Int(1))]),
                "2025-03-01",
                false,
            )
            .expect_err("a number for a string key");
        assert!(err.to_string().contains("which is no String"), "{err}");
    }

    #[test]
    fn a_record_without_a_key_plays_only_when_it_refers_in() {
        // A card handed in holds no `wie` and refers to nothing: it never
        // joins a partition.
        let outcome = play_for("x", &[of(grant("a"), "x"), handed_in("i", "2025-03-02")]).unwrap();
        assert_eq!(records_in(&outcome), ["a"]);
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
    }

    #[test]
    fn a_key_of_type_date_takes_a_date() {
        let mut service = service();
        service
            .load_law(
                &REDUCTION
                    .replace("$id: reductie", "$id: reductie_per_dag")
                    .replace("      projection: {}", "      projection: {key: dag}")
                    .replace(
                        "      execution:\n        input:",
                        "      execution:\n        parameters: [{name: dag, type: date}]\n        input:",
                    ),
            )
            .unwrap();
        let ask = |dag: &str| {
            service.evaluate_projection(
                "reductie_per_dag",
                &["heeft_kaart"],
                &[],
                BTreeMap::from([("dag".to_string(), Value::String(dag.to_string()))]),
                "2025-03-01",
                false,
            )
        };
        assert!(ask("2025-03-01").is_ok());
        let err = ask("morgen").expect_err("no date");
        assert!(err.to_string().contains("which is no Date"), "{err}");
    }

    #[test]
    fn a_keyed_projection_needs_the_key() {
        let err = keyed()
            .evaluate_projection(
                "reductie_per_wie",
                &["heeft_kaart"],
                &[],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .expect_err("no key passed");
        assert!(err.to_string().contains("but none is passed"), "{err}");
    }

    #[test]
    fn a_reduction_without_a_projection_is_refused() {
        let err = service()
            .evaluate_projection(
                "wet",
                &["verleend"],
                &[],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .expect_err("no projection");
        assert!(err.to_string().contains("has no projection"), "{err}");
    }

    #[test]
    fn the_trace_holds_a_node_per_event_and_the_records_in_force() {
        let outcome = service()
            .evaluate_projection(
                "reductie",
                &["heeft_kaart"],
                &[grant("a"), revoke("h", "a", true)],
                BTreeMap::new(),
                "2025-03-01",
                true,
            )
            .unwrap();
        let root = outcome.result.trace.expect("a trace");
        let node = &root.children[0];
        assert_eq!(node.name, "PROJECTION");
        assert_eq!(node.message.as_deref(), Some("in force: h"));
        let names: Vec<&str> = node.children.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["APPLY", "REWIND", "SKIP", "APPLY"]);
        assert_eq!(node.children[0].message.as_deref(), Some("a"));
    }

    /// A law for the schema v0.8.15 tests: article 1 grants for a period
    /// (`van`, `tot`), article 2 suspends `besluit` from `vanaf` until `tot`
    /// (a window on an undoing), article 3 is an application that a later
    /// application replaces, and article 4 requires a fact that a
    /// correction replaces.
    const LAW_V15: &str = r#"
$id: wet15
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        produces:
          - {type: BESCHIKKING, outputs: [verleend]}
        parameters:
          - {name: van, type: date}
          - {name: tot, type: date}
        output: [{name: verleend, type: boolean}]
        actions: [{output: verleend, value: true}]
  - number: '2'
    text: t
    machine_readable:
      execution:
        produces:
          - type: UITSPRAAK
            effects: [{undoes: besluit, from: vanaf, until: tot}]
        parameters:
          - {name: besluit, type: string}
          - {name: vanaf, type: date}
          - {name: tot, type: date}
        output: [{name: geschorst, type: boolean}]
        actions: [{output: geschorst, value: true}]
  - number: '3'
    text: t
    machine_readable:
      execution:
        produces:
          - type: AANVRAAG
            requests: 'wet15#1'
            parameters: [eerdere]
            effects: [{replaces: eerdere}]
          - {type: BESCHIKKING, outputs: [x]}
        parameters: [{name: eerdere, type: string}]
        output: [{name: x, type: boolean}]
        actions: [{output: x, value: true}]
  - number: '4'
    text: t
    machine_readable:
      requires:
        - {fact: opgave, parameters: [bedrag]}
        - fact: correctie
          parameters: [onjuist, bedrag]
          effects: [{replaces: onjuist}]
      execution:
        parameters:
          - {name: bedrag, type: number}
          - {name: onjuist, type: string}
"#;

    /// Reads the grants, the applications and the stated amounts in force;
    /// answers whether a grant holds on the calculation date.
    const REDUCTION_V15: &str = r#"
$id: reductie15
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      projection: {}
      execution:
        input:
          - {name: verleningen, type: array, source: {decisions: 'wet15#1'}}
          - {name: aanvragen, type: array, source: {submissions: 'wet15#3'}}
          - {name: opgaven, type: array, source: {facts: 'wet15#4#opgave'}}
          - {name: correcties, type: array, source: {facts: 'wet15#4#correctie'}}
        output:
          - {name: geldig, type: boolean}
          - {name: aantal_aanvragen, type: number}
          - {name: som, type: number}
        actions:
          - output: geldig
            value:
              operation: FOREACH
              collection: $verleningen
              as: v
              filter:
                operation: AND
                conditions:
                  - {operation: LESS_THAN_OR_EQUAL, subject: $v.van, value: $referencedate}
                  - {operation: LESS_THAN, subject: $referencedate, value: $v.tot}
              body: true
              combine: OR
          - output: aantal_aanvragen
            value: {operation: FOREACH, collection: $aanvragen, as: a, body: 1, combine: ADD}
          - output: som
            value:
              operation: ADD
              values:
                - {operation: FOREACH, collection: $opgaven, as: o, body: $o.bedrag, combine: ADD}
                - {operation: FOREACH, collection: $correcties, as: c, body: $c.bedrag, combine: ADD}
"#;

    fn service_v15() -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service.load_law(LAW_V15).unwrap();
        service.load_law(REDUCTION_V15).unwrap();
        service
    }

    fn record15(
        id: &str,
        kind: &str,
        article: &str,
        on: &str,
        fields: &[(&str, &str)],
    ) -> ChronicleRecord {
        ChronicleRecord {
            id: id.to_string(),
            kind: kind.to_string(),
            basis: format!("wet15#{article}"),
            fact: None,
            recorded_on: day(on),
            received_on: None,
            refers_to: None,
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
                .collect(),
            outputs: BTreeMap::new(),
        }
    }

    fn grant15(id: &str, on: &str, van: &str, tot: &str) -> ChronicleRecord {
        ChronicleRecord {
            outputs: BTreeMap::from([("verleend".to_string(), Value::Bool(true))]),
            ..record15(id, "BESCHIKKING", "1", on, &[("van", van), ("tot", tot)])
        }
    }

    fn suspension(id: &str, on: &str, besluit: &str, vanaf: &str, tot: &str) -> ChronicleRecord {
        ChronicleRecord {
            outputs: BTreeMap::from([("geschorst".to_string(), Value::Bool(true))]),
            ..record15(
                id,
                "UITSPRAAK",
                "2",
                on,
                &[("besluit", besluit), ("vanaf", vanaf), ("tot", tot)],
            )
        }
    }

    fn ask15(
        output: &str,
        records: &[ChronicleRecord],
        valid_on: &str,
        known_on: Option<&str>,
    ) -> Result<ProjectionOutcome> {
        service_v15().evaluate_projection_with(
            "reductie15",
            &[output],
            records,
            BTreeMap::new(),
            valid_on,
            ProjectionOptions {
                known_on,
                ..ProjectionOptions::default()
            },
        )
    }

    #[test]
    fn an_answer_is_about_one_day_and_known_on_another() {
        // Granted on 1 March from 8 March; suspended for ever by a ruling
        // recorded on 1 April.
        let records = [
            grant15("a", "2025-03-01", "2025-03-08", "2026-03-08"),
            suspension("s", "2025-04-01", "a", "2025-01-01", "2030-01-01"),
        ];
        let geldig = |valid_on, known_on| {
            ask15("geldig", &records, valid_on, known_on)
                .unwrap()
                .result
                .outputs["geldig"]
                .clone()
        };
        // About 3 March, as known on 1 March: not yet in force.
        assert_eq!(geldig("2025-03-03", Some("2025-03-01")), Value::Bool(false));
        // About 10 March, as known then and as known now.
        assert_eq!(geldig("2025-03-10", Some("2025-03-10")), Value::Bool(true));
        assert_eq!(geldig("2025-03-10", Some("2025-04-02")), Value::Bool(false));
        // Without `known_on`, the calculation date.
        assert_eq!(geldig("2025-03-10", None), Value::Bool(true));
    }

    #[test]
    fn an_effect_with_a_window_holds_only_for_a_day_in_it() {
        let records = [
            grant15("a", "2025-03-01", "2025-03-01", "2026-03-01"),
            suspension("s", "2025-03-01", "a", "2025-04-01", "2025-06-01"),
        ];
        let geldig = |valid_on| {
            ask15("geldig", &records, valid_on, Some("2025-03-01"))
                .unwrap()
                .result
                .outputs["geldig"]
                .clone()
        };
        assert_eq!(geldig("2025-03-31"), Value::Bool(true));
        assert_eq!(geldig("2025-04-01"), Value::Bool(false));
        assert_eq!(geldig("2025-05-31"), Value::Bool(false));
        // `until` is the first day it no longer holds.
        assert_eq!(geldig("2025-06-01"), Value::Bool(true));
        // A null bound sets no bound.
        let mut open = records.clone();
        open[1].fields.insert("tot".to_string(), Value::Null);
        let outcome = ask15("geldig", &open, "2025-12-01", Some("2025-03-01")).unwrap();
        assert_eq!(outcome.result.outputs["geldig"], Value::Bool(false));
        // A bound the record does not hold is refused.
        let mut missing = records.clone();
        missing[1].fields.remove("vanaf");
        let err =
            ask15("geldig", &missing, "2025-04-01", Some("2025-03-01")).expect_err("no vanaf");
        assert!(err.to_string().contains("holds no value under it"), "{err}");
        // A bound that is no date is refused.
        let mut odd = records.clone();
        odd[1]
            .fields
            .insert("vanaf".to_string(), Value::String("morgen".to_string()));
        let err = ask15("geldig", &odd, "2025-04-01", Some("2025-03-01")).expect_err("no date");
        assert!(err.to_string().contains("which is no date"), "{err}");
    }

    #[test]
    fn a_submission_and_a_fact_undo_what_they_name() {
        // A later application replaces an earlier one: one row is left. The
        // first names no earlier one: null names no record.
        let mut first = record15("s1", "AANVRAAG", "3", "2025-03-01", &[]);
        first.fields.insert("eerdere".to_string(), Value::Null);
        let second = record15("s2", "AANVRAAG", "3", "2025-03-01", &[("eerdere", "s1")]);
        let outcome = ask15(
            "aantal_aanvragen",
            &[first.clone(), second],
            "2025-03-01",
            None,
        )
        .unwrap();
        assert_eq!(outcome.result.outputs["aantal_aanvragen"], Value::Int(1));
        assert_eq!(outcome.in_force, ["s2"]);
        // A field that is not there at all is refused: the record cannot
        // say what it undoes.
        first.fields.clear();
        let err = ask15("aantal_aanvragen", &[first], "2025-03-01", None).expect_err("no field");
        assert!(err.to_string().contains("holds no record id"), "{err}");

        // A correction replaces a wrong fact: the sum counts it once.
        let fact = |id: &str, name: &str, fields: &[(&str, Value)]| ChronicleRecord {
            fact: Some(name.to_string()),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            ..record15(id, "EXECUTOGRAM", "4", "2025-03-01", &[])
        };
        let records = [
            fact("o", "opgave", &[("bedrag", Value::Int(300))]),
            fact("p", "opgave", &[("bedrag", Value::Int(100))]),
            fact(
                "c",
                "correctie",
                &[
                    ("onjuist", Value::String("o".into())),
                    ("bedrag", Value::Int(250)),
                ],
            ),
        ];
        let outcome = ask15("som", &records, "2025-03-01", None).unwrap();
        assert_eq!(outcome.result.outputs["som"], Value::Int(350));
        assert!(outcome.events.contains(&ProjectionEvent::Rewound {
            record: "c".to_string(),
            cause: RewindCause::Replaces,
            target: "o".to_string(),
        }));
        // An executogram of a fact its article does not require is refused.
        let err = ask15("som", &[fact("x", "iets_anders", &[])], "2025-03-01", None)
            .expect_err("no such fact");
        assert!(err.to_string().contains("does not require"), "{err}");
    }

    #[test]
    fn a_record_of_another_cell_is_known_from_the_day_it_was_received() {
        // Recorded by a court on 1 April, received on 10 April: on 5 April
        // it is not known, and its effects come from the law of 1 April.
        let ruling = ChronicleRecord {
            received_on: Some(day("2025-04-10")),
            ..suspension("rb/s", "2025-04-01", "a", "2025-01-01", "2030-01-01")
        };
        let records = [
            grant15("a", "2025-03-01", "2025-03-01", "2026-03-01"),
            ruling,
        ];
        // From 5 April article 2 has no effect any more: the court's day
        // decides, not the day of the receipt.
        let mut service = service_v15();
        service
            .load_law(
                &LAW_V15
                    .replace("valid_from: '2025-01-01'", "valid_from: '2025-04-05'")
                    .replace(
                        "            effects: [{undoes: besluit, from: vanaf, until: tot}]\n",
                        "",
                    ),
            )
            .unwrap();
        let geldig = |known_on| {
            service
                .evaluate_projection_with(
                    "reductie15",
                    &["geldig"],
                    &records,
                    BTreeMap::new(),
                    "2025-04-05",
                    ProjectionOptions {
                        known_on: Some(known_on),
                        ..ProjectionOptions::default()
                    },
                )
                .unwrap()
                .result
                .outputs["geldig"]
                .clone()
        };
        assert_eq!(geldig("2025-04-05"), Value::Bool(true));
        assert_eq!(geldig("2025-04-10"), Value::Bool(false));
    }
}
