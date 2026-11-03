//! The cell reads its chronicle back through its reductions (RFC-048), and
//! gives what they find as named answers.
//!
//! A reduction is rulework of its own kind (`regulatory_layer: REDUCTIE`,
//! the chronolexoreductie of RFC-022 §4.1), in the law format and run by the
//! engine, but no regulation. It sits with its cell
//! (`<cell_dir>/reductions/`, [`crate::identity`]), and it is the only
//! rulework that reads a chronicle. It asks for the decisions taken under an
//! article with `source: {decisions: <regulation>#<article>}`, or names the
//! article by a parameter (`decisions: $<name>`, schema v0.8.11); the cell
//! answers that input with one row per decision in its chronicle ([`rows`]).
//! It asks for the recorded instances of a fact an article requires with
//! `source: {facts: <regulation>#<article>#<fact>}` (schema v0.8.7,
//! RFC-051), one row per executogram ([`executogram_rows`]); the article's
//! `requires` entry for that fact says what each holds. For a norm or a
//! calculation it calls law or policy like any rulework
//! (`source.regulation`), and passes what it found in the chronicle as
//! parameters. It may also call a reduction of no cell, such as that of the
//! Awb: that one has no chronicle of its own, and the cell gives it the rows
//! of its own chronicle, as for every reduction the execution runs
//! (`Call::chronicle`, [`Reader::chronicle_rows`]). That a reduction calls
//! none of another cell, the loader of the corpus checks: the service does
//! not know where a reduction sits.
//!
//! A reduction fills no open term. The cell gives its outputs as named
//! answers: its catalogue (`cel.yaml`, `answers`, [`crate::identity`]) names
//! per answer the reduction whose output of that name it is. The policy of
//! the authority that decides binds an open term to an answer
//! (`source.reduction`, RFC-048), and the engine asks the cell for it with
//! the parameters the binding passes ([`Reader`], an [`OpenTermAnswerer`]).
//! The cell runs the reduction itself, on its chronicle, and gives the
//! engine the value with where it comes from: the cell, the answer, the
//! reduction and its article and version, and how far it read the
//! chronicle. The engine does not see the reduction run; the cell explains
//! its answer on request ([`crate::Cell::explain_answer`]). Without the cell,
//! the binding gives nothing, and the term is what it is without one, such
//! as a finding of the deciding authority. So a scenario or another system
//! can supply the same value.
//!
//! Each row carries the decision's `id` and `basis`, and a field of type
//! `decision` holds the id of the decision it names, as the chronolexogram
//! keeps it. A decision value a row holds, such as the decision a judgment
//! set in place, is a row in turn.
//! So a reduction can link a decision to another that refers to it, such as
//! a decision on an objection to the decision it contests (RFC-048 §6). A row
//! holds only what its own chronolexogram holds: whether another decision
//! revoked it, a reduction reads from the rows of that other decision.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_engine::{
    Answer as EngineAnswer, AnswerProvenance, AnswerRequest, ArticleBasedLaw, ArticleResult,
    Authority, Call, Chronicle as ChronicleSource, ChronicleRead, EngineError, LawExecutionService,
    OpenTermAnswerer, RegulatoryLayer, Value,
};
use serde::Serialize;
use serde_json::Map;

use crate::chronicle::Gram;
use crate::chronicle::EXECUTOGRAM;
use crate::error::{refused, setup, Result};
use crate::shape::{self, article_on};
use regelrecht_law_model::stored;

/// An open term of a law article, by the law, the article and its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Term {
    pub law: String,
    pub article: String,
    pub open_term: String,
}

impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' of {}#{}", self.open_term, self.law, self.article)
    }
}

/// An open term a policy binds to a named answer (`source.reduction`,
/// RFC-048): the term, the policy article that fills it, and the answer it
/// takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub term: Term,
    pub law_id: String,
    pub article: String,
    pub answer: String,
}

/// Every binding of an open term to an answer in the policies in force on
/// `day` in `service`: per article of a `UITVOERINGSBELEID` that fills a
/// term (`implements`), each answer it takes as an input. In the order of
/// the laws and their articles.
pub fn bindings(service: &LawExecutionService, day: NaiveDate) -> Vec<Binding> {
    let mut ids: Vec<&str> = service.list_laws();
    ids.sort_unstable();
    let mut uit = Vec::new();
    for id in ids {
        let Some(law) = service.resolver().get_law_for_date(id, Some(day)) else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            continue;
        }
        for article in &law.articles {
            let taken = article
                .get_inputs()
                .iter()
                .filter_map(|input| input.source.as_ref()?.reduction.as_ref());
            for answer in taken {
                for decl in article.get_implements().into_iter().flatten() {
                    uit.push(Binding {
                        term: Term {
                            law: decl.law.clone(),
                            article: decl.article.clone(),
                            open_term: decl.open_term.clone(),
                        },
                        law_id: law.id.clone(),
                        article: article.number.clone(),
                        answer: answer.clone(),
                    });
                }
            }
        }
    }
    uit
}

/// The decisions in `grams` taken under `basis` (`<regulation>#<article>`),
/// whose legal character is `kind`, as rows (`row`).
pub fn rows(
    service: &LawExecutionService,
    grams: &[Gram],
    basis: &str,
    kind: &str,
) -> Result<Value> {
    rows_of(
        service,
        grams
            .iter()
            .filter(|g| g.refers_to.is_some() && g.kind == kind && g.basis == basis),
    )
}

