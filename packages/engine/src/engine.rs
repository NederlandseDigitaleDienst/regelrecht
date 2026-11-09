//! Article execution engine
//!
//! Core engine for evaluating article-level machine_readable.execution sections.
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::{ArticleEngine, ArticleBasedLaw, Value};
//! use std::collections::BTreeMap;
//!
//! let law = ArticleBasedLaw::from_yaml_file("path/to/law.yaml")?;
//! let article = law.find_article_by_output("some_output").unwrap();
//!
//! let engine = ArticleEngine::new(article, &law);
//! let mut params = BTreeMap::new();
//! params.insert("BSN".to_string(), Value::String("123456789".to_string()));
//!
//! let result = engine.evaluate(params, "2025-01-01")?;
//! println!("Output: {:?}", result.outputs);
//! ```

use crate::answer::AnswerProvenance;
use crate::article::{Action, ActionOperation, Article, ArticleBasedLaw};
use crate::context::{LazyInputs, RuleContext};
use crate::error::{EngineError, Result};
use crate::operations::{evaluate_value, execute_operation};
use crate::resolver::{DeclarationNotInForce, DeclarationsFromOtherVersion, DelegationRefusal};
use crate::trace::{LegalAnchor, PathNode, TraceBuilder};
use crate::types::{PathNodeType, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Provenance of an output value: how it was produced during execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type")]
pub enum OutputProvenance {
    /// Produced by the article's own actions.
    Direct { law_id: String, article: String },
    /// Produced by a hook firing on a lifecycle event (e.g., AWB on BESCHIKKING).
    Reactive {
        law_id: String,
        article: String,
        hook_point: String,
    },
    /// Produced by a lex specialis override (RFC-007).
    Override { law_id: String, article: String },
    /// Removed by an override stating the output does not arise at all.
    ///
    /// The output is absent from `outputs` and present here, so a consumer can
    /// tell "the law says this entitlement does not exist" from "nobody asked
    /// for it". Without that distinction the absence is silence, and silence
    /// is what this design spends most of its effort removing.
    Voided {
        law_id: String,
        article: String,
        /// The words of the overriding article that establish it, verbatim.
        grounds: Option<String>,
    },
}

/// Who filled an open term in an execution: a finding of the authority
/// that decides it per case, a regulation that implements it (RFC-045), or
/// a policy article that binds it to the answer of one or more cells
/// (RFC-048). A consumer reads here which the outcome rests on, instead of
/// guessing from the value: a regulation can give what the authority found.
/// Serialized as `"finding"`, as `{"regulation": {"law_id", "article"}}`, so
/// the regulation's article does not collide with the declaring one, or as
/// `{"answer": {"law_id", "article", "answers": [...]}}`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilledBy {
    /// A finding for this case (`decided_per_case_by`), given by the caller
    /// or with a decision value called anew.
    Finding,
    /// An article of a regulation that implements the term (`implements`).
    Regulation { law_id: String, article: String },
    /// An article of a policy that implements the term and takes the named
    /// answer of a cell (`source.reduction`): the binding, and every answer
    /// it took that was not null, one per input that takes an answer, with
    /// what each cell says it answered from, in the order the inputs were
    /// asked. Inputs may take the answers of different cells (synthesis,
    /// RFC-048). Where every cell was silent and nothing falls back, the
    /// null answers; where no cell answered, none.
    Answer {
        law_id: String,
        article: String,
        answers: Vec<AnswerProvenance>,
    },
}

/// One open term of one article that was filled in an execution, and by
/// whom. A term filled by its default, or left null, is not listed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OpenTermFilled {
    /// The law and article that declare the term, so a term of an article
    /// called by the executed one is told apart from one of its own.
    pub law_id: String,
    pub article: String,
    /// The id of the open term.
    pub term: String,
    pub filled_by: FilledBy,
}

/// Result of article execution
#[derive(Debug, Clone)]
pub struct ArticleResult {
    /// Calculated output values
    pub outputs: BTreeMap<String, Value>,
    /// Per-output provenance: which law/article/mechanism produced each output.
    pub output_provenance: BTreeMap<String, OutputProvenance>,
    /// Resolved input values (from cross-law references)
    pub resolved_inputs: BTreeMap<String, Value>,
    /// Article number that was executed
    pub article_number: String,
    /// Law ID containing the article
    pub law_id: String,
    /// Law UUID if available
    pub law_uuid: Option<String>,
    /// Execution trace tree (only populated when tracing is enabled)
    pub trace: Option<PathNode>,
    /// Engine version that produced this result (RFC-013)
    pub engine_version: String,
    /// Schema version of the regulation (RFC-013)
    pub schema_version: Option<String>,
    /// SHA-256 hash of the regulation YAML content (RFC-013)
    pub regulation_hash: Option<String>,
    /// valid_from date of the regulation version that was evaluated (RFC-013)
    pub regulation_valid_from: Option<String>,
    /// Implementations refused by the delegation gate during this execution:
    /// regulations that declared they fill an open term but sit at a layer the
    /// declaring article does not delegate to. Populated by the service layer,
    /// independently of tracing, so a refusal reaches the receipt.
    pub delegation_refusals: Vec<DelegationRefusal>,
    /// Laws whose hooks, overrides and procedures were read from a version
    /// other than the one in force on the reference date. An empty list is a
    /// statement: every such answer came from the right version.
    pub declaration_version_notes: Vec<DeclarationsFromOtherVersion>,
    /// Hooks, overrides and implementations the indexes offered but that had no
    /// version in force on the reference date, so they did not run. Populated
    /// by the service layer, independently of tracing, so the skip reaches the
    /// receipt.
    pub declarations_not_in_force: Vec<DeclarationNotInForce>,
    /// The open terms a finding or a regulation filled during this
    /// execution, in the order they were resolved, nested calls included.
    /// Populated by the service layer, independently of tracing. A term can
    /// be listed twice, once per filler: an implementing article that reads
    /// the output it fills in reads it with the finding in its place.
    pub open_terms: Vec<OpenTermFilled>,
}

