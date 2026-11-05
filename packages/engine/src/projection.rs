//! A reduction that plays a cell's chronicle forward (schema v0.8.14).
//!
//! The reduction declares a state (`projection.state`) and, per article a
//! record rests on, what such a record does with it (`projection.steps`).
//! The engine plays the records the cell knew on the calculation date in the
//! order the cell recorded them. Each step is pure: the next state follows
//! from the state before and the record alone. One version of the reduction,
//! the one in force on the calculation date, gives the initial state, every
//! step and the answer: the reduction is the cell's reading, not law. The
//! reduction's `execution` answers from the last state, reading each
//! property as `$state.<name>`.
//!
//! What a record does with an earlier record is law, not part of the
//! reduction: the article of a decision says it revokes or replaces another
//! decision (`produces[].effects`, optionally only `when` an outcome is true),
//! under the version of that law in force on the day the record was
//! recorded. The engine carries that out by rewinding. When an effect of a
//! record that counts undoes an earlier record that was applied, the engine
//! plays the chronicle again from the start and skips the undone record. A
//! record is skipped for as long as at least one record that undoes it
//! counts. A record that is itself revoked or replaced undoes nothing any
//! more, so a record it alone undid counts again and the engine rewinds once
//! more; a replacement that should keep an undoing in force declares that
//! effect itself. The chronicle is never changed. Nothing is reverted
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
use regelrecht_law_model::{Article, EffectKind, Projection};
use serde::{Deserialize, Serialize};

use crate::context::RuleContext;
use crate::engine::ArticleResult;
use crate::error::{EngineError, Result};
use crate::operations::evaluate_value;
use crate::service::LawExecutionService;
use crate::trace::PathNode;
use crate::types::{PathNodeType, Value};

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
    pub fields: BTreeMap<String, Value>,
    pub outputs: BTreeMap<String, Value>,
}

/// What caused a rewind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewindCause {
    /// A record revokes an earlier one that was applied.
    Revokes,
    /// A record replaces an earlier one that was applied.
    Replaces,
    /// A record that undid an earlier one is itself revoked now, and no other
    /// record undoes the earlier one any more: it counts again.
    Revives,
}