/// The executograms in `grams` of the fact `fact`
/// (`<regulation>#<article>#<fact>`, RFC-051), as rows (`row`), in the
/// order the cell recorded them. The row of the receipt of a copy (RFC-053)
/// also gives the decision of the cell it concerns, under
/// [`ROW_REFERS_TO`]; another row holds null there.
pub fn executogram_rows(
    service: &LawExecutionService,
    grams: &[Gram],
    fact: &str,
) -> Result<Value> {
    let (basis, name) = crate::shape::split_fact(fact)?;
    let grams: Vec<Gram> = grams
        .iter()
        .filter(|g| g.kind == EXECUTOGRAM && g.basis == basis && g.fact.as_deref() == Some(name))
        .map(|g| {
            let mut row = g.clone();
            row.fields.insert(
                ROW_REFERS_TO.to_string(),
                g.refers_to
                    .clone()
                    .map_or(serde_json::Value::Null, serde_json::Value::String),
            );
            row
        })
        .collect();
    rows_of(service, grams.iter())
}

/// The name under which the row of an executogram gives the chronolexogram
/// it refers to (RFC-053). A fact holds no parameter of this name.
pub const ROW_REFERS_TO: &str = "refers_to";

/// `grams` as rows (`row`), each with the id of its chronolexogram.
fn rows_of<'g>(
    service: &LawExecutionService,
    grams: impl Iterator<Item = &'g Gram>,
) -> Result<Value> {
    let mut rows = Vec::new();
    for gram in grams {
        let label = format!("chronolexogram '{}'", gram.id);
        let parts = Parts {
            fields: &gram.fields.clone().into_iter().collect(),
            findings: None,
            outputs: &gram.outputs.clone().into_iter().collect(),
        };
        let row = row(service, gram, parts, &gram.basis, &label)?;
        rows.push(serde_json::Value::Object(row));
    }
    Ok(stored::from_stored(&serde_json::Value::Array(rows))?)
}

/// What a row is made of: the fields, the findings and the outputs of a
/// chronolexogram or of a decision value.
struct Parts<'p> {
    fields: &'p Map<String, serde_json::Value>,
    findings: Option<&'p Map<String, serde_json::Value>>,
    outputs: &'p Map<String, serde_json::Value>,
}

/// One row: the fields, the findings and the outputs under their own
/// names, then the article it was taken under ([`ROW_BASIS`]) and the
/// id of the chronolexogram `gram` that holds it ([`ROW_ID`]). A decision
/// value in a field or output (`{basis, fields, findings, outputs}`,
/// RFC-045 §6), such as the decision a judgment sets in place, is a row in
/// turn, with the id of `gram`; such a row holds every output its article
/// declares, null where the value has none (RFC-036). `label` names what
/// the row is of, for an error.
///
/// A name of its own `id` or `basis` is refused, and so is a name that
/// two of the parts hold with another value: the row would hide one.
fn row(
    service: &LawExecutionService,
    gram: &Gram,
    parts: Parts<'_>,
    basis: &str,
    label: &str,
) -> Result<Map<String, serde_json::Value>> {
    let mut row = Map::new();
    let named = [Some(parts.fields), parts.findings, Some(parts.outputs)];
    for (name, value) in named.into_iter().flatten().flatten() {
        let decision = value
            .as_object()
            .filter(|o| shape::is_decision_value(o))
            .and_then(|o| Some((o, o.get("basis")?.as_str()?)));
        let value = match decision {
            Some((decision, basis)) => {
                serde_json::Value::Object(decision_row(service, gram, decision, basis)?)
            }
            None => value.clone(),
        };
        if [ROW_ID, ROW_BASIS].contains(&name.as_str()) {
            return Err(setup(format!(
                "{label} holds '{name}' itself; its row gives its {} under that name",
                if name == ROW_ID { "id" } else { "article" }
            )));
        }
        match row.get(name) {
            Some(held)
                if !stored::same(held, &value)
                    .map_err(|e| setup(format!("{label} holds '{name}' twice: {e}")))? =>
            {
                return Err(setup(format!(
                    "{label} holds '{name}' twice, as {held} and as {value}"
                )))
            }
            _ => {
                row.insert(name.clone(), value);
            }
        }
    }
    row.insert(
        ROW_BASIS.to_string(),
        serde_json::Value::String(basis.to_string()),
    );
    row.insert(
        ROW_ID.to_string(),
        serde_json::Value::String(gram.id.clone()),
    );
    Ok(row)
}

/// The row of the decision value `decision` under `basis` that `gram`
/// holds (`row`): every output the article of its `basis` declares in
/// the version in force when `gram` was recorded, null where the value has
/// none.
fn decision_row(
    service: &LawExecutionService,
    gram: &Gram,
    decision: &Map<String, serde_json::Value>,
    basis: &str,
) -> Result<Map<String, serde_json::Value>> {
    let label = format!("the decision under {basis} in chronolexogram '{}'", gram.id);
    let part = |key: &str| decision.get(key).and_then(serde_json::Value::as_object);
    let empty = Map::new();
    let parts = Parts {
        fields: part("fields").unwrap_or(&empty),
        findings: part("findings"),
        outputs: part("outputs").unwrap_or(&empty),
    };
    let mut row = row(service, gram, parts, basis, &label)?;
    let declared = basis
        .split_once('#')
        .and_then(|(law_id, article)| article_on(service, law_id, article, gram.recorded_on))
        .ok_or_else(|| {
            setup(format!(
                "{label}: {basis} has no version in force on {}",
                gram.recorded_on
            ))
        })?;
    for output in declared.get_outputs() {
        row.entry(output.name.clone())
            .or_insert(serde_json::Value::Null);
    }
    Ok(row)
}

