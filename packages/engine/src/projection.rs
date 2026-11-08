//! A reduction that plays a cell's chronicle forward (schema v0.8.14).
//!
//! The reduction declares a state (`projection.state`), each property with
//! its type and starting from that type's start value, and, per article a
//! record rests on, the step for such a record (`projection.steps`). A
//! step is written like an article: it takes outputs of a law
//! (`input` with `source.regulation`), with parameters from the record and
//! the state, and its actions give each property a bare reference. What a
//! record does is in that law. The engine plays the records the cell knew
//! on the calculation date in the order the cell recorded them. Each step
//! is pure: the law it calls reads nothing but what the step passes, so the
//! next state follows from the state before and the record alone. The law
//! runs in the version in force on the day of the record; one version of
//! the reduction, the one in force on the calculation date, gives the
//! state, every step and the answer: the reduction is the cell's reading, not law. The
//! reduction's `execution` answers from the last state, reading each
//! property as `$state.<name>`.
//!
//! What a record does with an earlier record is law, not part of the
//! reduction: the article of a decision says it revokes another record
//! (`produces[].effects`, optionally only `when` an outcome of that same
//! decision is true), and since schema v0.8.15 a fact the article requires
//! may say so too (`requires[].effects`, `when` reading a field of the
//! fact), such as the receipt of a ruling that annuls a decision. Each
//! effect is read under the version of that law in force on the day the
//! record was recorded. Which records count follows from the effects alone:
//! a record counts unless a later record that itself counts revokes it. So
//! a record that is revoked revokes nothing, and a record it alone revoked
//! counts again. A decision in the place of another revokes it and counts
//! as a record of its own. Whether a record counts depends neither on the
//! state nor on the day asked about, so the engine decides it first, in one
//! pass from the last record back, and then runs the steps of the records
//! that count, in order. The chronicle is never changed, and nothing is
//! reverted: a revoked record is gone from the whole history (ex tunc). A
//! change from a later day on (ex nunc), such as a suspension or a
//! withdrawal with effect from a date, is a step with a date property.
//!
//! The engine records, per property, which record set it last, and per
//! record what became of it: applied (with the properties it set), revoked
//! (with the records that revoke it), or counting for its effects only. The
//! events come back with the answer and, when tracing, as a `PROJECTION`
//! node in the engine's trace, with a `COUNTS` node for the first phase and
//! a `FOLD` node for the second.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_law_model::{
    Article, ArticleBasedLaw, Projection, ProjectionStep, StateProperty, StepSource,
};
use serde::{Deserialize, Serialize};

use crate::answer::Call;
use crate::engine::ArticleResult;
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

/// One property a record set: its value before and after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateChange {
    pub property: String,
    pub from: Value,
    pub to: Value,
}

/// What became of one record of the partition, in the order the cell
/// recorded them: it was applied, it does not count because records that
/// count revoke it, or it counts for its effects only.
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
    /// The record does not count, because these records, which count,
    /// revoke it.
    Skipped {
        record: String,
        undone_by: Vec<String>,
    },
    /// The record belongs to someone else, but what it does with a record
    /// of this partition holds here too: its effects hold, while its step
    /// leaves the state as it is. `reason`
    /// says why it is someone else's: its own key differs (`key differs:
    /// <field>`), or it refers only to such records (`refers to <id>`).
    EffectsOnly { record: String, reason: String },
}

/// How [`LawExecutionService::evaluate_projection_with`] runs the
/// reduction's `execution` over the last state.
#[derive(Default)]
pub struct ProjectionOptions<'a> {
    /// The call it runs in: the findings, and the cell that answers and
    /// whose chronicle it reads.
    pub call: Call<'a>,
    /// Whether to trace, with a `PROJECTION` node first.
    pub trace: bool,
}

