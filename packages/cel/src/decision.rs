//! What the cell records a decision on an application with, derived from
//! the law (RFC-047).
//!
//! An article that establishes an application and decides on it follows a
//! procedure (RFC-008). The decision runs at the stage BESLUIT and is made
//! known at the stage BEKENDMAKING; at each, the engine names the articles
//! that take part ([`LawExecutionService::procedure_stage`]). The cell runs
//! them with, in this order of preference:
//!
//! 1. the application: a parameter of the deciding article, or one the
//!    application holds or gives (a hook's output) by the same name, or the
//!    parameter of its moment, such as the receipt (Awb 4:13);
//! 2. what the deciding authority supplies, such as a parameter of a hook the
//!    application does not hold;
//! 3. the day of the decision, for a date the stage requires.
//!
//! An open term is no input. A policy that fills it (`implements`) is the
//! engine's to run, and its value is the term's (RFC-048 §2). A term that
//! nothing fills and that the law or policy marks as judged per case
//! (`decided_per_case_by`) is a finding, which the cell keeps among the
//! fields. It is the finding of the authority that answers the term: the
//! one its `competent_authority` names, else the one that executes the
//! article. Which terms those are, and who answers each, the engine says
//! ([`LawExecutionService::decision_terms`]). What a process asks of whom,
//! and when, is not the cell's.

use chrono::NaiveDate;
use regelrecht_engine::{
    Authority, DecisionTerm, LawExecutionService, ParameterType, ProcedureStage,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::error::{refused, setup, Result};
use crate::shape::{self, add_field, article_on, Field};

/// The stages a decision runs, in order: the first of these pairs that the
/// procedure of its article holds. For a beschikking the decision itself
/// (Awb 1:3) and the announcement without which it has no effect (Awb 3:40);
/// for the judgment of an administrative court the judgment (Awb 8:70) and
/// the copy that the clerk makes available to the parties (Awb 8:79, RFC-053).
/// The cell announces or sends the decision on the day it takes it: that is
/// its assumption, not the law's.
pub const STAGES: [[&str; 2]; 2] = [["BESLUIT", "BEKENDMAKING"], ["UITSPRAAK", "TOEZENDING"]];

/// Where the cell takes the value of a parameter of a decision from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum Source {
    /// The application the decision is taken on (the chronolexogram's `refers_to`).
    Application,
    /// What the deciding authority supplies when it takes the decision.
    Supplied,
    /// The cell: the day of the decision.
    Cell,
}

/// A parameter of the decision, with where the cell takes it from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Input {
    #[serde(flatten)]
    pub field: Field,
    pub source: Source,
}

/// What a decision on an application holds, as the law in force on a day
/// says.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionShape {
    /// The legal character of the decision (`produces.legal_character`),
    /// such as `BESCHIKKING`.
    pub kind: String,
    pub law_id: String,
    pub article: String,
    /// The kind of application it decides on, such as `AANVRAAG`.
    pub submission_kind: String,
    /// The article that establishes that application, `<regulation>#<article>`:
    /// the one whose submission asks for this decision (`requests`), or this
    /// article in the object form.
    pub application: String,
    /// The stages of [`STAGES`] that its procedure holds, as the engine names
    /// them.
    pub stages: Vec<ProcedureStage>,
    /// The parameters the stages ask, the deciding article first.
    pub inputs: Vec<Input>,
    /// The outputs of the decision (its item in `produces`): what it decides.
    pub outputs: Vec<String>,
    /// The open terms the decision may need, as the engine names them
    /// ([`LawExecutionService::decision_terms`]): of the deciding article,
    /// and of each article that fills one of them.
    pub terms: Vec<DecisionTerm>,
}

impl DecisionShape {
    /// The deciding article, `<regulation>#<article>`.
    pub fn basis(&self) -> String {
        format!("{}#{}", self.law_id, self.article)
    }

    pub fn input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|i| i.field.name == name)
    }

    /// The finding `name` the decision may take from `authority`: an open
    /// term that nothing fills, of the deciding article or of a policy that
    /// fills one of its terms, judged per case and answered by `authority`
    /// ([`DecisionTerm::is_finding`]).
    pub fn finding(&self, name: &str, authority: &Authority) -> Option<&DecisionTerm> {
        self.terms
            .iter()
            .find(|t| t.id == name && t.is_finding(authority))
    }

    /// Split what the deciding authority gives when it takes the decision
    /// into what it supplies and its findings, as [`Cell::decide`] takes
    /// them: a parameter of the decision is supplied, anything else is a
    /// finding. Who may give each, and whether it fits, `decide` checks
    /// after the split, so `decide` still refuses a parameter the
    /// application holds, and a finding of another authority.
    ///
    /// [`Cell::decide`]: crate::Cell::decide
    pub fn split(&self, given: &Map<String, Value>) -> (Map<String, Value>, Map<String, Value>) {
        given
            .clone()
            .into_iter()
            .partition(|(name, _)| self.input(name).is_some())
    }

    /// Whether a regulation fills the open term `name` of the deciding
    /// article (`implements`): the engine resolves it, and the cell records
    /// the value it used.
    pub fn filled(&self, name: &str) -> bool {
        self.terms.iter().any(|t| {
            t.id == name
                && t.law_id == self.law_id
                && t.article == self.article
                && !t.filled_by.is_empty()
        })
    }
}