/// The name under which a row gives the id of its chronolexogram.
pub const ROW_ID: &str = "id";

/// The name under which a row gives the article its decision or fact was
/// taken or recorded under, `<regulation>#<article>`.
pub const ROW_BASIS: &str = "basis";

/// The legal character of the decision of article `reference` on `day`.
pub fn decision_kind(
    service: &LawExecutionService,
    reference: &str,
    day: NaiveDate,
) -> Result<String> {
    reference
        .split_once('#')
        .and_then(|(law_id, article)| article_on(service, law_id, article, day))
        .and_then(|article| article.get_decision())
        .and_then(|decision| decision.legal_character())
        .map(str::to_string)
        .ok_or_else(|| setup(format!("{reference} takes no decision in force on {day}")))
}

/// One answer the cell gave the engine ([`Reader`]).
#[derive(Debug, Clone)]
pub struct Asked {
    /// The name of the answer.
    pub name: String,
    pub value: Value,
    pub provenance: AnswerProvenance,
    /// The reduction's own execution with its trace, for the answer the
    /// reader explains.
    pub explained: Option<ArticleResult>,
}

/// The cell as answerer of the engine (RFC-048): for an answer its
/// catalogue gives, the cell runs the reduction the catalogue names, in the
/// version in force on the day, on its chronicle as `grams` holds it, and
/// gives the engine the value with where it comes from: the cell, the
/// answer, the reduction, its article and version, and how far it read the
/// chronicle. The engine does not see the reduction run; the cell explains
/// its answer on request ([`crate::Cell::explain_answer`]).
///
/// The reduction is the rulework the service holds under that `$id`: a
/// `REDUCTIE` the cell keeps (`reductions`). The answer
/// is its output of the answer's name. The article that gives it gets, by
/// name, the parameters the binding passes: each it declares, and none
/// other. An input that reads decisions (`source.decisions`) or facts
/// (`source.facts`), of this reduction or of one it calls, gets the rows of
/// the chronicle from the reader as the chronicle of the call
/// ([`Reader::chronicle_rows`]). An answer the
/// catalogue does not give is not the cell's, and before the reduction's
/// first version is in force it gives nothing: the engine counts the cell
/// silent. A reduction the service does not hold is an error.
pub struct Reader<'c> {
    recording_actor: &'c str,
    /// Who the cell is, for the engine's routing on competence (RFC-048 §4).
    authority: &'c Authority,
    /// The catalogue: per answer, the `$id` of the reduction.
    answers: &'c BTreeMap<String, String>,
    /// The `$id`s of the reductions the cell keeps.
    reductions: &'c BTreeSet<String>,
    grams: Vec<Gram>,
    explain: Option<&'c str>,
    asked: RefCell<Vec<Asked>>,
    /// The answers the cell is giving now, outermost first. A reduction
    /// whose rulework asks one of them again would answer itself without
    /// end; the cell refuses that instead.
    answering: RefCell<Vec<String>>,
    /// The rows the reader gave, per read (whether decisions, and the
    /// article or fact) and day: the chronicle does not change while it
    /// reads.
    read: RefCell<BTreeMap<(bool, String, NaiveDate), Value>>,
}

impl<'c> Reader<'c> {
    /// The cell of `recording_actor`, who is `authority`, with the
    /// catalogue `answers` and the reductions `reductions`, reading `grams`.
    /// The reader runs the reduction for the answer `explain` with its
    /// trace.
    pub fn new(
        recording_actor: &'c str,
        authority: &'c Authority,
        answers: &'c BTreeMap<String, String>,
        reductions: &'c BTreeSet<String>,
        grams: Vec<Gram>,
        explain: Option<&'c str>,
    ) -> Self {
        Self {
            recording_actor,
            authority,
            answers,
            reductions,
            grams,
            explain,
            asked: RefCell::new(Vec::new()),
            answering: RefCell::new(Vec::new()),
            read: RefCell::new(BTreeMap::new()),
        }
    }

    /// What the cell answered, in the order the engine asked.
    pub fn asked(self) -> Vec<Asked> {
        self.asked.into_inner()
    }

