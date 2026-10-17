//! A cell: it keeps a chronicle and records submissions and the decisions
//! on them in it.
//!
//! A submission may refer to a decision in the chronicle (`refers_to`). A
//! field of type `decision` then holds that decision, and a field that the
//! decision holds by name comes from it too (RFC-050). The chronolexogram
//! keeps a decision by its id; the engine gets its decision value
//! `{grondslag, taken_on, fields, findings, outputs}`, where `taken_on`
//! is the article of the submission it was taken on. A decision output of
//! the deciding article is a decision the cell records as well, in the same
//! append. The cell knows no article that does this; the law names it with
//! the type.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use regelrecht_engine::{
    ArticleResult, Call, FilledBy, LawExecutionService, OpenTermAnswerer, OpenTermFilled,
    OutputProvenance, ParameterType, Value,
};
use serde_json::Map;

use crate::chronicle::{Chronicle, Gram, GramMoment};
use crate::decision::{self, DecisionShape, Source};
use crate::error::{refused, setup, Result};
use crate::reading::{self, Answer, Asked, Loaded, Reader, Reduction, Term};
use crate::shape::{self, Shape};
use regelrecht_law_model::stored;

/// A cell. It holds who it is, its chronicle and its reductions, not the
/// law: every call that needs the law gets the service that executes it.
///
/// A cell is opened with its directory, and its reductions come from there
/// ([`Cell::open`], [`Cell::in_memory`]), so a cell cannot be without them.
/// Without its reductions a cell could not tell a term they fill from one the
/// deciding authority finds, and a replay would pass a reduction's answer off
/// as the authority's finding (RFC-048). A cell without reductions is opened
/// with a directory that holds none.
#[derive(Debug)]
pub struct Cell {
    recording_actor: String,
    chronicle: Chronicle,
    reductions: Reductions,
}

/// The reductions of a cell and the directory they came from.
#[derive(Debug, Clone)]
struct Reductions {
    dir: PathBuf,
    versions: Vec<Reduction>,
}

impl Reductions {
    /// The reductions in the cell directory `dir` of `recording_actor`, one
    /// version per file at `<dir>/reducties/<id>/<valid_from>.yaml`
    /// (RFC-048). A version that is no `REDUCTIE`, whose `$id` is not its
    /// directory's name, or whose file is not named after its `valid_from`
    /// is an error. So is any other entry under `reducties` that is not
    /// hidden (a name starting with `.`), and a `dir` that is no directory. Without a `reducties` directory the cell has
    /// none.
    fn load(recording_actor: &str, dir: &Path) -> Result<Self> {
        if !dir.is_dir() {
            return Err(setup(format!(
                "{}: no directory of the cell of '{recording_actor}'",
                dir.display()
            )));
        }
        let mut versions = Vec::new();
        let root = dir.join("reducties");
        if root.exists() && !root.is_dir() {
            return Err(setup(format!("{}: no directory", root.display())));
        }
        if !root.is_dir() {
            return Ok(Self {
                dir: root,
                versions,
            });
        }
        let mut paths = Vec::new();
        // A hidden entry (`.gitkeep`, `.DS_Store`) is no misplaced version.
        let hidden = |path: &Path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        };
        for entry in std::fs::read_dir(&root)? {
            let id_dir = entry?.path();
            if hidden(&id_dir) {
                continue;
            }
            if !id_dir.is_dir() {
                return Err(setup(format!(
                    "{}: not a directory of a reduction; a version belongs at \
                     reducties/<id>/<valid_from>.yaml",
                    id_dir.display()
                )));
            }
            for version in std::fs::read_dir(&id_dir)? {
                let path = version?.path();
                if hidden(&path) {
                    continue;
                }
                if !path.is_file() || path.extension().is_none_or(|x| x != "yaml") {
                    return Err(setup(format!(
                        "{}: not a version of a reduction; a version is a file \
                         <valid_from>.yaml",
                        path.display()
                    )));
                }
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            let reduction = Reduction::parse(std::fs::read_to_string(&path)?)
                .map_err(|e| setup(format!("{}: {e}", path.display())))?;
            let named = path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == reduction.id());
            if !named {
                return Err(setup(format!(
                    "{}: the reduction '{}' is not in a directory of that name",
                    path.display(),
                    reduction.id()
                )));
            }
            let valid_from = reduction.valid_from().format("%Y-%m-%d").to_string();
            if path.file_stem().is_none_or(|stem| *stem != *valid_from) {
                return Err(setup(format!(
                    "{}: the version is valid from {valid_from}, so its file is {valid_from}.yaml",
                    path.display()
                )));
            }
            versions.push(reduction);
        }
        Ok(Self {
            dir: root,
            versions,
        })
    }
}