impl From<EffectKind> for RewindCause {
    fn from(kind: EffectKind) -> Self {
        match kind {
            EffectKind::Revokes => Self::Revokes,
            EffectKind::Replaces => Self::Replaces,
        }
    }
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
    /// The record was played: the properties its step set (none when the
    /// reduction gives no step for it).
    Applied {
        record: String,
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
        mut parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        trace: bool,
    ) -> Result<ProjectionOutcome> {
        let day = parse_day(calculation_date)?;
        let Some(first) = output_names.first() else {
            return Err(EngineError::InvalidOperation("no output asked".to_string()));
        };
        let projection = {
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
            projection_of(law_id, article)?.clone()
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
        let played = self.play(law_id, &projection, &known)?;

        parameters.insert(STATE.to_string(), Value::Object(played.state.clone()));
        let mut result = if trace {
            self.evaluate_law_with_trace(law_id, output_names, parameters, calculation_date)?
        } else {
            self.evaluate_law(law_id, output_names, parameters, calculation_date)?
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
            let mut state: BTreeMap<String, Value> = projection
                .state
                .iter()
                .map(|p| (p.name.clone(), p.initial.clone()))
                .collect();
            let mut set_by: BTreeMap<String, Option<String>> = projection
                .state
                .iter()
                .map(|p| (p.name.clone(), None))
                .collect();
            let mut applied: BTreeSet<&str> = BTreeSet::new();
            let mut rewind = None;

            'records: for (i, record) in records.iter().enumerate() {
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
                    // again. A replacer that should keep an undoing in force
                    // says so itself, with an effect of its own.
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

                for (kind, target) in self.holding_effects(record, &records[..i])? {
                    let by = undone_by.entry(target).or_default();
                    let first_undoer = by.is_empty();
                    by.entry(record.id.as_str()).or_insert(kind);
                    if first_undoer && applied.contains(target) {
                        rewind = Some(ProjectionEvent::Rewound {
                            record: record.id.clone(),
                            cause: kind.into(),
                            target: target.to_string(),
                        });
                        break 'records;
                    }
                }

                let changes = step(law_id, projection, record, &state)?;
                for change in &changes {
                    state.insert(change.property.clone(), change.to.clone());
                    set_by.insert(change.property.clone(), Some(record.id.clone()));
                }
                events.push(ProjectionEvent::Applied {
                    record: record.id.clone(),
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
                        *by = now.keys().map(|id| id.to_string()).collect();
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

    /// The effects of `record` that hold, each with the earlier record it
    /// undoes. A record has effects only as a decision of its article
    /// (`produces[].effects`), under the law of the day it was recorded.
    fn holding_effects<'r>(
        &self,
        record: &ChronicleRecord,
        earlier: &'r [ChronicleRecord],
    ) -> Result<Vec<(EffectKind, &'r str)>> {
        // A record whose article cannot be found could undo another one
        // unseen; that is refused rather than read as "no effects".
        let unresolved = || {
            EngineError::InvalidOperation(format!(
                "record '{}' rests on '{}', which names no article in force on {}",
                record.id, record.basis, record.recorded_on
            ))
        };
        let (law_id, number) = record.basis.split_once('#').ok_or_else(unresolved)?;
        let article = self
            .resolver()
            .get_law_for_date(law_id, Some(record.recorded_on))
            .and_then(|law| law.find_article_by_number(number))
            .ok_or_else(unresolved)?;
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
            holding.push((effect.kind, target.id.as_str()));
        }
        Ok(holding)
    }
}

/// The state, its provenance and the events after the last replay.
struct Played {
    state: BTreeMap<String, Value>,
    set_by: BTreeMap<String, Option<String>>,
    events: Vec<ProjectionEvent>,
}

/// The properties the step for `record` sets, each computed from the state
/// before it and the record. None when the reduction gives no step for it.
/// One version of the reduction, the one in force on the calculation date,
/// plays the whole chronicle: the reduction is the cell's reading, not law.
fn step(
    law_id: &str,
    projection: &Projection,
    record: &ChronicleRecord,
    state: &BTreeMap<String, Value>,
) -> Result<Vec<StateChange>> {
    let Some(step) = projection.steps.iter().find(|s| {
        s.record == record.basis
            && s.record_type.as_ref().is_none_or(|t| t == &record.kind)
            && s.fact
                .as_ref()
                .is_none_or(|f| Some(f) == record.fact.as_ref())
    }) else {
        return Ok(Vec::new());
    };

    // The record's fields, then its outputs, then `id`, `basis` and `type`;
    // a later one wins over a field of the same name.
    let mut row = record.fields.clone();
    row.extend(record.outputs.clone());
    row.insert("id".to_string(), Value::String(record.id.clone()));
    row.insert("basis".to_string(), Value::String(record.basis.clone()));
    row.insert("type".to_string(), Value::String(record.kind.clone()));
    let parameters = BTreeMap::from([
        (STATE.to_string(), Value::Object(state.clone())),
        ("record".to_string(), Value::Object(row)),
    ]);
    let context = RuleContext::new(parameters, &record.recorded_on.to_string())?;

    let mut changes = Vec::new();
    for (property, value) in &step.set {
        let to = evaluate_value(value, &context, 0).map_err(|e| {
            EngineError::InvalidOperation(format!(
                "the step of '{law_id}' for record '{}' ({}) sets '{property}': {e}",
                record.id, record.basis
            ))
        })?;
        let from = state.get(property).cloned().unwrap_or(Value::Null);
        changes.push(StateChange {
            property: property.clone(),
            from,
            to,
        });
    }
    Ok(changes)
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
            ProjectionEvent::Applied { record, changes } => {
                let set: BTreeMap<String, Value> = changes
                    .iter()
                    .map(|c| (c.property.clone(), c.to.clone()))
                    .collect();
                (
                    "APPLY",
                    format!("{record}: {}", describe_changes(changes)),
                    Some(Value::Object(set)),
                )
            }
            ProjectionEvent::Skipped { record, undone_by } => (
                "SKIP",
                format!("{record}: undone by {}", undone_by.join(", ")),
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

/// The word for a rewind's cause, as the trace shows it.
pub fn cause_word(cause: RewindCause) -> &'static str {
    match cause {
        RewindCause::Revokes => "revokes",
        RewindCause::Replaces => "replaces",
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
          - {name: kaart, type: boolean, initial: false}
          - {name: aantal, type: number, initial: 0}
        steps:
          - record: wet#1
            type: BESCHIKKING
            set:
              kaart: $record.verleend
              aantal: {operation: ADD, values: [$state.aantal, 1]}
          - record: wet#3
            set: {kaart: $record.verleend}
          - record: wet#4
            type: EXECUTOGRAM
            fact: ingeleverd
            set: {kaart: false}
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
            cause: RewindCause::Replaces,
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
    fn a_step_matches_the_kind_and_the_fact() {
        // A submission under article 1 has no step: only its decisions do.
        let submission = ChronicleRecord {
            kind: "AANVRAAG".to_string(),
            ..record("s", "1", &[], &[])
        };
        let other_fact = ChronicleRecord {
            fact: Some("iets_anders".to_string()),
            ..handed_in("x")
        };
        let outcome = play(&[grant("a"), submission, other_fact]).unwrap();
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
                    .replace(
                        "aantal: {operation: ADD, values: [$state.aantal, 1]}",
                        "aantal: $state.aantal",
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
            Some("a: aantal: 0 -> 1, kaart: false -> true")
        );
    }
}
