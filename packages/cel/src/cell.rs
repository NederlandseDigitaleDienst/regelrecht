//! A cell: it keeps a chronicle and records submissions and the decisions
//! on them in it.
//!
//! A submission may refer to a decision in the chronicle (`refers_to`). A
//! field of type `decision` then holds that decision, and a field that the
//! decision holds by name comes from it too (RFC-050). The chronolexogram
//! keeps a decision by its id; the engine gets its decision value
//! `{basis, taken_on, fields, findings, outputs}`, where `taken_on`
//! is the article of the submission it was taken on. A decision output of
//! the deciding article is a decision the cell records as well, in the same
//! append. The cell knows no article that does this; the law names it with
//! the type.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::NaiveDate;
use regelrecht_engine::{
    ArticleResult, Authority, Call, FilledBy, LawExecutionService, OpenTermAnswerer,
    OutputProvenance, ParameterType, Value,
};
use serde_json::Map;

use crate::chronicle::{Afschrift, Chronicle, Gram, GramMoment, Received, EXECUTOGRAM};
use crate::decision::{self, finding_field, DecisionShape, Source};
use crate::error::{refused, setup, Result};
use crate::files::{Disk, Files};
use crate::identity;
use crate::reading::{self, Asked, Reader, Term};
use crate::shape::{self, Field, Shape};
use regelrecht_law_model::stored;

/// A cell. It holds who it is, its catalogue of answers and its chronicle,
/// not the law: every call that needs the law gets the service that
/// executes it, and the reductions the catalogue names are rulework in that
/// service (RFC-048).
///
/// A cell is opened with its directory, which holds `cel.yaml` and its own
/// reductions (`reductions`, [`crate::identity`]). The cell reads which
/// reductions it keeps from there; the service loads them.
#[derive(Debug)]
pub struct Cell {
    recording_actor: String,
    /// Who the cell is (`cel.yaml`, RFC-052).
    authority: Authority,
    /// Per answer the cell gives, the `$id` of its reduction (`cel.yaml`,
    /// `answers`, RFC-048).
    answers: BTreeMap<String, String>,
    /// The `$id`s of the reductions the cell keeps (`reductions`).
    reductions: BTreeSet<String>,
    chronicle: Chronicle,
}