/// Whether `article` answers from a projection (schema v0.8.14): only
/// [`LawExecutionService::evaluate_projection`] can run it.
pub fn has_projection(article: &Article) -> bool {
    article
        .machine_readable
        .as_ref()
        .is_some_and(|m| m.projection.is_some())
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
    /// record that is not an earlier record, an effect whose `when` the
    /// record does not hold as a boolean, a fact its article does not
    /// require, and a step that fails.
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
                call: Call::default(),
                trace,
            },
        )
    }

    /// [`Self::evaluate_projection`], with what `options` gives: the call
    /// the reduction's `execution` runs in (the cell that answers it and
    /// whose chronicle it reads, when a cell gives the answer through
    /// `source.reduction`), and whether to trace.
    pub fn evaluate_projection_with(
        &self,
        law_id: &str,
        output_names: &[&str],
        records: &[ChronicleRecord],
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        options: ProjectionOptions<'_>,
    ) -> Result<ProjectionOutcome> {
        let ProjectionOptions { call, trace } = options;
        let day = parse_day(calculation_date)?;
        let Some(first) = output_names.first() else {
            return Err(EngineError::InvalidOperation("no output asked".to_string()));
        };
        let (projection, key_type, question, since_v0_8_15) = {
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
            let question = article.get_parameters().to_vec();
            let key_type = projection.key.as_ref().and_then(|key| {
                article
                    .get_parameters()
                    .iter()
                    .find(|p| &p.name == key)
                    .map(|p| p.param_type)
            });
            let since_v0_8_15 = crate::load_check::from_version(law, (0, 8, 15));
            (projection, key_type, question, since_v0_8_15)
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
        self.check_steps(law_id, &projection, &question, day, since_v0_8_15)?;
        let partition = self.partition(&projection, key_type.as_ref(), &known, &parameters)?;
        let played = self.play(law_id, &projection, &known, &partition, &parameters, trace)?;

        let state = Some(Value::Object(played.state.clone()));
        let mut result = if trace {
            self.evaluate_law_traced_state(
                law_id,
                output_names,
                parameters,
                call,
                calculation_date,
                TraceBuilder::new(),
                state,
            )?
        } else {
            self.evaluate_law_in_state(
                law_id,
                output_names,
                parameters,
                call,
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

    /// Play the chronicle in two phases. First, which records count: a
    /// record counts unless a later record that itself counts revokes it
    /// (`produces[].effects`, `requires[].effects`). Effects only point back,
    /// and whether an effect holds depends on what its own record holds
    /// alone, so one pass from the last record back decides it, without the
    /// state. Then
    /// the fold: the steps of the records that count, in the order the cell
    /// recorded them, each from the state the one before left.
    fn play(
        &self,
        law_id: &str,
        projection: &Projection,
        records: &[ChronicleRecord],
        partition: &Partition,
        question: &BTreeMap<String, Value>,
        trace: bool,
    ) -> Result<Played> {
        // Phase 1: per record, the records that count and revoke it, found
        // from the last record back. A record that does not count revokes
        // nothing, so its effects are not even read.
        let mut undone_by: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (i, record) in records.iter().enumerate().rev() {
            if !partition.holds(&record.id) || undone_by.contains_key(record.id.as_str()) {
                continue;
            }
            // An effect on a record of another partition is that
            // partition's business.
            for target in self
                .holding_effects(record, &records[..i])?
                .into_iter()
                .filter(|t| partition.holds(t))
            {
                undone_by.entry(target).or_default().push(record.id.clone());
            }
        }

        // Phase 2: fold over the records that count.
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
        let mut events = Vec::new();
        // The traces of the laws a step called, by the index of its event.
        let mut nested: BTreeMap<usize, Vec<PathNode>> = BTreeMap::new();
        for record in records.iter().filter(|r| partition.holds(&r.id)) {
            if let Some(by) = undone_by.get(record.id.as_str()) {
                // Found from the back; listed in the order recorded.
                events.push(ProjectionEvent::Skipped {
                    record: record.id.clone(),
                    undone_by: by.iter().rev().cloned().collect(),
                });
                continue;
            }
            // A record of someone else counts for its effects only: the law
            // says once what it revokes, whoever the question is about, but
            // its step is about its own key.
            if let Some(reason) = partition.foreign.get(&record.id) {
                events.push(ProjectionEvent::EffectsOnly {
                    record: record.id.clone(),
                    reason: reason.clone(),
                });
                continue;
            }
            let AppliedStep {
                calls,
                changes,
                traces,
            } = match step_for(projection, record) {
                Some(step) => {
                    self.apply_step(law_id, projection, step, record, &state, question, trace)?
                }
                None => AppliedStep::default(),
            };
            for change in &changes {
                state.insert(change.property.clone(), change.to.clone());
                set_by.insert(change.property.clone(), Some(record.id.clone()));
            }
            if !traces.is_empty() {
                nested.insert(events.len(), traces);
            }
            events.push(ProjectionEvent::Applied {
                record: record.id.clone(),
                article: (!calls.is_empty()).then(|| calls.join(", ")),
                changes,
            });
        }
        Ok(Played {
            state,
            set_by,
            events,
            nested,
        })
    }

    /// Refuse a step that does not fit the laws it names, in their versions
    /// in force on `day`, the calculation date ([`check_step`]). The loader
    /// checks the same when the reduction loads, against the laws it holds
    /// then; this check also holds the step to a law that arrived later.
    fn check_steps(
        &self,
        law_id: &str,
        projection: &Projection,
        question: &[regelrecht_law_model::Parameter],
        day: NaiveDate,
        since_v0_8_15: bool,
    ) -> Result<()> {
        let law_of = |id: &str| self.resolver().get_law_for_date(id, Some(day));
        for step in &projection.steps {
            check_step(
                projection,
                step,
                question,
                &law_of,
                Versions::InForce(day),
                since_v0_8_15,
            )
            .map_err(|why| {
                EngineError::InvalidOperation(format!(
                    "the step of '{law_id}' on {}: {why}",
                    step.record
                ))
            })?;
        }
        Ok(())
    }

    /// What `step` does with `state` for `record`. Each input calls its law
    /// in the version in force on the day the record was recorded, not on
    /// the calculation date: what a record did follows from the law of its
    /// day, as its effects do. The law reads nothing but the parameters the
    /// input passes ([`refuse_impure`]). The properties then get what the
    /// step's actions give them.
    #[allow(clippy::too_many_arguments)]
    fn apply_step(
        &self,
        law_id: &str,
        projection: &Projection,
        step: &ProjectionStep,
        record: &ChronicleRecord,
        state: &BTreeMap<String, Value>,
        question: &BTreeMap<String, Value>,
        trace: bool,
    ) -> Result<AppliedStep> {
        let fail = |why: String| {
            EngineError::InvalidOperation(format!(
                "the step of '{law_id}' for record '{}' ({}): {why}",
                record.id, record.basis
            ))
        };
        let day = record.recorded_on;
        let date = day.to_string();
        // The record's fields, then its outputs; an output wins over a field
        // of the same name.
        let mut row = record.fields.clone();
        row.extend(record.outputs.clone());
        let mut inputs: BTreeMap<&str, Value> = BTreeMap::new();
        let read = |source: &StepSource, inputs: &BTreeMap<&str, Value>| match source {
            StepSource::Input(name) => inputs
                .get(name.as_str())
                .cloned()
                .ok_or_else(|| fail(format!("it has no input '{name}'"))),
            StepSource::Record(field) => row
                .get(field)
                .cloned()
                .ok_or_else(|| fail(format!("the record holds no '{field}'"))),
            StepSource::State(property) => Ok(state.get(property).cloned().unwrap_or(Value::Null)),
        };
        let mut applied = AppliedStep::default();
        for input in &step.input {
            let (regulation, output, parameters) = step_source(input).map_err(fail)?;
            let (_, article) = self
                .called_article(regulation, output, day)
                .ok_or_else(|| {
                    fail(format!(
                    "input '{}' takes '{output}' of {regulation}, which no article in force on \
                     {day} gives",
                    input.name
                ))
                })?;
            refuse_impure(article, output).map_err(|why| {
                fail(format!(
                    "input '{}' cannot call {regulation}: {why}",
                    input.name
                ))
            })?;
            let mut passed = BTreeMap::new();
            for (name, value) in &parameters {
                let source = StepSource::parse(value).ok_or_else(|| {
                    fail(format!("input '{}' passes '{name}' as {value}", input.name))
                })?;
                // In a parameter, `$<name>` is a parameter of the question.
                let value = match &source {
                    StepSource::Input(asked) => question.get(asked).cloned().ok_or_else(|| {
                        fail(format!(
                            "input '{}' passes ${asked}, which the question does not give",
                            input.name
                        ))
                    })?,
                    other => read(other, &inputs)?,
                };
                passed.insert(name.clone(), value);
            }
            let result = if trace {
                self.evaluate_law_with_trace_in(
                    regulation,
                    &[output],
                    passed,
                    Call::default(),
                    &date,
                )
            } else {
                self.evaluate_law_in(regulation, &[output], passed, Call::default(), &date)
            }
            .map_err(|e| fail(format!("input '{}': {e}", input.name)))?;
            let value = result.outputs.get(output).cloned().ok_or_else(|| {
                fail(format!(
                    "input '{}': {regulation} gave no '{output}'",
                    input.name
                ))
            })?;
            inputs.insert(input.name.as_str(), value);
            let call = format!("{regulation}#{}", article.number);
            if !applied.calls.contains(&call) {
                applied.calls.push(call);
            }
            applied.traces.extend(result.trace);
        }
        for action in &step.actions {
            let (property, value) = step_action(action).map_err(fail)?;
            let source = set_source(value).map_err(fail)?;
            let to = read(&source, &inputs)?;
            let declared = projection.state.iter().find(|p| &p.name == property);
            if let Some(declared) = declared.filter(|p| !fits_property(p, &to)) {
                return Err(fail(format!(
                    "it sets '{property}' from {source} to {to:?}, which is no {:?}",
                    declared.property_type
                )));
            }
            let from = state.get(property).cloned().unwrap_or(Value::Null);
            applied.changes.push(StateChange {
                property: property.clone(),
                from,
                to,
            });
        }
        // The trace lists the properties by name, whatever the order of the
        // actions.
        applied.changes.sort_by(|a, b| a.property.cmp(&b.property));
        Ok(applied)
    }

    /// The law `regulation` and its article that gives `output`, in the
    /// versions in force on `day`.
    fn called_article(
        &self,
        regulation: &str,
        output: &str,
        day: NaiveDate,
    ) -> Option<(&ArticleBasedLaw, &Article)> {
        let law = self.resolver().get_law_for_date(regulation, Some(day))?;
        let article = law.find_article_by_output(output)?;
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
    /// the fields that an effect of its article's decision names. That holds
    /// for every record under the article, so an objection that names the
    /// contested decision in such a field refers to it, as the decision on
    /// it does. A fact refers through its `refers_to` alone: its effects
    /// revoke that record (schema v0.8.15).
    fn references(&self, record: &ChronicleRecord) -> Result<Vec<String>> {
        let mut ids: Vec<String> = record.refers_to.iter().cloned().collect();
        if record.kind == EXECUTOGRAM {
            return Ok(ids);
        }
        let article = self.article_of(record)?;
        let effects = article
            .get_decision()
            .map(|d| d.effects())
            .unwrap_or_default();
        for effect in effects {
            if let Some(Value::String(id)) = record.fields.get(&effect.parameter) {
                ids.push(id.clone());
            }
        }
        Ok(ids)
    }

    /// The effects of `record` that hold, each with the earlier
    /// record it revokes. A decision has the effects its article gives it
    /// (`produces[].effects`), a fact those of its `requires` entry
    /// (`requires[].effects`, schema v0.8.15), under the law of the day it
    /// was recorded. An effect holds when its `when` is true; it reads only
    /// what the record itself holds. A decision names the record it revokes
    /// in a field, a fact revokes the record it refers to.
    fn holding_effects<'r>(
        &self,
        record: &ChronicleRecord,
        earlier: &'r [ChronicleRecord],
    ) -> Result<Vec<&'r str>> {
        let article = self.article_of(record)?;
        let refuse = |why: String| {
            EngineError::InvalidOperation(format!(
                "record '{}' ({}) {why}",
                record.id, record.basis
            ))
        };
        // A submission undoes nothing; a decision and a fact have the
        // effects their article gives them. A record of a kind its article
        // does not produce is refused, for the same reason as an unknown
        // article.
        let is_fact = record.kind == EXECUTOGRAM;
        let (effects, conditions) = if is_fact {
            let fact = fact_of(article, record).ok_or_else(|| {
                refuse(format!(
                    "is an instance of the fact '{}', which that article does not require",
                    record.fact.as_deref().unwrap_or("(none)")
                ))
            })?;
            // A fact holds fields only; `when` reads one of them.
            (fact.effects(), &record.fields)
        } else if article.get_submission().and_then(|s| s.submission_kind()) == Some(&record.kind) {
            return Ok(Vec::new());
        } else {
            let decision = article
                .get_decision()
                .filter(|d| d.legal_character() == Some(record.kind.as_str()))
                .ok_or_else(|| {
                    EngineError::InvalidOperation(format!(
                        "record '{}' is a {} under '{}', which that article does not produce",
                        record.id, record.kind, record.basis
                    ))
                })?;
            // `when` reads an outcome of the decision.
            (decision.effects(), &record.outputs)
        };
        let mut holding = Vec::new();
        for effect in effects {
            if let Some(when) = &effect.when {
                match conditions.get(when) {
                    Some(Value::Bool(true)) => {}
                    Some(Value::Bool(false)) => continue,
                    _ => {
                        return Err(refuse(format!(
                            "has an effect that depends on '{when}', but holds no boolean under it"
                        )))
                    }
                }
            }
            let target = if is_fact {
                record.refers_to.as_ref().ok_or_else(|| {
                    refuse("has an effect on the record it refers to, but refers to none".into())
                })?
            } else {
                let name = &effect.parameter;
                let Some(Value::String(target)) = record.fields.get(name) else {
                    return Err(refuse(format!(
                        "has an effect on '{name}', but holds no record id under it"
                    )));
                };
                target
            };
            let target = earlier.iter().find(|r| &r.id == target).ok_or_else(|| {
                refuse(format!(
                    "has an effect on '{target}', which is no record recorded before it"
                ))
            })?;
            holding.push(target.id.as_str());
        }
        Ok(holding)
    }
}

/// The fact `record` is an instance of, as its article requires it, when it
/// is an executogram.
fn fact_of<'a>(
    article: &'a Article,
    record: &ChronicleRecord,
) -> Option<&'a regelrecht_law_model::ArticleRequirement> {
    if record.kind != EXECUTOGRAM {
        return None;
    }
    article.required_fact(record.fact.as_deref()?)
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

/// Which versions of the laws a step names [`check_step`] holds it to.
#[derive(Clone, Copy)]
pub(crate) enum Versions {
    /// Those in force on a day, when the projection plays: a law without
    /// one is refused.
    InForce(NaiveDate),
    /// Those the loader holds: a law it does not hold yet is not checked,
    /// because the reduction and the law load in any order. A law that
    /// arrives later always loads, and the play checks the step against
    /// the versions in force.
    Loaded,
}

impl std::fmt::Display for Versions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InForce(day) => write!(f, "in force on {day}"),
            Self::Loaded => write!(f, "in the version loaded"),
        }
    }
}