/// Executes a single article's machine_readable.execution section.
///
/// The engine orchestrates the execution of an article's actions,
/// resolving variables and evaluating operations to produce outputs.
///
/// The engine evaluates **one article in isolation**: it expects every input
/// it needs to already be present in `parameters`. It does **not** resolve
/// cross-article (`source.output`) or cross-law (`source.regulation`)
/// references itself. An unresolved internal (`source.output`) input is left
/// untouched (a consuming action then surfaces a `VariableNotFound`), while an
/// unresolved external (`source.regulation`) input — which this engine can
/// never resolve on its own — yields [`EngineError::ExternalReferenceNotResolved`].
/// All reference resolution, cycle detection and depth limiting live in
/// [`crate::LawExecutionService`], which is the single resolution authority and
/// the entry point every production caller uses.
pub struct ArticleEngine<'a> {
    /// Article to execute
    article: &'a Article,
    /// Law containing the article
    law: &'a ArticleBasedLaw,
    /// Declared units for this article's symbols (RFC-023). Empty/all-unknown
    /// for un-annotated articles, which then skip unit checking entirely.
    symbols: crate::units::SymbolUnits,
}

/// The scope an article's rules execute in: the caller's parameters, the
/// optional parameters left out (RFC-036), the article's definitions, and,
/// when tracing, the provision every step is anchored to (RFC-039).
pub(crate) fn article_context<'l>(
    law: &ArticleBasedLaw,
    article: &Article,
    parameters: &BTreeMap<String, Value>,
    calculation_date: &str,
    trace: Option<&Rc<RefCell<TraceBuilder>>>,
) -> Result<RuleContext<'l>> {
    let mut context = RuleContext::new(parameters.clone(), calculation_date)?;
    context.set_law_scope(&law.id, unpassed_optional_parameters(article, parameters));
    if let Some(tb) = trace {
        context.set_trace(Rc::clone(tb));
        // This is where the engine knows both the law and the article, so it
        // is where the provision gets attached. Every step the rules push from
        // here carries it (RFC-039).
        context.set_anchor(LegalAnchor::from_article(law, article));
    }
    if let Some(definitions) = article.get_definitions() {
        context.set_definitions(definitions);
    }
    Ok(context)
}