/// The decision a submission refers to: one in the cell's own chronicle, by
/// its id, or one of another cell, by the copy that cell made of it
/// ([`Cell::afschrift`], RFC-053).
#[derive(Debug, Clone, Copy)]
pub enum Reference<'a> {
    Local(&'a str),
    Received(&'a Afschrift),
}

impl Cell {
    /// The cell of `recording_actor`, with its chronicle in the JSON Lines file
    /// at `path` and its identity and catalogue in its directory `cell_dir`
    /// (`cel.yaml`) and its reductions (`reductions`). A `cell_dir` that is no
    /// directory, or that is the directory of the reductions of no cell
    /// ([`identity::REDUCTIONS`] beside the cells), is an error, and so is an
    /// answer of a reduction the cell does not keep ([`identity::read`]). A
    /// chronolexogram in the chronicle recorded by another actor is an error,
    /// and so is an executogram that refers to a chronolexogram or holds
    /// outputs (RFC-051).
    pub fn open(
        recording_actor: impl Into<String>,
        path: impl AsRef<Path>,
        cell_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        Self::with_chronicle(
            &Disk,
            recording_actor.into(),
            cell_dir.as_ref(),
            Chronicle::open(path)?,
        )
    }

    /// The cell of `recording_actor`, with an empty chronicle in memory and
    /// its identity read from its directory `cell_dir`, as [`Cell::open`]
    /// reads it.
    pub fn in_memory(
        recording_actor: impl Into<String>,
        cell_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        Self::in_memory_from(&Disk, recording_actor, cell_dir)
    }

    /// [`Cell::in_memory`], with the cell directory read through `files`,
    /// such as a [`crate::Bundle`] a browser holds.
    pub fn in_memory_from(
        files: &dyn Files,
        recording_actor: impl Into<String>,
        cell_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        Self::with_chronicle(
            files,
            recording_actor.into(),
            cell_dir.as_ref(),
            Chronicle::in_memory(),
        )
    }

    /// The cell as [`Cell::in_memory_from`] opens it, with `grams` in its
    /// chronicle in memory: a chronicle kept elsewhere and read back, such
    /// as a browser keeps it across a reload. The chronolexograms are
    /// checked as [`Cell::open`] checks the lines of its file.
    pub fn restore(
        files: &dyn Files,
        recording_actor: impl Into<String>,
        cell_dir: impl AsRef<Path>,
        grams: Vec<Gram>,
    ) -> Result<Self> {
        Self::with_chronicle(
            files,
            recording_actor.into(),
            cell_dir.as_ref(),
            Chronicle::in_memory_with(grams)?,
        )
    }

    /// The cell with `chronicle` and its identity read from `cell_dir`
    /// through `files`, after the checks of [`Cell::open`] on the directory
    /// and the chronicle.
    fn with_chronicle(
        files: &dyn Files,
        recording_actor: String,
        cell_dir: &Path,
        chronicle: Chronicle,
    ) -> Result<Self> {
        if !files.is_dir(cell_dir) {
            return Err(setup(format!(
                "{}: no directory of the cell of '{recording_actor}'",
                cell_dir.display()
            )));
        }
        if identity::is_shared_reductions(cell_dir) {
            return Err(setup(format!(
                "{}: no cell; this directory holds the reductions of no cell, which every cell \
                 applies to its own chronicle",
                cell_dir.display()
            )));
        }
        let identity::Identity {
            authority,
            answers,
            reductions,
        } = identity::read(files, cell_dir)?;
        if let Some(g) = chronicle
            .grams()
            .iter()
            .find(|g| g.recording_actor != recording_actor)
        {
            return Err(setup(format!(
                "chronolexogram '{}' in the chronicle of '{recording_actor}' was recorded by '{}'",
                g.id, g.recording_actor
            )));
        }
        for (position, gram) in chronicle.grams().iter().enumerate() {
            check_executogram(gram, &recording_actor)?;
            let before = &chronicle.grams()[..position];
            // A decision taken again names a chronolexogram before it that
            // refers to the last decision on the same submission.
            if let Some(after) = gram.after.as_deref() {
                let last = gram
                    .refers_to
                    .as_deref()
                    .and_then(|on| decisions_on(before, on).last().copied());
                let later = before.iter().find(|g| g.id == after);
                let follows = later.zip(last).is_some_and(|(later, last)| {
                    later.refers_to.as_deref() == Some(last.id.as_str())
                });
                if !follows {
                    return Err(setup(format!(
                        "chronolexogram '{}' in the chronicle of '{recording_actor}' is taken \
                         after '{after}', which is not before it or does not refer to the last \
                         decision on its submission",
                        gram.id
                    )));
                }
            }
            // The receipt of a copy refers to a chronolexogram before it that
            // is no executogram. Whether that is a decision needs the law,
            // which `Cell::record_receipt` has and opening does not.
            if let (Some(_), Some(on)) = (gram.received.as_ref(), gram.refers_to.as_deref()) {
                if !before.iter().any(|g| g.id == on && g.kind != EXECUTOGRAM) {
                    return Err(setup(format!(
                        "the receipt '{}' in the chronicle of '{recording_actor}' refers to \
                         '{on}', which is not a chronolexogram before it or is an executogram",
                        gram.id
                    )));
                }
            }
        }
        Ok(Self {
            recording_actor,
            authority,
            answers,
            reductions,
            chronicle,
        })
    }

    /// The cell as the engine's answerer for a decision that reads `grams`
    /// ([`Reader`]), explaining the term `explain` if one is given.
    fn reader<'c>(&'c self, grams: Vec<Gram>, explain: Option<&'c str>) -> Reader<'c> {
        Reader::new(
            &self.recording_actor,
            &self.authority,
            &self.answers,
            &self.reductions,
            grams,
            explain,
        )
    }

    /// The answers the cell gives: per answer the `$id` of the reduction
    /// whose output it is (`cel.yaml`, `answers`, RFC-048).
    pub fn answers(&self) -> &BTreeMap<String, String> {
        &self.answers
    }

    /// The `$id`s of the reductions the cell keeps (`reductions`).
    pub fn reductions(&self) -> &BTreeSet<String> {
        &self.reductions
    }

    /// The open terms a policy in force on `on` in `service` binds to an
    /// answer of this cell ([`reading::bound_terms`]).
    pub fn bound_terms(&self, service: &LawExecutionService, on: NaiveDate) -> Vec<Term> {
        reading::bound_terms(service, &self.authority, &self.answers, on)
    }

    pub fn recording_actor(&self) -> &str {
        &self.recording_actor
    }

    /// Who the cell is, from its `cel.yaml` (RFC-052).
    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    /// The cell as another cell of the sphere answers a decision on `on`
    /// (RFC-052): from what it recorded up to and including that day. The
    /// decision keeps only the day, not a position in the other cell's
    /// chronicle; a replay asks it as of that day (RFC-050: a source says
    /// what it knew on a given day). A record it makes later, dated up to
    /// that day, enters the replay; RFC-052 says when that fails it.
    fn known_on(&self, on: NaiveDate) -> Reader<'_> {
        let grams = self
            .grams()
            .iter()
            .filter(|g| g.recorded_on <= on)
            .cloned()
            .collect();
        self.reader(grams, None)
    }

    pub fn grams(&self) -> &[Gram] {
        self.chronicle.grams()
    }

    /// A copy of this cell with its chronicle in memory, to try a decision
    /// on: what is recorded in the copy is not kept in this cell or its file.
    pub fn sandbox(&self) -> Self {
        Self {
            recording_actor: self.recording_actor.clone(),
            authority: self.authority.clone(),
            answers: self.answers.clone(),
            reductions: self.reductions.clone(),
            chronicle: self.chronicle.in_memory_copy(),
        }
    }

    /// Record the submission that article `article` of `law_id` establishes,
    /// received on `on`, with the values the applicant and the intake channel
    /// supplied in `supplied`, referring to the decision `reference` if it is
    /// given: one in this chronicle, or the copy of a decision of another
    /// cell (RFC-053).
    ///
    /// The cell refuses a field the law does not ask, the moment that counts
    /// (the cell fills it), and a value of the wrong type. A field may be
    /// left out. It takes `on` as the moment, passes it to the engine,
    /// executes the article (so the hooks on the submission run) and
    /// appends one chronolexogram.
    ///
    /// What the cell takes from the decision it refers to ([`Cell::referred`]):
    /// the field of type `decision`, of which a submission has at most one,
    /// and every other field that decision holds by name. The submitter
    /// supplies neither. Without a reference, a `decision` field is left
    /// out. The cell refuses a reference to what is no decision in its
    /// chronicle.
    ///
    /// A copy of a decision of another cell fills the `decision` field with
    /// the copy's decision value itself, not an id: this cell cannot replay
    /// that decision, and keeps what it received. The other fields come from
    /// the copied chronolexogram by name, as from a decision of its own. The
    /// chronolexogram holds `received` instead of `refers_to`. The cell
    /// refuses a copy of its own decision, and a copy of what is no decision.
    #[allow(clippy::too_many_arguments)]
    pub fn record_submission(
        &mut self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        law_id: &str,
        article: &str,
        reference: Option<Reference<'_>>,
        supplied: &Map<String, serde_json::Value>,
        on: NaiveDate,
    ) -> Result<Gram> {
        let shape = shape::derive(service, law_id, article, on)?;
        let (filled, from_referred) = self.referred(service, &shape, reference)?;
        check_supplied(&shape, supplied, &from_referred)?;
        self.record(service, sphere, &shape, supplied, filled, on)
    }

    /// What a submission of `shape` takes from the decision `refers_to`:
    /// its field of type `decision`, which holds that decision's id, and
    /// every other field that decision holds by name or, where an override
    /// voided it, gives (such as the end of a period that a hook on the
    /// decision computes). Returns what the cell fills, and the names the
    /// submitter may not supply: these, and the `decision` field also without
    /// `refers_to`.
    fn referred(
        &self,
        service: &LawExecutionService,
        shape: &Shape,
        reference: Option<Reference<'_>>,
    ) -> Result<(Filled, Vec<String>)> {
        let decisions: Vec<String> = shape
            .fields
            .iter()
            .filter(|f| f.type_ == ParameterType::Decision)
            .map(|f| f.name.clone())
            .collect();
        if decisions.len() > 1 {
            return Err(setup(format!(
                "the {} of {} holds {} decisions ({}); the cell fills one, from the \
                 chronolexogram it refers to",
                shape.kind,
                shape.basis(),
                decisions.len(),
                decisions.join(", ")
            )));
        }
        let (referred, mut filled, id) = match reference {
            None => return Ok((Filled::default(), decisions)),
            Some(Reference::Local(id)) => {
                let referred = self.decision_gram(service, id)?;
                let mut filled = Filled {
                    refers_to: Some(id.to_string()),
                    ..Filled::default()
                };
                if let Some(name) = decisions.first() {
                    filled
                        .fields
                        .insert(name.clone(), serde_json::Value::String(id.to_string()));
                }
                (referred, filled, id)
            }
            Some(Reference::Received(copy)) => {
                let referred = self.received_decision(service, copy)?;
                let mut filled = Filled {
                    received: Some(Received {
                        from: copy.from.clone(),
                        id: referred.id.clone(),
                    }),
                    ..Filled::default()
                };
                if let Some(name) = decisions.first() {
                    filled.fields.insert(name.clone(), copy.value.clone());
                }
                (referred, filled, referred.id.as_str())
            }
        };
        // What the decision took and gave, in the law it was taken under.
        // The terms this cell answers count only for a decision of its own:
        // for a copy they are the other cell's, which this cell does not
        // hold.
        let taken_on = referred.recorded_on;
        let referred_shape = decision_shape_of(service, referred, taken_on)?;
        let gives = gives(service, &referred_shape, taken_on);
        let mut from_referred = decisions.clone();
        for field in shape.fields.iter().filter(|f| !decisions.contains(&f.name)) {
            if let Some(value) = referred.held(&field.name) {
                // Coupled by name, so checked as a supplied value is: the
                // two articles may mean another thing by it.
                if !shape::fits(field, value) {
                    return Err(setup(format!(
                        "'{}' of the {} of {} comes from decision '{id}' under {}, which holds \
                         {value}; that does not fit a {}{:?} ({})",
                        field.name,
                        shape.kind,
                        shape.basis(),
                        referred.basis,
                        if field.nullable { "" } else { "non-nullable " },
                        field.type_,
                        field.first_declared_by()
                    )));
                }
                filled.fields.insert(field.name.clone(), value.clone());
            } else if !gives.contains(&field.name) {
                continue;
            }
            from_referred.push(field.name.clone());
        }
        Ok((filled, from_referred))
    }

    /// The decision `id` in this chronicle: a chronolexogram on a submission
    /// whose article, in the law in force on the day it was recorded, takes
    /// a decision of its kind.
    fn decision_gram(&self, service: &LawExecutionService, id: &str) -> Result<&Gram> {
        let gram = self.grams().iter().find(|g| g.id == id).ok_or_else(|| {
            refused(format!(
                "there is no chronolexogram '{id}' in the chronicle of '{}'",
                self.recording_actor
            ))
        })?;
        check_decision(service, gram)?;
        Ok(gram)
    }

    /// The decision a copy of another cell is of ([`Reference::Received`]):
    /// a decision as the law in force on its day takes it, made by the cell
    /// that recorded it, and not one of this cell.
    fn received_decision<'c>(
        &self,
        service: &LawExecutionService,
        copy: &'c Afschrift,
    ) -> Result<&'c Gram> {
        let gram = &copy.gram;
        if copy.from == self.recording_actor || gram.recording_actor != copy.from {
            return Err(refused(format!(
                "the copy of '{}' is made by '{}' of a chronolexogram recorded by '{}'; the cell \
                 of '{}' takes a copy of a decision that another cell recorded and copied",
                gram.id, copy.from, gram.recording_actor, self.recording_actor
            )));
        }
        check_decision(service, gram)?;
        let basis = copy.value.get("basis").and_then(serde_json::Value::as_str);
        if basis != Some(gram.basis.as_str()) {
            return Err(refused(format!(
                "the copy of '{}' holds a decision value under {basis:?}, not under {}",
                gram.id, gram.basis
            )));
        }
        Ok(gram)
    }

    /// Record a submission of `shape` on `on`: `supplied` from the
    /// submitter, already checked, and `filled` from the cell.
    fn record(
        &mut self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        shape: &Shape,
        supplied: &Map<String, serde_json::Value>,
        filled: Filled,
        on: NaiveDate,
    ) -> Result<Gram> {
        let law_id = shape.law_id.as_str();
        let article = shape.article.as_str();
        // A field the application leaves out and that can be absent is absent:
        // the application is what the applicant sent (RFC-036), and the
        // chronolexogram says so with `null`. One that cannot be absent stays
        // unknown to the engine, and the chronolexogram names it as missing: an
        // incomplete application is still an application (Awb 4:5).
        let mut fields = BTreeMap::new();
        let mut missing = Vec::new();
        for field in &shape.fields {
            let value = match filled.fields.get(&field.name).or(supplied.get(&field.name)) {
                Some(value) => value.clone(),
                None if field.nullable => serde_json::Value::Null,
                None => {
                    missing.push(field.name.clone());
                    continue;
                }
            };
            fields.insert(field.name.clone(), value);
        }
        let moment = shape.moment.as_ref().map(|m| GramMoment {
            parameter: m.parameter.clone(),
            on,
            declared_by: m.declared_by.clone(),
        });

        // The engine gets the fields and the moment.
        let day = json_date(on);
        let mut parameters: BTreeMap<String, Value> = fields
            .iter()
            .map(|(k, v)| Ok((k.clone(), stored::from_stored(v)?)))
            .collect::<Result<_>>()?;
        if let Some(name) = moment.as_ref().and_then(|m| m.parameter.as_ref()) {
            parameters.insert(name.clone(), day.clone());
        }
        // A decision the submission holds by its id reaches the engine as
        // its decision value.
        for field in shape
            .fields
            .iter()
            .filter(|f| f.type_ == ParameterType::Decision)
        {
            if let Some(serde_json::Value::String(id)) = fields.get(&field.name) {
                let gram = self.decision_gram(service, id)?.clone();
                let value = self.decision_value(service, sphere, &gram)?;
                parameters.insert(field.name.clone(), stored::from_stored(&value)?);
            }
        }
        let date = on.to_string();
        let result = match &shape.stage {
            // In a procedure, the application is recorded at its own stage,
            // with the outputs that belong to it and the hooks on it; the
            // decision's outputs wait for the stages of the decision
            // (RFC-047). A date the stage requires is the day of recording.
            Some(stage) => {
                let requires = service
                    .procedure_stage(law_id, article, stage, &date)?
                    .requires;
                for req in requires {
                    if req.req_type != ParameterType::Date {
                        return Err(setup(format!(
                            "stage {stage} of {law_id}#{article} requires '{}', a {:?}; the cell \
                             fills only a date (the day of recording)",
                            req.name, req.req_type
                        )));
                    }
                    parameters.entry(req.name).or_insert_with(|| day.clone());
                }
                service.execute_stage_at(law_id, article, stage, None, parameters, &date)?
            }
            None => {
                let outputs: Vec<&str> = shape.outputs.iter().map(String::as_str).collect();
                service.evaluate_law(law_id, &outputs, parameters, &date)?
            }
        };
        let mut output_values = BTreeMap::new();
        collect_outputs(&result, &mut output_values)?;

        let gram = Gram {
            id: uuid::Uuid::now_v7().to_string(),
            kind: shape.kind.clone(),
            basis: shape.basis(),
            fact: None,
            refers_to: filled.refers_to,
            received: filled.received,
            after: None,
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment,
            fields,
            missing,
            outputs: output_values,
        };
        Ok(self.chronicle.append(gram)?.clone())
    }

    /// Record an executogram on `on` (RFC-051): an instance of the fact
    /// `fact`, `<regulation>#<article>#<fact>`, that the article requires
    /// (`requires`, schema v0.8.7), such as the receipt of the report of a
    /// medical examination, with the values `supplied` for the parameters the
    /// fact holds. No regulation runs for it; the article must be in force on
    /// `on` and require that fact.
    ///
    /// The cell refuses a fact the article does not require, a field
    /// the fact does not hold, a value of the wrong type, and a field that
    /// cannot be absent and is left out: there is no submission whose
    /// incompleteness a law speaks of, so what is recorded is whole or not
    /// recorded. A field that can be absent and is left out is `null`. Where
    /// the fact names who establishes it (`competent_authority`, RFC-052),
    /// the cell records it only when that names the cell.
    pub fn record_executogram(
        &mut self,
        service: &LawExecutionService,
        fact: &str,
        supplied: &Map<String, serde_json::Value>,
        on: NaiveDate,
    ) -> Result<Gram> {
        let declared = shape::fact_fields(service, fact, on, Some(&self.authority))?;
        let what = format!("executogram of {fact}");
        let fields = whole(&declared, supplied, &what)?;
        let (basis, name) = shape::split_fact(fact)?;
        let gram = Gram {
            id: uuid::Uuid::now_v7().to_string(),
            kind: EXECUTOGRAM.to_string(),
            basis: basis.to_string(),
            fact: Some(name.to_string()),
            refers_to: None,
            received: None,
            after: None,
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment: None,
            fields,
            missing: Vec::new(),
            outputs: BTreeMap::new(),
        };
        Ok(self.chronicle.append(gram)?.clone())
    }

    /// A copy of the decision `id` for another cell (RFC-053): the
    /// chronolexogram, its decision value from a replay in this cell
    /// ([`Cell::decision_value`]), and what the submission it was taken on
    /// had received from another cell. Only the cell that took a decision
    /// can build its value; the replay asks the cells of `sphere` as the
    /// decision did. Refused for what is no decision.
    pub fn afschrift(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        id: &str,
    ) -> Result<Afschrift> {
        let gram = self.decision_gram(service, id)?.clone();
        let value = self.nested_value(service, sphere, &gram)?;
        let about = gram
            .refers_to
            .as_deref()
            .and_then(|submission| self.grams().iter().find(|g| g.id == submission))
            .and_then(|submission| submission.received.clone());
        Ok(Afschrift {
            from: self.recording_actor.clone(),
            gram,
            value,
            about,
        })
    }

    /// The decision value of `gram` for a copy: as [`Cell::decision_value`],
    /// with every decision a field of type `decision` holds by its id
    /// replaced by that decision's value, nested in the same way (RFC-053).
    /// Each replay asks the cells of `sphere`.
    /// The receiving cell cannot look an id up, and a court that calls the
    /// decision anew needs the decision it contested.
    fn nested_value(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        gram: &Gram,
    ) -> Result<serde_json::Value> {
        let mut value = self.decision_value(service, sphere, gram)?;
        let typed: Vec<String> = gram
            .basis
            .split_once('#')
            .and_then(|(law_id, article)| {
                shape::article_on(service, law_id, article, gram.recorded_on)
            })
            .and_then(|article| article.get_execution_spec())
            .and_then(|execution| execution.parameters.as_ref())
            .into_iter()
            .flatten()
            .filter(|p| p.param_type == ParameterType::Decision)
            .map(|p| p.name.clone())
            .collect();
        for name in typed {
            let Some(serde_json::Value::String(id)) =
                value.get("fields").and_then(|f| f.get(&name)).cloned()
            else {
                continue;
            };
            let held = self.decision_gram(service, &id)?.clone();
            let nested = self.nested_value(service, sphere, &held)?;
            if let Some(fields) = value
                .get_mut("fields")
                .and_then(serde_json::Value::as_object_mut)
            {
                fields.insert(name, nested);
            }
        }
        Ok(value)
    }

    /// Record the receipt of `copy`, a copy of a decision of another cell
    /// that concerns the decision `refers_to` of this cell, as an executogram
    /// of the fact `fact` on `on` (RFC-053): the cell records that it
    /// received the copy, with the parameters the fact holds taken by name
    /// from the copied chronolexogram. The executogram refers to the
    /// decision it concerns and holds what it received; that is what this
    /// adds to [`Cell::record_executogram`].
    ///
    /// Refused, besides what [`Cell::record_executogram`] refuses: a copy of
    /// what is no decision or that this cell made, a `refers_to` that is no
    /// decision of this cell, a copy that does not concern it (its `about`
    /// names another decision), a receipt dated before the copied decision,
    /// and a copy that lacks a field the fact cannot do without.
    pub fn record_receipt(
        &mut self,
        service: &LawExecutionService,
        fact: &str,
        copy: &Afschrift,
        refers_to: &str,
        on: NaiveDate,
    ) -> Result<Gram> {
        let received = self.received_decision(service, copy)?;
        self.decision_gram(service, refers_to)?;
        let concerns = Received {
            from: self.recording_actor.clone(),
            id: refers_to.to_string(),
        };
        if copy.about.as_ref() != Some(&concerns) {
            return Err(refused(format!(
                "the copy of '{}' concerns {}, not the decision '{refers_to}' of '{}'",
                received.id,
                copy.about.as_ref().map_or_else(
                    || "no decision of another cell".to_string(),
                    |a| format!("'{}' of '{}'", a.id, a.from)
                ),
                self.recording_actor
            )));
        }
        if on < received.recorded_on {
            return Err(refused(format!(
                "the copy of '{}', recorded on {}, cannot be received on {on}",
                received.id, received.recorded_on
            )));
        }
        let declared = shape::fact_fields(service, fact, on, Some(&self.authority))?;
        let what = format!("receipt of {fact}");
        let supplied: Map<String, serde_json::Value> = declared
            .iter()
            .filter_map(|f| received.held(&f.name).map(|v| (f.name.clone(), v.clone())))
            .collect();
        let fields = whole(&declared, &supplied, &what)?;
        let (basis, name) = shape::split_fact(fact)?;
        let gram = Gram {
            id: uuid::Uuid::now_v7().to_string(),
            kind: EXECUTOGRAM.to_string(),
            basis: basis.to_string(),
            fact: Some(name.to_string()),
            refers_to: Some(refers_to.to_string()),
            received: Some(Received {
                from: copy.from.clone(),
                id: received.id.clone(),
            }),
            after: None,
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment: None,
            fields,
            missing: Vec::new(),
            outputs: BTreeMap::new(),
        };
        Ok(self.chronicle.append(gram)?.clone())
    }

    /// Take the decision of article `article` of `law_id` on the application
    /// recorded as chronolexogram `application`, on `on` (RFC-047).
    ///
    /// What the decision asks comes from the application, the holder's
    /// policy and the cell ([`decision::derive`]). The cell answers the
    /// engine for the whole decision ([`Reader`], RFC-048): where a policy
    /// binds an open term to an answer of the cell, also in an article the
    /// decision calls, the engine asks the cell, which runs the reduction its
    /// catalogue names on its chronicle and gives the lexostatus. The
    /// reductions are rulework in `service`, as the corpus holds them.
    /// `supplied`
    /// holds what the deciding authority
    /// supplies itself, such as a parameter of a hook, and `findings` what it
    /// found for this case, by the id of the open term the law leaves to it
    /// per case (`decided_per_case_by`, RFC-050). All findings reach the
    /// engine apart from the parameters, so each applies to every article in
    /// the decision that leaves the term to the authority: the deciding
    /// article, and an article it calls, such as a decision called anew. The
    /// cell checks a finding on a term of the deciding article as one of its
    /// inputs, and refuses one on such a term that another authority
    /// answers: the one the term names (`competent_authority`), or the one
    /// that executes its article ([`regelrecht_engine::DecisionTerm`]). The
    /// engine refuses one that no article in the decision asks, or of the
    /// wrong type. A finding for a call anew is so given only once
    /// that call happens: given while the decision does not call anew (the
    /// objection is not admissible, say), no article asks it and the
    /// decision is refused. The chronolexogram keeps each finding among its
    /// fields; which of them the decision used, and not a regulation that
    /// set it aside, a replay says ([`Cell::findings`]).
    ///
    /// The cell refuses an application that is not in its chronicle or not
    /// one of this article, one already decided on, a value it takes from
    /// elsewhere, and a value of the wrong type. It runs the stages of
    /// [`decision::STAGES`] its procedure holds on the day `on` and appends
    /// one chronolexogram that refers to the application. The chronicle's
    /// clock keeps the decision on or after the application.
    ///
    /// A submission that refers to a decision D (`refers_to`):
    /// - when the deciding article has an output of type `decision`, so may
    ///   take a decision that replaces D, the reductions read the chronicle
    ///   without D, so D does not count against itself, also where the
    ///   engine asks the cell in D's article called anew; otherwise they read
    ///   D as any other decision;
    /// - a field of type `decision` reaches the engine as D's decision value
    ///   (`Cell::decision_value`); the chronolexogram keeps D's id;
    /// - in that value, the dates the stages of D's article require of the
    ///   cell are `on` (`refresh`).
    ///
    /// Every output of type `decision` that is not null is a decision the
    /// deciding article takes in turn: the cell records it as a
    /// chronolexogram of its own (`Cell::decided`), referring to the same
    /// submission, and keeps its id as that output. All of them land in the
    /// chronicle together, or none. Returns them, the decision on the
    /// submission first.
    #[allow(clippy::too_many_arguments)]
    pub fn decide(
        &mut self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        law_id: &str,
        article: &str,
        application: &str,
        supplied: &Map<String, serde_json::Value>,
        findings: &Map<String, serde_json::Value>,
        on: NaiveDate,
    ) -> Result<Vec<Gram>> {
        self.decide_on(
            service,
            sphere,
            law_id,
            article,
            application,
            None,
            supplied,
            findings,
            on,
        )
    }

    /// [`Cell::decide`] on a submission already decided on, once more
    /// (RFC-053). `after` names a later chronolexogram that refers to the
    /// last decision on that submission, such as the receipt of a judgment
    /// that annulled it; the new decision holds it as `after`. The rule names
    /// no article: whether the earlier decision still stands is for whoever
    /// reads the chronicle.
    ///
    /// A decision that replaced another in turn, taken by an earlier
    /// decision on the same submission, does not count when the cell checks
    /// that a decision is replaced once. Refused, besides what `decide`
    /// refuses: a submission not decided on yet, and an `after` that is not
    /// in the chronicle or refers to another chronolexogram than the last
    /// decision on it.
    #[allow(clippy::too_many_arguments)]
    pub fn decide_after(
        &mut self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        law_id: &str,
        article: &str,
        application: &str,
        after: &str,
        supplied: &Map<String, serde_json::Value>,
        findings: &Map<String, serde_json::Value>,
        on: NaiveDate,
    ) -> Result<Vec<Gram>> {
        self.decide_on(
            service,
            sphere,
            law_id,
            article,
            application,
            Some(after),
            supplied,
            findings,
            on,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn decide_on(
        &mut self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        law_id: &str,
        article: &str,
        application: &str,
        after: Option<&str>,
        supplied: &Map<String, serde_json::Value>,
        findings: &Map<String, serde_json::Value>,
        on: NaiveDate,
    ) -> Result<Vec<Gram>> {
        // A decision that may take one in the place of the decision its
        // submission refers to reads the chronicle without that one.
        let replaces = decision_outputs(service, law_id, article, on);
        let except = self
            .grams()
            .iter()
            .find(|g| g.id == application)
            .and_then(|g| g.refers_to.clone())
            .filter(|_| !replaces.is_empty());
        // The decision values it gets, from replays of those decisions.
        let shape = decision::derive(service, law_id, article, on)?;
        let applied = self.application(&shape, application, after)?.clone();
        // Taken again, the decision sets aside every earlier one on it.
        let set_aside: Vec<String> = decisions_on(self.grams(), &applied.id)
            .iter()
            .map(|g| g.id.clone())
            .collect();
        let decisions = self.decision_values(service, sphere, &shape, &applied, on)?;
        let grams = self.grams_but(except.as_deref());
        let reader = self.reader(grams, None);
        let others = known_on(sphere, on);
        let answerers = Answerers::new(&reader, &others);
        if let Some(name) = supplied
            .keys()
            .find(|name| is_finding(&shape, name, &self.authority))
        {
            return Err(refused(format!(
                "'{name}' of the decision of {} is a finding of the deciding authority; it \
                 is given among the findings",
                shape.basis()
            )));
        }
        // A finding on an open term of the decision, of the deciding article
        // or of a policy that fills one of its terms, is checked as what is
        // supplied is; any other is for an article the decision calls. All
        // of them reach the engine as findings.
        for (name, value) in findings {
            refuse_decimal_key(name, value)?;
            if shape.input(name).is_some() {
                return Err(refused(format!(
                    "'{name}' of the decision of {} is no finding; it is supplied, or comes \
                     from elsewhere",
                    shape.basis()
                )));
            }
            if shape.filled(name) {
                return Err(refused(format!(
                    "'{name}' of the decision of {} is filled by the policy; no finding takes \
                     its place",
                    shape.basis()
                )));
            }
            refuse_finding_of_another(&shape, name, &self.authority)?;
            if let Some(term) = shape.finding(name, &self.authority) {
                let field = finding_field(term);
                if !shape::fits(&field, value) {
                    return Err(refused(format!(
                        "'{name}' is {value}, which does not fit a {:?} ({})",
                        field.type_,
                        field.first_declared_by()
                    )));
                }
            }
        }
        // A finding the deciding article itself asks and cannot go without:
        // the cell refuses the decision without it, as for a value the
        // authority supplies. One a policy leaves to the authority for a
        // case the engine asks only when the case reaches it.
        if let Some(term) = shape.terms.iter().find(|t| {
            t.is_finding(&self.authority)
                && t.law_id == shape.law_id
                && t.article == shape.article
                && t.required
                && !t.has_default
                && !findings.contains_key(&t.id)
        }) {
            return Err(refused(format!(
                "the decision of {} asks '{}' of the deciding authority ({})",
                shape.basis(),
                term.id,
                term.declared_by()
            )));
        }
        check_decision_supplied(&shape, supplied)?;
        let inputs = asked(&shape, &applied, &decisions, supplied, on)?;
        let (mut fields, mut outputs) = take(service, &shape, inputs, findings, &answerers, on)?;
        for (name, (id, _)) in decisions {
            fields.insert(name, serde_json::Value::String(id));
        }
        // Its id first, so the ids follow the order in the chronicle.
        let id = uuid::Uuid::now_v7().to_string();
        let mut taken = Vec::new();
        for name in replaces {
            let Some(value) = outputs.get(&name).filter(|v| !v.is_null()) else {
                continue;
            };
            // A decision of another kind than the one the deciding article
            // takes is no decision this cell takes in turn: a court's
            // judgment may hold the decision it sets in the place of the
            // annulled one, which is still the administrative body's kind of
            // decision (RFC-053). The judgment holds it as its value.
            if of_another_kind(service, value, &shape.kind, on) {
                continue;
            }
            let gram = self.decided(service, &shape, &name, value, &applied, &answerers, on)?;
            outputs.insert(name, serde_json::Value::String(gram.id.clone()));
            taken.push(gram);
        }
        // A decision is replaced once: the cell does not record a second
        // decision in the place of the same one.
        drop(reader);
        if let Some(contested) = except.as_deref().filter(|_| !taken.is_empty()) {
            if let Some((earlier, replacement)) = self.replacement_of(contested, &set_aside) {
                return Err(refused(format!(
                    "the decision of {} would take a decision in the place of '{contested}', \
                     which chronolexogram '{replacement}' already replaces (taken by '{earlier}')",
                    shape.basis()
                )));
            }
        }
        let mut grams = vec![Gram {
            id,
            kind: shape.kind.clone(),
            basis: shape.basis(),
            fact: None,
            refers_to: Some(applied.id.clone()),
            received: None,
            after: after.map(str::to_string),
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment: None,
            fields,
            missing: Vec::new(),
            outputs,
        }];
        grams.extend(taken);
        Ok(self.chronicle.append_all(grams)?.to_vec())
    }

    /// Why the decision `id` came out as it did: the decision taken again
    /// with tracing (`Cell::replay`), so the trace shows every article it ran
    /// and what each read. For a decision taken in turn (a new decision in
    /// the place of a revoked one), that is the decision on the submission
    /// that took it: its trace holds the call that gave the new one.
    ///
    /// A term the cell answered is one node of that trace: the lexostatus,
    /// with the cell, the reduction and its version, and how far it read the
    /// chronicle. How the cell came to it is the cell's own account
    /// ([`Cell::explain_answer`]).
    pub fn explain(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        id: &str,
    ) -> Result<ArticleResult> {
        Ok(self.replay(service, sphere, id, None)?.result)
    }

    /// How the cell came to its answer `answer` (its name) in the decision
    /// `id`: the decision taken again (`Cell::replay`), and the cell's
    /// reduction for that answer run with tracing on the chronicle as the
    /// decision read it, with the parameters the binding passed then. One
    /// result per time the decision asked the answer, in that order, each
    /// with the reduction's own trace; it is no part of the decision's trace,
    /// and nothing of it is stored. A silent answer (null), on which the
    /// policy says what holds, is explained too. Refused when the
    /// decision did not ask the cell for `answer`.
    pub fn explain_answer(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        id: &str,
        answer: &str,
    ) -> Result<Vec<ArticleResult>> {
        let explained: Vec<ArticleResult> = self
            .replay(service, sphere, id, Some(answer))?
            .asked
            .into_iter()
            .filter(|asked| asked.name == answer)
            .filter_map(|asked| asked.explained)
            .collect();
        if explained.is_empty() {
            return Err(refused(format!(
                "the decision '{id}' did not ask the cell of '{}' for '{answer}'",
                self.recording_actor
            )));
        }
        Ok(explained)
    }

    /// The fields of the decision `id` that hold a finding of the deciding
    /// authority (RFC-050): a field on an open term of the decision that
    /// nothing fills, of the deciding article or of a policy that fills one
    /// of its terms, and a field the decision's article does not ask, given
    /// for an article it called. A value a policy filled is none: no finding
    /// takes its place (RFC-048 §2). Nor is a finding on the open term of
    /// such a policy that the policy did not need for this case: the replay
    /// reports it filled its term without it. The decision is taken again
    /// first ([`Cell::replay`]), so a chronicle that does not give the
    /// decision fails here.
    pub fn findings(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        id: &str,
    ) -> Result<Vec<String>> {
        let gram = self.decision_gram(service, id)?.clone();
        let replayed = self.replay(service, sphere, id, None)?;
        let shape = &replayed.shape;
        let report = &replayed.result.open_terms;
        Ok(candidates(shape, &gram)
            .keys()
            .filter(|name| {
                // A finding on the open term of a policy that fills a term
                // of the decision, where that policy filled it for this case
                // without the finding, is none: the case did not reach it.
                let Some(term) = shape.terms.iter().find(|t| {
                    t.id == **name
                        && t.is_finding(&self.authority)
                        && !(t.law_id == shape.law_id && t.article == shape.article)
                }) else {
                    return true;
                };
                let read = report
                    .iter()
                    .any(|f| f.term == **name && matches!(f.filled_by, FilledBy::Finding));
                let filled_without = report.iter().any(|f| match &f.filled_by {
                    FilledBy::Regulation { law_id, article }
                    | FilledBy::Answer {
                        law_id, article, ..
                    } => *law_id == term.law_id && *article == term.article,
                    FilledBy::Finding => false,
                });
                read || !filled_without
            })
            .cloned()
            .collect())
    }

    /// Take the decision `id` again, with tracing, as [`Cell::decide`] took
    /// it; for a decision taken in turn, the decision on the submission that
    /// took it. Returns the engine's result, what the decision `id` asks,
    /// and what the cell answered the engine, with the trace of its
    /// reduction for the term `explain`.
    ///
    /// It runs on the day of the decision, under the law in force that day,
    /// with the fields its chronolexogram holds, and with the cell answering
    /// from the chronicle as it stood before the decision, without the
    /// decision its submission refers to where the decision may replace that
    /// one. Its reductions run as they ran then, so they give the same
    /// lexostatus; an outcome that differs fails below. A
    /// field on a term the law leaves to the authority, or for an article the
    /// decision called, goes to the engine as a finding; where a regulation
    /// fills the term, it sets the finding aside, as it did then. What a
    /// policy filled, the engine fills again. The outputs of the decision
    /// item must come out as recorded; otherwise this is not the decision,
    /// and it fails.
    ///
    /// A decision it took in turn must come out with the outputs of the
    /// chronolexogram that recorded it.
    ///
    /// The replay evaluates the outputs of the decision item at once, where
    /// `decide` runs the stages BESLUIT and BEKENDMAKING; the decision item's
    /// outputs are the same at each stage.
    ///
    /// This assumes every source the decision read can say what it knew on
    /// the day of the decision (RFC-050). The cell's own sources (the
    /// chronicle and the law in force on a day) can.
    fn replay(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        id: &str,
        explain: Option<&str>,
    ) -> Result<Replayed> {
        let gram = self.decision_gram(service, id)?;
        let target = gram.clone();
        let submission_id = gram.refers_to.clone().ok_or_else(|| {
            setup(format!(
                "chronolexogram '{id}' refers to no submission, so it is no decision to replay"
            ))
        })?;
        let grams = self.grams();
        // The decision on the submission that is this one, or that holds it
        // as a decision it took in turn: a submission decided on again has
        // more than one.
        let position = grams
            .iter()
            .position(|g| {
                g.refers_to.as_deref() == Some(submission_id.as_str())
                    && (g.id == target.id
                        || g.outputs
                            .values()
                            .any(|v| v.as_str() == Some(target.id.as_str())))
            })
            .ok_or_else(|| {
                setup(format!(
                    "no decision in the chronicle of '{}' refers to '{submission_id}'",
                    self.recording_actor
                ))
            })?;
        let decision = grams[position].clone();
        let submission = grams[..position]
            .iter()
            .find(|g| g.id == submission_id)
            .ok_or_else(|| {
                setup(format!(
                    "the decision '{}' refers to '{submission_id}', no earlier submission in the \
                     chronicle of '{}'",
                    decision.id, self.recording_actor
                ))
            })?
            .clone();
        let (law_id, article) = decision.basis.split_once('#').ok_or_else(|| {
            setup(format!(
                "chronolexogram '{}' has basis '{}', not <regulation>#<article>",
                decision.id, decision.basis
            ))
        })?;
        let on = decision.recorded_on;
        // The decision values first: each replays its own decision.
        let shape = decision::derive(service, law_id, article, on)?;
        let decisions = self.decision_values(service, sphere, &shape, &submission, on)?;
        let replaces = decision_outputs(service, law_id, article, on);
        let except = submission
            .refers_to
            .as_deref()
            .filter(|_| !replaces.is_empty());
        let before: Vec<Gram> = grams[..position]
            .iter()
            .filter(|g| except != Some(g.id.as_str()))
            .cloned()
            .collect();
        let reader = self.reader(before, explain);
        let others = known_on(sphere, on);
        let given = candidates(&shape, &decision);
        let mut parameters = BTreeMap::new();
        let mut findings = BTreeMap::new();
        for (name, value) in &decision.fields {
            if given.contains_key(name) {
                findings.insert(name.clone(), stored::from_stored(value)?);
                continue;
            }
            if shape.input(name).is_none() {
                continue;
            }
            let value = decisions.get(name).map_or(value, |(_, v)| v);
            parameters.insert(name.clone(), stored::from_stored(value)?);
        }
        let outputs: Vec<&str> = shape.outputs.iter().map(String::as_str).collect();
        let date = on.to_string();
        let call = Answerers::new(&reader, &others).call(findings);
        let result =
            service.evaluate_law_with_trace_in(law_id, &outputs, parameters, call, &date)?;
        for name in &shape.outputs {
            let recorded = decision.outputs.get(name);
            let again = result
                .outputs
                .get(name)
                .map(stored::to_stored)
                .transpose()?;
            // A decision it took is an id in the chronolexogram and a value
            // here: the value's outputs are those of the chronolexogram of
            // that id.
            let same = if replaces.contains(name)
                && !recorded.is_some_and(serde_json::Value::is_object)
            {
                match (recorded.and_then(serde_json::Value::as_str), &again) {
                    (None, again) => {
                        recorded.is_none_or(serde_json::Value::is_null)
                            && again.as_ref().is_none_or(serde_json::Value::is_null)
                    }
                    (Some(taken), Some(value)) => {
                        let taken = grams.iter().find(|g| g.id == taken).ok_or_else(|| {
                            setup(format!(
                                "the decision '{}' holds '{taken}' as '{name}', no \
                                 chronolexogram in the chronicle",
                                decision.id
                            ))
                        })?;
                        let outputs = value
                            .get("outputs")
                            .and_then(serde_json::Value::as_object)
                            .cloned()
                            .unwrap_or_default();
                        let mut same = !outputs.is_empty();
                        for (key, output) in &outputs {
                            same = same && stored::same_held(Some(output), taken.outputs.get(key))?;
                        }
                        same
                    }
                    (Some(_), None) => false,
                }
            } else {
                stored::same_held(again.as_ref(), recorded)?
            };
            if !same {
                return Err(setup(format!(
                    "the decision '{}' recorded '{name}' as {}, but taken again it is {}; the \
                     replay is not this decision",
                    decision.id,
                    recorded.cloned().unwrap_or(serde_json::Value::Null),
                    again.unwrap_or(serde_json::Value::Null)
                )));
            }
        }
        let shape = if target.id == decision.id {
            shape
        } else {
            decision_shape_of(service, &target, target.recorded_on)?
        };
        let asked = reader.asked();
        Ok(Replayed {
            result,
            shape,
            asked,
        })
    }

    /// For each field of type `decision` that the decision of `shape` takes
    /// from the submission `applied`, the id it holds and the decision value
    /// the engine gets, refreshed on `on` ([`refresh`]).
    fn decision_values(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        shape: &DecisionShape,
        applied: &Gram,
        on: NaiveDate,
    ) -> Result<BTreeMap<String, (String, serde_json::Value)>> {
        let mut values = BTreeMap::new();
        for input in shape
            .inputs
            .iter()
            .filter(|i| i.field.type_ == ParameterType::Decision && i.source == Source::Application)
        {
            let Some(serde_json::Value::String(id)) = applied.held(&input.field.name) else {
                continue;
            };
            let gram = self.decision_gram(service, id)?.clone();
            let mut value = self.decision_value(service, sphere, &gram)?;
            refresh(service, &gram, &mut value, on)?;
            values.insert(input.field.name.clone(), (id.clone(), value));
        }
        Ok(values)
    }

    /// The chronicle without the decision `except`, if one is given.
    fn grams_but(&self, except: Option<&str>) -> Vec<Gram> {
        self.grams()
            .iter()
            .filter(|g| except != Some(g.id.as_str()))
            .cloned()
            .collect()
    }

    /// The decision that replaces the decision `id`, if the chronicle holds
    /// one: a decision recorded in turn on a submission that refers to `id`,
    /// which the decision on that submission holds by its id as an output.
    /// Returns the id of the decision on that submission and of the
    /// replacing decision. A replacement taken by a decision in `set_aside`
    /// does not count.
    fn replacement_of(&self, id: &str, set_aside: &[String]) -> Option<(&str, &str)> {
        let grams = self.grams();
        grams
            .iter()
            .filter(|s| s.refers_to.as_deref() == Some(id))
            .find_map(|submission| {
                let on_it = || {
                    grams
                        .iter()
                        .filter(|g| g.refers_to.as_deref() == Some(submission.id.as_str()))
                };
                on_it()
                    .filter(|decision| !set_aside.contains(&decision.id))
                    .find_map(|decision| {
                        on_it()
                            .filter(|r| r.id != decision.id)
                            .find(|r| {
                                decision
                                    .outputs
                                    .values()
                                    .any(|v| v.as_str() == Some(r.id.as_str()))
                            })
                            .map(|r| (decision.id.as_str(), r.id.as_str()))
                    })
            })
    }

    /// The decision value `{basis, taken_on, fields, findings,
    /// outputs}` of the decision `gram`, as the engine builds one (RFC-050),
    /// in the law in force on the day it was recorded: of its outputs those
    /// its decision item names, and of its fields:
    ///
    /// - under `findings`, the deciding authority's findings for this case
    ///   ([`Cell::findings`], from a replay) on the open terms of the
    ///   decision: of the deciding article, and of a policy that fills one of
    ///   its terms and leaves a case to the authority (RFC-048 §2);
    /// - no value a regulation filled: a call anew reads the regulation in
    ///   force on its own day;
    /// - no finding given for an article the decision called: that is
    ///   recorded with the decision that article produced, the new decision;
    /// - under `fields`, the parameters.
    ///
    /// Data derived for the decision is no part of it. A decision the fields
    /// or outputs hold stays its id.
    ///
    /// `taken_on` is the article of the submission the decision refers to:
    /// the one it was taken on. It is not stored; the chronicle gives it.
    /// The engine gives no `taken_on` in a value of `source.article`.
    fn decision_value(
        &self,
        service: &mut LawExecutionService,
        sphere: &[&Cell],
        gram: &Gram,
    ) -> Result<serde_json::Value> {
        let used = self.findings(service, sphere, &gram.id)?;
        let found = gram
            .basis
            .split_once('#')
            .and_then(|(law_id, article)| {
                shape::article_on(service, law_id, article, gram.recorded_on)
            })
            .ok_or_else(|| {
                setup(format!(
                    "the decision '{}' is under {}, which has no version in force on {}",
                    gram.id, gram.basis, gram.recorded_on
                ))
            })?;
        let decided = found
            .get_decision()
            .map(|decision| found.outputs_of(decision))
            .unwrap_or_default();
        let shape = decision_shape_of(service, gram, gram.recorded_on)?;
        let mut fields = Map::new();
        let mut findings = Map::new();
        for (name, value) in &gram.fields {
            if shape.input(name).is_some() {
                fields.insert(name.clone(), value.clone());
            } else if used.contains(name) && shape.finding(name, &self.authority).is_some() {
                findings.insert(name.clone(), value.clone());
            }
        }
        let outputs: Map<String, serde_json::Value> = gram
            .outputs
            .iter()
            .filter(|(name, _)| decided.contains(&name.as_str()))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        Ok(serde_json::json!({
            "basis": gram.basis,
            "taken_on": self.taken_on(gram)?,
            "fields": fields,
            "findings": findings,
            "outputs": outputs,
        }))
    }

    /// The article of the submission the decision `gram` refers to, in this
    /// chronicle.
    fn taken_on(&self, gram: &Gram) -> Result<&str> {
        gram.refers_to
            .as_deref()
            .and_then(|id| self.grams().iter().find(|g| g.id == id))
            .map(|submission| submission.basis.as_str())
            .ok_or_else(|| {
                setup(format!(
                    "the decision '{}' refers to {:?}, no submission in the chronicle of '{}'",
                    gram.id, gram.refers_to, self.recording_actor
                ))
            })
    }

    /// The decision `value` that output `output` of the decision of
    /// `deciding` gave, taken by its own article on `on` and recorded as a
    /// chronolexogram that refers to the submission `submission`. Not yet
    /// appended.
    ///
    /// Its `refers_to` is the submission it was taken on, not a submission of
    /// its own article: a new decision on an objection refers to the
    /// objection, as the decision on that objection does.
    ///
    /// The cell runs the stages of that article again with the value's
    /// `fields` and `findings`, and fills a date a stage requires with `on`, so a hook at
    /// a later stage gives the new decision its own outcome (such as its own
    /// period from its own announcement). The outputs of its decision item must be
    /// those of the value, as a chronicle stores them; otherwise the value
    /// is not this article's decision on these fields. A value that holds
    /// `taken_on` must name the article of `submission`: the new decision
    /// is taken on it.
    #[allow(clippy::too_many_arguments)]
    fn decided(
        &self,
        service: &LawExecutionService,
        deciding: &DecisionShape,
        output: &str,
        value: &serde_json::Value,
        submission: &Gram,
        answerers: &Answerers<'_>,
        on: NaiveDate,
    ) -> Result<Gram> {
        let not_a_decision = || {
            setup(format!(
                "'{output}' of the decision of {} is {value}, no decision value \
                 {{basis, fields, findings, outputs}}",
                deciding.basis()
            ))
        };
        let object = value
            .as_object()
            .filter(|o| shape::is_decision_value(o))
            .ok_or_else(not_a_decision)?;
        let basis = object
            .get("basis")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(not_a_decision)?;
        let part = |key: &str| {
            object
                .get(key)
                .and_then(serde_json::Value::as_object)
                .ok_or_else(not_a_decision)
        };
        let (given_fields, given_findings) = (part("fields")?, part("findings")?);
        let expected = part("outputs")?;
        let (law_id, article) = basis.split_once('#').ok_or_else(not_a_decision)?;
        let shape = decision::derive(service, law_id, article, on)?;
        let mut fields = Values::new();
        for input in &shape.inputs {
            let name = &input.field.name;
            // A policy value of the day of the old decision is no input:
            // the stages read the policy in force on `on`, as for `decide`.
            let value = match input.source {
                Source::Cell => Some(serde_json::Value::String(on.to_string())),
                _ => given_fields
                    .get(name)
                    .or_else(|| given_findings.get(name))
                    .cloned(),
            };
            if let Some(value) = value {
                fields.insert(name.clone(), value);
            }
        }
        // The value's findings reach the engine as findings, as in `decide`.
        let parameters = engine_values(
            fields
                .iter()
                .filter(|(name, _)| !given_findings.contains_key(*name)),
        )?;
        for (name, finding) in given_findings {
            fields
                .entry(name.clone())
                .or_insert_with(|| finding.clone());
        }
        let outputs = run_stages(
            service,
            &shape,
            parameters,
            engine_values(given_findings)?,
            answerers,
            on,
            &mut fields,
        )?;
        // A decision it takes in turn would be a value in its outputs, not
        // an id: the cell records one level.
        if let Some(nested) = decision_outputs(service, &shape.law_id, &shape.article, on)
            .into_iter()
            .find(|name| outputs.get(name).is_some_and(|v| !v.is_null()))
        {
            return Err(setup(format!(
                "'{output}' of the decision of {} is a decision under {basis} that takes \
                 a decision in turn ('{nested}'); the cell records one level",
                deciding.basis()
            )));
        }
        // As a chronicle stores them, compared as the engine's `EQUALS`
        // compares records ([`stored::same_held`]).
        let mut same = expected.keys().all(|k| shape.outputs.contains(k));
        for o in &shape.outputs {
            same = same && stored::same_held(outputs.get(o), expected.get(o))?;
        }
        if !same {
            return Err(setup(format!(
                "'{output}' of the decision of {} is a decision under {basis} with outputs \
                 {}, but taken again on these fields that article decides {}",
                deciding.basis(),
                serde_json::Value::Object(expected.clone()),
                serde_json::json!(shape
                    .outputs
                    .iter()
                    .filter_map(|o| Some((o.clone(), outputs.get(o)?.clone())))
                    .collect::<Map<_, _>>())
            )));
        }
        if let Some(taken_on) = object
            .get("taken_on")
            .and_then(serde_json::Value::as_str)
            .filter(|t| *t != submission.basis)
        {
            return Err(setup(format!(
                "'{output}' of the decision of {} is a decision taken on {taken_on}, but the \
                 cell takes it on '{}', under {}",
                deciding.basis(),
                submission.id,
                submission.basis
            )));
        }
        Ok(Gram {
            id: uuid::Uuid::now_v7().to_string(),
            kind: shape.kind.clone(),
            basis: shape.basis(),
            fact: None,
            refers_to: Some(submission.id.clone()),
            received: None,
            after: None,
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment: None,
            fields,
            missing: Vec::new(),
            outputs,
        })
    }

    /// The application chronolexogram `id` that the decision of `shape` is taken on:
    /// one in this chronicle, of this article and kind, not decided on yet,
    /// or, with `after`, decided on and named by `after` again
    /// ([`Cell::decide_after`]): `after` refers to the last decision on it.
    fn application<'g>(
        &'g self,
        shape: &DecisionShape,
        id: &str,
        after: Option<&str>,
    ) -> Result<&'g Gram> {
        let gram = self.grams().iter().find(|g| g.id == id).ok_or_else(|| {
            refused(format!(
                "there is no chronolexogram '{id}' in the chronicle of '{}'",
                self.recording_actor
            ))
        })?;
        if gram.kind != shape.submission_kind || gram.basis != shape.application {
            return Err(refused(format!(
                "chronolexogram '{id}' is a {} under {}, not an {} under {} that {} decides on",
                gram.kind,
                gram.basis,
                shape.submission_kind,
                shape.application,
                shape.basis()
            )));
        }
        let last = decisions_on(self.grams(), id).last().copied();
        match (last, after) {
            (None, None) => Ok(gram),
            (Some(decided), None) => Err(refused(format!(
                "the application '{id}' is already decided on, by chronolexogram '{}'",
                decided.id
            ))),
            (None, Some(after)) => Err(refused(format!(
                "the application '{id}' is not decided on yet, so there is nothing to decide \
                 again after '{after}'"
            ))),
            (Some(decided), Some(after)) => {
                let later = self.grams().iter().find(|g| g.id == after).ok_or_else(|| {
                    refused(format!(
                        "there is no chronolexogram '{after}' in the chronicle of '{}'",
                        self.recording_actor
                    ))
                })?;
                if later.refers_to.as_deref() != Some(decided.id.as_str()) {
                    return Err(refused(format!(
                        "chronolexogram '{after}' refers to {:?}, not to '{}', the last \
                         decision on '{id}'",
                        later.refers_to, decided.id
                    )));
                }
                Ok(gram)
            }
        }
    }
}

