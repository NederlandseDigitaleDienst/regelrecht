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
        let (projection, key_type, question) = {
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
            (projection, key_type, question)
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
        self.check_steps(law_id, &projection, &question, day)?;
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

    /// Refuse a step that does not say which records of its article it is
    /// about: an article with more than one kind of record needs the kind
    /// (`<article>#<KIND>`), the kind must be one the article gives, and a
    /// step cannot pick one of several facts of an article. The article is
    /// the one in force on `day`. This runs when the projection is played,
    /// because the reduction and the law are separate regelwerken that load
    /// in any order.
    fn check_steps(
        &self,
        law_id: &str,
        projection: &Projection,
        question: &[regelrecht_law_model::Parameter],
        day: NaiveDate,
    ) -> Result<()> {
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
            self.check_wiring(projection, step, article, question, day)
                .or_else(refuse)?;
        }
        Ok(())
    }

    /// Refuse a step that does not fit the laws it calls, in their versions
    /// in force on `day`: each input must name an output of an article a step
    /// can call ([`refuse_impure`]), pass only parameters that article
    /// declares, from a field of the record or a property of the state of
    /// the same type, and have the type of that output; each property the
    /// step sets must get a value of its type.
    fn check_wiring(
        &self,
        projection: &Projection,
        step: &ProjectionStep,
        record_article: &Article,
        question: &[regelrecht_law_model::Parameter],
        day: NaiveDate,
    ) -> std::result::Result<(), String> {
        let read_type = |source: &StepSource| match source {
            StepSource::Input(name) => step
                .input
                .iter()
                .find(|i| &i.name == name)
                .map(|i| i.input_type),
            StepSource::Record(field) => field_type(record_article, field),
            StepSource::State(property) => state_type(projection, property),
        };
        for input in &step.input {
            let (regulation, output, parameters) = step_source(input)?;
            let Some((_, article)) = self.called_article(regulation, output, day) else {
                return Err(format!(
                    "input '{}' takes '{output}' of {regulation}, which no article in force on \
                     {day} gives",
                    input.name
                ));
            };
            refuse_impure(article)
                .map_err(|why| format!("input '{}' cannot call {regulation}: {why}", input.name))?;
            for (name, value) in &parameters {
                let Some(parameter) = article.get_parameters().iter().find(|p| &p.name == name)
                else {
                    return Err(format!(
                        "input '{}' passes '{name}', which {regulation} article {} does not take",
                        input.name, article.number
                    ));
                };
                let source = StepSource::parse(value)
                    .ok_or_else(|| format!("input '{}' passes '{name}' as {value}", input.name))?;
                // In a parameter, `$<name>` is a parameter of the question.
                let given = match &source {
                    StepSource::Input(asked) => question
                        .iter()
                        .find(|p| &p.name == asked)
                        .map(|p| p.param_type),
                    other => read_type(other),
                };
                match given {
                    None => {
                        return Err(format!(
                            "input '{}' passes {source} as '{name}', but the record has no such \
                             field",
                            input.name
                        ))
                    }
                    Some(t) if t != parameter.param_type => {
                        return Err(format!(
                            "input '{}' passes {source}, a {t:?}, as '{name}', a {:?}",
                            input.name, parameter.param_type
                        ))
                    }
                    Some(_) => {}
                }
            }
            let given = article
                .get_outputs()
                .iter()
                .find(|o| o.name == output)
                .map(|o| o.output_type);
            if given.is_some_and(|t| t != input.input_type) {
                return Err(format!(
                    "input '{}' is a {:?}, but '{output}' of {regulation} is a {:?}",
                    input.name,
                    input.input_type,
                    given.unwrap_or(input.input_type)
                ));
            }
        }
        for action in &step.actions {
            let (property, value) = step_action(action)?;
            let source = set_source(value)?;
            let given = read_type(&source);
            match (given, state_type(projection, property)) {
                (None, _) => {
                    return Err(format!(
                        "sets '{property}' from {source}, but the record has no such field"
                    ))
                }
                (Some(t), Some(want)) if t != want => {
                    return Err(format!(
                        "sets '{property}' from {source}, a {t:?}, which is no {want:?}"
                    ))
                }
                _ => {}
            }
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
            refuse_impure(article).map_err(|why| {
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
    /// the fields that an effect of its article names: an effect of its
    /// decision, or for a fact, an effect of that fact (schema v0.8.15).
    /// That holds for every record under the article, so an objection that
    /// names the contested decision in such a field refers to it, as the
    /// decision on it does.
    fn references(&self, record: &ChronicleRecord) -> Result<Vec<String>> {
        let mut ids: Vec<String> = record.refers_to.iter().cloned().collect();
        let article = self.article_of(record)?;
        // A fact refers only through its own effects; an executogram of a
        // fact its article does not require is refused when its effects
        // are read, so it refers through none here.
        let effects = if record.kind == EXECUTOGRAM {
            fact_of(article, record)
                .map(|f| f.effects())
                .unwrap_or_default()
        } else {
            article
                .get_decision()
                .map(|d| d.effects())
                .unwrap_or_default()
        };
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
    /// what the record itself holds.
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
        let (effects, conditions) = if record.kind == EXECUTOGRAM {
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
            // The record it names is checked first, so a broken record is
            // refused whatever `when` says.
            let name = &effect.parameter;
            let Some(Value::String(target)) = record.fields.get(name) else {
                return Err(refuse(format!(
                    "has an effect on '{name}', but holds no record id under it"
                )));
            };
            let target = earlier.iter().find(|r| &r.id == target).ok_or_else(|| {
                refuse(format!(
                    "has an effect on '{target}', which is no record recorded before it"
                ))
            })?;
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

/// Refuse an article a step cannot call: one that reads anything but the
/// parameters the step passes. An input (with or without a `source`) is
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
            "the article reads input '{}'; a law a step calls reads only what the step passes",
            input.name
        ));
    }
    if article.get_outputs().is_empty() {
        return Err("the article has no outputs".to_string());
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

    /// Article 1 grants (`verleend`), article 2 revokes `bestreden` when
    /// `herroepen`, article 3 takes a decision in the place of `bestreden`
    /// (it revokes it), article 4 requires the
    /// fact `ingeleverd`. Schema v0.8.15: article 7 requires the fact
    /// `vernietigd`, which revokes `besluit` when it holds `vernietigd` true.
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
          parameters: [besluit, vernietigd]
          effects: [{revokes: besluit, when: vernietigd}]
      execution:
        parameters: [{name: besluit, type: string}, {name: vernietigd, type: boolean}]
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
        fact(
            id,
            "7",
            "vernietigd",
            &[
                ("besluit", Value::String(target.to_string())),
                ("vernietigd", Value::Bool(holds)),
            ],
        )
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
    fn a_fact_needs_a_boolean_condition_and_a_record_id() {
        for (fields, why) in [
            (
                vec![("besluit", Value::String("a".into()))],
                "holds no boolean under it",
            ),
            (
                vec![
                    ("besluit", Value::String("a".into())),
                    ("vernietigd", Value::String("ja".into())),
                ],
                "holds no boolean under it",
            ),
            (
                vec![("vernietigd", Value::Bool(true))],
                "holds no record id under it",
            ),
        ] {
            let err = play(&[grant("a"), fact("v", "7", "vernietigd", &fields)]).expect_err(why);
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
    fn a_broken_effect_is_refused_also_when_its_condition_is_false() {
        // `when` is false, yet a target that is no earlier record is refused
        // all the same.
        for record in [annulled("s", "x", false), revoke("s", "x", false)] {
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
        let parameters = "parameters: {verleend: $record.verleend, aantal: $state.aantal}";
        let with = |a: &str, b: &str| REDUCTION.replace(a, b);
        for (law, reduction, why) in [
            (
                LAW.to_string(),
                with("output: kaart\n", "output: geen\n"),
                "input 'kaart_na' takes 'geen' of wet, which no article in force on 2025-03-01 gives",
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
                LAW.to_string(),
                with("              - name: kaart_na\n                type: boolean", "              - name: kaart_na\n                type: number"),
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
            (
                LAW.replace(
                    "        parameters: [{name: verleend, type: boolean}, {name: aantal, type: number}]\n",
                    "        parameters: [{name: verleend, type: boolean}, {name: aantal, type: number}]\n        input:\n          - {name: buiten, type: boolean, source: {regulation: elders, output: iets}}\n",
                ),
                REDUCTION.to_string(),
                "input 'kaart_na' cannot call wet: the article reads input 'buiten'",
            ),
            (
                LAW.replace(
                    "    machine_readable:\n      execution:\n        parameters: [{name: verleend, type: boolean}, {name: aantal",
                    "    machine_readable:\n      open_terms:\n        - {id: drempel, type: number, required: false}\n      execution:\n        parameters: [{name: verleend, type: boolean}, {name: aantal",
                ),
                REDUCTION.to_string(),
                "input 'kaart_na' cannot call wet: the article has open terms",
            ),
        ] {
            let err = ask(&law, &reduction);
            assert!(err.contains(why), "{why}: {err}");
        }
    }
}
