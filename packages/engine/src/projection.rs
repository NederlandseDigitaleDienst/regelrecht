//! A reduction that plays a cell's chronicle forward (schema v0.8.14).
//!
//! The reduction declares a state (`projection.state`), each property with
//! its type and starting from that type's start value, and, per article a
//! record rests on, the step for such a record (`projection.steps`). A
//! step applies an article of a law, which says what the record does with
//! the state, and only wires: each parameter of the article comes from the
//! record or the state, and each property it sets from an output. The
//! engine plays the records the cell knew on the calculation date in the
//! order the cell recorded them. Each step is pure: the applied article
//! reads nothing but what the step wires in, so the next state follows from
//! the state before and the record alone. The applied article runs in the
//! version in force on the day of the record; one version of the reduction,
//! the one in force on the calculation date, gives the state, every step
//! and the answer: the reduction is the cell's reading, not law. The
//! reduction's `execution` answers from the last state, reading each
//! property as `$state.<name>`.
//!
//! What a record does with an earlier record is law, not part of the
//! reduction: the article of a decision says it revokes another decision
//! (`produces[].effects`, optionally only `when` an outcome is true),
//! under the version of that law in force on the day the record was
//! recorded. The engine carries that out by rewinding. When an effect of a
//! record that counts undoes an earlier record that was applied, the engine
//! plays the chronicle again from the start and skips the undone record. A
//! record is skipped for as long as at least one record that undoes it
//! counts. A record that is itself revoked undoes nothing any more, so a
//! record it alone undid counts again and the engine rewinds once more. A
//! decision in the place of another revokes it and counts as a record of
//! its own; one that should keep an undoing in force declares that effect
//! itself. The chronicle is never changed. Nothing is reverted
//! property by property: a property a later record also set keeps the later
//! value, because the replay computes it again. An undone record is gone
//! from the whole history (ex tunc); a change from a later day on is a step
//! with a date property.
//!
//! The engine records, per property, which record set it last, and every
//! step it takes: each record applied (with the properties it set), each
//! record skipped (with who undid it), and each rewind (with what caused it).
//! The events come back with the answer and, when tracing, as a `PROJECTION`
//! node in the engine's trace.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_law_model::{
    Article, ArticleBasedLaw, Projection, ProjectionStep, StateProperty, StepSource,
};
use serde::{Deserialize, Serialize};

use crate::answer::Call;
use crate::engine::{ArticleEngine, ArticleResult};
use crate::error::{EngineError, Result};
use crate::service::LawExecutionService;
use crate::trace::{PathNode, TraceBuilder};
use crate::types::{ParameterType, PathNodeType, Value};

/// The kind of a record of a fact (RFC-051), as the cell writes it.
const EXECUTOGRAM: &str = "EXECUTOGRAM";

/// The name under which a step and the answer read the state.
pub const STATE: &str = "state";

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
    pub recorded_on: NaiveDate,
    /// The record it refers to, such as the submission a decision was taken
    /// on, or the decision an objection contests.
    pub refers_to: Option<String>,
    pub fields: BTreeMap<String, Value>,
    pub outputs: BTreeMap<String, Value>,
}

/// What caused a rewind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewindCause {
    /// A record revokes an earlier one that was applied.
    Revokes,
    /// A record that undid an earlier one is itself revoked now, and no
    /// other record undoes the earlier one any more: it counts again.
    Revives,
}

/// One property a record set: its value before and after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateChange {
    pub property: String,
    pub from: Value,
    pub to: Value,
}

/// One step of playing the chronicle, in the order the engine took them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProjectionEvent {
    /// The record was played: the article its step applied and the
    /// properties it set, each with its value before and after (no article
    /// and no changes when the reduction gives no step for it).
    Applied {
        record: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        article: Option<String>,
        changes: Vec<StateChange>,
    },
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
    /// engine rewind, while its step leaves the state as it is. `reason`
    /// says why it is someone else's: its own key differs (`key differs:
    /// <field>`), or it refers only to such records (`refers to <id>`).
    EffectsOnly { record: String, reason: String },
}

/// The answer of a projection: the last state, which record set each
/// property last (`None` for the initial value), every event, and the
/// result of the reduction's `execution` over the last state.
#[derive(Debug, Clone)]
pub struct ProjectionOutcome {
    pub state: BTreeMap<String, Value>,
    pub set_by: BTreeMap<String, Option<String>>,
    pub events: Vec<ProjectionEvent>,
    pub result: ArticleResult,
}

impl LawExecutionService {
    /// Play `records`, in their order, through the projection of the
    /// reduction `law_id` that gives `output_names`, and evaluate those
    /// outputs from the last state, with `parameters` beside `state`.
    ///
    /// Refuses a reduction article without a projection, an effect on a
    /// record that is not an earlier record, an effect whose `when` outcome
    /// the record does not hold as a boolean, a step that fails, and a replay
    /// that does not settle.
    pub fn evaluate_projection(
        &self,
        law_id: &str,
        output_names: &[&str],
        records: &[ChronicleRecord],
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: bool,
    ) -> Result<ProjectionOutcome> {
        let day = parse_day(calculation_date)?;
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

        // The reduction reads its state as `$state`; a caller cannot pass
        // another one (the loader refuses a declared parameter of that name).
        if parameters.contains_key(STATE) {
            return Err(EngineError::InvalidOperation(format!(
                "a projection gives '{STATE}' itself; it cannot be passed to '{law_id}'"
            )));
        }
        // What the cell recorded after the calculation date is not known on
        // it, so it is not played.
        let known: Vec<ChronicleRecord> = records
            .iter()
            .filter(|r| r.recorded_on <= day)
            .cloned()
            .collect();
        self.check_steps(law_id, &projection, day)?;
        let partition = self.partition(&projection, key_type.as_ref(), &known, &parameters)?;
        let played = self.play(law_id, &projection, &known, &partition)?;

        let state = Some(Value::Object(played.state.clone()));
        let mut result = if trace {
            self.evaluate_law_traced_state(
                law_id,
                output_names,
                parameters,
                Call::default(),
                calculation_date,
                TraceBuilder::new(),
                state,
            )?
        } else {
            self.evaluate_law_in_state(
                law_id,
                output_names,
                parameters,
                Call::default(),
                calculation_date,
                state,
            )?
        };
        if let Some(root) = result.trace.as_mut() {
            root.children.insert(0, trace_node(&played));
        }
        Ok(ProjectionOutcome {
            state: played.state,
            set_by: played.set_by,
            events: played.events,
            result,
        })
    }