impl Cell {
    /// The cell of `recording_actor`, with its chronicle in the JSON Lines file
    /// at `path` and its reductions in its directory `cell_dir`, one version
    /// per file at `<cell_dir>/reducties/<id>/<valid_from>.yaml` (RFC-048).
    /// A `cell_dir` that is no directory is an error; one without
    /// `reducties` gives a cell without reductions. A version that is no
    /// `REDUCTIE`, whose `$id` is not its directory's name, or whose file is
    /// not named after its `valid_from` is an error. So is any other entry
    /// under `reducties`. A chronolexogram in the chronicle recorded by
    /// another actor is an error too.
    pub fn open(
        recording_actor: impl Into<String>,
        path: impl AsRef<Path>,
        cell_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let recording_actor = recording_actor.into();
        let reductions = Reductions::load(&recording_actor, cell_dir.as_ref())?;
        let chronicle = Chronicle::open(path)?;
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
        Ok(Self {
            recording_actor,
            chronicle,
            reductions,
        })
    }

    /// The cell of `recording_actor`, with an empty chronicle in memory and
    /// its reductions in its directory `cell_dir`, loaded as [`Cell::open`]
    /// loads them.
    pub fn in_memory(
        recording_actor: impl Into<String>,
        cell_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let recording_actor = recording_actor.into();
        let reductions = Reductions::load(&recording_actor, cell_dir.as_ref())?;
        Ok(Self {
            recording_actor,
            chronicle: Chronicle::in_memory(),
            reductions,
        })
    }