/// The decisions taken on the submission `id` in `grams`, in their order:
/// those that refer to it, without a decision that one of them took in turn
/// and holds by its id.
fn decisions_on<'g>(grams: &'g [Gram], id: &str) -> Vec<&'g Gram> {
    let on_it: Vec<&Gram> = grams
        .iter()
        .filter(|g| g.refers_to.as_deref() == Some(id))
        .collect();
    on_it
        .iter()
        .copied()
        .filter(|g| {
            !on_it.iter().any(|d| {
                d.outputs
                    .values()
                    .any(|v| v.as_str() == Some(g.id.as_str()))
            })
        })
        .collect()
}

/// Who answers the engine in a decision of a cell (RFC-052): the cell itself,
/// and the other cells of the sphere, each as it knew things on the day.
struct Answerers<'r> {
    own: &'r dyn OpenTermAnswerer,
    sphere: Vec<&'r dyn OpenTermAnswerer>,
}

impl<'r> Answerers<'r> {
    fn new(own: &'r Reader<'_>, others: &'r [Reader<'_>]) -> Self {
        Self {
            own,
            sphere: others.iter().map(|r| r as &dyn OpenTermAnswerer).collect(),
        }
    }

    /// A call of the engine with `findings` and these answerers.
    fn call(&self, findings: BTreeMap<String, Value>) -> Call<'r> {
        Call {
            findings,
            answerer: Some(self.own),
            chronicle: None,
            sphere: self.sphere.clone(),
        }
    }
}