    /// Play the chronicle, rewinding until no record changes whether an
    /// earlier record counts.
    fn play(
        &self,
        law_id: &str,
        projection: &Projection,
        records: &[ChronicleRecord],
        partition: &Partition,
    ) -> Result<Played> {
        // undone_by[t]: the records with an effect on t that held when they
        // were last played, each with what it does to t. A record is skipped
        // while this is not empty.
        let mut undone_by: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
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
            let mut state: BTreeMap<String, Value> = projection
                .state
                .iter()
                .map(|p| (p.name.clone(), p.start()))
                .collect();
            let mut set_by: BTreeMap<String, Option<String>> = projection
                .state
                .iter()
                .map(|p| (p.name.clone(), None))
                .collect();
            let mut applied: BTreeSet<&str> = BTreeSet::new();
            let mut rewind = None;

            'records: for (i, record) in records.iter().enumerate() {
                if !partition.holds(&record.id) {
                    continue;
                }
                let undoers: Vec<String> = undone_by
                    .get(record.id.as_str())
                    .map(|u| u.iter().map(|id| id.to_string()).collect())
                    .unwrap_or_default();
                if !undoers.is_empty() {
                    events.push(ProjectionEvent::Skipped {
                        record: record.id.clone(),
                        undone_by: undoers,
                    });
                    // A record that is skipped or revoked undoes nothing any
                    // more: a record it alone undid counts again. A decision
                    // in the place of another that should keep a revocation
                    // in force says so itself, with an effect of its own.
                    let mut revived = None;
                    for (target, by) in undone_by.iter_mut() {
                        if by.remove(record.id.as_str()) && by.is_empty() && revived.is_none() {
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
                let effects = self.holding_effects(record, &records[..i])?;
                for target in effects.into_iter().filter(|t| partition.holds(t)) {
                    undone_by
                        .entry(target)
                        .or_default()
                        .insert(record.id.as_str());
                    // A target that was applied had no undoer yet: one would
                    // have made the replay skip it.
                    if applied.contains(target) {
                        rewind = Some(ProjectionEvent::Rewound {
                            record: record.id.clone(),
                            cause: RewindCause::Revokes,
                            target: target.to_string(),
                        });
                        break 'records;
                    }
                }

                // A record of someone else counts for its effects only: the
                // law says once what it undoes, whoever the question is
                // about, but its step is about its own key.
                if let Some(reason) = partition.foreign.get(&record.id) {
                    events.push(ProjectionEvent::EffectsOnly {
                        record: record.id.clone(),
                        reason: reason.clone(),
                    });
                    applied.insert(record.id.as_str());
                    continue;
                }

                let (article, changes) = match step_for(projection, record) {
                    Some(step) => (
                        Some(step.apply.clone()),
                        self.apply_step(law_id, projection, step, record, &state)?,
                    ),
                    None => (None, Vec::new()),
                };
                for change in &changes {
                    state.insert(change.property.clone(), change.to.clone());
                    set_by.insert(change.property.clone(), Some(record.id.clone()));
                }
                events.push(ProjectionEvent::Applied {
                    record: record.id.clone(),
                    article,
                    changes,
                });
                applied.insert(record.id.as_str());
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
                        *by = now.iter().map(|id| id.to_string()).collect();
                    }
                }
            }
            match rewind {
                Some(event) => events.push(event),
                None => {
                    return Ok(Played {
                        state,
                        set_by,
                        events,
                    })
                }
            }
        }
        Err(EngineError::InvalidOperation(format!(
            "the projection of '{law_id}' does not settle after {max_passes} replays"
        )))
    }

    /// Refuse a step that does not say which records of its article it is
    /// about: an article with more than one kind of record needs the kind
    /// (`<article>#<KIND>`), the kind must be one the article gives, and a
    /// step cannot pick one of several facts of an article. The article is
    /// the one in force on `day`. This runs when the projection is played,
    /// because the reduction and the law are separate regelwerken that load
    /// in any order.
    fn check_steps(&self, law_id: &str, projection: &Projection, day: NaiveDate) -> Result<()> {
        for step in &projection.steps {
            let (basis, kind) = step.basis_and_kind();
            let refuse = |why: String| {
                Err(EngineError::InvalidOperation(format!(
                    "the step of '{law_id}' on {}: {why}",
                    step.record
                )))
            };
            let article = basis.split_once('#').and_then(|(law, number)| {
                self.resolver()
                    .get_law_for_date(law, Some(day))
                    .and_then(|l| l.find_article_by_number(number))
            });
            let Some(article) = article else {
                return refuse(format!("{basis} names no article in force on {day}"));
            };
            let kinds = kinds_of(article);
            let mut distinct = kinds.clone();
            distinct.dedup();
            match kind {
                None if distinct.len() > 1 => {
                    return refuse(format!(
                        "needs a kind: the article gives {}; write {basis}#{}",
                        distinct.join(" and "),
                        distinct.last().map_or("", String::as_str)
                    ))
                }
                Some(kind) if !kinds.iter().any(|k| k == kind) => {
                    return refuse(format!(
                        "the article gives no {kind}, only {}",
                        if distinct.is_empty() {
                            "nothing".to_string()
                        } else {
                            distinct.join(" and ")
                        }
                    ))
                }
                _ if kinds.iter().filter(|k| *k == EXECUTOGRAM).count() > 1
                    && kind.is_none_or(|k| k == EXECUTOGRAM) =>
                {
                    return refuse(
                        "the article requires more than one fact; a step cannot pick one"
                            .to_string(),
                    )
                }
                _ => {}
            }
            self.check_wiring(projection, step, article, day)
                .or_else(refuse)?;
        }
        Ok(())
    }

    /// Refuse a step whose wiring does not fit the article it applies, in
    /// the version in force on `day`: the article must be one a step can
    /// apply ([`refuse_impure`]), each of its parameters must be bound
    /// ([`bindings`]) to a field of the record or a property of the state of
    /// the same type, and each property the step sets ([`outputs_to_state`])
    /// must come from an output of the property's type.
    fn check_wiring(
        &self,
        projection: &Projection,
        step: &ProjectionStep,
        record_article: &Article,
        day: NaiveDate,
    ) -> std::result::Result<(), String> {
        let Some((_, article)) = self.applied_article(&step.apply, day) else {
            return Err(format!("{} names no article in force on {day}", step.apply));
        };
        refuse_impure(article).map_err(|why| format!("{} cannot be applied: {why}", step.apply))?;
        if let Some(name) = step
            .parameters
            .keys()
            .find(|n| !article.get_parameters().iter().any(|p| &p.name == *n))
        {
            return Err(format!("{} has no parameter '{name}'", step.apply));
        }
        for (parameter, source) in bindings(step, article, projection, record_article)? {
            let given = match &source {
                StepSource::State(property) => state_type(projection, property),
                StepSource::Record(field) => field_type(record_article, field),
            };
            match given {
                None => {
                    return Err(format!(
                        "wires {source} into '{}', but the record has no such field",
                        parameter.name
                    ))
                }
                Some(t) if t != parameter.param_type => {
                    return Err(format!(
                        "wires {source}, a {t:?}, into '{}', a {:?}",
                        parameter.name, parameter.param_type
                    ))
                }
                Some(_) => {}
            }
        }
        if let Some(output) = step
            .output
            .values()
            .find(|o| !article.get_outputs().iter().any(|d| &d.name == *o))
        {
            return Err(format!("{} has no output '{output}'", step.apply));
        }
        for (property, output) in outputs_to_state(step, article, projection)? {
            let declared = article
                .get_outputs()
                .iter()
                .find(|o| o.name == output)
                .map(|o| o.output_type);
            if let (Some(t), Some(d)) = (state_type(projection, &property), declared) {
                if t != d {
                    return Err(format!(
                        "sets '{property}' from '{output}', a {d:?}, which is no {t:?}"
                    ));
                }
            }
        }
        Ok(())
    }

    /// What `step` does with `state` for `record`: the article it applies,
    /// in the version in force on the day the record was recorded, run on
    /// the values the step wires in, and the properties its outputs set.
    /// The article reads nothing but those values ([`refuse_impure`]).
    fn apply_step(
        &self,
        law_id: &str,
        projection: &Projection,
        step: &ProjectionStep,
        record: &ChronicleRecord,
        state: &BTreeMap<String, Value>,
    ) -> Result<Vec<StateChange>> {
        let fail = |why: String| {
            EngineError::InvalidOperation(format!(
                "the step of '{law_id}' for record '{}' ({}) applies {}: {why}",
                record.id, record.basis, step.apply
            ))
        };
        let (law, article) = self
            .applied_article(&step.apply, record.recorded_on)
            .ok_or_else(|| fail(format!("no article in force on {}", record.recorded_on)))?;
        refuse_impure(article).map_err(fail)?;

        // The record's fields, then its outputs; an output wins over a field
        // of the same name.
        let mut row = record.fields.clone();
        row.extend(record.outputs.clone());
        let record_article = self.article_of(record)?;
        let mut parameters = BTreeMap::new();
        for (parameter, source) in
            bindings(step, article, projection, record_article).map_err(fail)?
        {
            let value = match &source {
                StepSource::Record(field) => row
                    .get(field)
                    .cloned()
                    .ok_or_else(|| fail(format!("the record holds no '{field}'")))?,
                StepSource::State(property) => state.get(property).cloned().unwrap_or(Value::Null),
            };
            parameters.insert(parameter.name.clone(), value);
        }
        let result = ArticleEngine::new(article, law)
            .evaluate(parameters, &record.recorded_on.to_string())
            .map_err(|e| fail(e.to_string()))?;

        let mut changes = Vec::new();
        for (property, output) in outputs_to_state(step, article, projection).map_err(fail)? {
            let to = result
                .outputs
                .get(&output)
                .cloned()
                .ok_or_else(|| fail(format!("the article gives no output '{output}'")))?;
            let declared = projection.state.iter().find(|p| p.name == property);
            if let Some(declared) = declared.filter(|p| !fits_property(p, &to)) {
                return Err(fail(format!(
                    "its output '{output}' is {to:?}, which is no {:?} for '{property}'",
                    declared.property_type
                )));
            }
            let from = state.get(&property).cloned().unwrap_or(Value::Null);
            changes.push(StateChange { property, from, to });
        }
        Ok(changes)
    }

    /// The law and article `reference` names, in the version in force on
    /// `day`.
    fn applied_article(
        &self,
        reference: &str,
        day: NaiveDate,
    ) -> Option<(&ArticleBasedLaw, &Article)> {
        let (law_id, number) = reference.split_once('#')?;
        let law = self.resolver().get_law_for_date(law_id, Some(day))?;
        let article = law.find_article_by_number(number)?;
        Some((law, article))
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
    /// A record belongs when it holds the asked key under the key's name,
    /// as a field or an output. A record that refers to a record of the
    /// partition (`refers_to`, or a field an effect of its article names,
    /// such as `bestreden_besluit`) joins it too, transitively. It joins for
    /// its effects only when it is someone else's: when it holds a key of its
    /// own with another value, or holds none and refers only to records
    /// that are someone else's. Such a record still revokes what it names, because the law says once what a record does
    /// with another, whatever the question is keyed on; its step is not
    /// applied. Following references through it keeps the effects right: a
    /// record that undoes it joins too, and what it undid counts again.
    /// Records only join through what refers to them later, never through
    /// what they refer to, so the partition stays the records reachable
    /// from the key. A record without a key therefore plays only when it
    /// refers in. The chronicle lists a
    /// record after what it refers to, which one pass in its order relies on.
    fn partition(
        &self,
        projection: &Projection,
        key_type: Option<&ParameterType>,
        records: &[ChronicleRecord],
        parameters: &BTreeMap<String, Value>,
    ) -> Result<Partition> {
        let Some(key_parameter) = &projection.key else {
            return Ok(Partition::default());
        };
        let key = parameters.get(key_parameter).ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "the projection plays the records of one '{key_parameter}', but none is passed"
            ))
        })?;
        // A key of another type than the parameter declares would match no
        // record and give the initial state without a word.
        if let Some(declared) = key_type.filter(|t| !fits_state_type(t, key)) {
            return Err(EngineError::InvalidOperation(format!(
                "'{key_parameter}' is passed as {key:?}, which is no {declared:?}"
            )));
        }
        let mut members: BTreeSet<String> = BTreeSet::new();
        let mut foreign: BTreeMap<String, String> = BTreeMap::new();
        for record in records {
            let own = record
                .fields
                .get(key_parameter)
                .or_else(|| record.outputs.get(key_parameter))
                // A key that holds null is no key.
                .filter(|value| !matches!(value, Value::Null))
                .map(|value| (key_parameter, value));
            if own.is_some_and(|(_, value)| value == key) {
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
                Some((field, _)) => Some(format!("key differs: {field}")),
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
    /// the fields that an effect of its article names. That holds for every
    /// record under the article, so an objection that names the contested
    /// decision in such a field refers to it, as the decision on it does.
    fn references(&self, record: &ChronicleRecord) -> Result<Vec<String>> {
        let mut ids: Vec<String> = record.refers_to.iter().cloned().collect();
        let article = self.article_of(record)?;
        if let Some(decision) = article.get_decision() {
            for effect in decision.effects() {
                if let Some(Value::String(id)) = record.fields.get(&effect.parameter) {
                    ids.push(id.clone());
                }
            }
        }
        Ok(ids)
    }

    /// The effects of `record` that hold, each with the earlier record it
    /// undoes. A record has effects only as a decision of its article
    /// (`produces[].effects`), under the law of the day it was recorded.
    fn holding_effects<'r>(
        &self,
        record: &ChronicleRecord,
        earlier: &'r [ChronicleRecord],
    ) -> Result<Vec<&'r str>> {
        let article = self.article_of(record)?;
        // A submission and an executogram undo nothing; a decision has the
        // effects its article gives it. A record of a kind its article does
        // not produce is refused, for the same reason as an unknown article.
        if record.kind == EXECUTOGRAM
            || article.get_submission().and_then(|s| s.submission_kind()) == Some(&record.kind)
        {
            return Ok(Vec::new());
        }
        let decision = article
            .get_decision()
            .filter(|d| d.legal_character() == Some(record.kind.as_str()))
            .ok_or_else(|| {
                EngineError::InvalidOperation(format!(
                    "record '{}' is a {} under '{}', which that article does not produce",
                    record.id, record.kind, record.basis
                ))
            })?;
        let mut holding = Vec::new();
        for effect in decision.effects() {
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
            let name = &effect.parameter;
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
            holding.push(target.id.as_str());
        }
        Ok(holding)
    }
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