/// The parameters `article` declares with `required: false` that `parameters`
/// does not carry (RFC-036).
///
/// A reference to one of these resolves to an Unknown for lack of that
/// parameter: the article said it can do without, so the caller leaving it out
/// is not an error, but the fact is missing and the outcome has to say so. A
/// required parameter is never in this set; a misspelled key in `parameters:`
/// stays the `VariableNotFound` it always was.
pub(crate) fn unpassed_optional_parameters(
    article: &Article,
    parameters: &BTreeMap<String, Value>,
) -> BTreeSet<String> {
    article
        .get_execution_spec()
        .and_then(|exec| exec.parameters.as_ref())
        .map(|declared| {
            declared
                .iter()
                .filter(|p| p.required == Some(false) && !parameters.contains_key(&p.name))
                .map(|p| p.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

impl<'a> ArticleEngine<'a> {
    /// Create a new article engine.
    ///
    /// # Arguments
    /// * `article` - Article to execute
    /// * `law` - Law containing the article
    pub fn new(article: &'a Article, law: &'a ArticleBasedLaw) -> Self {
        let symbols = crate::units::SymbolUnits::from_article(article);
        Self {
            article,
            law,
            symbols,
        }
    }

    /// Execute this article's logic.
    ///
    /// # Arguments
    /// * `parameters` - Input parameters (e.g., {"BSN": "123456789"})
    /// * `calculation_date` - Date for which calculations are performed (YYYY-MM-DD)
    ///
    /// # Returns
    /// * `Ok(ArticleResult)` - Execution result with outputs and metadata
    /// * `Err(EngineError)` - If execution fails
    pub fn evaluate(
        &self,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
    ) -> Result<ArticleResult> {
        self.evaluate_with_output(parameters, calculation_date, None)
    }

    /// Execute this article's logic, optionally calculating only a specific output.
    ///
    /// # Arguments
    /// * `parameters` - Input parameters (e.g., {"BSN": "123456789"})
    /// * `calculation_date` - Date for which calculations are performed (YYYY-MM-DD)
    /// * `requested_output` - Output to calculate. Only the actions it depends on
    ///   run (RFC-043); `None` runs every action.
    ///
    /// # Returns
    /// * `Ok(ArticleResult)` - Execution result with outputs and metadata
    /// * `Err(EngineError)` - If execution fails
    pub fn evaluate_with_output(
        &self,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
    ) -> Result<ArticleResult> {
        let required = self.required_for(requested_output);
        self.evaluate_outputs(parameters, calculation_date, required.as_ref(), None, None)
    }

    /// Execute this article's logic with trace support.
    ///
    /// Same as `evaluate_with_output` but accepts a shared trace builder.
    pub fn evaluate_with_trace(
        &self,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        requested_output: Option<&str>,
        trace: Rc<RefCell<TraceBuilder>>,
    ) -> Result<ArticleResult> {
        let required = self.required_for(requested_output);
        self.evaluate_outputs(
            parameters,
            calculation_date,
            required.as_ref(),
            Some(trace),
            None,
        )
    }

    /// The outputs `requested_output` depends on (RFC-043); `None` for all.
    fn required_for(&self, requested_output: Option<&str>) -> Option<BTreeSet<String>> {
        requested_output.map(|name| crate::demand::required_outputs(self.get_actions(), &[name]))
    }

    /// Execute the actions producing `outputs` (a dependency closure, see
    /// [`crate::demand::required_outputs`]); `None` runs every action
    /// (RFC-043). With `lazy`, an input or open term is resolved when an
    /// operation first reads it; without it, `parameters` must already
    /// contain every value this article needs (cross-article and cross-law
    /// resolution is [`crate::LawExecutionService`]'s job).
    pub(crate) fn evaluate_outputs(
        &self,
        parameters: BTreeMap<String, Value>,
        calculation_date: &str,
        outputs: Option<&BTreeSet<String>>,
        trace: Option<Rc<RefCell<TraceBuilder>>>,
        lazy: Option<&dyn LazyInputs>,
    ) -> Result<ArticleResult> {
        tracing::debug!(
            law_id = %self.law.id,
            article = %self.article.number,
            outputs = ?outputs,
            "Starting article evaluation"
        );

        let mut context = article_context(
            self.law,
            self.article,
            &parameters,
            calculation_date,
            trace.as_ref(),
        )?;

        // Guard against any input that still carries an unresolved external
        // source, unless inputs resolve on first read (RFC-043).
        match lazy {
            Some(lazy) => context.set_lazy(lazy),
            None => self.check_input_sources(&parameters)?,
        }

        // Execute actions (with trace instrumentation)
        self.execute_actions_traced(&mut context, outputs)?;

        // Build result
        // Tag all outputs as Direct (hooks/overrides are tagged by the service layer)
        let output_provenance: BTreeMap<String, OutputProvenance> = context
            .outputs()
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    OutputProvenance::Direct {
                        law_id: self.law.id.clone(),
                        article: self.article.number.clone(),
                    },
                )
            })
            .collect();

        let result = ArticleResult {
            outputs: context.outputs().clone(),
            output_provenance,
            // Filled by the service with what it resolved for the article.
            resolved_inputs: BTreeMap::new(),
            article_number: self.article.number.clone(),
            law_id: self.law.id.clone(),
            law_uuid: self.law.uuid.clone(),
            trace: None, // Trace is extracted by the caller (service layer)
            engine_version: crate::VERSION.to_string(),
            schema_version: self.law.schema_version().map(String::from),
            regulation_hash: self.law.content_hash.clone(),
            regulation_valid_from: self.law.valid_from.clone(),
            delegation_refusals: Vec::new(),
            declaration_version_notes: Vec::new(),
            declarations_not_in_force: Vec::new(),
            open_terms: Vec::new(),
        };

        tracing::debug!(
            law_id = %self.law.id,
            article = %self.article.number,
            outputs = ?result.outputs.keys().collect::<Vec<_>>(),
            "Article evaluation completed"
        );

        Ok(result)
    }

    /// Validate that every input carrying an external `source` was pre-resolved.
    ///
    /// Reference resolution is owned by [`crate::LawExecutionService`], which
    /// resolves inputs (data sources, cross-law and cross-article references)
    /// and passes the values in via `parameters`. This single-article engine
    /// only checks that a `source.regulation` or `source.article` input it cannot resolve itself is
    /// already present; internal (`source.output`) and empty (`source: {}`)
    /// inputs are left untouched — they are either already in `parameters` or
    /// stay unresolved (a consuming action then surfaces a `VariableNotFound`).
    ///
    /// # Arguments
    /// * `parameters` - Input parameters (already-resolved values)
    fn check_input_sources(&self, parameters: &BTreeMap<String, Value>) -> Result<()> {
        for input in self.article.get_inputs() {
            let Some(source) = &input.source else {
                continue; // No source, nothing to check
            };

            // An external (cross-law) reference can only be resolved by the
            // service layer. If it was not pre-resolved into parameters, this
            // standalone engine cannot proceed.
            if let Some(regulation) = &source.regulation {
                if !parameters.contains_key(&input.name) {
                    return Err(EngineError::ExternalReferenceNotResolved {
                        input_name: input.name.clone(),
                        regulation: regulation.clone(),
                        output: source.output.clone().unwrap_or_default(),
                    });
                }
            }
            // A call to an article (RFC-045) runs another article, which
            // only the service can do.
            if let Some(target) = &source.article {
                if !parameters.contains_key(&input.name) {
                    return Err(EngineError::ExternalReferenceNotResolved {
                        input_name: input.name.clone(),
                        regulation: target.clone(),
                        output: source.output.clone().unwrap_or_default(),
                    });
                }
            }
            // The decisions taken under an article come from a data source
            // only (RFC-048); this engine has none, so they must be passed.
            // So do the recorded facts an article requires (RFC-051).
            if let Some(read) = crate::load_check::chronicle_read(source) {
                if !parameters.contains_key(&input.name) {
                    return Err(EngineError::DataSourceError(format!(
                        "input '{}' reads {read}, and no data source supplies them",
                        input.name
                    )));
                }
            }
            // Internal (`source.output`) and empty (`source: {}`) inputs need no
            // action here: resolved by the service when present in parameters,
            // otherwise intentionally left unresolved.
        }

        Ok(())
    }

    /// Execute the actions the requested outputs depend on, in dependency
    /// order (see [`crate::demand::execution_order`]), with optional trace
    /// instrumentation (RFC-043). An action outside
    /// that closure does not run: it fetches nothing, computes nothing, and
    /// cannot fail the article.
    fn execute_actions_traced(
        &self,
        context: &mut RuleContext,
        outputs: Option<&BTreeSet<String>>,
    ) -> Result<()> {
        let actions = self.get_actions();
        let tracing_active = context.has_trace();

        // Dependency order, not file order: an action runs after what it
        // reads, so the order of the actions in the file never changes a
        // value (apart from the assignments of one output among themselves).
        let order = crate::demand::execution_order(actions, outputs).map_err(|output| {
            EngineError::CircularReference(format!(
                "output '{output}' of {} article {} depends on itself",
                self.law.id, self.article.number
            ))
        })?;

        for (position, &index) in order.iter().enumerate() {
            let action = &actions[index];
            if let (Some(outputs), Some(name)) = (outputs, &action.output) {
                if !outputs.contains(name) {
                    continue;
                }
            }

            // An action without `output` is a computation with nowhere to
            // land. The schema requires the field and the model has it as an
            // `Option`, because the model must also read files written before
            // the field was; the execution may not treat that leniency as
            // permission. Skipping used to drop the action without a trace
            // node, without an error, and with the declared value simply
            // missing from `ArticleResult.outputs` — a calculation that
            // vanished without leaving a mark, which is the failure form this
            // project exists to make impossible.
            let Some(output_name) = &action.output else {
                return Err(EngineError::InvalidOperation(
                    "action without `output`: the computation has no name to be stored under, \
                     so its result would be dropped in silence (schema v0.7.0 requires the field)"
                        .to_string(),
                ));
            };

            if tracing_active {
                context.trace_push(output_name, PathNodeType::Action);
                context.trace_set_message(format!("Computing {}", output_name));
                // RFC-039: the anchor says which article the engine was in; this
                // says which provision the modeller holds the action to. They
                // agree here, and where they do not, that difference is the
                // thing worth seeing.
                if let Some(ref basis) = action.legal_basis {
                    context.trace_set_legal_basis(LegalAnchor::from_provision(basis));
                }
            }

            let value = match self.evaluate_action(action, context) {
                Ok(v) => v,
                Err(e) => {
                    if tracing_active {
                        context.trace_set_message(format!("Action failed: {}", e));
                        context.trace_pop();
                    }
                    return Err(e);
                }
            };

            if tracing_active {
                context.trace_set_result(value.clone());
                // The unit the law declares for this output (RFC-023, RFC-039).
                // An action producing a declared output is not a value computed
                // mid-expression: the law states its unit, so the step can
                // report it and a reader sees an amount rather than a bare
                // count of cents.
                if let Some(ts) = self
                    .article
                    .find_output(output_name)
                    .and_then(|o| o.type_spec.as_ref())
                {
                    context.trace_set_type_spec(ts.clone());
                }
            }

            // An absence is a value only where the law declared it one. An
            // output that is not nullable and still came out `null` (an IF
            // with no matching case and no default, a MIN/MAX over nothing, a
            // null input passed through) is the law saying nothing where it
            // claimed to always say something (RFC-036).
            if value.is_null()
                && self
                    .article
                    .find_output(output_name)
                    .is_some_and(|output| !output.is_nullable())
            {
                let err = EngineError::NullOutput {
                    law_id: self.law.id.clone(),
                    output: output_name.clone(),
                };
                if tracing_active {
                    context.trace_set_message(format!("Action failed: {}", err));
                    context.trace_pop();
                }
                return Err(err);
            }

            // A replacing override takes effect where the output is set, so
            // every action reading it, which runs after it, reads the value
            // the special rule gives, the same value every other article
            // reads (RFC-007). Only after the last action writing the output:
            // an output assigned twice is replaced once, as what the article
            // ends up with.
            let is_last_write = !order[position + 1..]
                .iter()
                .any(|&later| actions[later].output.as_deref() == Some(output_name.as_str()));
            let replaced = if is_last_write {
                context.replaced_output(output_name, &value)
            } else {
                None
            };
            let value = match replaced {
                None => value,
                Some(Ok(replaced)) => {
                    // The action node keeps the value the article computed,
                    // which is what the override departs from; the override
                    // node nested under it carries the replaced value.
                    if tracing_active {
                        context.trace_set_message(format!(
                            "Computing {output_name} = {value}, replaced by a lex specialis \
                             override: {replaced}"
                        ));
                    }
                    replaced
                }
                Some(Err(e)) => {
                    if tracing_active {
                        context.trace_set_message(format!("Action failed: {}", e));
                        context.trace_pop();
                    }
                    return Err(e);
                }
            };

            tracing::debug!("Output {} = {}", output_name, value);
            context.set_output(output_name, value.clone());

            if tracing_active {
                context.trace_pop();
            }
        }

        Ok(())
    }

    /// Evaluate a single action.
    ///
    /// # Arguments
    /// * `action` - Action specification
    /// * `context` - Execution context
    ///
    /// # Returns
    /// Calculated value
    fn evaluate_action(&self, action: &Action, context: &RuleContext) -> Result<Value> {
        // Check for operation at action level FIRST
        // When an action has an operation, the value/subject fields are operands, not direct results
        if let Some(operation) = &action.operation {
            let action_op = self.action_to_operation(action, operation)?;
            // RFC-023: reject incompatible units (e.g. eurocent + days). Gated on
            // the article declaring any unit, so un-annotated laws are untouched.
            if self.symbols.has_any_unit() {
                crate::units::infer_operation_unit(&action_op, &self.symbols)?;
            }
            return execute_operation(&action_op, context, 0);
        }

        // Check for direct value (only when no operation is specified)
        if let Some(value) = &action.value {
            if self.symbols.has_any_unit() {
                crate::units::infer_unit(value, &self.symbols)?;
            }
            return evaluate_value(value, context, 0);
        }

        // No value or operation specified
        Ok(Value::Null)
    }

    /// Convert an Action to an ActionOperation for execution.
    ///
    /// See [`action_to_operation`]; kept as a method so the call site reads
    /// as before.
    fn action_to_operation(
        &self,
        action: &Action,
        operation: &crate::types::Operation,
    ) -> Result<ActionOperation> {
        action_to_operation(action, operation)
    }

    /// Get actions from the article's execution spec.
    fn get_actions(&self) -> &[Action] {
        self.article
            .get_execution_spec()
            .and_then(|exec| exec.actions.as_deref())
            .unwrap_or(&[])
    }
}

/// Convert an inline action (`operation:` next to `subject`/`value`/
/// `values`/`conditions`) to the nested [`ActionOperation`] form the
/// evaluator and the static type check both work on.
///
/// This is needed because actions can have operations specified inline
/// rather than as nested `ActionValue::Operation`.
///
/// Only comparison, arithmetic, aggregate, and logical operations are supported
/// at the action level because the `Action` struct only has `subject`, `value`,
/// `values`, and `conditions` fields. IF, date operations, and LIST must be
/// nested inside `value` as an `ActionValue::Operation`.
pub(crate) fn action_to_operation(
    action: &Action,
    operation: &crate::types::Operation,
) -> Result<ActionOperation> {
    use crate::types::Operation;

    let require_subject = |op: &Operation| {
        action.subject.clone().ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "{} requires 'subject' at action level",
                op.name()
            ))
        })
    };
    let require_value = |op: &Operation| {
        action.value.clone().ok_or_else(|| {
            EngineError::InvalidOperation(format!("{} requires 'value' at action level", op.name()))
        })
    };
    let require_values = |op: &Operation| {
        action.values.clone().ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "{} requires 'values' at action level",
                op.name()
            ))
        })
    };
    let require_conditions = |op: &Operation| {
        action.conditions.clone().ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "{} requires 'conditions' at action level",
                op.name()
            ))
        })
    };
    let require_precision = |op: &Operation| {
        action.precision.ok_or_else(|| {
            EngineError::InvalidOperation(format!(
                "{} requires 'precision' at action level",
                op.name()
            ))
        })
    };

    match operation {
        // Comparison operations (subject + value)
        Operation::Equals => Ok(ActionOperation::Equals {
            subject: require_subject(operation)?,
            value: require_value(operation)?,
        }),
        Operation::NotEquals => Ok(ActionOperation::NotEquals {
            subject: require_subject(operation)?,
            value: require_value(operation)?,
        }),
        Operation::GreaterThan => Ok(ActionOperation::GreaterThan {
            subject: require_subject(operation)?,
            value: require_value(operation)?,
        }),
        Operation::LessThan => Ok(ActionOperation::LessThan {
            subject: require_subject(operation)?,
            value: require_value(operation)?,
        }),
        Operation::GreaterThanOrEqual => Ok(ActionOperation::GreaterThanOrEqual {
            subject: require_subject(operation)?,
            value: require_value(operation)?,
        }),
        Operation::LessThanOrEqual => Ok(ActionOperation::LessThanOrEqual {
            subject: require_subject(operation)?,
            value: require_value(operation)?,
        }),

        // Arithmetic operations (values)
        Operation::Add => Ok(ActionOperation::Add {
            values: require_values(operation)?,
        }),
        Operation::Subtract => Ok(ActionOperation::Subtract {
            values: require_values(operation)?,
        }),
        Operation::Multiply => Ok(ActionOperation::Multiply {
            values: require_values(operation)?,
        }),
        Operation::Divide => Ok(ActionOperation::Divide {
            values: require_values(operation)?,
        }),

        // Aggregate operations (values)
        Operation::Max => Ok(ActionOperation::Max {
            values: require_values(operation)?,
        }),
        Operation::Min => Ok(ActionOperation::Min {
            values: require_values(operation)?,
        }),

        // Rounding operations (unary value + precision; RFC-024)
        Operation::Round => Ok(ActionOperation::Round {
            value: require_value(operation)?,
            precision: require_precision(operation)?,
        }),
        Operation::Ceil => Ok(ActionOperation::Ceil {
            value: require_value(operation)?,
            precision: require_precision(operation)?,
        }),
        Operation::Floor => Ok(ActionOperation::Floor {
            value: require_value(operation)?,
            precision: require_precision(operation)?,
        }),

        // Logical operations
        Operation::And => Ok(ActionOperation::And {
            conditions: require_conditions(operation)?,
        }),
        Operation::Or => Ok(ActionOperation::Or {
            conditions: require_conditions(operation)?,
        }),
        Operation::Not => Ok(ActionOperation::Not {
            value: require_value(operation)?,
        }),

        // Null check operations (subject only)
        Operation::IsNull => Ok(ActionOperation::IsNull {
            subject: require_subject(operation)?,
        }),
        Operation::NotNull => Ok(ActionOperation::NotNull {
            subject: require_subject(operation)?,
        }),

        // Collection: IN/NOT_IN (subject + value/values)
        Operation::In => Ok(ActionOperation::In {
            subject: require_subject(operation)?,
            value: action.value.clone(),
            values: action.values.clone(),
        }),
        Operation::NotIn => Ok(ActionOperation::NotIn {
            subject: require_subject(operation)?,
            value: action.value.clone(),
            values: action.values.clone(),
        }),

        // Operations not supported at action level
        Operation::If
        | Operation::List
        | Operation::ForEach
        | Operation::Age
        | Operation::DateAdd
        | Operation::Date
        | Operation::DayOfWeek
        | Operation::DateDiff
        | Operation::DatePart
        | Operation::StartOf => Err(EngineError::InvalidOperation(format!(
            "{} must be nested inside 'value', not used directly at action level",
            operation.name()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::article::{ArticleBasedLaw, LawLoad};
    use rust_decimal_macros::dec;

    fn make_simple_law() -> ArticleBasedLaw {
        let yaml = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article
    machine_readable:
      definitions:
        MAX_AGE:
          value: 67
        MIN_AGE:
          value: 18
      execution:
        parameters:
          - name: age
            type: number
            required: true
        output:
          - name: is_adult
            type: boolean
          - name: age_check_result
            type: string
        actions:
          - output: is_adult
            operation: GREATER_THAN_OR_EQUAL
            subject: $age
            value: $MIN_AGE
          - output: age_check_result
            value:
              operation: IF
              cases:
                - when:
                    operation: GREATER_THAN_OR_EQUAL
                    subject: $age
                    value: $MIN_AGE
                  then: "adult"
              default: "minor"
"#;
        ArticleBasedLaw::from_yaml_str(yaml).unwrap()
    }

    #[test]
    fn test_unpassed_optional_parameters_lists_only_omitted_optional_ones() {
        // RFC-036: an optional parameter the caller omits resolves to unknown,
        // a required one it omits stays an error, and a passed one is a value.
        let yaml = r#"
$id: optioneel
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
          - name: aanvraag_bedrag
            type: number
            required: false
          - name: toelichting
            type: string
            required: false
          - name: zonder_vlag
            type: string
        output:
          - name: past
            type: boolean
        actions:
          - output: past
            value:
              operation: GREATER_THAN
              subject: $aanvraag_bedrag
              value: 100
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_output("past").unwrap();
        let mut params = BTreeMap::new();
        params.insert("toelichting".to_string(), Value::String("ja".to_string()));

        let unpassed = unpassed_optional_parameters(article, &params);
        assert_eq!(unpassed, BTreeSet::from(["aanvraag_bedrag".to_string()]));

        // Executing the article then yields an unknown that names the fact.
        let engine = ArticleEngine::new(article, &law);
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        let past = result.outputs.get("past").unwrap();
        assert_eq!(
            past.missing_facts(),
            &[crate::types::MissingFact {
                law: "optioneel".to_string(),
                name: "aanvraag_bedrag".to_string(),
                kind: crate::types::MissingKind::NotPassed,
            }]
        );
    }

    fn make_arithmetic_law() -> ArticleBasedLaw {
        let yaml = r#"
$id: calc_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Calculation article
    machine_readable:
      definitions:
        TAX_RATE:
          value: 0.21
        BASE_DEDUCTION:
          value: 1000
      execution:
        parameters:
          - name: income
            type: number
            required: true
        output:
          - name: taxable_income
            type: number
          - name: tax_amount
            type: number
        actions:
          - output: taxable_income
            operation: MAX
            values:
              - 0
              - operation: SUBTRACT
                values:
                  - $income
                  - $BASE_DEDUCTION
          - output: tax_amount
            operation: MULTIPLY
            values:
              - $taxable_income
              - $TAX_RATE
"#;
        ArticleBasedLaw::from_yaml_str(yaml).unwrap()
    }

    /// A law that adds a eurocent amount to a duration in days — a unit mismatch
    /// the engine must reject at runtime (RFC-023).
    fn make_unit_mismatch_law() -> ArticleBasedLaw {
        let yaml = r#"
$id: unit_mismatch_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Unit mismatch article
    machine_readable:
      execution:
        input:
          - name: bedrag
            type: amount
            type_spec:
              unit: eurocent
          - name: duur
            type: number
            type_spec:
              unit: days
        output:
          - name: onzin
            type: amount
            type_spec:
              unit: eurocent
        actions:
          - output: onzin
            operation: ADD
            values:
              - $bedrag
              - $duur
"#;
        ArticleBasedLaw::from_yaml_str(yaml).unwrap()
    }

    // -------------------------------------------------------------------------
    // Basic Execution Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_unit_mismatch_rejected_at_runtime() {
        let law = make_unit_mismatch_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("bedrag".to_string(), Value::Int(100));
        params.insert("duur".to_string(), Value::Int(5));

        let result = engine.evaluate(params, "2025-01-01");
        assert!(
            matches!(result, Err(crate::error::EngineError::UnitMismatch { .. })),
            "Expected UnitMismatch (eurocent + days), got {:?}",
            result
        );
    }

    /// An action without `output` is a computation with nowhere to land, and
    /// it used to be skipped: no error, no trace node, and the declared value
    /// simply missing from `outputs`. Schema v0.7.0 requires the field; the
    /// model has it optional because it must read older files, and the
    /// execution may not read that leniency as permission.
    #[test]
    fn een_actie_zonder_output_laat_de_uitvoering_niet_stil_vallen() {
        let yaml = r"
$id: action_without_output_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De actie berekent en noemt geen naam om het onder op te bergen.
    machine_readable:
      execution:
        parameters:
          - name: grondslag
            type: number
            required: true
        output:
          - name: bedrag
            type: number
        actions:
          - value: $grondslag
";
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("grondslag".to_string(), Value::Int(100));

        let result = engine.evaluate(params, "2025-01-01");
        let Err(crate::error::EngineError::InvalidOperation(message)) = result else {
            panic!("de uitvoering moet weigeren, niet stil overslaan: {result:?}");
        };
        assert!(
            message.contains("output"),
            "de melding noemt het veld dat ontbreekt: {message}"
        );
    }

    #[test]
    fn test_evaluate_simple_comparison() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(25));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        assert_eq!(result.article_number, "1");
        assert_eq!(result.law_id, "test_law");
        assert_eq!(result.outputs.get("is_adult"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_an_undeclared_output_that_evaluates_to_null_is_not_an_error() {
        // The null-output rule (RFC-036) holds a declared output to its
        // promise. An action whose output is not in the `output:` list made
        // no promise, so its null passes; only a declared non-nullable output
        // is refused.
        let yaml = r#"
$id: ongedeclareerd
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        parameters:
          - name: n
            type: number
            required: true
        output:
          - name: klasse
            type: string
        actions:
          - output: tussenstap
            value:
              operation: IF
              cases:
                - when:
                    operation: GREATER_THAN
                    subject: $n
                    value: 5
                  then: hoog
          - output: klasse
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $tussenstap
                    value: hoog
                  then: hoog
              default: laag
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);
        let mut params = BTreeMap::new();
        params.insert("n".to_string(), Value::Int(1));
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(
            result.outputs.get("klasse"),
            Some(&Value::String("laag".to_string()))
        );
        // Declared, the same null is refused.
        let declared = yaml.replace(
            "          - name: klasse\n",
            "          - name: tussenstap\n            type: string\n          - name: klasse\n",
        );
        let law = ArticleBasedLaw::from_yaml_str(&declared).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);
        let mut params = BTreeMap::new();
        params.insert("n".to_string(), Value::Int(1));
        let err = engine.evaluate(params, "2025-01-01").unwrap_err();
        assert!(
            matches!(&err, crate::error::EngineError::NullOutput { output, .. } if output == "tussenstap"),
            "{err:?}"
        );
    }

    #[test]
    fn test_evaluate_with_definitions() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // Age 15 is less than MIN_AGE (18)
        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(15));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        assert_eq!(result.outputs.get("is_adult"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_evaluate_nested_if() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // Adult case
        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(25));
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(
            result.outputs.get("age_check_result"),
            Some(&Value::String("adult".to_string()))
        );

        // Minor case
        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(15));
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(
            result.outputs.get("age_check_result"),
            Some(&Value::String("minor".to_string()))
        );
    }

    // -------------------------------------------------------------------------
    // Arithmetic Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evaluate_arithmetic_operations() {
        let law = make_arithmetic_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("income".to_string(), Value::Int(5000));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        // taxable_income = MAX(0, 5000 - 1000) = 4000
        assert_eq!(
            result.outputs.get("taxable_income"),
            Some(&Value::Int(4000))
        );

        // tax_amount = 4000 * 0.21 = 840.0
        assert_eq!(
            result.outputs.get("tax_amount"),
            Some(&Value::Decimal(dec!(840)))
        );
    }

    #[test]
    fn test_evaluate_arithmetic_with_floor() {
        let law = make_arithmetic_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // Income below deduction
        let mut params = BTreeMap::new();
        params.insert("income".to_string(), Value::Int(500));

        let result = engine.evaluate(params, "2025-01-01").unwrap();

        // taxable_income = MAX(0, 500 - 1000) = MAX(0, -500) = 0
        assert_eq!(result.outputs.get("taxable_income"), Some(&Value::Int(0)));

        // tax_amount = 0 * 0.21 = 0.0
        assert_eq!(
            result.outputs.get("tax_amount"),
            Some(&Value::Decimal(dec!(0)))
        );
    }

    // -------------------------------------------------------------------------
    // Selective Output Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_evaluate_specific_output() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(25));

        let result = engine
            .evaluate_with_output(params.clone(), "2025-01-01", Some("is_adult"))
            .unwrap();

        // Only the requested output and what it depends on are computed
        // (RFC-043); `age_check_result` does not read `is_adult`.
        assert!(result.outputs.contains_key("is_adult"));
        assert!(!result.outputs.contains_key("age_check_result"));

        // Without a requested output, every action runs.
        let all = engine.evaluate(params, "2025-01-01").unwrap();
        assert!(all.outputs.contains_key("is_adult"));
        assert!(all.outputs.contains_key("age_check_result"));
    }

    /// An action outside the closure of the requested output cannot fail the
    /// article (RFC-043): here it reads a variable nobody passed.
    #[test]
    fn an_action_nobody_asked_for_does_not_fail_the_article() {
        let yaml = r#"
$id: demand_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: gevraagd
            type: number
          - name: tussen
            type: number
          - name: niet_gevraagd
            type: number
        actions:
          - output: tussen
            value: 2
          - output: niet_gevraagd
            value: $bestaat_niet
          - output: gevraagd
            value:
              operation: MULTIPLY
              values: [$tussen, 3]
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine
            .evaluate_with_output(BTreeMap::new(), "2025-01-01", Some("gevraagd"))
            .unwrap();
        assert_eq!(result.outputs.get("gevraagd"), Some(&Value::Int(6)));
        assert_eq!(result.outputs.get("tussen"), Some(&Value::Int(2)));
        assert!(!result.outputs.contains_key("niet_gevraagd"));

        assert!(engine.evaluate(BTreeMap::new(), "2025-01-01").is_err());
    }

    // -------------------------------------------------------------------------
    // Error Handling Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_missing_parameter() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        // No parameters - age is missing
        let params = BTreeMap::new();
        let result = engine.evaluate(params, "2025-01-01");

        assert!(matches!(result, Err(EngineError::VariableNotFound(_))));
    }

    #[test]
    fn test_invalid_date() {
        let law = make_simple_law();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let mut params = BTreeMap::new();
        params.insert("age".to_string(), Value::Int(25));

        let result = engine.evaluate(params, "not-a-date");
        assert!(matches!(result, Err(EngineError::InvalidDate(_))));
    }

    // -------------------------------------------------------------------------
    // Reference Date Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_reference_date_accessible() {
        let yaml = r#"