/// The cells of `sphere` as they answer a decision on `on`.
fn known_on<'c>(sphere: &[&'c Cell], on: NaiveDate) -> Vec<Reader<'c>> {
    sphere.iter().map(|cell| cell.known_on(on)).collect()
}

/// A decision taken again ([`Cell::replay`]): the engine's result, what the
/// decision asked for which it was taken again, and what the cell answered.
struct Replayed {
    result: ArticleResult,
    shape: DecisionShape,
    asked: Vec<Asked>,
}

/// Values by name: the fields or the outputs of a chronolexogram.
type Values = BTreeMap<String, serde_json::Value>;

/// Values as the engine takes them ([`stored::from_stored`]).
fn engine_values<'v>(
    values: impl IntoIterator<Item = (&'v String, &'v serde_json::Value)>,
) -> Result<BTreeMap<String, Value>> {
    values
        .into_iter()
        .map(|(k, v)| Ok((k.clone(), stored::from_stored(v)?)))
        .collect()
}

/// What the cell fills in a submission itself, and what it refers to.
#[derive(Debug, Default)]
struct Filled {
    fields: Values,
    refers_to: Option<String>,
    received: Option<Received>,
}

/// Take the decision of `shape` on `on` in `service`, with the cell as
/// `answerer` for the terms its reductions fill ([`Reader`]). `fields` is what
/// [`asked`] gathered, and `findings` every finding the authority gave, also
/// for an article the decision calls. The engine gets the findings apart from
/// the parameters (RFC-050), so one applies to every article in the decision
/// that leaves the term to the authority. Returns the fields, with what a
/// policy or a finding filled, and the outputs.
fn take(
    service: &LawExecutionService,
    shape: &DecisionShape,
    mut fields: Values,
    findings: &Map<String, serde_json::Value>,
    answerers: &Answerers<'_>,
    on: NaiveDate,
) -> Result<(Values, Values)> {
    let parameters = engine_values(
        fields
            .iter()
            .filter(|(name, _)| !findings.contains_key(*name)),
    )?;
    for (name, finding) in findings {
        fields
            .entry(name.clone())
            .or_insert_with(|| finding.clone());
    }
    let outputs = run_stages(
        service,
        shape,
        parameters,
        engine_values(findings)?,
        answerers,
        on,
        &mut fields,
    )?;
    Ok((fields, outputs))
}