/// The field a finding on `term` takes: of its type, absent only where the
/// term is not required and has no default, declared by its article.
pub fn finding_field(term: &DecisionTerm) -> Field {
    Field {
        name: term.id.clone(),
        type_: term.term_type,
        nullable: !term.required && !term.has_default,
        declared_by: vec![term.declared_by()],
    }
}

/// Derive what the decision of article `article` of `law_id` on its
/// application asks, in the law in force on `day`.
pub fn derive(
    service: &LawExecutionService,
    law_id: &str,
    article: &str,
    day: NaiveDate,
) -> Result<DecisionShape> {
    let reference = format!("{law_id}#{article}");
    let deciding = article_on(service, law_id, article, day)
        .ok_or_else(|| refused(format!("{reference} has no version in force on {day}")))?;
    let decision = deciding.get_decision();
    let kind = decision
        .and_then(|p| p.legal_character())
        .map(str::to_string)
        .ok_or_else(|| refused(format!("{reference} takes no decision (legal_character)")))?;
    // The application it decides on: the submission that asks for this
    // decision with `requests` (RFC-047), in this article or another.
    let date = day.to_string();
    let applications = service.applications_for(law_id, article, &date)?;
    let application_reference = match applications.as_slice() {
        [one] => one.clone(),
        [] => {
            return Err(refused(format!(
                "{reference}: no application asks for this decision (produces requests)"
            )))
        }
        several => {
            return Err(refused(format!(
                "{reference}: several applications ask for this decision ({}); the cell \
                 decides on one kind of application per decision",
                several.join(", ")
            )))
        }
    };
    let (application_law, application_article) = application_reference
        .split_once('#')
        .ok_or_else(|| setup(format!("'{application_reference}' is no article reference")))?;
    let application = shape::derive(service, application_law, application_article, day)?;
    let submission_kind = application.kind.clone();
    let held: Vec<&str> = service
        .procedure_of(law_id, deciding)?
        .map(|procedure| procedure.stages.iter().map(|s| s.name.as_str()).collect())
        .unwrap_or_default();
    let pair = STAGES
        .iter()
        .find(|pair| pair.iter().all(|stage| held.contains(stage)))
        .unwrap_or(&STAGES[0]);
    let stages = pair
        .iter()
        .map(|stage| service.procedure_stage(law_id, article, stage, &date))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    // The decision's outputs: what it decides.
    let outputs: Vec<&str> = decision.map(|d| deciding.outputs_of(d)).unwrap_or_default();
    // What the execution supplies at every stage of the decision: those, and
    // the data derived for it (RFC-050). A hook may read either as a
    // parameter; it is no field of the decision.
    let computed: Vec<&str> = outputs
        .iter()
        .copied()
        .chain(
            decision
                .map(|_| deciding.derived_outputs())
                .unwrap_or_default(),
        )
        .collect();

    let mut fields: Vec<Field> = Vec::new();
    let mut sources: HashMap<String, Source> = HashMap::new();
    for stage in &stages {
        for part in &stage.articles {
            let declared_by = format!("{}#{}", part.law_id, part.article_number);
            let found = article_on(service, &part.law_id, &part.article_number, day)
                .ok_or_else(|| setup(format!("{declared_by} has no version in force on {day}")))?;
            for p in found
                .get_execution_spec()
                .and_then(|e| e.parameters.as_ref())
                .into_iter()
                .flatten()
            {
                if part.hook_point.is_some() && computed.contains(&p.name.as_str()) {
                    continue;
                }
                let field = Field {
                    name: p.name.clone(),
                    type_: p.param_type,
                    nullable: p.is_nullable(),
                    declared_by: vec![declared_by.clone()],
                };
                add_field(&mut fields, field, &reference)?;
                let source = if application.field(&p.name).is_some()
                    || application.given.contains(&p.name)
                    || application.is_moment(&p.name)
                {
                    Source::Application
                } else {
                    Source::Supplied
                };
                sources.entry(p.name.clone()).or_insert(source);
            }
        }
        let required_by = format!("{} stage {}", stage.procedure_id, stage.stage);
        for req in &stage.requires {
            if req.req_type != ParameterType::Date {
                return Err(setup(format!(
                    "{required_by} requires '{}', a {:?}; the cell fills only a date \
                     (the day of the decision)",
                    req.name, req.req_type
                )));
            }
            let field = Field {
                name: req.name.clone(),
                type_: ParameterType::Date,
                nullable: false,
                declared_by: vec![required_by.clone()],
            };
            add_field(&mut fields, field, &reference)?;
            // A date the stage requires is the day of the decision, also
            // where a hook asks it as a parameter (Awb 6:8).
            sources.insert(req.name.clone(), Source::Cell);
        }
    }
    // For no instance in particular: the cell does not pass its identity to
    // the law, so a policy scoped by `gemeente_code` is not found (an open
    // point of RFC-048).
    let terms = service.decision_terms(law_id, article, &date, &HashMap::new())?;
    let inputs = fields
        .into_iter()
        .map(|field| {
            let source = sources.remove(&field.name).unwrap_or(Source::Supplied);
            Input { field, source }
        })
        .collect();
    Ok(DecisionShape {
        kind,
        law_id: law_id.to_string(),
        article: article.to_string(),
        submission_kind,
        application: application_reference,
        stages,
        inputs,
        outputs: outputs.iter().map(|o| (*o).to_string()).collect(),
        terms,
    })
}