    /// Run the reduction `reduction` for the answer of `request`, on `day`:
    /// the provenance of the answer, and the reduction's execution. `None`
    /// before the first version of the reduction is in force.
    fn run(
        &self,
        service: &LawExecutionService,
        reduction: &str,
        request: &AnswerRequest<'_>,
        day: NaiveDate,
    ) -> Result<Option<(AnswerProvenance, ArticleResult)>> {
        let name = request.name;
        if !service.has_law(reduction) {
            return Err(setup(format!(
                "the answer '{name}' comes from the reduction '{reduction}', which is not loaded"
            )));
        }
        // Before its first version the reduction gives nothing: the cell is
        // silent, and the policy that takes the answer says what then holds,
        // as it did then (a decision taken then still replays).
        let Some(law) = service.resolver().get_law_for_date(reduction, Some(day)) else {
            return Ok(None);
        };
        if law.regulatory_layer != RegulatoryLayer::Reductie {
            return Err(setup(format!(
                "the answer '{name}' comes from '{reduction}', which is {}, not a REDUCTIE",
                law.regulatory_layer.as_str()
            )));
        }
        // A cell gives only the answers of its own reductions. Opening the
        // cell checks its catalogue against them; a reader made otherwise
        // is held to the same.
        if !self.reductions.contains(reduction) {
            return Err(refused(format!(
                "the answer '{name}' comes from the reduction '{reduction}', which the cell of \
                 '{}' does not keep; a cell gives only the answers of its own reductions",
                self.recording_actor
            )));
        }
        let article = law.find_article_by_output(name).ok_or_else(|| {
            setup(format!(
                "the answer '{name}' comes from the reduction '{reduction}', which has no output \
                 '{name}'"
            ))
        })?;
        let declared = article.get_parameters();
        if let Some(extra) = request
            .parameters
            .keys()
            .find(|given| !declared.iter().any(|p| &p.name == *given))
        {
            return Err(refused(format!(
                "the binding passes '{extra}' for the answer '{name}', which reduction \
                 '{reduction}' article {} does not take",
                article.number
            )));
        }
        let mut parameters = BTreeMap::new();
        for p in declared {
            match request.parameters.get(&p.name) {
                Some(value) => {
                    parameters.insert(p.name.clone(), value.clone());
                }
                None if p.required == Some(false) => {}
                None => {
                    return Err(refused(format!(
                        "the answer '{name}' takes '{}' (reduction '{reduction}' article {}), \
                         which the binding does not pass",
                        p.name, article.number
                    )))
                }
            }
        }
        // The reduction may call rulework that binds a term to an answer of
        // this cell too, such as a regulation that asks where the school is
        // established (RFC-051): the cell answers it there as well. Inside a
        // reduction the cell asks only itself (RFC-048 §4): synthesis happens
        // at the consumer, never in a cell.
        // The reduction reads the cell's chronicle, and so does every
        // reduction it calls ([`ChronicleSource`]).
        let call = Call {
            findings: BTreeMap::new(),
            answerer: Some(self),
            chronicle: Some(self),
            sphere: Vec::new(),
        };
        let outputs = [name];
        let date = request.calculation_date;
        let result = if self.explain == Some(name) {
            service.evaluate_law_with_trace_in(reduction, &outputs, parameters, call, date)?
        } else {
            service.evaluate_law_in(reduction, &outputs, parameters, call, date)?
        };
        let provenance = AnswerProvenance {
            answerer: self.recording_actor.to_string(),
            answer: name.to_string(),
            law_id: reduction.to_string(),
            article: article.number.clone(),
            valid_from: law.valid_from.clone().unwrap_or_default(),
            records_read: self.grams.len(),
            last_record: self.grams.last().map(|g| g.id.clone()),
        };
        Ok(Some((provenance, result)))
    }
}

impl Reader<'_> {
    /// The rows `read` gives for an input of the reduction `law` on `day`,
    /// from the cell's chronicle ([`rows`], [`executogram_rows`]). Every
    /// reduction the execution runs is applied to this chronicle: one of the
    /// cell, or one of no cell, such as that of the Awb, which reads from
    /// the chronicle of the cell that executes it.
    pub fn chronicle_rows(
        &self,
        service: &LawExecutionService,
        law: &ArticleBasedLaw,
        read: ChronicleRead<'_>,
        day: NaiveDate,
    ) -> Result<Value> {
        let key = match read {
            ChronicleRead::Decisions(basis) => (true, basis.to_string(), day),
            ChronicleRead::Facts(fact) => (false, fact.to_string(), day),
        };
        if let Some(rows) = self.read.borrow().get(&key) {
            return Ok(rows.clone());
        }
        let rows = match read {
            ChronicleRead::Decisions(basis) => {
                let kind = decision_kind(service, basis, day)?;
                rows(service, &self.grams, basis, &kind)?
            }
            ChronicleRead::Facts(fact) => {
                // A fact its article does not require, the cell never records.
                crate::shape::fact_fields(service, fact, day, None).map_err(|e| {
                    setup(format!(
                        "reduction '{}' reads the facts {fact}: {e}",
                        law.id
                    ))
                })?;
                executogram_rows(service, &self.grams, fact)?
            }
        };
        self.read.borrow_mut().insert(key, rows.clone());
        Ok(rows)
    }
}

impl ChronicleSource for Reader<'_> {
    fn keeper(&self) -> &str {
        self.recording_actor
    }

    fn rows(
        &self,
        service: &LawExecutionService,
        law: &ArticleBasedLaw,
        read: ChronicleRead<'_>,
        calculation_date: &str,
    ) -> regelrecht_engine::Result<Option<Value>> {
        let day = NaiveDate::parse_from_str(calculation_date, "%Y-%m-%d")
            .map_err(|e| EngineError::InvalidDate(format!("{calculation_date}: {e}")))?;
        self.chronicle_rows(service, law, read, day)
            .map(Some)
            .map_err(|e| match e {
                crate::Error::Engine(e) => e,
                other => EngineError::ResolutionError(format!(
                    "the cell of '{}' reading {read} for reduction '{}': {other}",
                    self.recording_actor, law.id
                )),
            })
    }
}