/// What holds a finding of the deciding authority in the decision `gram`
/// of `shape`: every field that is no parameter of the decision and no
/// value a policy filled. That is a finding on an open term of the decision
/// (of the deciding article, or of a policy that fills one of its terms),
/// or one given for an article it called. A null is none.
fn candidates(shape: &DecisionShape, gram: &Gram) -> Map<String, serde_json::Value> {
    gram.fields
        .iter()
        .filter(|(name, value)| {
            !value.is_null() && shape.input(name).is_none() && !shape.filled(name)
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// Answer the decision value `value` of `gram` again, on `on`, as its
/// article would take it that day: a date a stage of its decision requires
/// of the cell is `on`, as when the cell takes a decision
/// ([`Cell::decided`]). A deciding article that calls that article anew so
/// passes it the stage dates of the day of the decision, not of the day of
/// `gram`.
///
/// What a regulation filled is not in the value ([`Cell::decision_value`]),
/// so the call anew asks it again of the regulation in force that day, and
/// a term the cell answers of the cell, which answers it from the chronicle
/// as the decision reads it.
fn refresh(
    service: &LawExecutionService,
    gram: &Gram,
    value: &mut serde_json::Value,
    on: NaiveDate,
) -> Result<()> {
    let shape = decision_shape_of(service, gram, on)?;
    let fields = value
        .get_mut("fields")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| setup(format!("the decision value of '{}' has no fields", gram.id)))?;
    for input in shape.inputs.iter().filter(|i| i.source == Source::Cell) {
        fields.insert(
            input.field.name.clone(),
            serde_json::Value::String(on.to_string()),
        );
    }
    Ok(())
}

/// What the decision of `shape` on the application `applied` asks, from
/// where the law says it comes. What a regulation or the cell fills is left
/// to the engine. An application that lacks what
/// the decision needs (Awb 4:5) is not decided on here: the cell does not
/// guess the value.
fn asked(
    shape: &DecisionShape,
    applied: &Gram,
    decisions: &BTreeMap<String, (String, serde_json::Value)>,
    supplied: &Map<String, serde_json::Value>,
    on: NaiveDate,
) -> Result<Values> {
    let mut fields = BTreeMap::new();
    for input in &shape.inputs {
        let name = &input.field.name;
        let value = match &input.source {
            Source::Application if decisions.contains_key(name) => {
                decisions.get(name).map(|(_, value)| value.clone())
            }
            Source::Application => match applied.held(name) {
                Some(value) => Some(value.clone()),
                None if applied.missing.contains(name) => {
                    return Err(refused(format!(
                        "the application '{}' lacks '{name}', which the decision of {} needs \
                         (Awb 4:5)",
                        applied.id,
                        shape.basis()
                    )))
                }
                None => None,
            },
            Source::Supplied => match supplied.get(name) {
                Some(value) => Some(value.clone()),
                None if input.field.nullable => Some(serde_json::Value::Null),
                None => {
                    return Err(refused(format!(
                        "the decision of {} asks '{name}' of the deciding authority ({})",
                        shape.basis(),
                        input.field.first_declared_by()
                    )))
                }
            },
            Source::Cell => Some(serde_json::Value::String(on.to_string())),
        };
        if let Some(value) = value {
            fields.insert(name.clone(), value);
        }
    }
    Ok(fields)
}

/// Run the stages of a decision on `on`, with the authority's `findings`
/// for the articles it calls and the cell as `answerer`: add what a policy
/// filled to `fields`, and return the outputs.
fn run_stages(
    service: &LawExecutionService,
    shape: &DecisionShape,
    parameters: BTreeMap<String, Value>,
    findings: BTreeMap<String, Value>,
    answerers: &Answerers<'_>,
    on: NaiveDate,
    fields: &mut BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let date = on.to_string();
    let mut outputs = BTreeMap::new();
    for stage in &shape.stages {
        // Every stage computes the decision's outputs, so a hook at a later
        // stage can read them; they are the same at each.
        let call = answerers.call(findings.clone());
        let result = service.execute_stage_in(
            &shape.law_id,
            &shape.article,
            &stage.stage,
            None,
            parameters.clone(),
            call,
            &date,
        )?;
        collect_outputs(&result, &mut outputs)?;
        // What a policy filled of the deciding article's open terms: the
        // gram takes what the engine used.
        for filled in result.open_terms.iter().filter(|f| {
            f.law_id == shape.law_id
                && f.article == shape.article
                && !matches!(f.filled_by, FilledBy::Finding)
        }) {
            if let Some(value) = result.resolved_inputs.get(&filled.term) {
                fields.insert(filled.term.clone(), stored::to_stored(value)?);
            }
        }
    }
    Ok(outputs)
}

/// Whether `value` names an article whose decision, in the law in force on
/// `on`, is of another legal character than `kind`. A value that names no
/// such article is left to [`Cell::decided`], which refuses what is no
/// decision value.
fn of_another_kind(
    service: &LawExecutionService,
    value: &serde_json::Value,
    kind: &str,
    on: NaiveDate,
) -> bool {
    value
        .get("basis")
        .and_then(serde_json::Value::as_str)
        .and_then(|g| g.split_once('#'))
        .and_then(|(law_id, article)| shape::article_on(service, law_id, article, on))
        .and_then(|article| article.get_decision())
        .and_then(|decision| decision.legal_character())
        .is_some_and(|character| character != kind)
}

/// The outputs of type `decision` of article `article` of `law_id`, in the
/// law in force on `on`.
fn decision_outputs(
    service: &LawExecutionService,
    law_id: &str,
    article: &str,
    on: NaiveDate,
) -> Vec<String> {
    shape::article_on(service, law_id, article, on)
        .and_then(|article| article.get_execution_spec())
        .and_then(|execution| execution.output.as_ref())
        .into_iter()
        .flatten()
        .filter(|output| output.output_type == ParameterType::Decision)
        .map(|output| output.name.clone())
        .collect()
}

/// What the decision of the article of `decision` asks, in the law in
/// force on `on`.
fn decision_shape_of(
    service: &LawExecutionService,
    decision: &Gram,
    on: NaiveDate,
) -> Result<DecisionShape> {
    let (law_id, article) = decision.basis.split_once('#').ok_or_else(|| {
        setup(format!(
            "chronolexogram '{}' has basis '{}', not <regulation>#<article>",
            decision.id, decision.basis
        ))
    })?;
    decision::derive(service, law_id, article, on)
}

/// The outputs of the articles that take part in the decision of `shape`:
/// what such a decision gives, whether a chronolexogram of it holds them or
/// not.
fn gives(service: &LawExecutionService, shape: &DecisionShape, on: NaiveDate) -> Vec<String> {
    shape
        .stages
        .iter()
        .flat_map(|stage| &stage.articles)
        .filter_map(|part| shape::article_on(service, &part.law_id, &part.article_number, on))
        .filter_map(|article| article.get_execution_spec())
        .flat_map(|execution| execution.output.iter().flatten())
        .map(|output| output.name.clone())
        .collect()
}

fn json_date(on: NaiveDate) -> Value {
    Value::from(serde_json::Value::String(on.to_string()))
}

/// Add what an execution gave to `values`. An output an override voids is
/// not in `outputs`.
fn collect_outputs(
    result: &ArticleResult,
    values: &mut BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    for (name, value) in &result.outputs {
        if matches!(
            result.output_provenance.get(name),
            Some(OutputProvenance::Voided { .. })
        ) {
            continue;
        }
        values.insert(name.clone(), stored::to_stored(value)?);
    }
    Ok(())
}

/// Whether `name` is a finding of `authority` on the decision of `shape`:
/// an open term of the decision that nothing fills, judged per case
/// (`decided_per_case_by`) and answered by `authority`.
fn is_finding(shape: &DecisionShape, name: &str, authority: &Authority) -> bool {
    shape.finding(name, authority).is_some()
}

/// Refuse a finding on the open term `name` of the decision of `shape` that
/// another authority answers: the one the term names in its
/// `competent_authority`, or the one that executes its article. The cell
/// of `authority` gives only its own findings.
fn refuse_finding_of_another(
    shape: &DecisionShape,
    name: &str,
    authority: &Authority,
) -> Result<()> {
    let asked: Vec<_> = shape
        .terms
        .iter()
        .filter(|t| t.id == name && t.asks_finding())
        .collect();
    if asked.iter().any(|t| t.answers(authority)) {
        return Ok(());
    }
    let Some(term) = asked.first() else {
        return Ok(());
    };
    let names: Vec<String> = term
        .answered_by
        .iter()
        .map(|n| format!("'{}'", n.name))
        .collect();
    Err(refused(format!(
        "'{name}' of {} is answered by {}, and the cell is '{}'{}; a cell gives only its own \
         findings",
        term.declared_by(),
        names.join(" or "),
        authority.name,
        authority
            .category
            .as_deref()
            .map(|c| format!(" ({c})"))
            .unwrap_or_default()
    )))
}

/// Refuse what is no decision: a chronolexogram on a submission whose
/// article, in the law in force on the day it was recorded, takes a decision
/// of its kind.
fn check_decision(service: &LawExecutionService, gram: &Gram) -> Result<()> {
    let id = &gram.id;
    let found = gram
        .basis
        .split_once('#')
        .and_then(|(law_id, article)| shape::article_on(service, law_id, article, gram.recorded_on))
        .ok_or_else(|| {
            setup(format!(
                "chronolexogram '{id}' is under {}, which has no version in force on {}",
                gram.basis, gram.recorded_on
            ))
        })?;
    let takes = found
        .get_decision()
        .and_then(|decision| decision.legal_character())
        .is_some_and(|kind| kind == gram.kind);
    if !takes || gram.refers_to.is_none() {
        return Err(refused(format!(
            "chronolexogram '{id}' is a {} under {}, not a decision",
            gram.kind, gram.basis
        )));
    }
    Ok(())
}

/// Refuse what the deciding authority does not supply: a value the
/// decision takes from the application, the policy or the cell, a name the
/// decision does not ask, a value of the wrong type.
fn check_decision_supplied(
    shape: &DecisionShape,
    supplied: &Map<String, serde_json::Value>,
) -> Result<()> {
    let decision = shape.basis();
    for (name, value) in supplied {
        if shape.filled(name) {
            return Err(refused(format!(
                "'{name}' of the decision of {decision} comes from the policy"
            )));
        }
        let input = shape
            .input(name)
            .ok_or_else(|| refused(format!("the decision of {decision} does not ask '{name}'")))?;
        let from = match &input.source {
            Source::Supplied => None,
            Source::Application => Some("the application".to_string()),
            Source::Cell => Some("the cell: it is the day of the decision".to_string()),
        };
        if let Some(from) = from {
            return Err(refused(format!(
                "'{name}' of the decision of {decision} comes from {from}"
            )));
        }
        // A decision is one in the chronicle, which the cell fills from the
        // submission's `refers_to`; nobody supplies one as a value.
        if input.field.type_ == ParameterType::Decision {
            return Err(refused(format!(
                "'{name}' of the decision of {decision} is a decision; the cell fills it from \
                 the chronolexogram a submission refers to"
            )));
        }
        refuse_decimal_key(name, value)?;
        if !shape::fits(&input.field, value) {
            return Err(refused(format!(
                "'{name}' is {value}, which does not fit a {:?} ({})",
                input.field.type_,
                input.field.first_declared_by()
            )));
        }
    }
    Ok(())
}

/// An executogram read back from the chronicle records a fact and nothing
/// else, as [`Cell::record_executogram`] records it: it names its fact, and
/// refers to no chronolexogram, holds no outputs, names no moment and misses
/// nothing. A chronolexogram of another kind names no fact.
/// Its fields are checked against the article that establishes it when it
/// is recorded; the cell opens without the law, so it does not check them
/// again here.
fn check_executogram(gram: &Gram, recording_actor: &str) -> Result<()> {
    if gram.kind != EXECUTOGRAM {
        if gram.fact.is_some() {
            return Err(setup(format!(
                "chronolexogram '{}' in the chronicle of '{recording_actor}' names a fact but is \
                 a {}",
                gram.id, gram.kind
            )));
        }
        return Ok(());
    }
    // The receipt of a copy refers to the decision of the cell it concerns
    // and holds what it received (RFC-053); any other refers to nothing.
    let refused_as = |what: &str| {
        Err(setup(format!(
            "executogram '{}' in the chronicle of '{recording_actor}' {what}; an executogram \
             records a fact and nothing else",
            gram.id
        )))
    };
    match (gram.refers_to.is_some(), gram.received.is_some()) {
        (true, false) => {
            return refused_as(
                "refers to a chronolexogram without having received one (only the receipt of \
                 a copy refers to the decision it concerns)",
            )
        }
        (false, true) => {
            return refused_as(
                "holds what it received but refers to no decision (the receipt of a copy \
                 refers to the decision it concerns)",
            )
        }
        _ => {}
    }
    if gram.after.is_some() {
        return refused_as("names `after`, which only a decision taken again does");
    }
    if gram.fact.is_none()
        || !gram.outputs.is_empty()
        || gram.moment.is_some()
        || !gram.missing.is_empty()
    {
        return Err(setup(format!(
            "executogram '{}' in the chronicle of '{recording_actor}' holds outputs, names a \
             moment or misses a field, or names no fact; an executogram records a fact and \
             nothing else",
            gram.id
        )));
    }
    Ok(())
}

/// The values `supplied` for `fields`, whole: every name is a field, every
/// value fits its field, and every field that cannot be absent has a value;
/// one that can be absent and is left out is `null`. `what` names what is
/// recorded, for the errors.
fn whole(
    fields: &[Field],
    supplied: &Map<String, serde_json::Value>,
    what: &str,
) -> Result<Values> {
    for (name, value) in supplied {
        let field = fields
            .iter()
            .find(|f| &f.name == name)
            .ok_or_else(|| refused(format!("the {what} holds no '{name}'")))?;
        refuse_decimal_key(name, value)?;
        if !shape::fits(field, value) {
            return Err(refused(format!(
                "'{name}' is {value}, which does not fit a {}{:?} ({})",
                if field.nullable { "" } else { "non-nullable " },
                field.type_,
                field.first_declared_by()
            )));
        }
    }
    let mut values = Values::new();
    for field in fields {
        let value = match supplied.get(&field.name) {
            Some(value) => value.clone(),
            None if field.nullable => serde_json::Value::Null,
            None => {
                return Err(refused(format!(
                    "the {what} needs '{}', which is left out",
                    field.name
                )))
            }
        };
        values.insert(field.name.clone(), value);
    }
    Ok(values)
}

/// Refuse what the law does not ask of the submitter, and what comes from
/// the decision the submission refers to (`from_referred`).
fn check_supplied(
    shape: &Shape,
    supplied: &Map<String, serde_json::Value>,
    from_referred: &[String],
) -> Result<()> {
    let submission = shape.basis();
    for (name, value) in supplied {
        if from_referred.contains(name) {
            return Err(refused(format!(
                "'{name}' of the {} of {submission} comes from the decision it refers to",
                shape.kind
            )));
        }
        if let Some(moment) = shape.moment.as_ref().filter(|_| shape.is_moment(name)) {
            return Err(refused(format!(
                "'{name}' is the moment that counts ({}); the cell records it",
                moment.declared_by
            )));
        }
        let field = shape.field(name).ok_or_else(|| {
            refused(format!(
                "the law does not ask '{name}' in the {} of {submission}",
                shape.kind
            ))
        })?;
        refuse_decimal_key(name, value)?;
        if !shape::fits(field, value) {
            return Err(refused(format!(
                "'{name}' is {value}, which does not fit a {}{:?} ({})",
                if field.nullable { "" } else { "non-nullable " },
                field.type_,
                field.first_declared_by()
            )));
        }
    }
    Ok(())
}

/// Refuse a supplied value that holds the key the chronicle writes a
/// decimal under ([`stored::DECIMAL_KEY`]): read back, it would be a
/// decimal the submitter never sent.
pub(crate) fn refuse_decimal_key(name: &str, value: &serde_json::Value) -> Result<()> {
    if stored::holds_reserved_key(value) {
        return Err(refused(format!(
            "'{name}' holds the key '{}', which the chronicle keeps for a decimal",
            stored::DECIMAL_KEY
        )));
    }
    Ok(())
}