/// Refuse a step that does not fit the laws it names, as `law_of` gives
/// them in `versions`. The step must say which records of its article it is
/// about: an article with more than one kind of record needs the kind
/// (`<article>#<KIND>`), the kind must be one the article gives, and a step
/// cannot pick one of several facts of an article. Each input must name an
/// output of an article whose reading of that output is pure
/// ([`refuse_impure`]), pass every parameter that article requires and only
/// parameters it declares, each from a field of the record (as its law
/// declares the record, [`field_type`]), a property of the state or a
/// parameter of the question, of the parameter's type, and have the type of
/// that output; each property the step sets must get a value of its type.
///
/// Three of these rules came with schema v0.8.15 and hold only for a
/// reduction on v0.8.15 or later (`since_v0_8_15`), so a reduction valid
/// under the released v0.8.14 still loads: the article must give a record,
/// `$record` reads only the fields of the kind of record the step is about,
/// and every required parameter the output reads must be passed. Under
/// v0.8.14 `$record` reads any parameter or output of the article.
pub(crate) fn check_step<'l>(
    projection: &Projection,
    step: &ProjectionStep,
    question: &[regelrecht_law_model::Parameter],
    law_of: &dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
    versions: Versions,
    since_v0_8_15: bool,
) -> std::result::Result<(), String> {
    // The article of `law_id` that `pick` finds: `Ok(None)` when the loader
    // does not hold that law yet, so there is nothing to check against.
    let article = |law_id: &str,
                   pick: &dyn Fn(&'l ArticleBasedLaw) -> Option<&'l Article>,
                   missing: &dyn Fn() -> String|
     -> std::result::Result<Option<&'l Article>, String> {
        match (law_of(law_id), versions) {
            (Some(law), _) => pick(law).map(Some).ok_or_else(missing),
            (None, Versions::Loaded) => Ok(None),
            (None, Versions::InForce(_)) => Err(missing()),
        }
    };
    let (basis, kind) = step.basis_and_kind();
    let (record_law, number) = basis.split_once('#').unwrap_or((basis, ""));
    let record_article = article(
        record_law,
        &|law| law.find_article_by_number(number),
        &|| format!("{basis} names no article {versions}"),
    )?;
    let record = match record_article {
        Some(article) => match record_kind(article, basis, kind)? {
            None if since_v0_8_15 => {
                return Err(
                    "the article gives no record, so no record rests on it to take a step for"
                        .to_string(),
                )
            }
            // Before v0.8.15 a step on such an article loads and never
            // applies, and `$record` reads the whole article.
            kind => Some((article, kind.filter(|_| since_v0_8_15))),
        },
        None => None,
    };
    // The type of what `source` reads: `None` when the law that would say
    // is not loaded yet, `Some(None)` when there is no such thing.
    let read_type = |source: &StepSource| -> Option<Option<ParameterType>> {
        match source {
            StepSource::Input(name) => Some(
                step.input
                    .iter()
                    .find(|i| &i.name == name)
                    .map(|i| i.input_type),
            ),
            StepSource::Record(field) => record
                .as_ref()
                .map(|(article, kind)| field_type(article, kind.as_deref(), field)),
            StepSource::State(property) => Some(state_type(projection, property)),
        }
    };
    for input in &step.input {
        let (regulation, output, parameters) = step_source(input)?;
        let Some(called) = article(
            regulation,
            &|law| law.find_article_by_output(output),
            &|| {
                format!(
                    "input '{}' takes '{output}' of {regulation}, which no article {versions} \
                     gives",
                    input.name
                )
            },
        )?
        else {
            continue;
        };
        refuse_impure(called, output)
            .map_err(|why| format!("input '{}' cannot call {regulation}: {why}", input.name))?;
        for (name, value) in &parameters {
            let Some(parameter) = called.get_parameters().iter().find(|p| &p.name == name) else {
                return Err(format!(
                    "input '{}' passes '{name}', which {regulation} article {} does not take",
                    input.name, called.number
                ));
            };
            let source = StepSource::parse(value)
                .ok_or_else(|| format!("input '{}' passes '{name}' as {value}", input.name))?;
            // In a parameter, `$<name>` is a parameter of the question.
            let given = match &source {
                StepSource::Input(asked) => Some(
                    question
                        .iter()
                        .find(|p| &p.name == asked)
                        .map(|p| p.param_type),
                ),
                other => read_type(other),
            };
            match given {
                None => {}
                Some(None) => {
                    return Err(format!(
                        "input '{}' passes {source} as '{name}', but {}",
                        input.name,
                        no_such(&source, true)
                    ))
                }
                Some(Some(t)) if t != parameter.param_type => {
                    return Err(format!(
                        "input '{}' passes {source}, a {t:?}, as '{name}', a {:?}",
                        input.name, parameter.param_type
                    ))
                }
                Some(Some(_)) => {}
            }
        }
        // A required parameter the output does not read, such as a field
        // a fact of the article holds, need not be passed.
        let reads = output_reads(called, output);
        if let Some(required) = called.get_parameters().iter().find(|p| {
            since_v0_8_15
                && p.required != Some(false)
                && reads.contains(&p.name)
                && !parameters.contains_key(&p.name)
        }) {
            return Err(format!(
                "input '{}' passes no '{}', which '{output}' of {regulation} article {} reads",
                input.name, required.name, called.number
            ));
        }
        let given = called
            .get_outputs()
            .iter()
            .find(|o| o.name == output)
            .map(|o| o.output_type);
        if let Some(t) = given.filter(|t| *t != input.input_type) {
            return Err(format!(
                "input '{}' is a {:?}, but '{output}' of {regulation} is a {t:?}",
                input.name, input.input_type
            ));
        }
    }
    for action in &step.actions {
        let (property, value) = step_action(action)?;
        let source = set_source(value)?;
        match (read_type(&source), state_type(projection, property)) {
            (Some(None), _) => {
                return Err(format!(
                    "sets '{property}' from {source}, but {}",
                    no_such(&source, false)
                ))
            }
            (Some(Some(t)), Some(want)) if t != want => {
                return Err(format!(
                    "sets '{property}' from {source}, a {t:?}, which is no {want:?}"
                ))
            }
            _ => {}
        }
    }
    Ok(())
}