$id: date_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Date test
    machine_readable:
      execution:
        output:
          - name: current_year
            type: number
        actions:
          - output: current_year
            value: $referencedate.year
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(BTreeMap::new(), "2025-06-15").unwrap();

        assert_eq!(result.outputs.get("current_year"), Some(&Value::Int(2025)));
    }

    #[test]
    fn test_external_reference_error() {
        // External reference (with regulation) should fail with helpful error
        let yaml = r#"
$id: external_ref_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article with external reference
    machine_readable:
      execution:
        input:
          - name: external_value
            type: number
            source:
              regulation: other_law
              output: some_output
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_value
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let result = engine.evaluate(BTreeMap::new(), "2025-01-01");

        assert!(
            matches!(
                result,
                Err(EngineError::ExternalReferenceNotResolved { .. })
            ),
            "Expected ExternalReferenceNotResolved error, got: {:?}",
            result
        );
        if let Err(EngineError::ExternalReferenceNotResolved {
            input_name,
            regulation,
            output,
        }) = result
        {
            assert_eq!(input_name, "external_value");
            assert_eq!(regulation, "other_law");
            assert_eq!(output, "some_output");
        }
    }

    #[test]
    fn test_article_call_needs_the_service_unless_passed() {
        // A call to an article (RFC-045) runs another article: this engine
        // cannot, so the input must be passed.
        let yaml = r#"
$id: article_call_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Article calling another article
    machine_readable:
      execution:
        input:
          - name: verleend
            type: boolean
            source:
              article: other_law#1
              output: toegekend
        output:
          - name: result
            type: boolean
        actions:
          - output: result
            value: $verleend
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        match engine.evaluate(BTreeMap::new(), "2025-01-01") {
            Err(EngineError::ExternalReferenceNotResolved {
                input_name,
                regulation,
                output,
            }) => {
                assert_eq!(input_name, "verleend");
                assert_eq!(regulation, "other_law#1");
                assert_eq!(output, "toegekend");
            }
            other => panic!("Expected ExternalReferenceNotResolved, got: {other:?}"),
        }

        let passed = BTreeMap::from([("verleend".to_string(), Value::Bool(true))]);
        let result = engine.evaluate(passed, "2025-01-01").unwrap();
        assert_eq!(result.outputs.get("result"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_decisions_come_from_parameters_only() {
        // This engine has no data source: decisions passed in are read,
        // decisions not passed are an error (RFC-048).
        let yaml = r#"
$id: reductie_test
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Reduction over the decisions under an article
    machine_readable:
      execution:
        input:
          - name: besluiten
            type: array
            source:
              decisions: andere_wet#1
        output:
          - name: besluiten_terug
            type: array
        actions:
          - output: besluiten_terug
            value: $besluiten
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = law.find_article_by_number("1").unwrap();
        let engine = ArticleEngine::new(article, &law);

        let missing = engine.evaluate(BTreeMap::new(), "2025-01-01");
        assert!(
            matches!(&missing, Err(EngineError::DataSourceError(m)) if m.contains("andere_wet#1")),
            "{missing:?}"
        );

        let rows = Value::Array(vec![Value::Int(1)]);
        let params = BTreeMap::from([("besluiten".to_string(), rows.clone())]);
        let result = engine.evaluate(params, "2025-01-01").unwrap();
        assert_eq!(result.outputs.get("besluiten_terug"), Some(&rows));
    }

    fn get_regulation_path() -> std::path::PathBuf {
        std::env::var("REGULATION_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("corpus")
                    .join("regulation")
            })
    }

    // -------------------------------------------------------------------------
    // IoC Integration Tests
    // -------------------------------------------------------------------------

    mod ioc {
        use super::*;

        #[test]
        fn test_parse_participatiewet_ioc() {
            // Test that participatiewet uses IoC: article 8 has open_terms,
            // article 43 references article 8 via source.output
            let path = get_regulation_path().join("nl/wet/participatiewet/2022-03-15.yaml");

            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Article 8 should declare open_terms
            let article8 = law.find_article_by_number("8").unwrap();
            let mr = article8.machine_readable.as_ref().unwrap();
            let open_terms = mr.open_terms.as_ref().unwrap();
            assert_eq!(open_terms.len(), 2);
            assert_eq!(open_terms[0].id, "verlaging_percentage");
            assert_eq!(open_terms[1].id, "duur_maanden");

            // Article 43 should reference article 8 via source.output
            let article43 = law.find_article_by_number("43").unwrap();
            let exec = article43.get_execution_spec().unwrap();
            let inputs = exec.input.as_ref().unwrap();
            let input = inputs
                .iter()
                .find(|i| i.name == "verlaging_percentage")
                .unwrap();
            let source = input.source.as_ref().unwrap();
            assert_eq!(source.output.as_deref(), Some("verlaging_percentage"));
        }
    }

    // -------------------------------------------------------------------------
    // Integration Tests with Real Regulation Files
    // -------------------------------------------------------------------------

    mod integration {
        use super::*;

        #[test]
        fn test_execute_wet_op_de_zorgtoeslag_vermogen_check() {
            let path = get_regulation_path().join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Find article that calculates vermogen_onder_grens
            let article = law.find_article_by_output("vermogen_onder_grens");
            assert!(
                article.is_some(),
                "Should find vermogen_onder_grens article"
            );

            let article = article.unwrap();
            let engine = ArticleEngine::new(article, &law);

            // Test with vermogen under threshold for a person without a partner.
            // Without a service no other law runs, so every input is passed in;
            // no partner means the partner's bsn and rendementsgrondslag are
            // absent (null), which both inputs declare nullable (RFC-036).
            // Thresholds: €141.896 own, €179.429 joint.
            let mut params = BTreeMap::new();
            params.insert("vermogen".to_string(), Value::Int(100000)); // €1000 in cents, well under €141.896
            params.insert("heeft_toeslagpartner".to_string(), Value::Bool(false));
            params.insert("bsn_toeslagpartner".to_string(), Value::Null);
            params.insert("vermogen_toeslagpartner".to_string(), Value::Null);
            params.insert(
                "heeft_gehele_berekeningsjaar_dezelfde_partner".to_string(),
                Value::Bool(false),
            );

            let result = engine.evaluate(params, "2025-01-01").unwrap();

            // Should have vermogen_onder_grens output
            assert!(result.outputs.contains_key("vermogen_onder_grens"));
            assert_eq!(
                result.outputs.get("vermogen_onder_grens"),
                Some(&Value::Bool(true))
            );
        }

        #[test]
        fn test_execute_regeling_standaardpremie() {
            let path = get_regulation_path()
                .join("nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Find article with standaardpremie output
            let article = law.find_article_by_output("standaardpremie");
            assert!(article.is_some(), "Should find standaardpremie article");

            let article = article.unwrap();
            let engine = ArticleEngine::new(article, &law);

            // Execute with minimal params
            let result = engine.evaluate(BTreeMap::new(), "2025-01-01").unwrap();

            // Should have standaardpremie output (211200 eurocent = €2112 for 2025)
            assert_eq!(
                result.outputs.get("standaardpremie"),
                Some(&Value::Int(211200))
            );
        }
    }
}