    /// Load the cell's reductions for a decision on `on`
    /// ([`reading::load`]), so it can answer the engine with them
    /// ([`Reader`]). Refused when a term one of them fills has no reduction
    /// in force on `on`: the term would then be the deciding authority's
    /// finding, or its default, where the cell's own reading is expected. A
    /// replay so fails too. So is a reduction in force on `on` that fills a
    /// term its article does not declare that day.
    fn load<'s>(&self, service: &'s mut LawExecutionService, on: NaiveDate) -> Result<Loaded<'s>> {
        let reductions = &self.reductions;
        if let Some(term) = reading::unfilled(&reductions.versions, on) {
            return Err(setup(format!(
                "open term {term} is filled by a reduction in {}, but none is in force on {on}; \
                 the cell does not take it as a finding of the deciding authority",
                reductions.dir.display()
            )));
        }
        if let Some(term) = reading::filled_twice(&reductions.versions, on) {
            return Err(setup(format!(
                "open term {term} is filled by two reductions in force on {on} in {}",
                reductions.dir.display()
            )));
        }
        reading::refuse_undeclared_terms(service, &reductions.versions, on)?;
        reading::load(service, &self.recording_actor, &reductions.versions)
    }

    /// The cell as the engine's answerer for a decision that reads `grams`
    /// ([`Reader`]), explaining the term `explain` if one is given.
    fn reader<'c>(&'c self, grams: Vec<Gram>, explain: Option<&'c str>) -> Reader<'c> {
        Reader::new(
            &self.recording_actor,
            &self.reductions.versions,
            grams,
            explain,
        )
    }

    /// The open terms the cell's reductions in force on `on` fill.
    fn terms_on(&self, on: NaiveDate) -> Vec<Term> {
        reading::terms_on(&self.reductions.versions, on)
    }

    /// The lexostatussen the cell can answer: per version of each reduction
    /// in its directory, the open term it fills (`implements`), with the
    /// reduction's `$id` and the version's `valid_from`. Ordered by
    /// reduction, then by `valid_from`. The list comes from the reductions
    /// the cell loaded when it was opened.
    pub fn answers(&self) -> Vec<Answer> {
        self.reductions
            .versions
            .iter()
            .flat_map(Reduction::answers)
            .collect()
    }

    pub fn recording_actor(&self) -> &str {
        &self.recording_actor
    }

    pub fn grams(&self) -> &[Gram] {
        self.chronicle.grams()
    }

    /// The cell's reductions (RFC-048), as [`Cell::open`] or
    /// [`Cell::in_memory`] read them from its directory.
    pub fn reductions(&self) -> &[Reduction] {
        &self.reductions.versions
    }

    /// A copy of this cell with its chronicle in memory, to try a decision
    /// on: what is recorded in the copy is not kept in this cell or its file.
    pub fn sandbox(&self) -> Self {
        Self {
            recording_actor: self.recording_actor.clone(),
            chronicle: self.chronicle.in_memory_copy(),
            reductions: self.reductions.clone(),
        }
    }

    /// Record the submission that article `article` of `law_id` establishes,
    /// received on `on`, with the values the applicant and the intake channel
    /// supplied in `supplied`, referring to the decision `refers_to` in this
    /// chronicle if it is given.
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
    /// supplies neither. Without `refers_to`, a `decision` field is left
    /// out. The cell refuses a reference to what is no decision in its
    /// chronicle.
    pub fn record_submission(
        &mut self,
        service: &mut LawExecutionService,
        law_id: &str,
        article: &str,
        refers_to: Option<&str>,
        supplied: &Map<String, serde_json::Value>,
        on: NaiveDate,
    ) -> Result<Gram> {
        let shape = shape::derive(service, law_id, article, on)?;
        let (filled, from_referred) = self.referred(service, &shape, refers_to)?;
        check_supplied(&shape, supplied, &from_referred)?;
        self.record(service, &shape, supplied, filled, on)
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
        refers_to: Option<&str>,
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
                shape.grondslag(),
                decisions.len(),
                decisions.join(", ")
            )));
        }
        let Some(id) = refers_to else {
            return Ok((Filled::default(), decisions));
        };
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
        // What the decision took and gave, in the law it was taken under.
        let taken_on = referred.recorded_on;
        let referred_shape =
            decision_shape_of(service, referred, taken_on, &self.terms_on(taken_on))?;
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
                        shape.grondslag(),
                        referred.grondslag,
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
        let found = gram
            .grondslag
            .split_once('#')
            .and_then(|(law_id, article)| {
                shape::article_on(service, law_id, article, gram.recorded_on)
            })
            .ok_or_else(|| {
                setup(format!(
                    "chronolexogram '{id}' is under {}, which has no version in force on {}",
                    gram.grondslag, gram.recorded_on
                ))
            })?;
        let takes = found
            .get_decision()
            .and_then(|decision| decision.legal_character())
            .is_some_and(|kind| kind == gram.kind);
        if !takes || gram.refers_to.is_none() {
            return Err(refused(format!(
                "chronolexogram '{id}' is a {} under {}, not a decision",
                gram.kind, gram.grondslag
            )));
        }
        Ok(gram)
    }

    /// Record a submission of `shape` on `on`: `supplied` from the
    /// submitter, already checked, and `filled` from the cell.
    fn record(
        &mut self,
        service: &mut LawExecutionService,
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
                let value = self.decision_value(service, &gram)?;
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
            grondslag: shape.grondslag(),
            refers_to: filled.refers_to,
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment,
            fields,
            missing,
            outputs: output_values,
        };
        Ok(self.chronicle.append(gram)?.clone())
    }

    /// Take the decision of article `article` of `law_id` on the application
    /// recorded as chronolexogram `application`, on `on` (RFC-047).
    ///
    /// What the decision asks comes from the application, the holder's
    /// policy and the cell ([`decision::derive_with`]). The cell answers the
    /// engine for the whole decision ([`Reader`], RFC-048): where an article
    /// reads an open term one of its reductions fills, also an article the
    /// decision calls, the engine asks the cell, which runs that reduction on
    /// its chronicle and gives the lexostatus; the service holds the
    /// reductions during the decision only. `supplied`
    /// holds what the deciding authority
    /// supplies itself, such as a parameter of a hook, and `findings` what it
    /// found for this case, by the id of the open term the law leaves to it
    /// per case (`decided_per_case_by`, RFC-050). All findings reach the
    /// engine apart from the parameters, so each applies to every article in
    /// the decision that leaves the term to the authority: the deciding
    /// article, and an article it calls, such as a decision called anew. The
    /// cell checks a finding on a term of the deciding article as one of its
    /// inputs; the engine refuses one that no article in the decision asks,
    /// or of the wrong type. A finding for a call anew is so given only once
    /// that call happens: given while the decision does not call anew (the
    /// objection is not admissible, say), no article asks it and the
    /// decision is refused. The chronolexogram keeps each finding among its
    /// fields; which of them the decision used, and not a regulation that
    /// set it aside, a replay says ([`Cell::findings`]).
    ///
    /// The cell refuses an application that is not in its chronicle or not
    /// one of this article, one already decided on, a value it takes from
    /// elsewhere, and a value of the wrong type. It runs the stages of
    /// [`decision::STAGES`] on the day `on` and appends one chronolexogram
    /// that refers to the application. The chronicle's clock keeps the
    /// decision on or after the application.
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
        law_id: &str,
        article: &str,
        application: &str,
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
        // The decision values it gets, from replays of those decisions
        // (each loads the reductions itself), before the reductions are
        // loaded for this one.
        let answered = self.terms_on(on);
        let unloaded = decision::derive_with(service, law_id, article, on, &answered)?;
        let applied = self.application(&unloaded, application)?.clone();
        let decisions = self.decision_values(service, &unloaded, &applied, on)?;
        let grams = self.grams_but(except.as_deref());
        let loaded = self.load(service, on)?;
        let reader = self.reader(grams, None);
        // A term the cell answers is the policy's first.
        let shape = unloaded;
        if let Some(name) = supplied.keys().find(|name| is_finding(&shape, name)) {
            return Err(refused(format!(
                "'{name}' of the decision of {} is a finding of the deciding authority; it \
                 is given among the findings",
                shape.grondslag()
            )));
        }
        // A finding the decision asks is one of its inputs, which the cell
        // checks as it checks what is supplied; any other is for an article
        // the decision calls. All of them reach the engine as findings.
        for (name, value) in findings {
            refuse_decimal_key(name, value)?;
        }
        let own: Map<_, _> = findings
            .iter()
            .filter(|(name, _)| shape.input(name).is_some())
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        if let Some(name) = own.keys().find(|name| !is_finding(&shape, name)) {
            return Err(refused(format!(
                "'{name}' of the decision of {} is no finding; it is supplied, or comes from \
                 elsewhere",
                shape.grondslag()
            )));
        }
        let mut given = supplied.clone();
        given.extend(own);
        let given = &given;
        check_decision_supplied(&shape, given)?;
        let inputs = asked(&shape, &applied, &decisions, given, on)?;
        let (mut fields, mut outputs) = take(&loaded, &shape, inputs, findings, &reader, on)?;
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
            let gram = self.decided(&loaded, &shape, &name, value, &applied, &reader, on)?;
            outputs.insert(name, serde_json::Value::String(gram.id.clone()));
            taken.push(gram);
        }
        // A decision is replaced once: the cell does not record a second
        // decision in the place of the same one.
        drop(reader);
        drop(loaded);
        if let Some(contested) = except.as_deref().filter(|_| !taken.is_empty()) {
            if let Some((earlier, replacement)) = self.replacement_of(contested) {
                return Err(refused(format!(
                    "the decision of {} would take a decision in the place of '{contested}', \
                     which chronolexogram '{replacement}' already replaces (taken by '{earlier}')",
                    shape.grondslag()
                )));
            }
        }
        let mut grams = vec![Gram {
            id,
            kind: shape.kind.clone(),
            grondslag: shape.grondslag(),
            refers_to: Some(applied.id.clone()),
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
    pub fn explain(&self, service: &mut LawExecutionService, id: &str) -> Result<ArticleResult> {
        Ok(self.replay(service, id, None)?.result)
    }

    /// How the cell came to its answer on the open term `term` (its id) in
    /// the decision `id`: the decision taken again (`Cell::replay`), and the
    /// cell's reduction for `term` run with tracing on the chronicle as the
    /// decision read it, with the parameters the engine gave it then. One
    /// result per time the decision asked the term, in that order, each
    /// with the reduction's own trace; it is no part of the decision's trace,
    /// and nothing of it is stored. A silent answer (null), which the engine
    /// set aside for the finding, is explained too. The term is matched by
    /// its id, as findings are, so two laws with a term of that id give
    /// both. Refused when the decision did not ask the cell for `term`.
    pub fn explain_answer(
        &self,
        service: &mut LawExecutionService,
        id: &str,
        term: &str,
    ) -> Result<Vec<ArticleResult>> {
        let explained: Vec<ArticleResult> = self
            .replay(service, id, Some(term))?
            .asked
            .into_iter()
            .filter(|asked| asked.term.open_term == term)
            .filter_map(|asked| asked.explained)
            .collect();
        if explained.is_empty() {
            return Err(refused(format!(
                "the decision '{id}' did not ask the cell of '{}' for '{term}'",
                self.recording_actor
            )));
        }
        Ok(explained)
    }

    /// The fields of the decision `id` that hold a finding of the deciding
    /// authority it used (RFC-050), derived from a replay: the engine reports
    /// who filled each open term (`ArticleResult::open_terms`). A field on a
    /// term the law leaves to the authority is not one when the replay
    /// reports only a regulation filled it, a reduction of the cell among
    /// them: the regulation decided it, also where it gives what the
    /// authority found. It stays one where a finding filled it too, and where
    /// the decision did not read it. A field the decision's article does not
    /// ask is a finding given for an article it called. The report is read by
    /// the term's id, across every article the decision runs.
    ///
    /// Nothing about findings is stored: the execution is deterministic, so
    /// the replay of the decision in its own context gives the same report.
    pub fn findings(&self, service: &mut LawExecutionService, id: &str) -> Result<Vec<String>> {
        let gram = self.decision_gram(service, id)?.clone();
        let replayed = self.replay(service, id, None)?;
        Ok(used(
            &candidates(&replayed.shape, &gram),
            &replayed.result.open_terms,
        ))
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
        let position = grams
            .iter()
            .position(|g| g.refers_to.as_deref() == Some(submission_id.as_str()))
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
        let (law_id, article) = decision.grondslag.split_once('#').ok_or_else(|| {
            setup(format!(
                "chronolexogram '{}' has grondslag '{}', not <regulation>#<article>",
                decision.id, decision.grondslag
            ))
        })?;
        let on = decision.recorded_on;
        // The decision values first: each replays its own decision, which
        // loads the reductions itself.
        let answered = self.terms_on(on);
        let shape = decision::derive_with(service, law_id, article, on, &answered)?;
        let decisions = self.decision_values(service, &shape, &submission, on)?;
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
        let loaded = self.load(service, on)?;
        let reader = self.reader(before, explain);
        let given = candidates(&shape, &decision);
        let mut parameters = BTreeMap::new();
        let mut findings = BTreeMap::new();
        for (name, value) in &decision.fields {
            if given.contains_key(name) {
                findings.insert(name.clone(), stored::from_stored(value)?);
                continue;
            }
            let Some(input) = shape.input(name) else {
                continue;
            };
            if input.source == Source::Policy {
                continue;
            }
            let value = decisions.get(name).map_or(value, |(_, v)| v);
            parameters.insert(name.clone(), stored::from_stored(value)?);
        }
        let outputs: Vec<&str> = shape.outputs.iter().map(String::as_str).collect();
        let date = on.to_string();
        let call = Call {
            findings,
            answerer: Some(&reader),
        };
        let result =
            loaded.evaluate_law_with_trace_in(law_id, &outputs, parameters, call, &date)?;
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
            let same = if replaces.contains(name) {
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
            let answered = self.terms_on(target.recorded_on);
            decision_shape_of(&loaded, &target, target.recorded_on, &answered)?
        };
        let asked = reader.asked();
        drop(loaded);
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
            let mut value = self.decision_value(service, &gram)?;
            refresh(service, &gram, &mut value, on, &self.terms_on(on))?;
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
    /// replacing decision.
    fn replacement_of(&self, id: &str) -> Option<(&str, &str)> {
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
                on_it().find_map(|decision| {
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

    /// The decision value `{grondslag, taken_on, fields, findings,
    /// outputs}` of the decision `gram`, as the engine builds one (RFC-050),
    /// in the law in force on the day it was recorded: of its outputs those
    /// its decision item names, and of its fields:
    ///
    /// - under `findings`, the deciding authority's findings for this case
    ///   that the decision used ([`Cell::findings`], from a replay) on the
    ///   open terms the deciding article leaves to it per case
    ///   (`decided_per_case_by`): only the deciding article's own;
    /// - no value for such a term that a regulation filled in the
    ///   authority's place: the regulation decided it, and a call anew reads
    ///   the regulation in force on its own day;
    /// - no finding given for an article the decision called: that is
    ///   recorded with the decision that article produced, the new decision;
    /// - under `fields`, the others.
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
        gram: &Gram,
    ) -> Result<serde_json::Value> {
        let used = self.findings(service, &gram.id)?;
        let found = gram
            .grondslag
            .split_once('#')
            .and_then(|(law_id, article)| {
                shape::article_on(service, law_id, article, gram.recorded_on)
            })
            .ok_or_else(|| {
                setup(format!(
                    "the decision '{}' is under {}, which has no version in force on {}",
                    gram.id, gram.grondslag, gram.recorded_on
                ))
            })?;
        let decided = found
            .get_decision()
            .map(|decision| found.outputs_of(decision))
            .unwrap_or_default();
        let judged: Vec<&str> = found
            .get_open_terms()
            .into_iter()
            .flatten()
            .filter(|term| term.decided_per_case_by.is_some())
            .map(|term| term.id.as_str())
            .collect();
        let mut fields = Map::new();
        let mut findings = Map::new();
        for (name, value) in &gram.fields {
            let by_authority = used.contains(name);
            match (judged.contains(&name.as_str()), by_authority) {
                (true, true) => {
                    findings.insert(name.clone(), value.clone());
                }
                (true, false) | (false, true) => {}
                (false, false) => {
                    fields.insert(name.clone(), value.clone());
                }
            }
        }
        let outputs: Map<String, serde_json::Value> = gram
            .outputs
            .iter()
            .filter(|(name, _)| decided.contains(&name.as_str()))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        Ok(serde_json::json!({
            "grondslag": gram.grondslag,
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
            .map(|submission| submission.grondslag.as_str())
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
        answerer: &dyn OpenTermAnswerer,
        on: NaiveDate,
    ) -> Result<Gram> {
        let not_a_decision = || {
            setup(format!(
                "'{output}' of the decision of {} is {value}, no decision value \
                 {{grondslag, fields, findings, outputs}}",
                deciding.grondslag()
            ))
        };
        let object = value
            .as_object()
            .filter(|o| shape::is_decision_value(o))
            .ok_or_else(not_a_decision)?;
        let grondslag = object
            .get("grondslag")
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
        let (law_id, article) = grondslag.split_once('#').ok_or_else(not_a_decision)?;
        let shape = decision::derive_with(service, law_id, article, on, &self.terms_on(on))?;
        let mut fields = Values::new();
        for input in &shape.inputs {
            let name = &input.field.name;
            // A policy value of the day of the old decision is no input:
            // the stages read the policy in force on `on`, as for `decide`.
            let value = match input.source {
                Source::Cell => Some(serde_json::Value::String(on.to_string())),
                Source::Policy => None,
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
            answerer,
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
                "'{output}' of the decision of {} is a decision under {grondslag} that takes \
                 a decision in turn ('{nested}'); the cell records one level",
                deciding.grondslag()
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
                "'{output}' of the decision of {} is a decision under {grondslag} with outputs \
                 {}, but taken again on these fields that article decides {}",
                deciding.grondslag(),
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
            .filter(|t| *t != submission.grondslag)
        {
            return Err(setup(format!(
                "'{output}' of the decision of {} is a decision taken on {taken_on}, but the \
                 cell takes it on '{}', under {}",
                deciding.grondslag(),
                submission.id,
                submission.grondslag
            )));
        }
        Ok(Gram {
            id: uuid::Uuid::now_v7().to_string(),
            kind: shape.kind.clone(),
            grondslag: shape.grondslag(),
            refers_to: Some(submission.id.clone()),
            recording_actor: self.recording_actor.clone(),
            recorded_on: on,
            moment: None,
            fields,
            missing: Vec::new(),
            outputs,
        })
    }

    /// The application chronolexogram `id` that the decision of `shape` is taken on:
    /// one in this chronicle, of this article and kind, not decided on yet.
    fn application(&self, shape: &DecisionShape, id: &str) -> Result<&Gram> {
        let gram = self.grams().iter().find(|g| g.id == id).ok_or_else(|| {
            refused(format!(
                "there is no chronolexogram '{id}' in the chronicle of '{}'",
                self.recording_actor
            ))
        })?;
        if gram.kind != shape.submission_kind || gram.grondslag != shape.application {
            return Err(refused(format!(
                "chronolexogram '{id}' is a {} under {}, not an {} under {} that {} decides on",
                gram.kind,
                gram.grondslag,
                shape.submission_kind,
                shape.application,
                shape.grondslag()
            )));
        }
        if let Some(decided) = self
            .grams()
            .iter()
            .find(|g| g.refers_to.as_deref() == Some(id))
        {
            return Err(refused(format!(
                "the application '{id}' is already decided on, by chronolexogram '{}'",
                decided.id
            )));
        }
        Ok(gram)
    }
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

/// The names in `given` whose finding the decision used: all but those
/// the engine reports a regulation filled in the authority's place
/// ([`ArticleResult::open_terms`]), also where the regulation gives what
/// the authority found. A finding the decision did not read is not set
/// aside, so it stays a finding. A finding applies by the term's id to
/// every article in the decision (RFC-050), so the report is read by id.
/// A term listed as filled by both counts as used: the regulation that
/// fills it read the finding in its place, so the finding was used.
fn used(given: &Map<String, serde_json::Value>, filled: &[OpenTermFilled]) -> Vec<String> {
    let by = |name: &str, finding: bool| {
        filled
            .iter()
            .any(|f| f.term == name && matches!(f.filled_by, FilledBy::Finding) == finding)
    };
    given
        .keys()
        .filter(|name| by(name, true) || !by(name, false))
        .cloned()
        .collect()
}

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
    answerer: &dyn OpenTermAnswerer,
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
        answerer,
        on,
        &mut fields,
    )?;
    Ok((fields, outputs))
}

/// What may hold a finding of the deciding authority in the decision
/// `gram` of `shape`: a field on a term the law leaves to it per case, and
/// a field the decision's article does not ask, given for an article it
/// called.
///
/// A null on such a term is none: the cell puts it in for a judgement the
/// authority left out, and the engine resolves the term as null without it.
fn candidates(shape: &DecisionShape, gram: &Gram) -> Map<String, serde_json::Value> {
    gram.fields
        .iter()
        .filter(|(name, value)| {
            !value.is_null() && (shape.input(name).is_none() || is_finding(shape, name))
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
    answered: &[Term],
) -> Result<()> {
    let shape = decision_shape_of(service, gram, on, answered)?;
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
                        shape.grondslag()
                    )))
                }
                None => None,
            },
            Source::Caller => match supplied.get(name) {
                Some(value) => Some(value.clone()),
                // A regulation fills it first; where it is silent for this
                // case, the engine says the judgement is missing.
                None if input.policy_first => None,
                None if input.field.nullable => Some(serde_json::Value::Null),
                None => {
                    return Err(refused(format!(
                        "the decision of {} asks '{name}' of the deciding authority ({})",
                        shape.grondslag(),
                        input.field.first_declared_by()
                    )))
                }
            },
            Source::Cell => Some(serde_json::Value::String(on.to_string())),
            // The engine resolves it; the gram takes what it used.
            Source::Policy => None,
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
    answerer: &dyn OpenTermAnswerer,
    on: NaiveDate,
    fields: &mut BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, serde_json::Value>> {
    let date = on.to_string();
    let mut outputs = BTreeMap::new();
    for stage in &shape.stages {
        // Every stage computes the decision's outputs, so a hook at a later
        // stage can read them; they are the same at each.
        let call = Call {
            findings: findings.clone(),
            answerer: Some(answerer),
        };
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
        for input in &shape.inputs {
            let name = &input.field.name;
            let Some(value) = result.resolved_inputs.get(name) else {
                continue;
            };
            let value = stored::to_stored(value)?;
            match &input.source {
                Source::Policy => {
                    fields.insert(name.clone(), value);
                }
                // A judgement on a term a regulation also fills: the gram
                // takes what the engine used, which is the regulation's
                // value where it fills the term (RFC-047).
                Source::Caller if input.policy_first => {
                    fields.insert(name.clone(), value);
                }
                _ => {}
            }
        }
    }
    Ok(outputs)
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
    answered: &[Term],
) -> Result<DecisionShape> {
    let (law_id, article) = decision.grondslag.split_once('#').ok_or_else(|| {
        setup(format!(
            "chronolexogram '{}' has grondslag '{}', not <regulation>#<article>",
            decision.id, decision.grondslag
        ))
    })?;
    decision::derive_with(service, law_id, article, on, answered)
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

/// Whether `name` is a finding of the deciding authority on the decision of
/// `shape`: an open term the deciding article leaves to it per case
/// (`decided_per_case_by`), also one a regulation may fill.
fn is_finding(shape: &DecisionShape, name: &str) -> bool {
    shape.judgements.iter().any(|j| j == name)
        || shape.input(name).is_some_and(|input| input.policy_first)
}

/// Refuse what the deciding authority does not supply: a value the
/// decision takes from the application, the policy or the cell, a name the
/// decision does not ask, a value of the wrong type.
fn check_decision_supplied(
    shape: &DecisionShape,
    supplied: &Map<String, serde_json::Value>,
) -> Result<()> {
    let decision = shape.grondslag();
    for (name, value) in supplied {
        let input = shape
            .input(name)
            .ok_or_else(|| refused(format!("the decision of {decision} does not ask '{name}'")))?;
        let from = match &input.source {
            Source::Caller => None,
            Source::Application => Some("the application".to_string()),
            Source::Policy => Some("the policy".to_string()),
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

/// Refuse what the law does not ask of the submitter, and what comes from
/// the decision the submission refers to (`from_referred`).
fn check_supplied(
    shape: &Shape,
    supplied: &Map<String, serde_json::Value>,
    from_referred: &[String],
) -> Result<()> {
    let submission = shape.grondslag();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn filling(term: &str, filled_by: FilledBy) -> OpenTermFilled {
        OpenTermFilled {
            law_id: "wet".to_string(),
            article: "1".to_string(),
            term: term.to_string(),
            filled_by,
        }
    }

    fn regulation() -> FilledBy {
        FilledBy::Regulation {
            law_id: "beleid".to_string(),
            article: "1".to_string(),
        }
    }

    #[test]
    fn used_keeps_a_finding_a_regulation_did_not_set_aside() {
        let given: Map<String, serde_json::Value> = serde_json::from_value(serde_json::json!({
            "door_vaststelling": true,
            "door_regeling": true,
            "door_beide": true,
            "niet_gelezen": true,
        }))
        .unwrap();
        let filled = [
            filling("door_vaststelling", FilledBy::Finding),
            filling("door_regeling", regulation()),
            filling("door_beide", regulation()),
            filling("door_beide", FilledBy::Finding),
        ];
        let mut found = used(&given, &filled);
        found.sort();
        assert_eq!(found, ["door_beide", "door_vaststelling", "niet_gelezen"]);
    }
}