/// The step the reduction gives for `record`, if any: the one on its
/// article, of its kind when the step names one. A step without a kind is
/// on an article that has one kind of record ([`LawExecutionService::check_steps`]).
fn step_for<'p>(
    projection: &'p Projection,
    record: &ChronicleRecord,
) -> Option<&'p ProjectionStep> {
    projection.steps.iter().find(|s| {
        let (basis, kind) = s.basis_and_kind();
        basis == record.basis && kind.is_none_or(|k| k == record.kind)
    })
}

/// The kinds of record an article gives: per item it produces, its kind (a
/// submission kind or a legal character), and `EXECUTOGRAM` per fact it
/// requires.
fn kinds_of(article: &Article) -> Vec<String> {
    let produced = article.get_produced().into_iter().filter_map(|p| {
        p.submission_kind()
            .or(p.legal_character())
            .map(str::to_string)
    });
    let facts = article
        .get_requires()
        .iter()
        .filter(|r| r.fact.is_some())
        .map(|_| EXECUTOGRAM.to_string());
    produced.chain(facts).collect()
}

/// Where each parameter of `article` takes its value from: what the step
/// wires in `parameters`, or else the field of the record or the property of
/// the state of the same name. A name both hold is ambiguous and must be
/// wired; a name neither holds cannot be bound.
fn bindings<'a>(
    step: &ProjectionStep,
    article: &'a Article,
    projection: &Projection,
    record_article: &Article,
) -> std::result::Result<Vec<(&'a regelrecht_law_model::Parameter, StepSource)>, String> {
    article
        .get_parameters()
        .iter()
        .map(|parameter| {
            let name = &parameter.name;
            if let Some(source) = step.parameters.get(name) {
                return Ok((parameter, source.clone()));
            }
            let in_record = field_type(record_article, name).is_some();
            let in_state = state_type(projection, name).is_some();
            match (in_record, in_state) {
                (true, true) => Err(format!(
                    "parameter '{name}' of {} could come from $record.{name} or $state.{name}; \
                     wire it in parameters",
                    step.apply
                )),
                (true, false) => Ok((parameter, StepSource::Record(name.clone()))),
                (false, true) => Ok((parameter, StepSource::State(name.clone()))),
                (false, false) => Err(format!(
                    "{} needs parameter '{name}', which the step does not wire and neither the \
                     record nor the state holds",
                    step.apply
                )),
            }
        })
        .collect()
}