impl OpenTermAnswerer for Reader<'_> {
    fn authority(&self) -> &Authority {
        self.authority
    }

    fn answer(
        &self,
        service: &LawExecutionService,
        request: &AnswerRequest<'_>,
    ) -> regelrecht_engine::Result<Option<EngineAnswer>> {
        let name = request.name;
        let Some(reduction) = self.answers.get(name) else {
            return Ok(None);
        };
        let day = NaiveDate::parse_from_str(request.calculation_date, "%Y-%m-%d")
            .map_err(|e| EngineError::InvalidDate(format!("{}: {e}", request.calculation_date)))?;
        if self.answering.borrow().iter().any(|a| a == name) {
            let chain = self.answering.borrow().join(" -> ");
            return Err(EngineError::CircularReference(format!(
                "the cell of '{}' is asked the answer '{name}' while giving it ({chain}): its \
                 reduction '{reduction}' asks its own answer",
                self.recording_actor
            )));
        }
        self.answering.borrow_mut().push(name.to_string());
        let ran = self.run(service, reduction, request, day);
        self.answering.borrow_mut().pop();
        let Some((provenance, result)) = ran.map_err(|e| match e {
            crate::Error::Engine(e) => e,
            other => EngineError::ResolutionError(format!(
                "the cell of '{}' giving the answer '{name}': {other}",
                self.recording_actor
            )),
        })?
        else {
            return Ok(None);
        };
        let value = result.outputs.get(name).cloned().ok_or_else(|| {
            EngineError::ResolutionError(format!(
                "reduction '{reduction}' of the cell of '{}' gave no '{name}'",
                self.recording_actor
            ))
        })?;
        let explained = (self.explain == Some(name)).then_some(result);
        self.asked.borrow_mut().push(Asked {
            name: name.to_string(),
            value: value.clone(),
            provenance: provenance.clone(),
            explained,
        });
        Ok(Some(EngineAnswer { value, provenance }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    const ACTOR: &str = "obs_de_klimop";
    const BASIS: &str = "leerplichtwet_1969#13a";

    fn gram(kind: &str, basis: &str, refers_to: Option<&str>) -> Gram {
        Gram {
            id: format!("{kind}-{basis}"),
            kind: kind.into(),
            basis: basis.into(),
            fact: None,
            refers_to: refers_to.map(str::to_string),
            received: None,
            after: None,
            recording_actor: ACTOR.into(),
            recorded_on: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            moment: None,
            fields: [("eerste_verlofdag".to_string(), json!("2027-02-01"))].into(),
            missing: Vec::new(),
            outputs: [("verlof_verleend".to_string(), json!(true))].into(),
        }
    }

    #[test]
    fn rows_are_the_decisions_under_the_article_with_fields_and_outputs() {
        let grams = [
            gram("AANVRAAG", BASIS, None),
            gram("BESCHIKKING", BASIS, Some("a")),
            gram("BESCHIKKING", "leerplichtwet_1969#11", Some("b")),
            gram("TOETS", BASIS, Some("c")),
        ];
        let rows = serde_json::to_value(
            rows(&LawExecutionService::new(), &grams, BASIS, "BESCHIKKING").unwrap(),
        )
        .unwrap();
        assert_eq!(
            rows,
            json!([{
                "id": "BESCHIKKING-leerplichtwet_1969#13a",
                "basis": BASIS,
                "eerste_verlofdag": "2027-02-01",
                "verlof_verleend": true,
            }])
        );
    }

    #[test]
    fn a_name_basis_on_the_decision_is_refused() {
        let mut decision = gram("BESCHIKKING", BASIS, Some("a"));
        decision.outputs.insert("basis".into(), json!("x#1"));
        let e = rows(
            &LawExecutionService::new(),
            &[decision],
            BASIS,
            "BESCHIKKING",
        )
        .unwrap_err();
        assert!(e.to_string().contains("holds 'basis' itself"), "{e}");
    }

    #[test]
    fn a_decision_value_in_a_row_is_a_row_with_every_output_its_article_declares() {
        // A judgment's receipt that holds the decision the court set in
        // place: that decision is a row, flat, with the receipt's id, its
        // article, and null for an output the value does not hold.
        let mut receipt = gram(EXECUTOGRAM, "wet_fictief#2", None);
        receipt.fact = Some("afschrift".into());
        receipt.outputs.clear();
        receipt.fields = [(
            "in_de_plaats".to_string(),
            json!({
                "basis": "wet_fictief#1",
                "taken_on": "wet_fictief#0",
                "fields": { "bsn": "999990019" },
                "findings": { "al_verleend": false },
                "outputs": {},
            }),
        )]
        .into();
        let rows = serde_json::to_value(
            executogram_rows(&service(), &[receipt.clone()], "wet_fictief#2#afschrift").unwrap(),
        )
        .unwrap();
        assert_eq!(
            rows[0]["in_de_plaats"],
            json!({
                "id": receipt.id,
                "basis": "wet_fictief#1",
                "bsn": "999990019",
                "al_verleend": false,
                "verleend": null,
            })
        );
    }

    #[test]
    fn a_name_id_on_the_decision_is_refused() {
        let mut decision = gram("BESCHIKKING", BASIS, Some("a"));
        decision.fields.insert("id".into(), json!("x"));
        let e = rows(
            &LawExecutionService::new(),
            &[decision],
            BASIS,
            "BESCHIKKING",
        )
        .unwrap_err();
        assert!(e.to_string().contains("holds 'id' itself"), "{e}");
    }

    #[test]
    fn a_name_that_is_a_field_and_another_output_is_refused() {
        let mut decision = gram("BESCHIKKING", BASIS, Some("a"));
        decision
            .outputs
            .insert("eerste_verlofdag".into(), json!("2027-03-01"));
        assert!(rows(
            &LawExecutionService::new(),
            &[decision],
            BASIS,
            "BESCHIKKING"
        )
        .is_err());
    }

    #[test]
    fn a_name_that_is_a_field_and_the_same_output_is_one_value() {
        // The same decimal in another scale is the same value.
        let mut decision = gram("BESCHIKKING", BASIS, Some("a"));
        decision
            .fields
            .insert("aandeel".into(), json!({"__decimal": "5.0"}));
        decision
            .outputs
            .insert("aandeel".into(), json!({"__decimal": "5.00"}));
        assert!(rows(
            &LawExecutionService::new(),
            &[decision],
            BASIS,
            "BESCHIKKING"
        )
        .is_ok());
    }

    #[test]
    fn a_name_that_is_an_integer_field_and_the_same_decimal_output_is_an_error() {
        // 5 and 5.0: the cell does not decide on its own that they are one
        // value, nor that they differ; it follows the engine's EQUALS.
        let mut decision = gram("BESCHIKKING", BASIS, Some("a"));
        decision.fields.insert("aandeel".into(), json!(5));
        decision
            .outputs
            .insert("aandeel".into(), json!({"__decimal": "5.0"}));
        let err = rows(
            &LawExecutionService::new(),
            &[decision],
            BASIS,
            "BESCHIKKING",
        )
        .unwrap_err();
        assert!(
            matches!(&err, crate::error::Error::Setup(m)
                if m.contains("'aandeel'") && m.contains("numbers written alike")),
            "{err}"
        );
    }

    /// A deciding law whose article 1 is a BESCHIKKING with the open term
    /// `al_verleend`, which the authority decides per case.
    const DECIDING: &str = r#"
$id: wet_fictief
regulatory_layer: WET
bwb_id: BWBR0000001
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/wet
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/wet#1
    machine_readable:
      open_terms:
        - id: al_verleend
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      execution:
        produces:
          legal_character: BESCHIKKING
        parameters:
          - name: bsn
            type: string
        output:
          - name: verleend
            type: boolean
        actions:
          - output: verleend
            value:
              operation: NOT
              value: $al_verleend
"#;

    /// The policy of the cell's holder that binds `al_verleend` to the
    /// answer `al_verleend`, passing `bsn`.
    const BINDING: &str = r#"
$id: beleid_fictief
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid
competent_authority:
  name: obs_de_klimop
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/beleid#1
    machine_readable:
      open_terms:
        - id: al_verleend_volgens_hoofd
          type: boolean
          required: true
          decided_per_case_by: het hoofd
      implements:
        - law: wet_fictief
          article: '1'
          open_term: al_verleend
      execution:
        parameters:
          - name: bsn
            type: string
        input:
          - name: antwoord
            type: boolean
            nullable: true
            source:
              reduction: al_verleend
              parameters:
                bsn: $bsn
        output:
          - name: al_verleend
            type: boolean
        actions:
          - output: al_verleend
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord, value: null}
                  then: $al_verleend_volgens_hoofd
              default: $antwoord
"#;

    /// A reduction `id` whose output `al_verleend` says whether a decision
    /// in the chronicle granted leave.
    fn reduction(id: &str, valid_from: &str) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '{valid_from}'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/reductie#1
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
        input:
          - name: besluiten
            type: array
            source:
              decisions: wet_fictief#1
        output:
          - name: al_verleend
            type: boolean
        actions:
          - output: al_verleend
            value:
              operation: FOREACH
              collection: $besluiten
              as: besluit
              filter:
                operation: EQUALS
                subject: $besluit.verleend
                value: true
              body: true
              combine: OR
"#
        )
    }

    /// Who the cell of these tests is.
    fn authority() -> &'static Authority {
        static AUTHORITY: std::sync::OnceLock<Authority> = std::sync::OnceLock::new();
        AUTHORITY.get_or_init(|| Authority {
            name: ACTOR.to_string(),
            category: None,
        })
    }

    /// The reductions the cell keeps: `reductie_a`.
    fn reductions() -> &'static BTreeSet<String> {
        static REDUCTIONS: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
        REDUCTIONS.get_or_init(|| ["reductie_a".to_string()].into())
    }

    /// The catalogue of the cell: `al_verleend` from `reductie_a`.
    fn answers() -> &'static BTreeMap<String, String> {
        static ANSWERS: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
        ANSWERS.get_or_init(|| [("al_verleend".to_string(), "reductie_a".to_string())].into())
    }

    /// The deciding law, the binding and `reductie_a` of the cell.
    fn service() -> LawExecutionService {
        service_with(&reduction("reductie_a", "2026-01-01"))
    }

    fn service_with(reduction: &str) -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service.load_law(DECIDING).unwrap();
        service.load_law(BINDING).unwrap();
        service.load_law(reduction).unwrap();
        service
    }

    fn decisions() -> Vec<Gram> {
        let mut decision = gram("BESCHIKKING", "wet_fictief#1", Some("a"));
        decision.outputs = [("verleend".to_string(), json!(true))].into();
        vec![decision]
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()
    }

    fn parameters() -> BTreeMap<String, Value> {
        [("bsn".to_string(), Value::String("999990019".into()))].into()
    }

    fn granted(
        service: &LawExecutionService,
        reader: &Reader<'_>,
    ) -> regelrecht_engine::Result<ArticleResult> {
        let call = regelrecht_engine::Call {
            findings: BTreeMap::new(),
            answerer: Some(reader),
            chronicle: None,
            sphere: Vec::new(),
        };
        service.evaluate_law_with_trace_in(
            "wet_fictief",
            &["verleend"],
            parameters(),
            call,
            "2026-10-20",
        )
    }

    /// A reduction of no cell, which reads the decisions under the
    /// article its parameter names (schema v0.8.11), such as that of the
    /// Awb.
    const WITHOUT_HOLDER: &str = r#"