/// Why a step cannot read `source`: there is no such field, property, input
/// or, in a parameter (`in_parameter`), parameter of the question.
fn no_such(source: &StepSource, in_parameter: bool) -> &'static str {
    match source {
        StepSource::Record(_) => "the record has no such field",
        StepSource::State(_) => "the state has no such property",
        StepSource::Input(_) if in_parameter => "the question has no such parameter",
        StepSource::Input(_) => "the step has no such input",
    }
}

/// The kind of record a step on `article` is about: the kind it names, or
/// the one kind the article gives (`None` when it gives none, so no record
/// rests on it). Refused when the article gives more than one kind and the
/// step names none, when it names a kind the article does not give, and
/// when it would have to pick one of several facts.
fn record_kind(
    article: &Article,
    basis: &str,
    kind: Option<&str>,
) -> std::result::Result<Option<String>, String> {
    let kinds = kinds_of(article);
    let mut distinct = kinds.clone();
    distinct.dedup();
    match kind {
        None if distinct.len() > 1 => {
            return Err(format!(
                "needs a kind: the article gives {}; write {basis}#{}",
                distinct.join(" and "),
                distinct.last().map_or("", String::as_str)
            ))
        }
        Some(kind) if !kinds.iter().any(|k| k == kind) => {
            return Err(format!(
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
            return Err(
                "the article requires more than one fact; a step cannot pick one".to_string(),
            )
        }
        _ => {}
    }
    Ok(kind.map(str::to_string).or_else(|| distinct.pop()))
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

/// The regulation, output and parameters an input of a step takes. A step
/// calls a law and nothing else: the loader refuses any other source.
fn step_source(
    input: &regelrecht_law_model::Input,
) -> std::result::Result<(&str, &str, std::collections::BTreeMap<String, String>), String> {
    let source = input.source.as_ref();
    match (
        source.and_then(|s| s.regulation.as_deref()),
        source.and_then(|s| s.output.as_deref()),
    ) {
        (Some(regulation), Some(output)) => Ok((
            regulation,
            output,
            source
                .and_then(|s| s.parameters.clone())
                .unwrap_or_default(),
        )),
        _ => Err(format!(
            "input '{}' names no regulation and output",
            input.name
        )),
    }
}

/// The property an action of a step gives a value, and that value.
fn step_action(
    action: &regelrecht_law_model::Action,
) -> std::result::Result<(&String, &regelrecht_law_model::ActionValue), String> {
    match (&action.output, &action.value) {
        (Some(property), Some(value)) => Ok((property, value)),
        _ => Err("has an action without an output and a value".to_string()),
    }
}

/// The source the value of an action of a step reads: a bare reference,
/// nothing else.
fn set_source(
    value: &regelrecht_law_model::ActionValue,
) -> std::result::Result<StepSource, String> {
    match value {
        regelrecht_law_model::ActionValue::Literal(Value::String(s)) => StepSource::parse(s),
        _ => None,
    }
    .ok_or_else(|| {
        "sets a value that is no $<input>, $record.<field> or $state.<property>".to_string()
    })
}

/// The type of property `name` of the projection's state.
fn state_type(projection: &Projection, name: &str) -> Option<ParameterType> {
    projection
        .state
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.property_type)
}

/// The type of field `name` of a record of `kind` under `article`, as the
/// law declares that record. A fact (`EXECUTOGRAM`) holds the parameters
/// the article requires it to hold. A decision holds the parameters of its
/// article, its outputs and the data derived for it; a submission the
/// parameters and outputs its item lists (in the object form of `produces`
/// every parameter and output). Without a kind (a reduction before schema
/// v0.8.15, [`check_step`]) every parameter and output counts.
fn field_type(article: &Article, kind: Option<&str>, name: &str) -> Option<ParameterType> {
    let parameter = || {
        article
            .get_parameters()
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.param_type)
    };
    let output = || {
        article
            .get_outputs()
            .iter()
            .find(|o| o.name == name)
            .map(|o| o.output_type)
    };
    let Some(kind) = kind else {
        return parameter().or_else(output);
    };
    if kind == EXECUTOGRAM {
        let holds = article
            .get_requires()
            .iter()
            .filter(|r| r.fact.is_some())
            .flat_map(|r| r.parameters.iter().flatten())
            .any(|p| p == name);
        return holds.then(parameter).flatten();
    }
    let item = article
        .get_produced()
        .into_iter()
        .find(|p| p.submission_kind().or(p.legal_character()) == Some(kind))?;
    let decision = item.legal_character().is_some();
    let in_parameters = decision || article.parameters_of(item).contains(&name);
    let in_outputs = article.outputs_of(item).contains(&name)
        || (decision && article.derived_outputs().contains(&name));
    in_parameters
        .then(parameter)
        .flatten()
        .or_else(|| in_outputs.then(output).flatten())
}

/// Every name `output` of `article` reads, through the actions that compute
/// it and the outputs of the article they read in turn
/// ([`crate::demand::required_outputs`]).
fn output_reads(article: &Article, output: &str) -> BTreeSet<String> {
    let actions = article
        .get_execution_spec()
        .and_then(|e| e.actions.as_deref())
        .unwrap_or_default();
    let needed = crate::demand::required_outputs(actions, &[output]);
    actions
        .iter()
        .filter(|a| a.output.as_ref().is_some_and(|o| needed.contains(o)))
        .flat_map(crate::demand::referenced_names)
        .collect()
}

/// Refuse an output a step cannot read: one that depends on anything but
/// the parameters the step passes. Purity is a matter of what the step
/// reads, not of the whole article, so the check follows `output` through
/// the actions of its article ([`crate::demand::required_outputs`]): the
/// output and every output it reads, transitively. None of them may read an
/// input of the article (with or without a `source`: data from outside the
/// record and the state, such as a data source or the answer of a cell) or
/// an open term (filled by another regulation). Another output of the same
/// article may read either. An article that plays a projection itself is
/// refused as a whole. `$referencedate` is the day of the record, the day
/// the step calls the law on, so it reads nothing from outside.
fn refuse_impure(article: &Article, output: &str) -> std::result::Result<(), String> {
    let Some(machine) = article.machine_readable.as_ref() else {
        return Err("the article has no machine_readable".to_string());
    };
    if machine.projection.is_some() {
        return Err("the article plays a projection itself".to_string());
    }
    let reads = output_reads(article, output);
    // An input named like an output still reaches that output's first
    // assignment (the pass-through `x: $x`), so a read of the name counts.
    if let Some(input) = article
        .get_inputs()
        .iter()
        .find(|i| reads.contains(&i.name))
    {
        return Err(format!(
            "'{output}' reads input '{}'; what a step reads depends only on what the step passes",
            input.name
        ));
    }
    if let Some(term) = article
        .get_open_terms()
        .into_iter()
        .flatten()
        .find(|t| reads.contains(&t.id))
    {
        return Err(format!(
            "'{output}' reads open term '{}', which another regulation fills",
            term.id
        ));
    }
    Ok(())
}

/// The state, its provenance and what became of each record.
struct Played {
    state: BTreeMap<String, Value>,
    set_by: BTreeMap<String, Option<String>>,
    events: Vec<ProjectionEvent>,
    /// When tracing: per applied record, by the index of its event, the
    /// traces of the laws its step called.
    nested: BTreeMap<usize, Vec<PathNode>>,
}

/// What a step did for one record: the articles it called
/// (`<regulation>#<article>`), the properties it set, and, when tracing,
/// the trace of each call.
#[derive(Default)]
struct AppliedStep {
    calls: Vec<String>,
    changes: Vec<StateChange>,
    traces: Vec<PathNode>,
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
    let leaf = |name: &str, message: String| {
        let mut child = PathNode::new(PathNodeType::Operation, name);
        child.message = Some(message);
        child
    };
    // Phase 1: whether each record counts.
    let mut counts = PathNode::new(PathNodeType::Operation, "COUNTS");
    // Phase 2: the fold over the records that count.
    let mut fold = PathNode::new(PathNodeType::Operation, "FOLD");
    for (index, event) in played.events.iter().enumerate() {
        match event {
            ProjectionEvent::Applied {
                record,
                article,
                changes,
            } => {
                counts
                    .children
                    .push(leaf("COUNT", format!("{record}: counts")));
                let set: BTreeMap<String, Value> = changes
                    .iter()
                    .map(|c| (c.property.clone(), c.to.clone()))
                    .collect();
                let mut child = leaf(
                    "APPLY",
                    format!(
                        "{record}: {}",
                        describe_applied(article.as_deref(), changes)
                    ),
                );
                child.result = Some(Value::Object(set));
                if let Some(traces) = played.nested.get(&index) {
                    child.children.extend(traces.iter().cloned());
                }
                fold.children.push(child);
            }
            ProjectionEvent::Skipped { record, undone_by } => counts.children.push(leaf(
                "SKIP",
                format!("{record}: revoked by {}", undone_by.join(", ")),
            )),
            ProjectionEvent::EffectsOnly { record, reason } => counts.children.push(leaf(
                "EFFECTS_ONLY",
                format!("{record}: counts for its effects only, {reason}"),
            )),
        }
    }
    node.children.push(counts);
    node.children.push(fold);
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
        None => describe_changes(changes),
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
    use crate::LawLoad;

    /// Article 1 grants (`verleend`), article 2 revokes `bestreden` when
    /// `herroepen`, article 3 takes a decision in the place of `bestreden`
    /// (it revokes it), article 4 requires the
    /// fact `ingeleverd`. Schema v0.8.15: article 7 requires the fact
    /// `vernietigd`, which revokes the record it refers to when it holds
    /// `vernietigd` true.
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
        output: [{name: kaart_weg, type: boolean}]
        actions: [{output: kaart_weg, value: false}]
  - number: '7'
    text: t
    machine_readable:
      requires:
        - fact: vernietigd
          parameters: [vernietigd]
          effects: [{revokes: refers_to, when: vernietigd}]
      execution:
        parameters: [{name: vernietigd, type: boolean}]
  - number: '8'
    text: t
    machine_readable:
      execution:
        produces:
          - type: BESCHIKKING
            effects: [{revokes: bestreden}]
        parameters: [{name: bestreden, type: decision}]
        output: [{name: herroepen, type: boolean}]
        actions: [{output: herroepen, value: true}]
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
            input:
              - name: kaart_na
                type: boolean
                source:
                  regulation: wet
                  output: kaart
                  parameters: {verleend: $record.verleend, aantal: $state.aantal}
              - name: aantal_na
                type: number
                source:
                  regulation: wet
                  output: aantal_na
                  parameters: {verleend: $record.verleend, aantal: $state.aantal}
            actions: [{output: kaart, value: $kaart_na}, {output: aantal, value: $aantal_na}]
          - record: wet#3
            actions: [{output: kaart, value: $record.verleend}]
          - record: wet#4
            input:
              - {name: weg, type: boolean, source: {regulation: wet, output: kaart_weg}}
            actions: [{output: kaart, value: $weg}]
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

    /// An instance of the fact `fact` under `article`, with `fields`.
    fn fact(id: &str, article: &str, fact: &str, fields: &[(&str, Value)]) -> ChronicleRecord {
        ChronicleRecord {
            kind: "EXECUTOGRAM".to_string(),
            fact: Some(fact.to_string()),
            ..record(id, article, fields, &[])
        }
    }

    /// A fact under article 7 that revokes `target` when `holds`.
    fn annulled(id: &str, target: &str, holds: bool) -> ChronicleRecord {
        ChronicleRecord {
            refers_to: Some(target.to_string()),
            ..fact(id, "7", "vernietigd", &[("vernietigd", Value::Bool(holds))])
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

    fn skipped(outcome: &ProjectionOutcome) -> Vec<&str> {
        outcome
            .events
            .iter()
            .filter_map(|e| match e {
                ProjectionEvent::Skipped { record, .. } => Some(record.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_revoked_record_does_not_count() {
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
        assert_eq!(skipped(&outcome), ["a"]);
        assert!(outcome.events.contains(&ProjectionEvent::Skipped {
            record: "a".to_string(),
            undone_by: vec!["h".to_string()],
        }));
    }

    #[test]
    fn an_effect_whose_outcome_is_false_revokes_nothing() {
        let outcome = play(&[grant("a"), revoke("h", "a", false)]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert!(skipped(&outcome).is_empty());
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
        assert_eq!(skipped(&outcome), ["h1"]);
        assert!(outcome.events.contains(&ProjectionEvent::Skipped {
            record: "h1".to_string(),
            undone_by: vec!["h2".to_string()],
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
        assert_eq!(skipped(&outcome), ["a", "h1"]);
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
        assert!(outcome.events.contains(&ProjectionEvent::Skipped {
            record: "a".to_string(),
            undone_by: vec!["n".to_string()],
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
        assert_eq!(skipped(&outcome), ["n1"]);
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
        assert_eq!(skipped(&outcome), ["a", "n2"]);
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
    fn a_fact_revokes_when_what_it_holds_says_so() {
        let outcome = play(&[grant("a"), annulled("v", "a", true)]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(
            outcome.events[0],
            ProjectionEvent::Skipped {
                record: "a".to_string(),
                undone_by: vec!["v".to_string()],
            }
        );
        let outcome = play(&[grant("a"), annulled("v", "a", false)]).unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
        assert!(skipped(&outcome).is_empty());
    }

    #[test]
    fn revoking_a_fact_revives_what_it_revoked() {
        // A decision revokes the fact; a fact revokes a fact.
        for undo in [revoke("h", "v", true), annulled("w", "v", true)] {
            let outcome = play(&[grant("a"), annulled("v", "a", true), undo]).unwrap();
            assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));
            assert_eq!(skipped(&outcome), ["v"]);
        }
    }

    #[test]
    fn a_fact_needs_a_boolean_condition_and_a_record_it_refers_to() {
        let referring = |fields: &[(&str, Value)], to: Option<&str>| ChronicleRecord {
            refers_to: to.map(str::to_string),
            ..fact("v", "7", "vernietigd", fields)
        };
        for (record, why) in [
            (referring(&[], Some("a")), "holds no boolean under it"),
            (
                referring(&[("vernietigd", Value::String("ja".into()))], Some("a")),
                "holds no boolean under it",
            ),
            (
                referring(&[("vernietigd", Value::Bool(true))], None),
                "has an effect on the record it refers to, but refers to none",
            ),
        ] {
            let err = play(&[grant("a"), record]).expect_err(why);
            assert!(
                err.to_string().contains("record 'v' (wet#7) has an effect"),
                "{err}"
            );
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
        // A fact its article does not require is refused, rather than read
        // as a fact without effects.
        let err = play(&[grant("a"), fact("v", "7", "anders", &[])]).expect_err("no such fact");
        assert!(
            err.to_string().contains(
                "is an instance of the fact 'anders', which that article does not require"
            ),
            "{err}"
        );
    }

    #[test]
    fn a_decision_revokes_through_a_parameter_of_type_decision_by_its_id() {
        // Article 8 names the decision it revokes in a parameter of type
        // `decision`, as Awb 7:11 names `bestreden_besluit`. The cell keeps
        // such a decision by its id, so the record holds the id.
        let revocation = record("h", "8", &[("bestreden", Value::String("a".into()))], &[]);
        let outcome = play(&[grant("a"), revocation]).unwrap();
        assert_eq!(skipped(&outcome), ["a"]);
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(false));
    }

    #[test]
    fn an_effect_on_a_record_that_is_not_earlier_is_refused() {
        for record in [annulled("s", "x", true), revoke("s", "x", true)] {
            let err = play(&[grant("a"), record]).expect_err("x is no record");
            assert!(
                err.to_string()
                    .contains("has an effect on 'x', which is no record recorded before it"),
                "{err}"
            );
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
        assert_eq!(skipped(&outcome), ["h1", "h3", "h5", "h7"]);
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
        assert_eq!(
            outcome.events[0],
            ProjectionEvent::Skipped {
                record: "a".to_string(),
                undone_by: vec!["h2".to_string()],
            }
        );
    }

    #[test]
    fn a_record_whose_article_cannot_be_found_is_refused() {
        let unknown = record("x", "99", &[], &[]);
        let err = play(&[grant("a"), unknown]).expect_err("no article 99");
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
                    .replace(
                        "actions: [{output: kaart, value: $kaart_na}, {output: aantal, value: $aantal_na}]",
                        "actions: [{output: kaart, value: $kaart_na}]",
                    ),
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

    /// The definition, checked without the engine's pass: a record counts
    /// unless a later record that itself counts revokes it, by a decision or
    /// a fact (schema v0.8.15), always or by its outcome. For each of
    /// many generated chronicles the oracle tries every assignment of
    /// "counts" to the records, keeps those that satisfy the definition,
    /// finds exactly one, and the engine must skip exactly the records that
    /// one says do not count, and fold the grants that count.
    #[test]
    fn the_records_that_count_are_the_one_assignment_the_definition_allows() {
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
            // (target, holds) per record; None for a grant. An effect is a
            // decision's or a fact's, and holds always or by its outcome.
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
                match next(3) {
                    0 => {
                        let holds = next(4) != 0;
                        records.push(revoke(&id, &target_id, holds));
                        effect_of.push(Some((target, holds)));
                    }
                    1 => {
                        records.push(replace(&id, &target_id));
                        effect_of.push(Some((target, true)));
                    }
                    _ => {
                        let holds = next(4) != 0;
                        records.push(annulled(&id, &target_id, holds));
                        effect_of.push(Some((target, holds)));
                    }
                }
            }
            let satisfies = |counts: &[bool]| {
                (0..n).all(|i| {
                    counts[i]
                        != (0..n).any(|j| j > i && counts[j] && effect_of[j] == Some((i, true)))
                })
            };
            let allowed: Vec<Vec<bool>> = (0u32..1 << n)
                .map(|bits| (0..n).map(|i| bits & (1 << i) != 0).collect::<Vec<bool>>())
                .filter(|counts| satisfies(counts))
                .collect();
            assert_eq!(allowed.len(), 1, "one assignment: {records:#?}");
            let counts = &allowed[0];
            let expected: Vec<String> = (0..n)
                .filter(|&i| !counts[i])
                .map(|i| format!("r{i}"))
                .collect();
            let outcome = play(&records).unwrap();
            assert_eq!(skipped(&outcome), expected, "{records:#?}");
            // The fold: every grant that counts adds one.
            let grants = (0..n)
                .filter(|&i| counts[i] && effect_of[i].is_none())
                .count();
            assert_eq!(
                outcome.result.outputs["aantal"],
                Value::Int(grants as i64),
                "{records:#?}"
            );
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
    fn the_trace_shows_which_records_count_and_then_the_fold() {
        let outcome = service()
            .evaluate_projection(
                "reductie",
                &["heeft_kaart"],
                &[
                    grant("a"),
                    revoke("h1", "a", true),
                    revoke("h2", "h1", true),
                ],
                BTreeMap::new(),
                "2025-03-01",
                true,
            )
            .unwrap();
        let root = outcome.result.trace.expect("a trace");
        let node = &root.children[0];
        let phases: Vec<&str> = node.children.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(phases, ["COUNTS", "FOLD"]);
        let messages = |n: &PathNode| -> Vec<String> {
            n.children
                .iter()
                .filter_map(|c| c.message.clone())
                .collect()
        };
        // A revoked revocation revokes nothing, so the grant counts again.
        assert_eq!(
            messages(&node.children[0]),
            ["a: counts", "h1: revoked by h2", "h2: counts"]
        );
        assert_eq!(
            messages(&node.children[1]),
            ["a: wet#5: aantal: 0 -> 1, kaart: false -> true", "h2: -"]
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

    /// The records applied or skipped, in order.
    fn records_in(outcome: &ProjectionOutcome) -> Vec<&str> {
        outcome
            .events
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
        assert!(x.events.contains(&ProjectionEvent::Skipped {
            record: "a".to_string(),
            undone_by: vec!["n".to_string()],
        }));
        assert_eq!(skipped(&x), ["a"]);
        assert_eq!(x.result.outputs["heeft_kaart"], Value::Bool(false));
        assert_eq!(x.result.outputs["aantal"], Value::Int(0));
        // For y, `n` is its own record, and revokes a record outside y's
        // partition: that effect is not y's business.
        let y = play_for("y", &records).unwrap();
        assert_eq!(records_in(&y), ["n"]);
        assert!(skipped(&y).is_empty());
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
            let reduction = REDUCTION
                .replace("$id: reductie", "$id: reductie_soort")
                .replace(
                    "          - record: wet#1#BESCHIKKING\n",
                    &format!("          - record: {steps}\n"),
                );
            let mut service = service();
            service.load_law(&reduction)?;
            service.evaluate_projection(
                "reductie_soort",
                &["heeft_kaart"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
        };
        // Article 1 gives an application and a decision. The loader
        // refuses the step when the reduction loads after the law.
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
            let law = LAW.replace(
                "        - fact: ingeleverd\n          parameters: [op]\n",
                "        - fact: ingeleverd\n          parameters: [op]\n        - fact: verloren\n          parameters: [op]\n",
            );
            let reduction = REDUCTION.replace("record: wet#4\n", &format!("record: {step}\n"));
            for err in refused_in_either_order(&law, &reduction) {
                assert!(
                    err.contains("the article requires more than one fact; a step cannot pick one"),
                    "{step}: {err}"
                );
            }
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
                &[grant("a"), revoke("h", "a", false)],
                BTreeMap::new(),
                "2025-03-01",
                true,
            )
            .unwrap();
        let root = outcome.result.trace.expect("a trace");
        let node = &root.children[0];
        assert_eq!(node.name, "PROJECTION");
        let names =
            |n: &PathNode| -> Vec<String> { n.children.iter().map(|c| c.name.clone()).collect() };
        assert_eq!(names(node), ["COUNTS", "FOLD"]);
        assert_eq!(names(&node.children[0]), ["COUNT", "COUNT"]);
        let fold = &node.children[1];
        assert_eq!(names(fold), ["APPLY", "APPLY"]);
        assert_eq!(
            fold.children[0].message.as_deref(),
            Some("a: wet#5: aantal: 0 -> 1, kaart: false -> true")
        );
        // The laws the step called hang under its node, as any law call
        // does in a trace.
        assert!(
            !fold.children[0].children.is_empty(),
            "the calls of the step under APPLY: {:?}",
            fold.children[0]
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

    /// A step calls laws and nothing else: each input must name an output
    /// of a law in force on the day of the record, pass parameters that law
    /// takes, of the right type, from the record or the state, and the law
    /// may read nothing else; each property gets a value of its type.
    #[test]
    fn a_step_must_fit_the_laws_it_calls() {
        let parameters = "parameters: {verleend: $record.verleend, aantal: $state.aantal}";
        let with = |a: &str, b: &str| REDUCTION.replace(a, b);
        for (law, reduction, why) in [
            (
                LAW.to_string(),
                with("output: kaart\n", "output: geen\n"),
                "input 'kaart_na' takes 'geen' of wet, which no article",
            ),
            (
                LAW.to_string(),
                with(parameters, "parameters: {verleend: $record.verleend, aantal: $state.aantal, ander: $state.kaart}"),
                "input 'kaart_na' passes 'ander', which wet article 5 does not take",
            ),
            (
                LAW.to_string(),
                with(parameters, "parameters: {verleend: $state.aantal, aantal: $state.aantal}"),
                "input 'kaart_na' passes $state.aantal, a Number, as 'verleend', a Boolean",
            ),
            (
                LAW.to_string(),
                // A parameter of the question, of the wrong type.
                with(parameters, "parameters: {verleend: $record.verleend, aantal: $peil}").replace(
                    "      execution:\n        output: [{name: heeft_kaart",
                    "      execution:\n        parameters: [{name: peil, type: date}, {name: n, type: number}]\n        output: [{name: heeft_kaart",
                ),
                "input 'kaart_na' passes $peil, a Date, as 'aantal', a Number",
            ),
            (
                LAW.to_string(),
                with(parameters, "parameters: {verleend: $record.bestaat_niet, aantal: $state.aantal}"),
                "input 'kaart_na' passes $record.bestaat_niet as 'verleend', but the record has no such field",
            ),
            (
                // `kaart` does not read `aantal`, so 'kaart_na' need not pass
                // it; `aantal_na` does.
                LAW.to_string(),
                with(parameters, "parameters: {verleend: $record.verleend}"),
                "input 'aantal_na' passes no 'aantal', which 'aantal_na' of wet article 5 reads",
            ),
            (
                // `verleend` is an output of the decision, not of the
                // application of the same article.
                LAW.to_string(),
                with("record: wet#1#BESCHIKKING", "record: wet#1#AANVRAAG"),
                "input 'kaart_na' passes $record.verleend as 'verleend', but the record has no such field",
            ),
            (
                // A fact holds the parameters the article requires it to
                // hold, not every parameter of the article.
                LAW.replace(
                    "        parameters: [{name: op, type: date}]\n",
                    "        parameters: [{name: op, type: date}, {name: los, type: boolean}]\n",
                ),
                with("actions: [{output: kaart, value: $weg}]", "actions: [{output: kaart, value: $record.los}]"),
                "sets 'kaart' from $record.los, but the record has no such field",
            ),
            (
                LAW.to_string(),
                with("              - name: kaart_na\n                type: boolean", "              - name: kaart_na\n                type: number")
                    .replace(
                        "actions: [{output: kaart, value: $kaart_na}, {output: aantal, value: $aantal_na}]",
                        "actions: [{output: aantal, value: $kaart_na}]",
                    ),
                "input 'kaart_na' is a Number, but 'kaart' of wet is a Boolean",
            ),
            (
                LAW.to_string(),
                with("actions: [{output: kaart, value: $kaart_na}, {output: aantal, value: $aantal_na}]", "actions: [{output: kaart, value: $aantal_na}]"),
                "sets 'kaart' from $aantal_na, a Number, which is no Boolean",
            ),
            (
                LAW.to_string(),
                with("actions: [{output: kaart, value: $record.verleend}]", "actions: [{output: kaart, value: $record.bestaat_niet}]"),
                "sets 'kaart' from $record.bestaat_niet, but the record has no such field",
            ),
        ] {
            for err in refused_in_either_order(&law, &reduction) {
                assert!(err.contains(why), "{why}: {err}");
            }
        }
    }

    /// Three rules on a step came with schema v0.8.15: the article gives a
    /// record, `$record` reads only the fields of its kind, and every
    /// required parameter the output reads is passed. A reduction that
    /// declares v0.8.14, which the released schema allows, still loads in
    /// either order and plays; from v0.8.15 it is refused.
    #[test]
    fn the_rules_of_v0_8_15_leave_a_reduction_on_v0_8_14_alone() {
        let on = |reduction: &str, v: &str| {
            reduction.replacen(
                "$id: reductie",
                &format!(
                    "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{v}/schema/{v}/schema.json\n$id: reductie"
                ),
                1,
            )
        };
        let with = |a: &str, b: &str| REDUCTION.replace(a, b);
        for (reduction, why) in [
            (
                with(
                    "          - record: wet#3\n",
                    "          - record: wet#6\n            actions: [{output: kaart, value: $record.kaart_weg}]\n          - record: wet#3\n",
                ),
                "on wet#6: the article gives no record",
            ),
            (
                with(
                    "record: wet#1#BESCHIKKING",
                    "record: wet#1#AANVRAAG",
                ),
                "input 'kaart_na' passes $record.verleend as 'verleend', but the record has no such field",
            ),
            (
                with(
                    "parameters: {verleend: $record.verleend, aantal: $state.aantal}",
                    "parameters: {verleend: $record.verleend}",
                ),
                "input 'aantal_na' passes no 'aantal'",
            ),
        ] {
            for err in refused_in_either_order(LAW, &on(&reduction, "v0.8.15")) {
                assert!(err.contains(why), "{why}: {err}");
            }
            let old = on(&reduction, "v0.8.14");
            for law_first in [true, false] {
                let mut service = LawExecutionService::new();
                let order = if law_first { [LAW, &old] } else { [&old, LAW] };
                for law in order {
                    service.load_law(law).expect("valid under v0.8.14");
                }
                service
                    .evaluate_projection(
                        "reductie",
                        &["heeft_kaart"],
                        &[],
                        BTreeMap::new(),
                        "2025-03-01",
                        false,
                    )
                    .expect("plays under v0.8.14");
            }
        }
    }

    /// The errors when `law` loads before `reduction`, and when it loads
    /// after it. Loaded after the law, the reduction is refused. Loaded
    /// before it, the reduction loads, and so does the law: a law never
    /// depends on a reduction that reads it. The play then refuses the
    /// reduction. A step that does not fit its own reduction, such as a
    /// value of the wrong type for a property, is refused before any law
    /// loads.
    fn refused_in_either_order(law: &str, reduction: &str) -> [String; 2] {
        let mut service = LawExecutionService::new();
        service.load_law(law).unwrap();
        let after = service
            .load_law(reduction)
            .expect_err(reduction)
            .to_string();
        let mut service = LawExecutionService::new();
        let before = match service.load_law(reduction) {
            Err(alone) => alone.to_string(),
            Ok(_) => {
                service
                    .load_law(law)
                    .expect("the law loads after the reduction");
                service
                    .evaluate_projection(
                        "reductie",
                        &["heeft_kaart"],
                        &[],
                        BTreeMap::new(),
                        "2025-03-01",
                        false,
                    )
                    .expect_err(reduction)
                    .to_string()
            }
        };
        [after, before]
    }

    /// The play holds a step to the laws in force on the day asked about,
    /// also where the loader had nothing to check it against.
    #[test]
    fn the_play_refuses_a_step_on_a_law_not_in_force() {
        let mut service = LawExecutionService::new();
        service.load_law(REDUCTION).unwrap();
        let err = service
            .evaluate_projection(
                "reductie",
                &["heeft_kaart"],
                &[],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .expect_err("no law");
        assert!(
            err.to_string()
                .contains("wet#1 names no article in force on 2025-03-01"),
            "{err}"
        );
    }

    /// A law with one output a step may read and outputs it may not: one
    /// reads an input, one reads that output in turn, and one reads an open
    /// term.
    const MIXED: &str = r#"
$id: gemengd
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      open_terms:
        - {id: drempel, type: boolean, required: false}
      execution:
        parameters: [{name: verleend, type: boolean}]
        input:
          - {name: buiten, type: boolean, source: {regulation: elders, output: iets}}
        output:
          - {name: zuiver, type: boolean}
          - {name: van_buiten, type: boolean}
          - {name: via_buiten, type: boolean}
          - {name: met_drempel, type: boolean}
        actions:
          - {output: zuiver, value: $verleend}
          - {output: van_buiten, value: $buiten}
          - output: via_buiten
            value: {operation: AND, conditions: [$verleend, $van_buiten]}
          - output: met_drempel
            value: {operation: EQUALS, subject: $drempel, value: null}
"#;

    /// Purity is about the output a step reads, not the whole article: a
    /// step reads the pure output of an article whose other outputs read
    /// an input or an open term, and is refused one that reads either,
    /// also through another output.
    #[test]
    fn a_step_may_read_only_a_pure_output() {
        let reduction = |output: &str| {
            format!(
                r#"
$id: reductie
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      projection:
        state: [{{name: kaart, type: boolean}}]
        steps:
          - record: wet#1#BESCHIKKING
            input:
              - name: na
                type: boolean
                source:
                  regulation: gemengd
                  output: {output}
                  parameters: {{verleend: $record.verleend}}
            actions: [{{output: kaart, value: $na}}]
      execution:
        output: [{{name: heeft_kaart, type: boolean}}]
        actions: [{{output: heeft_kaart, value: $state.kaart}}]
"#
            )
        };
        let mut service = LawExecutionService::new();
        service.load_law(LAW).unwrap();
        service.load_law(MIXED).unwrap();
        service.load_law(&reduction("zuiver")).unwrap();
        // The input of the article is never resolved: `elders` is not even
        // loaded.
        let outcome = service
            .evaluate_projection(
                "reductie",
                &["heeft_kaart"],
                &[grant("a")],
                BTreeMap::new(),
                "2025-03-01",
                false,
            )
            .unwrap();
        assert_eq!(outcome.result.outputs["heeft_kaart"], Value::Bool(true));

        for (output, why) in [
            ("van_buiten", "'van_buiten' reads input 'buiten'"),
            ("via_buiten", "'via_buiten' reads input 'buiten'"),
            (
                "met_drempel",
                "'met_drempel' reads open term 'drempel', which another regulation fills",
            ),
        ] {
            let mut service = LawExecutionService::new();
            service.load_law(LAW).unwrap();
            service.load_law(MIXED).unwrap();
            let err = service
                .load_law(&reduction(output))
                .expect_err(output)
                .to_string();
            assert!(
                err.contains(&format!("input 'na' cannot call gemengd: {why}")),
                "{output}: {err}"
            );
        }
    }

    /// A law that arrives after a reduction that reads it always loads,
    /// whether the step names it as the law its records rest on or as a law
    /// an input calls. The play refuses the reduction that no longer fits.
    #[test]
    fn a_law_that_arrives_later_loads_and_the_play_refuses_the_reduction() {
        let reduction = |output: &str, field: &str| {
            format!(
                r#"
$id: reductie
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      projection:
        state: [{{name: kaart, type: boolean}}]
        steps:
          - record: wet#1#BESCHIKKING
            input:
              - name: na
                type: boolean
                source:
                  regulation: gemengd
                  output: {output}
                  parameters: {{verleend: $record.{field}}}
            actions: [{{output: kaart, value: $na}}]
      execution:
        output: [{{name: heeft_kaart, type: boolean}}]
        actions: [{{output: heeft_kaart, value: $state.kaart}}]
"#
            )
        };
        let play = |service: &LawExecutionService| {
            service
                .evaluate_projection(
                    "reductie",
                    &["heeft_kaart"],
                    &[],
                    BTreeMap::new(),
                    "2025-03-01",
                    false,
                )
                .expect_err("a step that does not fit")
                .to_string()
        };
        // Only an input names the law that arrives.
        let mut service = LawExecutionService::new();
        service.load_law(LAW).unwrap();
        service
            .load_law(&reduction("van_buiten", "verleend"))
            .unwrap();
        service.load_law(MIXED).unwrap();
        let err = play(&service);
        assert!(err.contains("'van_buiten' reads input 'buiten'"), "{err}");
        // Only the record names the law that arrives.
        let mut service = LawExecutionService::new();
        service.load_law(MIXED).unwrap();
        service
            .load_law(&reduction("zuiver", "bestaat_niet"))
            .unwrap();
        service.load_law(LAW).unwrap();
        let err = play(&service);
        assert!(err.contains("the record has no such field"), "{err}");
    }

    /// What a step reads that is no field, property, input or parameter of
    /// the question, refused with what it is not. The loader refuses these
    /// on the shape of the step already, so this calls the check directly.
    #[test]
    fn a_step_names_what_it_cannot_read() {
        let law = ArticleBasedLaw::from_yaml_str(LAW).unwrap();
        let check = |reduction: &str| {
            let reduction = ArticleBasedLaw::from_yaml_str(reduction).unwrap();
            let article = &reduction.articles[0];
            let projection = projection_of("reductie", article).unwrap();
            let law_of = |id: &str| (id == "wet").then_some(&law);
            projection
                .steps
                .iter()
                .map(|step| {
                    check_step(
                        projection,
                        step,
                        article.get_parameters(),
                        &law_of,
                        Versions::Loaded,
                        true,
                    )
                })
                .find_map(std::result::Result::err)
                .unwrap_or_default()
        };
        let parameters = "parameters: {verleend: $record.verleend, aantal: $state.aantal}";
        for (from, to, why) in [
            (
                parameters,
                "parameters: {verleend: $record.verleend, aantal: $onbekend}",
                "passes $onbekend as 'aantal', but the question has no such parameter",
            ),
            (
                parameters,
                "parameters: {verleend: $record.verleend, aantal: $state.onbekend}",
                "passes $state.onbekend as 'aantal', but the state has no such property",
            ),
            (
                "actions: [{output: kaart, value: $record.verleend}]",
                "actions: [{output: kaart, value: $geen_invoer}]",
                "sets 'kaart' from $geen_invoer, but the step has no such input",
            ),
        ] {
            let err = check(&REDUCTION.replace(from, to));
            assert!(err.contains(why), "{why}: {err}");
        }
    }

    /// The fields of a record are those the law gives its kind.
    #[test]
    fn a_record_holds_the_fields_its_kind_is_given() {
        let law = ArticleBasedLaw::from_yaml_str(
            r#"
$id: soorten
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      requires:
        - fact: gemeld
          parameters: [p]
      execution:
        produces:
          - {type: AANVRAAG, requests: 'soorten#1', parameters: [p], outputs: [a]}
          - {type: BESCHIKKING, outputs: [b], derived: [c]}
        parameters: [{name: p, type: boolean}, {name: q, type: number}]
        output:
          - {name: a, type: boolean}
          - {name: b, type: boolean}
          - {name: c, type: number}
        actions:
          - {output: a, value: $p}
          - {output: b, value: $p}
          - {output: c, value: $q}
"#,
        )
        .unwrap();
        let article = &law.articles[0];
        let fields = |kind: Option<&str>| -> Vec<&str> {
            ["p", "q", "a", "b", "c"]
                .into_iter()
                .filter(|name| field_type(article, kind, name).is_some())
                .collect()
        };
        assert_eq!(fields(Some("AANVRAAG")), ["p", "a"]);
        assert_eq!(fields(Some("BESCHIKKING")), ["p", "q", "b", "c"]);
        assert_eq!(fields(Some(EXECUTOGRAM)), ["p"]);
        assert_eq!(fields(None), ["p", "q", "a", "b", "c"]);
        assert_eq!(
            field_type(article, Some("BESCHIKKING"), "c"),
            Some(ParameterType::Number)
        );
    }
}