/// Which output of `article` sets which property of the state: what the
/// step maps in `output`, and else each output that has the name of a
/// property. An output that names no property is not used. A step that
/// sets nothing at all is refused.
fn outputs_to_state(
    step: &ProjectionStep,
    article: &Article,
    projection: &Projection,
) -> std::result::Result<Vec<(String, String)>, String> {
    let mut map: BTreeMap<String, String> = step.output.clone();
    for output in article.get_outputs() {
        if state_type(projection, &output.name).is_some() && !map.contains_key(&output.name) {
            map.insert(output.name.clone(), output.name.clone());
        }
    }
    if map.is_empty() {
        return Err(format!(
            "{} sets no property of the state: no output has the name of one, and the step \
             maps none",
            step.apply
        ));
    }
    Ok(map.into_iter().collect())
}

/// The type of property `name` of the projection's state.
fn state_type(projection: &Projection, name: &str) -> Option<ParameterType> {
    projection
        .state
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.property_type)
}

/// The type of field `name` of a record under `article`: a parameter or an
/// output of it.
fn field_type(article: &Article, name: &str) -> Option<ParameterType> {
    article
        .get_parameters()
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.param_type)
        .or_else(|| {
            article
                .get_outputs()
                .iter()
                .find(|o| o.name == name)
                .map(|o| o.output_type)
        })
}

/// Refuse an article a step cannot apply: one that reads anything but the
/// parameters the step wires in. An input (with or without a `source`) is
/// data from outside the record and the state, and an open term is filled
/// by another regulation, so an article with either is refused, as is one
/// without outputs or one that plays a projection itself.
fn refuse_impure(article: &Article) -> std::result::Result<(), String> {
    let Some(machine) = article.machine_readable.as_ref() else {
        return Err("the article has no machine_readable".to_string());
    };
    if machine.projection.is_some() {
        return Err("the article plays a projection itself".to_string());
    }
    if machine.open_terms.as_ref().is_some_and(|t| !t.is_empty()) {
        return Err("the article has open terms, which another regulation fills".to_string());
    }
    let execution = machine.execution.as_ref();
    if let Some(input) = execution
        .and_then(|e| e.input.as_ref())
        .and_then(|i| i.first())
    {
        return Err(format!(
            "the article reads input '{}'; an applied article reads only what the step wires in",
            input.name
        ));
    }
    if article.get_outputs().is_empty() {
        return Err("the article has no outputs".to_string());
    }
    Ok(())
}

/// The state, its provenance and the events after the last replay.
struct Played {
    state: BTreeMap<String, Value>,
    set_by: BTreeMap<String, Option<String>>,
    events: Vec<ProjectionEvent>,
}