$id: reductie_zonder_houder
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/reductie#1
    machine_readable:
      execution:
        parameters:
          - name: basis
            type: string
        input:
          - name: besluiten
            type: array
            source:
              decisions: $basis
        output:
          - name: verleend
            type: array
        actions:
          - output: verleend
            value:
              operation: FOREACH
              collection: $besluiten
              as: besluit
              body: $besluit.id
"#;

    /// A reduction of the cell that calls [`WITHOUT_HOLDER`] for the
    /// decisions under `wet_fictief#1`.
    const CALLING: &str = r#"
$id: reductie_a
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/reductie#1
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
        input:
          - name: verleend
            type: array
            source:
              regulation: reductie_zonder_houder
              output: verleend
              parameters:
                basis: wet_fictief#1
        output:
          - name: al_verleend
            type: boolean
        actions:
          - output: al_verleend
            value:
              operation: FOREACH
              collection: $verleend
              as: id
              body: true
              combine: OR
"#;

    #[test]
    fn a_called_reduction_of_no_cell_reads_the_executing_cell() {
        let mut service = service_with(CALLING);
        service.load_law(WITHOUT_HOLDER).unwrap();
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let result = granted(&service, &reader).unwrap();
        // The reduction of the Awb kind found the grant in this cell's
        // chronicle: leave is already granted.
        assert_eq!(result.outputs["verleend"], Value::Bool(false));
        // Read straight, it reads the same chronicle.
        let law = service
            .resolver()
            .get_law_for_date("reductie_zonder_houder", Some(day()))
            .unwrap();
        let rows = reader
            .chronicle_rows(
                &service,
                law,
                ChronicleRead::Decisions("wet_fictief#1"),
                day(),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(&rows).unwrap()[0]["id"],
            json!("BESCHIKKING-wet_fictief#1")
        );
    }

    #[test]
    fn the_cell_gives_the_answer_the_binding_names_and_the_engine_records_who() {
        let service = service();
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let result = granted(&service, &reader).unwrap();
        // One grant in the chronicle: leave is already granted.
        assert_eq!(result.outputs["verleend"], Value::Bool(false));
        let filled = &result.open_terms[0];
        let regelrecht_engine::FilledBy::Answer {
            law_id,
            article,
            answers,
        } = &filled.filled_by
        else {
            panic!("{filled:?}");
        };
        assert_eq!((law_id.as_str(), article.as_str()), ("beleid_fictief", "1"));
        let [provenance] = answers.as_slice() else {
            panic!("{answers:?}");
        };
        assert_eq!(provenance.answerer, ACTOR);
        assert_eq!(provenance.answer, "al_verleend");
        assert_eq!(provenance.law_id, "reductie_a");
        assert_eq!(provenance.article, "1");
        assert_eq!(provenance.valid_from, "2026-01-01");
        assert_eq!(provenance.records_read, 1);
        assert_eq!(
            provenance.last_record.as_deref(),
            Some("BESCHIKKING-wet_fictief#1")
        );
        // The reduction's FOREACH is not in the trace of the execution.
        let trace = result.trace.unwrap().render_box_drawing();
        assert!(
            trace.contains("Answered by answer 'al_verleend' of obs_de_klimop"),
            "{trace}"
        );
        assert!(!trace.contains("FOREACH"), "{trace}");
        let asked = reader.asked();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].name, "al_verleend");
        assert!(asked[0].explained.is_none());
    }

    #[test]
    fn the_reduction_the_cell_explains_runs_with_its_trace() {
        let service = service();
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            Some("al_verleend"),
        );
        granted(&service, &reader).unwrap();
        let asked = reader.asked();
        let trace = asked[0].explained.as_ref().unwrap().trace.as_ref().unwrap();
        assert!(trace.render_box_drawing().contains("FOREACH"));
    }

    #[test]
    fn without_the_cell_the_binding_gives_nothing_and_the_policy_asks_the_finding() {
        // The reduction is loaded, but nothing runs it: the binding asks a
        // cell, and there is none.
        let service = service();
        let e = service
            .evaluate_law("wet_fictief", &["verleend"], parameters(), "2026-10-20")
            .unwrap_err();
        assert!(e.to_string().contains("al_verleend"), "{e}");
        let findings = [("al_verleend_volgens_hoofd".to_string(), Value::Bool(false))].into();
        let result = service
            .evaluate_law_with_findings(
                "wet_fictief",
                &["verleend"],
                parameters(),
                findings,
                "2026-10-20",
            )
            .unwrap();
        assert_eq!(result.outputs["verleend"], Value::Bool(true));
        assert!(result
            .open_terms
            .iter()
            .any(|f| f.term == "al_verleend_volgens_hoofd"
                && f.filled_by == regelrecht_engine::FilledBy::Finding));
    }

    #[test]
    fn without_the_decision_the_reduction_reads_an_empty_chronicle() {
        let service = service();
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            Vec::new(),
            None,
        );
        let result = granted(&service, &reader).unwrap();
        assert_eq!(result.outputs["verleend"], Value::Bool(true));
        let regelrecht_engine::FilledBy::Answer { answers, .. } = &result.open_terms[0].filled_by
        else {
            panic!("{:?}", result.open_terms);
        };
        assert_eq!(
            (answers[0].records_read, answers[0].last_record.as_deref()),
            (0, None)
        );
    }

    #[test]
    fn before_its_reduction_is_in_force_the_cell_is_silent() {
        // The term is then the deciding authority's, as it was when a
        // decision of that day was taken.
        let service = service_with(&reduction("reductie_a", "2026-12-01"));
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let e = granted(&service, &reader).unwrap_err();
        assert!(e.to_string().contains("al_verleend"), "{e}");
        assert!(reader.asked().is_empty());
    }

    #[test]
    fn an_answer_whose_reduction_is_not_loaded_is_an_error() {
        // Never silently the finding: the cell gives this answer, and cannot.
        let service = service();
        let elsewhere: BTreeMap<String, String> =
            [("al_verleend".to_string(), "reductie_b".to_string())].into();
        let reader = Reader::new(
            ACTOR,
            authority(),
            &elsewhere,
            reductions(),
            decisions(),
            None,
        );
        let e = granted(&service, &reader).unwrap_err();
        assert!(
            e.to_string().contains("'reductie_b', which is not loaded"),
            "{e}"
        );
    }

    #[test]
    fn an_answer_the_catalogue_does_not_give_leaves_the_cell_silent() {
        let service = service();
        let none = BTreeMap::new();
        let reader = Reader::new(ACTOR, authority(), &none, reductions(), decisions(), None);
        let findings = [("al_verleend_volgens_hoofd".to_string(), Value::Bool(false))].into();
        let call = regelrecht_engine::Call {
            findings,
            answerer: Some(&reader),
            chronicle: None,
            sphere: Vec::new(),
        };
        let result = service
            .evaluate_law_in(
                "wet_fictief",
                &["verleend"],
                parameters(),
                call,
                "2026-10-20",
            )
            .unwrap();
        assert!(result
            .open_terms
            .iter()
            .any(|f| f.term == "al_verleend_volgens_hoofd"
                && f.filled_by == regelrecht_engine::FilledBy::Finding));
        assert!(reader.asked().is_empty());
    }

    #[test]
    fn a_parameter_the_binding_does_not_pass_fails_the_answer() {
        let service = service();
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let request = AnswerRequest {
            name: "al_verleend",
            parameters: &BTreeMap::new(),
            calculation_date: "2026-10-20",
        };
        let e = reader.answer(&service, &request).unwrap_err();
        assert!(
            e.to_string().contains("takes 'bsn'") && e.to_string().contains("does not pass"),
            "{e}"
        );
    }

    #[test]
    fn a_parameter_the_reduction_may_go_without_need_not_be_passed() {
        let optional = reduction("reductie_a", "2026-01-01").replace(
            "          - name: bsn\n            type: string\n        input:",
            "          - name: bsn\n            type: string\n            required: false\n        input:",
        );
        let service = service_with(&optional);
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let request = AnswerRequest {
            name: "al_verleend",
            parameters: &BTreeMap::new(),
            calculation_date: "2026-10-20",
        };
        let answer = reader.answer(&service, &request).unwrap().unwrap();
        assert_eq!(answer.value, Value::Bool(true));
    }

    #[test]
    fn a_parameter_the_reduction_does_not_take_fails_the_answer() {
        let service = service();
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let mut given = parameters();
        given.insert("bsn_jongere".into(), Value::String("x".into()));
        let request = AnswerRequest {
            name: "al_verleend",
            parameters: &given,
            calculation_date: "2026-10-20",
        };
        let e = reader.answer(&service, &request).unwrap_err();
        assert!(e.to_string().contains("'bsn_jongere'"), "{e}");
    }

    #[test]
    fn an_answer_of_a_reduction_the_cell_does_not_keep_is_refused() {
        let service = service();
        let none = BTreeSet::new();
        let reader = Reader::new(ACTOR, authority(), answers(), &none, decisions(), None);
        let e = granted(&service, &reader).unwrap_err();
        assert!(e.to_string().contains("does not keep"), "{e}");
    }

    #[test]
    fn rulework_other_than_a_reduction_gives_no_answer() {
        let service = service();
        let policy: BTreeMap<String, String> =
            [("al_verleend".to_string(), "beleid_fictief".to_string())].into();
        let reader = Reader::new(ACTOR, authority(), &policy, reductions(), decisions(), None);
        let e = granted(&service, &reader).unwrap_err();
        assert!(e.to_string().contains("not a REDUCTIE"), "{e}");
    }

    #[test]
    fn a_reduction_without_the_output_of_the_answer_is_an_error() {
        let service = service_with(
            &reduction("reductie_a", "2026-01-01")
                .replace("- name: al_verleend", "- name: iets_anders")
                .replace("- output: al_verleend", "- output: iets_anders"),
        );
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let e = granted(&service, &reader).unwrap_err();
        assert!(e.to_string().contains("has no output 'al_verleend'"), "{e}");
    }

    #[test]
    fn a_reduction_that_asks_its_own_answer_is_refused() {
        // The reduction reads the deciding law, whose term the binding
        // binds to this very answer.
        let looping = reduction("reductie_a", "2026-01-01").replace(
            "          - name: besluiten\n            type: array\n            source:\n              \
             decisions: wet_fictief#1\n",
            "          - name: besluiten\n            type: array\n            source:\n              \
             decisions: wet_fictief#1\n          - name: verleend\n            type: boolean\n            \
             source:\n              regulation: wet_fictief\n              output: verleend\n              \
             parameters:\n                bsn: $bsn\n",
        );
        let looping = looping.replace(
            "value: true\n              body: true",
            "value: $verleend\n              body: true",
        );
        let service = service_with(&looping);
        let reader = Reader::new(
            ACTOR,
            authority(),
            answers(),
            reductions(),
            decisions(),
            None,
        );
        let e = granted(&service, &reader).unwrap_err();
        assert!(e.to_string().contains("while giving it"), "{e}");
    }

    #[test]
    fn a_binding_names_the_term_the_policy_article_and_the_answer() {
        let service = service();
        assert_eq!(
            bindings(&service, day()),
            [Binding {
                term: Term {
                    law: "wet_fictief".into(),
                    article: "1".into(),
                    open_term: "al_verleend".into(),
                },
                law_id: "beleid_fictief".into(),
                article: "1".into(),
                answer: "al_verleend".into(),
            }]
        );
    }
}