/// Whether a step may give `property` the value `value`: one of its type,
/// or null for a string or a date, whose start value is null.
pub(crate) fn fits_property(property: &StateProperty, value: &Value) -> bool {
    match value {
        Value::Null => property.start().is_null(),
        _ => fits_state_type(&property.property_type, value),
    }
}

/// Whether `value` is a value of `ty`, one of the types a property of a
/// projection's state may have: boolean, number, string, amount (whole
/// eurocents) or date.
pub(crate) fn fits_state_type(ty: &ParameterType, value: &Value) -> bool {
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

fn parse_day(date: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| EngineError::InvalidDate(format!("{date}: {e}")))
}

/// The events as a `PROJECTION` node of the engine trace: one child per
/// event, named after it, with a message saying what happened and, on an
/// applied record, the properties it set as the result.
fn trace_node(played: &Played) -> PathNode {
    let mut node = PathNode::new(PathNodeType::Operation, "PROJECTION")
        .with_result(Value::Object(played.state.clone()));
    for event in &played.events {
        let (name, message, result) = match event {
            ProjectionEvent::Applied {
                record,
                article,
                changes,
            } => {
                let set: BTreeMap<String, Value> = changes
                    .iter()
                    .map(|c| (c.property.clone(), c.to.clone()))
                    .collect();
                (
                    "APPLY",
                    format!(
                        "{record}: {}",
                        describe_applied(article.as_deref(), changes)
                    ),
                    Some(Value::Object(set)),
                )
            }
            ProjectionEvent::Skipped { record, undone_by } => (
                "SKIP",
                format!("{record}: undone by {}", undone_by.join(", ")),
                None,
            ),
            ProjectionEvent::EffectsOnly { record, reason } => (
                "EFFECTS_ONLY",
                format!("{record}: effects only, {reason}"),
                None,
            ),
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
                None,
            ),
        };
        let mut child = PathNode::new(PathNodeType::Operation, name);
        child.message = Some(message);
        child.result = result;
        node.children.push(child);
    }
    node
}

/// `property: from -> to`, comma-separated; `-` for a record without a step.
pub fn describe_changes(changes: &[StateChange]) -> String {
    if changes.is_empty() {
        return "-".to_string();
    }
    changes
        .iter()
        .map(|c| format!("{}: {} -> {}", c.property, show(&c.from), show(&c.to)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A record that was applied, as the trace shows it: the article its step
/// applied and each property it set, before and after; `-` without a step.
pub fn describe_applied(article: Option<&str>, changes: &[StateChange]) -> String {
    match article {
        Some(article) => format!("{article}: {}", describe_changes(changes)),
        None => "-".to_string(),
    }
}

/// The word for a rewind's cause, as the trace shows it.
pub fn cause_word(cause: RewindCause) -> &'static str {
    match cause {
        RewindCause::Revokes => "revokes",
        RewindCause::Revives => "revives",
    }
}

fn show(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        other => serde_json::to_string(other).unwrap_or_else(|_| format!("{other:?}")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Article 1 grants (`verleend`), article 2 revokes `bestreden` when
    /// `herroepen`, article 3 takes a decision in the place of `bestreden`
    /// (it revokes it), article 4 requires the
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
            effects: [{revokes: bestreden, when: herroepen}]
        parameters: [{name: bestreden, type: string}]
        output: [{name: herroepen, type: boolean}]
        actions: [{output: herroepen, value: true}]
  - number: '3'
    text: t
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
            effects: [{revokes: bestreden}]
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
  - number: '5'
    text: t
    machine_readable:
      execution:
        parameters: [{name: verleend, type: boolean}, {name: aantal, type: number}]
        output: [{name: kaart, type: boolean}, {name: aantal_na, type: number}]
        actions:
          - {output: kaart, value: $verleend}
          - {output: aantal_na, value: {operation: ADD, values: [$aantal, 1]}}
  - number: '6'
    text: t
    machine_readable:
      execution:
        output: [{name: kaart, type: boolean}]
        actions: [{output: kaart, value: false}]
"#;

    /// Counts the grants that count and whether the last record left a card.
    const REDUCTION: &str = r#"
$id: reductie
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      projection:
        state:
          - {name: kaart, type: boolean}
          - {name: aantal, type: number}
        steps:
          - record: wet#1#BESCHIKKING
            apply: wet#5
            output: {aantal: aantal_na}
          - record: wet#3
            apply: wet#5
          - record: wet#4
            apply: wet#6
      execution:
        output: [{name: heeft_kaart, type: boolean}, {name: aantal, type: number}]
        actions:
          - {output: heeft_kaart, value: $state.kaart}
          - {output: aantal, value: $state.aantal}
"#;

    fn service() -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service.load_law(LAW).unwrap();
        service.load_law(REDUCTION).unwrap();
        service
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
            recorded_on: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
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

    fn handed_in(id: &str) -> ChronicleRecord {
        ChronicleRecord {
            kind: "EXECUTOGRAM".to_string(),
            fact: Some("ingeleverd".to_string()),
            ..record(id, "4", &[], &[])
        }
    }

    fn play(records: &[ChronicleRecord]) -> Result<ProjectionOutcome> {
        service().evaluate_projection(
            "reductie",
            &["heeft_kaart", "aantal"],
            records,
            BTreeMap::new(),
            "2025-03-01",
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
            handed_in("i"),
            grant("b"),
            revoke("h", "a", true),
        ])
        .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(1));
        assert_eq!(outcome.set_by["kaart"].as_deref(), Some("b"));
        assert_eq!(skipped_in_last_replay(&outcome), ["a"]);
        assert!(outcome.events.contains(&ProjectionEvent::Rewound {
            record: "h".to_string(),
            cause: RewindCause::Revokes,
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
    fn a_replacing_record_applies_its_own_step() {
        let replace = record(
            "n",
            "3",
            &[("bestreden", Value::String("a".to_string()))],
            &[("verleend", Value::Bool(false))],
        );
        let outcome = play(&[grant("a"), replace]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(0));
        assert!(outcome.events.contains(&ProjectionEvent::Rewound {
            record: "n".to_string(),
            cause: RewindCause::Revokes,
            target: "a".to_string(),
        }));
    }

    fn replace(id: &str, target: &str) -> ChronicleRecord {
        record(
            id,
            "3",
            &[("bestreden", Value::String(target.to_string()))],
            &[("verleend", Value::Bool(false))],
        )
    }

    #[test]
    fn a_replaced_replacement_undoes_nothing_any_more() {
        // n1 replaces a; n2 replaces n1, so n1 counts no more and neither
        // does its replacing of a: a counts again, beside n2.
        let outcome = play(&[grant("a"), replace("n1", "a"), replace("n2", "n1")]).unwrap();
        assert_eq!(skipped_in_last_replay(&outcome), ["n1"]);
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
    fn a_step_matches_the_kind_its_record_names() {
        // A submission under article 1 has no step: only its decisions do.
        let submission = ChronicleRecord {
            kind: "AANVRAAG".to_string(),
            ..record("s", "1", &[], &[])
        };
        let outcome = play(&[grant("a"), submission]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(outcome.set_by["kaart"].as_deref(), Some("a"));
        let outcome = play(&[grant("a"), handed_in("i")]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
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
    fn the_caller_cannot_pass_the_state() {
        let err = service()
            .evaluate_projection(
                "reductie",
                &["heeft_kaart"],
                &[],
                BTreeMap::from([("state".to_string(), Value::Bool(true))]),
                "2025-03-01",
                false,
            )
            .expect_err("a state passed in");
        assert!(err.to_string().contains("cannot be passed"), "{err}");
    }

    #[test]
    fn one_version_of_the_reduction_plays_the_whole_chronicle() {
        // From 2025-06-01 a grant no longer counts towards `aantal`. The
        // version in force on the calculation date plays every record.
        let mut service = service();
        service
            .load_law(
                &REDUCTION
                    .replace("valid_from: '2025-01-01'", "valid_from: '2025-06-01'")
                    .replace("            output: {aantal: aantal_na}\n", ""),
            )
            .unwrap();
        let later = ChronicleRecord {
            recorded_on: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
            ..grant("b")
        };
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
        let later = ChronicleRecord {
            recorded_on: NaiveDate::from_ymd_opt(2025, 4, 1).unwrap(),
            ..revoke("h", "a", true)
        };
        let outcome = play(&[grant("a"), later]).unwrap();
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

    /// Whether each record counts does not depend on the state, so one pass
    /// from the last record back gives it: a record counts unless a record
    /// after it that counts has an effect on it that holds. Rewinding must
    /// arrive at the same records, over many generated chronicles.
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
            let expected: Vec<String> = (0..n)
                .filter(|&i| !counts[i])
                .map(|i| format!("r{i}"))
                .collect();
            let outcome = play(&records).unwrap();
            assert_eq!(skipped_in_last_replay(&outcome), expected, "{records:#?}");
        }
    }

    #[test]
    fn a_step_that_computes_a_value_of_the_wrong_type_is_refused() {
        // `verleend` holds a string here, and `kaart` is a boolean.
        let odd = record("a", "1", &[], &[("verleend", Value::String("ja".into()))]);
        let err = play(&[odd]).expect_err("a string in a boolean");
        assert!(err.to_string().contains("which is no Boolean"), "{err}");
    }

    #[test]
    fn articles_of_one_reduction_may_read_each_other_while_it_plays() {
        // Article 1 (the projection) reads `geldig` of article 2, which reads
        // `aantal` of article 1: the played state is passed on, not refused.
        let looped = REDUCTION
            .replace("$id: reductie", "$id: reductie_lus")
            .replace(
                "      execution:\n        output: [{name: heeft_kaart, type: boolean}, {name: aantal, type: number}]",
                "      execution:\n        input:\n          - {name: geldig, type: boolean, source: {output: geldig}}\n        output: [{name: heeft_kaart, type: boolean}, {name: aantal, type: number}]",
            )
            .replace(
                "          - {output: heeft_kaart, value: $state.kaart}",
                "          - {output: heeft_kaart, value: $geldig}",
            )
            + "  - number: '2'\n    text: t\n    machine_readable:\n      execution:\n        input:\n          - {name: n, type: number, source: {output: aantal}}\n        output: [{name: geldig, type: boolean}]\n        actions:\n          - output: geldig\n            value: {operation: GREATER_THAN, subject: $n, value: 0}\n";
        assert!(looped.contains("source: {output: geldig}"));
        let mut service = service();
        service.load_law(&looped).unwrap();
        let outcome = service
            .evaluate_projection(
                "reductie_lus",
                &["heeft_kaart"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
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
        // The same reduction with a second article that reads the first.
        service
            .load_law(&REDUCTION.replace("$id: reductie", "$id: reductie_twee").replace(
                "          - {output: aantal, value: $state.aantal}\n",
                "          - {output: aantal, value: $state.aantal}\n  - number: '2'\n    text: t\n    machine_readable:\n      execution:\n        input:\n          - {name: k, type: boolean, source: {output: heeft_kaart}}\n        output: [{name: y, type: boolean}]\n        actions: [{output: y, value: $k}]\n",
            ))
            .unwrap();
        let state = BTreeMap::from([(
            "state".to_string(),
            Value::Object(BTreeMap::from([("kaart".to_string(), Value::Bool(true))])),
        )]);
        for (law, output, parameters) in [
            ("reductie", "heeft_kaart", BTreeMap::new()),
            ("reductie", "heeft_kaart", state.clone()),
            ("andere_reductie", "x", BTreeMap::new()),
            ("reductie_twee", "y", BTreeMap::new()),
            ("reductie_twee", "y", state.clone()),
        ] {
            let err = service
                .evaluate_law(law, &[output], parameters, "2025-03-01")
                .expect_err("no played state");
            assert!(
                err.to_string().contains("with evaluate_projection"),
                "{law}: {err}"
            );
        }
    }

    #[test]
    fn an_empty_chronicle_answers_from_the_initial_state() {
        let outcome = play(&[]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert!(outcome.events.is_empty());
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
            ["n revokes a: play again from the start"]
        );
        assert_eq!(
            messages(&[
                grant("a"),
                revoke("h1", "a", true),
                revoke("h2", "h1", true)
            ]),
            [
                "h1 revokes a: play again from the start",
                "h2 revokes h1: play again from the start",
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
                    .replace(
                        "      projection:\n        state:",
                        "      projection:\n        key: wie\n        state:",
                    )

                    .replace(
                        "      execution:\n        output: [{name: heeft_kaart",
                        "      execution:\n        parameters: [{name: wie, type: string}]\n        output: [{name: heeft_kaart",
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
            "2025-03-01",
            false,
        )
    }

    /// The records the last replay applied or skipped, in order.
    fn records_in(outcome: &ProjectionOutcome) -> Vec<&str> {
        let start = outcome
            .events
            .iter()
            .rposition(|e| matches!(e, ProjectionEvent::Rewound { .. }))
            .map_or(0, |i| i + 1);
        outcome.events[start..]
            .iter()
            .filter_map(|e| match e {
                ProjectionEvent::Applied { record, .. }
                | ProjectionEvent::Skipped { record, .. } => Some(record.as_str()),
                _ => None,
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
        // `n` revokes x's grant in its place and carries y's key: for x it revokes the
        // grant all the same, but its step does not give x a card.
        let records = [of(grant("a"), "x"), of(replace("n", "a"), "y")];
        let x = play_for("x", &records).unwrap();
        assert!(x.events.contains(&ProjectionEvent::EffectsOnly {
            record: "n".to_string(),
            reason: "key differs: wie".to_string(),
        }));
        assert!(x.events.contains(&ProjectionEvent::Rewound {
            record: "n".to_string(),
            cause: RewindCause::Revokes,
            target: "a".to_string(),
        }));
        assert_eq!(skipped_in_last_replay(&x), ["a"]);
        assert_eq!(x.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(x.result.outputs["aantal"], Value::Int(0));
        // For y, `n` is its own record, and revokes a record outside y's
        // partition: that effect is not y's business.
        let y = play_for("y", &records).unwrap();
        assert_eq!(records_in(&y), ["n"]);
        assert!(!y
            .events
            .iter()
            .any(|e| matches!(e, ProjectionEvent::Rewound { .. })));
    }

    #[test]
    fn undoing_a_record_of_someone_else_revives_what_it_revoked() {
        // `n` (y) revokes x's grant; `h` revokes `n` and carries no key. It
        // refers only to a record of someone else, so it counts for x for its
        // effects only, and x's grant counts again.
        let records = [
            of(grant("a"), "x"),
            of(replace("n", "a"), "y"),
            revoke("h", "n", true),
        ];
        let x = play_for("x", &records).unwrap();
        assert_eq!(records_in(&x), ["a", "n"]);
        assert!(x.events.contains(&ProjectionEvent::EffectsOnly {
            record: "h".to_string(),
            reason: "refers to n".to_string(),
        }));
        assert_eq!(x.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(x.set_by["kaart"].as_deref(), Some("a"));
        let y = play_for("y", &records).unwrap();
        assert_eq!(records_in(&y), ["n", "h"]);
    }

    #[test]
    fn a_record_that_refers_to_one_of_its_own_records_is_its_own() {
        // `h` revokes `n` (y's) and also names x's grant `a` by `refers_to`:
        // it refers to a record of x, so x applies its step too.
        let mut h = revoke("h", "n", true);
        h.refers_to = Some("a".to_string());
        let records = [of(grant("a"), "x"), of(replace("n", "a"), "y"), h];
        let x = play_for("x", &records).unwrap();
        assert!(!x
            .events
            .iter()
            .any(|e| matches!(e, ProjectionEvent::EffectsOnly { record, .. } if record == "h")));
    }

    #[test]
    fn a_null_key_field_is_no_key() {
        let mut nameless = replace("n", "a");
        nameless.fields.insert("wie".to_string(), Value::Null);
        let x = play_for("x", &[of(grant("a"), "x"), nameless]).unwrap();
        assert_eq!(records_in(&x), ["a", "n"]);
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
    fn a_keyless_step_plays_only_records_that_refer_in() {
        // A card handed in holds no `wie`: such a
        // record refers to nothing and never joins a partition.
        let outcome = play_for("x", &[of(grant("a"), "x"), handed_in("i")]).unwrap();
        assert_eq!(records_in(&outcome), ["a"]);
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
    }

    #[test]
    fn a_step_names_the_kind_only_where_the_article_has_more_than_one() {
        let ask = |steps: &str| {
            let mut service = service();
            service
                .load_law(
                    &REDUCTION
                        .replace("$id: reductie", "$id: reductie_soort")
                        .replace(
                            "          - record: wet#1#BESCHIKKING\n",
                            &format!("          - record: {steps}\n"),
                        ),
                )
                .unwrap();
            service.evaluate_projection(
                "reductie_soort",
                &["heeft_kaart"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
        };
        // Article 1 gives an application and a decision.
        let err = ask("wet#1").expect_err("no kind");
        assert!(
            err.to_string().contains(
                "needs a kind: the article gives AANVRAAG and BESCHIKKING; write wet#1#BESCHIKKING"
            ),
            "{err}"
        );
        let err = ask("wet#1#BEZWAAR").expect_err("a kind it does not give");
        assert!(
            err.to_string()
                .contains("the article gives no BEZWAAR, only AANVRAAG and BESCHIKKING"),
            "{err}"
        );
        assert!(ask("wet#1#BESCHIKKING").is_ok());
        // A kind on an article with one kind is allowed, but not needed.
        let mut service = service();
        service
            .load_law(
                &REDUCTION
                    .replace("$id: reductie", "$id: reductie_soort")
                    .replace(
                        "          - record: wet#3\n",
                        "          - record: wet#3#BESCHIKKING\n",
                    ),
            )
            .unwrap();
        assert!(service
            .evaluate_projection(
                "reductie_soort",
                &["heeft_kaart"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .is_ok());
    }

    #[test]
    fn a_step_cannot_pick_one_of_two_facts() {
        // Without a kind, and with EXECUTOGRAM as its kind: neither picks a fact.
        for step in ["wet#4", "wet#4#EXECUTOGRAM"] {
            let mut service = LawExecutionService::new();
            service
                .load_law(&LAW.replace(
                    "        - fact: ingeleverd\n          parameters: [op]\n",
                    "        - fact: ingeleverd\n          parameters: [op]\n        - fact: verloren\n          parameters: [op]\n",
                ))
                .unwrap();
            service
                .load_law(&REDUCTION.replace("record: wet#4\n", &format!("record: {step}\n")))
                .unwrap();
            let err = service
                .evaluate_projection(
                    "reductie",
                    &["heeft_kaart"],
                    &[],
                    BTreeMap::new(),
                    "2025-03-01",
                    false,
                )
                .expect_err("two facts");
            assert!(
                err.to_string()
                    .contains("the article requires more than one fact; a step cannot pick one"),
                "{step}: {err}"
            );
        }
    }

    #[test]
    fn a_state_starts_from_the_value_of_its_type() {
        let property = |ty: &str| -> StateProperty {
            serde_yaml_ng::from_str(&format!("{{name: p, type: {ty}}}")).unwrap()
        };
        assert_eq!(property("boolean").start(), Value::Bool(false));
        assert_eq!(property("number").start(), Value::Int(0));
        assert_eq!(property("amount").start(), Value::Int(0));
        assert_eq!(property("string").start(), Value::Null);
        assert_eq!(property("date").start(), Value::Null);
        // A string or a date may be set to null again; a boolean may not.
        assert!(fits_property(&property("string"), &Value::Null));
        assert!(!fits_property(&property("boolean"), &Value::Null));
        // A date is a string that reads as one.
        assert!(fits_property(
            &property("date"),
            &Value::String("2025-01-01".into())
        ));
        assert!(!fits_property(
            &property("date"),
            &Value::String("morgen".into())
        ));
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
    fn the_trace_holds_a_node_per_event() {
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
        let names: Vec<&str> = node.children.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["APPLY", "REWIND", "SKIP", "APPLY"]);
        assert_eq!(
            node.children[0].message.as_deref(),
            Some("a: wet#5: aantal: 0 -> 1, kaart: false -> true")
        );
    }

    /// A step applies the article in the version in force on the day the
    /// record was recorded, as an effect does: a law that changes later
    /// does not change what an earlier record did.
    #[test]
    fn a_step_applies_the_article_of_the_day_of_its_record() {
        let mut service = service();
        // From 2025-06-01, a grant counts twice.
        service
            .load_law(
                &LAW.replace("valid_from: '2025-01-01'", "valid_from: '2025-06-01'")
                    .replace("values: [$aantal, 1]", "values: [$aantal, 2]"),
            )
            .unwrap();
        let later = ChronicleRecord {
            recorded_on: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
            ..grant("b")
        };
        let outcome = service
            .evaluate_projection(
                "reductie",
                &["aantal"],
                &[grant("a"), later],
                BTreeMap::new(),
                "2025-08-01",
                false,
            )
            .unwrap();
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(3));
    }

    /// A parameter the step does not wire takes the field of the record or
    /// the property of the state of the same name, and an output with the
    /// name of a property sets it; what the step maps itself wins.
    #[test]
    fn a_step_binds_by_name_unless_it_says_otherwise() {
        // `verleend` from the record, `aantal` from the state, `kaart` by
        // its name, `aantal` from `aantal_na` as the step says.
        let outcome = play(&[grant("a"), grant("b")]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(2));
        // The step maps `kaart` to another output: that wins over the name.
        let mut service = LawExecutionService::new();
        service
            .load_law(&LAW.replace(
                "          - {output: aantal_na, value: {operation: ADD, values: [$aantal, 1]}}\n",
                "          - {output: aantal_na, value: {operation: ADD, values: [$aantal, 1]}}\n          - {output: anders, value: false}\n",
            ).replace(
                "{name: aantal_na, type: number}]",
                "{name: aantal_na, type: number}, {name: anders, type: boolean}]",
            ))
            .unwrap();
        service
            .load_law(&REDUCTION.replace(
                "output: {aantal: aantal_na}",
                "output: {aantal: aantal_na, kaart: anders}",
            ))
            .unwrap();
        let outcome = service
            .evaluate_projection(
                "reductie",
                &["heeft_kaart", "aantal"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(outcome.result.outputs["aantal"], Value::Int(1));
    }

    /// A step only wires: the article it applies must find every parameter
    /// in the record or the state, with the right type, give the outputs
    /// the step maps, set at least one property, and read nothing else.
    #[test]
    fn a_step_must_fit_the_article_it_applies() {
        let ask = |law: &str, reduction: &str| {
            let mut service = LawExecutionService::new();
            service.load_law(law).unwrap();
            service.load_law(reduction).unwrap();
            service
                .evaluate_projection(
                    "reductie",
                    &["heeft_kaart"],
                    &[grant("a")],
                    BTreeMap::new(),
                    "2025-03-01",
                    false,
                )
                .expect_err(reduction)
                .to_string()
        };
        let first = "            output: {aantal: aantal_na}\n";
        let step = |wiring: &str| REDUCTION.replace(first, wiring);
        for (law, reduction, why) in [
            (
                LAW.to_string(),
                step("            output: {kaart: geen}\n"),
                "wet#5 has no output 'geen'",
            ),
            (
                LAW.replace("{name: aantal, type: number}", "{name: stand, type: number}")
                    .replace("values: [$aantal, 1]", "values: [$stand, 1]"),
                REDUCTION.to_string(),
                "wet#5 needs parameter 'stand', which the step does not wire and neither the \
                 record nor the state holds",
            ),
            (
                LAW.to_string(),
                REDUCTION.replace(
                    "          - {name: aantal, type: number}\n",
                    "          - {name: aantal, type: number}\n          - {name: verleend, type: boolean}\n",
                ),
                "parameter 'verleend' of wet#5 could come from $record.verleend or \
                 $state.verleend; wire it in parameters",
            ),
            (
                LAW.to_string(),
                step("            parameters: {ander: $state.kaart}\n"),
                "wet#5 has no parameter 'ander'",
            ),
            (
                LAW.to_string(),
                step("            parameters: {verleend: $state.aantal}\n"),
                "wires $state.aantal, a Number, into 'verleend', a Boolean",
            ),
            (
                LAW.to_string(),
                step("            parameters: {verleend: $record.bestaat_niet}\n"),
                "wires $record.bestaat_niet into 'verleend', but the record has no such field",
            ),
            (
                LAW.to_string(),
                step("            output: {kaart: aantal_na}\n"),
                "sets 'kaart' from 'aantal_na', a Number, which is no Boolean",
            ),
            (
                LAW.replace(
                    "        output: [{name: kaart, type: boolean}]\n        actions: [{output: kaart, value: false}]",
                    "        output: [{name: weg, type: boolean}]\n        actions: [{output: weg, value: false}]",
                ),
                REDUCTION.to_string(),
                "wet#6 sets no property of the state",
            ),
            (
                LAW.replace(
                    "        parameters: [{name: verleend, type: boolean}, {name: aantal, type: number}]\n",
                    "        parameters: [{name: verleend, type: boolean}, {name: aantal, type: number}]\n        input:\n          - {name: buiten, type: boolean, source: {regulation: elders, output: iets}}\n",
                ),
                REDUCTION.to_string(),
                "wet#5 cannot be applied: the article reads input 'buiten'",
            ),
            (
                LAW.replace(
                    "    machine_readable:\n      execution:\n        parameters: [{name: verleend, type: boolean}, {name: aantal",
                    "    machine_readable:\n      open_terms:\n        - {id: drempel, type: number, required: false}\n      execution:\n        parameters: [{name: verleend, type: boolean}, {name: aantal",
                ),
                REDUCTION.to_string(),
                "wet#5 cannot be applied: the article has open terms",
            ),
            (
                LAW.to_string(),
                REDUCTION.replace("apply: wet#6", "apply: wet#9"),
                "wet#9 names no article in force",
            ),
        ] {
            let err = ask(&law, &reduction);
            assert!(err.contains(why), "{why}: {err}");
        }
    }
}
