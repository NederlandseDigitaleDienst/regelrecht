//! The cell reads its chronicle back through its reductions (RFC-048).
//!
//! A reduction is rulework of its own kind (`regulatory_layer: REDUCTIE`,
//! the chronolexoreductie of RFC-022 §4.1), in the law format and run by the
//! engine, but no regulation. It is kept with the cell
//! (`corpus/cells/<cell>/reducties/<id>/<valid_from>.yaml`) and only the cell
//! loads it. It asks for the decisions taken under an article with
//! `source: {decisions: <regulation>#<article>}`; the cell answers that input
//! with one row per decision in its chronicle ([`rows`]). For the logic it
//! calls law or policy like any rulework (`source.regulation`).
//!
//! What a reduction yields is the answer to an open term of a law, which
//! it names with `implements` (schema v0.8.5): the lexostatus the cell
//! answers. Reduction takes place in the cell, and the cell hands the
//! lexostatus to the process that asks for it (RFC-022). So the engine does
//! not run the reduction: where an article reads the term, at any depth of
//! the execution, the engine asks the cell ([`Reader`], an
//! [`OpenTermAnswerer`]), and records who answered, from which reduction
//! and how far into the chronicle. The cell runs its reduction itself and
//! explains its answer on request ([`crate::Cell::explain_answer`]). The law
//! names no reduction and no cell: without one, the term is what it is
//! without an answer, such as a finding of the deciding authority. So a
//! scenario or another system can supply the same value.
//!
//! Each row carries the decision's `id`, and a field of type `decision`
//! holds the id of the decision it names, as the chronolexogram keeps it.
//! So a reduction can link a decision to another that refers to it, such as
//! a decision on an objection to the decision it contests (RFC-050). A row
//! holds only what its own chronolexogram holds: whether another decision
//! revoked it, a reduction reads from the rows of that other decision.
//!
//! The reductions are loaded into the service for the length of one
//! decision ([`load`]), so the cell can run them there; they go when the
//! [`Loaded`] guard drops, so an error or a panic during the decision leaves
//! nothing behind.

use std::cell::RefCell;
use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::{
    Answer as EngineAnswer, AnswerProvenance, Article, ArticleBasedLaw, ArticleResult, EngineError,
    LawExecutionService, LawLoad, OpenTermAnswerer, Question, RegulatoryLayer, Value,
};
use serde::Serialize;
use serde_json::Map;

use crate::chronicle::Gram;
use crate::error::{setup, Result};
use crate::shape::article_on;
use regelrecht_law_model::stored;

/// One version of a reduction of the cell, as written.
#[derive(Debug, Clone)]
pub struct Reduction {
    id: String,
    yaml: String,
    law: ArticleBasedLaw,
    terms: Vec<Term>,
    valid_from: NaiveDate,
}

/// An open term a reduction fills: its id and the article that declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Term {
    pub law: String,
    pub article: String,
    pub open_term: String,
}

/// A lexostatus the cell can answer: the open term that a version of one
/// of its reductions fills, from the day that version is valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Answer {
    /// The open term, as the reduction's `implements` names it.
    pub term: Term,
    /// The reduction, by `$id`.
    pub reduction: String,
    /// The `valid_from` of the version.
    pub valid_from: NaiveDate,
}

impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' of {}#{}", self.open_term, self.law, self.article)
    }
}

/// The open terms the articles of `law` fill (`implements`).
fn terms_of(law: &ArticleBasedLaw) -> Vec<Term> {
    law.articles.iter().flat_map(terms_of_article).collect()
}

/// The open terms `article` fills (`implements`).
fn terms_of_article(article: &Article) -> Vec<Term> {
    article
        .get_implements()
        .into_iter()
        .flatten()
        .map(|i| Term {
            law: i.law.clone(),
            article: i.article.clone(),
            open_term: i.open_term.clone(),
        })
        .collect()
}

impl Reduction {
    /// Read a version of a reduction. Rulework of another kind than
    /// `REDUCTIE` is refused: a regulation belongs in the corpus, not with
    /// the cell. So is a reduction that fills no open term (`implements`):
    /// nothing would ask its answer. So is one without a `valid_from` the
    /// cell can read as a day: it decides from which day the reduction
    /// fills its terms.
    pub fn parse(yaml: impl Into<String>) -> Result<Self> {
        let yaml = yaml.into();
        let law = ArticleBasedLaw::from_yaml_str(&yaml)?;
        if law.regulatory_layer != RegulatoryLayer::Reductie {
            return Err(setup(format!(
                "'{}' is {}, not a REDUCTIE; a cell keeps only its reductions",
                law.id,
                law.regulatory_layer.as_str()
            )));
        }
        let terms = terms_of(&law);
        if terms.is_empty() {
            return Err(setup(format!(
                "reduction '{}' fills no open term (implements), so nothing asks its answer",
                law.id
            )));
        }
        let valid_from = law
            .valid_from
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .ok_or_else(|| {
                setup(format!(
                    "reduction '{}' has valid_from {:?}, not a day written YYYY-MM-DD",
                    law.id, law.valid_from
                ))
            })?;
        Ok(Self {
            id: law.id.clone(),
            yaml,
            law,
            terms,
            valid_from,
        })
    }

    /// The reduction, by `$id`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The day this version is valid from.
    pub fn valid_from(&self) -> NaiveDate {
        self.valid_from
    }

    /// The open terms this version fills, each as an [`Answer`].
    pub fn answers(&self) -> impl Iterator<Item = Answer> + '_ {
        self.terms.iter().map(|term| Answer {
            term: term.clone(),
            reduction: self.id.clone(),
            valid_from: self.valid_from,
        })
    }

    /// The reduction as written.
    pub fn yaml(&self) -> &str {
        &self.yaml
    }

    /// The reduction as parsed.
    pub fn law(&self) -> &ArticleBasedLaw {
        &self.law
    }
}

/// The decisions in `grams` taken under `grondslag` (`<regulation>#<article>`),
/// whose legal character is `kind`, as rows: the fields, then the outputs,
/// then the chronolexogram's `id`. A field or output named `id` is refused.
/// A name that is both a field and an output with another value is
/// refused: the row would hide one.
pub fn rows(grams: &[Gram], grondslag: &str, kind: &str) -> Result<Value> {
    let mut rows = Vec::new();
    for gram in grams
        .iter()
        .filter(|g| g.refers_to.is_some() && g.kind == kind && g.grondslag == grondslag)
    {
        let mut row: Map<String, serde_json::Value> = gram.fields.clone().into_iter().collect();
        for (name, value) in &gram.outputs {
            match row.get(name) {
                Some(field)
                    if !stored::same(field, value).map_err(|e| {
                        setup(format!(
                            "chronolexogram '{}' holds '{name}' as a field and as an output: {e}",
                            gram.id
                        ))
                    })? =>
                {
                    return Err(setup(format!(
                        "chronolexogram '{}' holds '{name}' as a field ({field}) and as an output \
                         ({value})",
                        gram.id
                    )))
                }
                _ => {
                    row.insert(name.clone(), value.clone());
                }
            }
        }
        if row.contains_key(ROW_ID) {
            return Err(setup(format!(
                "chronolexogram '{}' holds '{ROW_ID}' itself; its row gives its id under that name",
                gram.id
            )));
        }
        row.insert(
            ROW_ID.to_string(),
            serde_json::Value::String(gram.id.clone()),
        );
        rows.push(serde_json::Value::Object(row));
    }
    Ok(stored::from_stored(&serde_json::Value::Array(rows))?)
}

/// The name under which a row gives the id of its chronolexogram.
pub const ROW_ID: &str = "id";

/// The version of each reduction in `reductions` in force on `day`: the one
/// with the latest `valid_from` on or before it.
fn in_force(reductions: &[Reduction], day: NaiveDate) -> Vec<&Reduction> {
    let mut latest: BTreeMap<&str, &Reduction> = BTreeMap::new();
    for version in reductions.iter().filter(|r| r.valid_from <= day) {
        latest
            .entry(version.id.as_str())
            .and_modify(|entry| *entry = std::cmp::max_by_key(*entry, version, |r| r.valid_from))
            .or_insert(version);
    }
    latest.into_values().collect()
}

/// The open terms the reductions in force on `day` fill: the lexostatussen
/// the cell answers that day.
pub fn terms_on(reductions: &[Reduction], day: NaiveDate) -> Vec<Term> {
    in_force(reductions, day)
        .into_iter()
        .flat_map(|r| r.terms.iter().cloned())
        .collect()
}

/// The first open term that two reductions in force on `day` both fill: the
/// cell would have two answers to one question, so it refuses to decide.
pub fn filled_twice(reductions: &[Reduction], day: NaiveDate) -> Option<Term> {
    let terms = terms_on(reductions, day);
    terms
        .iter()
        .enumerate()
        .find(|(i, term)| terms[i + 1..].contains(term))
        .map(|(_, term)| term.clone())
}

/// The first open term that a version of one of `reductions` fills but no
/// reduction in force on `day` fills (RFC-048), from the first day a version
/// fills it: before that day the term was the deciding authority's, so a
/// decision taken then still replays. The cell refuses to decide then:
/// without the reduction the term would be the deciding authority's
/// finding, or its default, where the cell's own reading of its chronicle
/// is expected.
pub fn unfilled(reductions: &[Reduction], day: NaiveDate) -> Option<&Term> {
    let filled = terms_on(reductions, day);
    let since = |term: &Term| {
        reductions
            .iter()
            .filter(|r| r.terms.contains(term))
            .map(|r| r.valid_from)
            .min()
    };
    reductions
        .iter()
        .flat_map(|r| &r.terms)
        .filter(|term| since(term).is_none_or(|first| first <= day))
        .find(|term| !filled.contains(term))
}

/// Each term a reduction in force on `day` fills must be an open term its
/// article declares on that day: the engine would never ask it, and the
/// term would be left to what fills it without one.
pub fn refuse_undeclared_terms(
    service: &LawExecutionService,
    reductions: &[Reduction],
    day: NaiveDate,
) -> Result<()> {
    for reduction in in_force(reductions, day) {
        for term in &reduction.terms {
            let declared = article_on(service, &term.law, &term.article, day)
                .and_then(|article| article.get_open_terms())
                .is_some_and(|terms| terms.iter().any(|t| t.id == term.open_term));
            if !declared {
                return Err(setup(format!(
                    "reduction '{}' fills open term {term}, which that article does not declare \
                     on {day}",
                    reduction.id
                )));
            }
        }
    }
    Ok(())
}

/// Load `reductions` into the service until the returned guard drops, so
/// the cell can run them there ([`Reader`]). The engine does not run a
/// reduction for the term it fills by itself: a reduction's `implements`
/// is no implementation to the engine, only the cell's declaration of the
/// term it answers. Refused when a law is already loaded under a
/// reduction's `$id`. They go when the guard drops, so an error or a panic
/// during the decision leaves nothing behind.
pub fn load<'a>(
    service: &'a mut LawExecutionService,
    recording_actor: &str,
    reductions: &[Reduction],
) -> Result<Loaded<'a>> {
    let mut loaded = Loaded {
        service,
        laws: Vec::new(),
    };
    for reduction in reductions {
        if !loaded.laws.contains(&reduction.id) {
            if loaded.service.has_law(&reduction.id) {
                return Err(setup(format!(
                    "a law '{}' is already loaded; the cell of '{recording_actor}' loads its \
                     reduction by that id itself",
                    reduction.id
                )));
            }
            loaded.laws.push(reduction.id.clone());
        }
        loaded.service.load_law(&reduction.yaml)?;
    }
    Ok(loaded)
}

/// The legal character of the decision of article `reference` on `day`.
fn decision_kind(service: &LawExecutionService, reference: &str, day: NaiveDate) -> Result<String> {
    reference
        .split_once('#')
        .and_then(|(law_id, article)| article_on(service, law_id, article, day))
        .and_then(|article| article.get_decision())
        .and_then(|decision| decision.legal_character())
        .map(str::to_string)
        .ok_or_else(|| setup(format!("{reference} takes no decision in force on {day}")))
}

/// The service with the cell's reductions loaded; dropping it unloads them.
pub struct Loaded<'a> {
    service: &'a mut LawExecutionService,
    laws: Vec<String>,
}

impl std::ops::Deref for Loaded<'_> {
    type Target = LawExecutionService;
    fn deref(&self) -> &LawExecutionService {
        self.service
    }
}

impl Drop for Loaded<'_> {
    fn drop(&mut self) {
        for id in &self.laws {
            self.service.unload_law(id);
        }
    }
}

/// One answer the cell gave the engine ([`Reader`]).
#[derive(Debug, Clone)]
pub struct Asked {
    pub term: Term,
    pub value: Value,
    pub provenance: AnswerProvenance,
    /// The reduction's own execution with its trace, for the term the
    /// reader explains.
    pub explained: Option<ArticleResult>,
}

/// The cell as answerer of the engine (RFC-048): for an open term one of
/// its reductions in force on the day fills, the cell runs that reduction
/// itself, on its chronicle as `grams` holds it, and gives the engine the
/// lexostatus with where it comes from: the cell, the reduction and its
/// version, and how far it read the chronicle. The engine does not see the
/// reduction run; the cell explains its answer on request
/// ([`crate::Cell::explain_answer`]).
///
/// The reduction gets, by name, the parameters it declares from the article
/// that asks the term, and for each input that reads decisions
/// (`source.decisions`) the decisions in `grams` as rows ([`rows`]), passed
/// in under the input's name. A term none of its reductions fills on the
/// day is not the cell's: the engine goes on without an answer.
pub struct Reader<'c> {
    recording_actor: &'c str,
    reductions: &'c [Reduction],
    grams: Vec<Gram>,
    explain: Option<&'c str>,
    asked: RefCell<Vec<Asked>>,
}

impl<'c> Reader<'c> {
    /// The cell of `recording_actor` with `reductions`, reading `grams`.
    /// The reader runs the reduction for the term `explain` with its trace.
    pub fn new(
        recording_actor: &'c str,
        reductions: &'c [Reduction],
        grams: Vec<Gram>,
        explain: Option<&'c str>,
    ) -> Self {
        Self {
            recording_actor,
            reductions,
            grams,
            explain,
            asked: RefCell::new(Vec::new()),
        }
    }

    /// What the cell answered, in the order the engine asked.
    pub fn asked(self) -> Vec<Asked> {
        self.asked.into_inner()
    }

    fn run(
        &self,
        service: &LawExecutionService,
        reduction: &Reduction,
        question: &Question<'_>,
        term: &Term,
        day: NaiveDate,
    ) -> Result<ArticleResult> {
        // The article whose `implements` names this term.
        let article = reduction
            .law
            .articles
            .iter()
            .find(|article| terms_of_article(article).contains(term))
            .ok_or_else(|| {
                setup(format!(
                    "reduction '{}' has no article that fills '{}'",
                    reduction.id, question.term
                ))
            })?;
        let mut parameters = BTreeMap::new();
        for p in article
            .get_execution_spec()
            .and_then(|execution| execution.parameters.as_ref())
            .into_iter()
            .flatten()
        {
            if let Some(value) = question.parameters.get(&p.name) {
                parameters.insert(p.name.clone(), value.clone());
            }
        }
        for input in article.get_inputs() {
            let Some(decisions) = input.source.as_ref().and_then(|s| s.decisions.as_ref()) else {
                continue;
            };
            let kind = decision_kind(service, decisions, day)?;
            parameters.insert(input.name.clone(), rows(&self.grams, decisions, &kind)?);
        }
        let outputs = [question.term];
        let date = question.calculation_date;
        Ok(if self.explain == Some(question.term) {
            service.evaluate_law_with_trace(&reduction.id, &outputs, parameters, date)?
        } else {
            service.evaluate_law(&reduction.id, &outputs, parameters, date)?
        })
    }
}

impl OpenTermAnswerer for Reader<'_> {
    fn answer(
        &self,
        service: &LawExecutionService,
        question: &Question<'_>,
    ) -> regelrecht_engine::Result<Option<EngineAnswer>> {
        let term = Term {
            law: question.law_id.to_string(),
            article: question.article.to_string(),
            open_term: question.term.to_string(),
        };
        let day = NaiveDate::parse_from_str(question.calculation_date, "%Y-%m-%d")
            .map_err(|e| EngineError::InvalidDate(format!("{}: {e}", question.calculation_date)))?;
        let Some(reduction) = in_force(self.reductions, day)
            .into_iter()
            .find(|r| r.terms.contains(&term))
        else {
            return Ok(None);
        };
        let in_cell = |e: crate::Error| match e {
            crate::Error::Engine(e) => e,
            other => EngineError::ResolutionError(format!(
                "the cell of '{}' answering {term}: {other}",
                self.recording_actor
            )),
        };
        let result = self
            .run(service, reduction, question, &term, day)
            .map_err(in_cell)?;
        let value = result.outputs.get(question.term).cloned().ok_or_else(|| {
            EngineError::ResolutionError(format!(
                "reduction '{}' of the cell of '{}' gave no '{}'",
                reduction.id, self.recording_actor, question.term
            ))
        })?;
        let provenance = AnswerProvenance {
            answerer: self.recording_actor.to_string(),
            law_id: reduction.id.clone(),
            valid_from: reduction.valid_from.format("%Y-%m-%d").to_string(),
            records_read: self.grams.len(),
            last_record: self.grams.last().map(|g| g.id.clone()),
        };
        let explained = (self.explain == Some(question.term)).then_some(result);
        self.asked.borrow_mut().push(Asked {
            term,
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
    const GRONDSLAG: &str = "leerplichtwet_1969#13a";

    fn gram(kind: &str, grondslag: &str, refers_to: Option<&str>) -> Gram {
        Gram {
            id: format!("{kind}-{grondslag}"),
            kind: kind.into(),
            grondslag: grondslag.into(),
            refers_to: refers_to.map(str::to_string),
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
            gram("AANVRAAG", GRONDSLAG, None),
            gram("BESCHIKKING", GRONDSLAG, Some("a")),
            gram("BESCHIKKING", "leerplichtwet_1969#11", Some("b")),
            gram("TOETS", GRONDSLAG, Some("c")),
        ];
        let rows = serde_json::to_value(rows(&grams, GRONDSLAG, "BESCHIKKING").unwrap()).unwrap();
        assert_eq!(
            rows,
            json!([{
                "id": "BESCHIKKING-leerplichtwet_1969#13a",
                "eerste_verlofdag": "2027-02-01",
                "verlof_verleend": true,
            }])
        );
    }

    #[test]
    fn a_name_id_on_the_decision_is_refused() {
        let mut decision = gram("BESCHIKKING", GRONDSLAG, Some("a"));
        decision.fields.insert("id".into(), json!("x"));
        let e = rows(&[decision], GRONDSLAG, "BESCHIKKING").unwrap_err();
        assert!(e.to_string().contains("holds 'id' itself"), "{e}");
    }

    #[test]
    fn a_name_that_is_a_field_and_another_output_is_refused() {
        let mut decision = gram("BESCHIKKING", GRONDSLAG, Some("a"));
        decision
            .outputs
            .insert("eerste_verlofdag".into(), json!("2027-03-01"));
        assert!(rows(&[decision], GRONDSLAG, "BESCHIKKING").is_err());
    }

    #[test]
    fn a_name_that_is_a_field_and_the_same_output_is_one_value() {
        // The same decimal in another scale is the same value.
        let mut decision = gram("BESCHIKKING", GRONDSLAG, Some("a"));
        decision
            .fields
            .insert("aandeel".into(), json!({"__decimal": "5.0"}));
        decision
            .outputs
            .insert("aandeel".into(), json!({"__decimal": "5.00"}));
        assert!(rows(&[decision], GRONDSLAG, "BESCHIKKING").is_ok());
    }

    #[test]
    fn a_name_that_is_an_integer_field_and_the_same_decimal_output_is_an_error() {
        // 5 and 5.0: the cell does not decide on its own that they are one
        // value, nor that they differ; it follows the engine's EQUALS.
        let mut decision = gram("BESCHIKKING", GRONDSLAG, Some("a"));
        decision.fields.insert("aandeel".into(), json!(5));
        decision
            .outputs
            .insert("aandeel".into(), json!({"__decimal": "5.0"}));
        let err = rows(&[decision], GRONDSLAG, "BESCHIKKING").unwrap_err();
        assert!(
            matches!(&err, crate::error::Error::Setup(m)
                if m.contains("'aandeel'") && m.contains("numbers written alike")),
            "{err}"
        );
    }

    /// A deciding law whose article 1 is a BESCHIKKING with the open term
    /// `al_verleend`, which the authority decides per case, and a reduction
    /// `id` that fills it from its decisions, with the parameter `bsn`.
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

    fn reduction(id: &str) -> Reduction {
        Reduction::parse(format!(
            r#"
$id: {id}
regulatory_layer: REDUCTIE
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/reductie
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/reductie#1
    machine_readable:
      implements:
        - law: wet_fictief
          article: '1'
          open_term: al_verleend
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
        ))
        .unwrap()
    }

    fn service() -> LawExecutionService {
        let mut service = LawExecutionService::new();
        service.load_law(DECIDING).unwrap();
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
        };
        service.evaluate_law_with_trace_in(
            "wet_fictief",
            &["verleend"],
            parameters(),
            call,
            "2026-10-20",
        )
    }

    #[test]
    fn the_cell_answers_the_term_it_fills_and_the_engine_records_who_answered() {
        let mut service = service();
        let reductions = [reduction("reductie_a")];
        let loaded = load(&mut service, ACTOR, &reductions).unwrap();
        let reader = Reader::new(ACTOR, &reductions, decisions(), None);
        let result = granted(&loaded, &reader).unwrap();
        // One grant in the chronicle: leave is already granted.
        assert_eq!(result.outputs["verleend"], Value::Bool(false));
        let filled = &result.open_terms[0];
        let regelrecht_engine::FilledBy::Answer(provenance) = &filled.filled_by else {
            panic!("{filled:?}");
        };
        assert_eq!(provenance.answerer, ACTOR);
        assert_eq!(provenance.law_id, "reductie_a");
        assert_eq!(provenance.valid_from, "2026-01-01");
        assert_eq!(provenance.records_read, 1);
        assert_eq!(
            provenance.last_record.as_deref(),
            Some("BESCHIKKING-wet_fictief#1")
        );
        // The reduction's FOREACH is not in the trace of the execution.
        let trace = result.trace.unwrap().render_box_drawing();
        assert!(trace.contains("answered by obs_de_klimop"), "{trace}");
        assert!(!trace.contains("FOREACH"), "{trace}");
        let asked = reader.asked();
        assert_eq!(asked.len(), 1);
        assert!(asked[0].explained.is_none());
        drop(loaded);
        assert!(!service.has_law("reductie_a"));
    }

    #[test]
    fn the_reduction_the_cell_explains_runs_with_its_trace() {
        let mut service = service();
        let reductions = [reduction("reductie_a")];
        let loaded = load(&mut service, ACTOR, &reductions).unwrap();
        let reader = Reader::new(ACTOR, &reductions, decisions(), Some("al_verleend"));
        granted(&loaded, &reader).unwrap();
        let asked = reader.asked();
        let trace = asked[0].explained.as_ref().unwrap().trace.as_ref().unwrap();
        assert!(trace.render_box_drawing().contains("FOREACH"));
    }

    #[test]
    fn the_engine_does_not_run_a_loaded_reduction_itself() {
        // Without the cell as answerer, the loaded reduction fills nothing:
        // its implements is the cell's declaration, no implementation.
        let mut service = service();
        let reductions = [reduction("reductie_a")];
        let loaded = load(&mut service, ACTOR, &reductions).unwrap();
        let e = loaded
            .evaluate_law("wet_fictief", &["verleend"], parameters(), "2026-10-20")
            .unwrap_err();
        assert!(e.to_string().contains("al_verleend"), "{e}");
    }

    #[test]
    fn without_the_decision_the_reduction_reads_an_empty_chronicle() {
        let mut service = service();
        let reductions = [reduction("reductie_a")];
        let loaded = load(&mut service, ACTOR, &reductions).unwrap();
        let reader = Reader::new(ACTOR, &reductions, Vec::new(), None);
        let result = granted(&loaded, &reader).unwrap();
        assert_eq!(result.outputs["verleend"], Value::Bool(true));
        let regelrecht_engine::FilledBy::Answer(provenance) = &result.open_terms[0].filled_by
        else {
            panic!("{:?}", result.open_terms);
        };
        assert_eq!(
            (provenance.records_read, provenance.last_record.as_deref()),
            (0, None)
        );
    }

    #[test]
    fn a_reduction_not_in_force_on_the_day_answers_nothing() {
        let later = Reduction::parse(
            reduction("reductie_a")
                .yaml
                .replace("valid_from: '2026-01-01'", "valid_from: '2026-12-01'"),
        )
        .unwrap();
        let reductions = [later];
        assert!(terms_on(&reductions, day()).is_empty());
        let mut service = service();
        let loaded = load(&mut service, ACTOR, &reductions).unwrap();
        let reader = Reader::new(ACTOR, &reductions, decisions(), None);
        assert!(granted(&loaded, &reader).is_err());
        assert!(reader.asked().is_empty());
    }

    #[test]
    fn a_term_a_reduction_filled_before_but_not_on_the_day_is_unfilled() {
        let first = reduction("reductie_a");
        let later = Reduction::parse(
            reduction("reductie_a")
                .yaml
                .replace("valid_from: '2026-01-01'", "valid_from: '2026-06-01'")
                .replace("open_term: al_verleend", "open_term: iets_anders"),
        )
        .unwrap();
        let reductions = [first, later];
        assert!(unfilled(&reductions, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()).is_none());
        assert_eq!(
            unfilled(&reductions, day()).map(|t| t.open_term.as_str()),
            Some("al_verleend")
        );
        assert!(unfilled(&reductions, NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()).is_none());
    }

    #[test]
    fn a_term_two_reductions_in_force_fill_is_filled_twice() {
        let reductions = [reduction("reductie_a"), reduction("reductie_b")];
        assert_eq!(
            filled_twice(&reductions, day()).map(|t| t.open_term),
            Some("al_verleend".to_string())
        );
        assert!(filled_twice(&reductions[..1], day()).is_none());
    }

    #[test]
    fn a_law_already_loaded_under_the_id_is_not_replaced() {
        let mut service = service();
        let reductions = [reduction("wet_fictief")];
        let e = load(&mut service, ACTOR, &reductions).err().unwrap();
        assert!(e.to_string().contains("already loaded"), "{e}");
        assert!(service.has_law("wet_fictief"));
    }

    #[test]
    fn a_reduction_that_fills_a_term_its_article_does_not_declare_is_refused() {
        let service = service();
        let typo = Reduction::parse(
            reduction("reductie_a")
                .yaml
                .replace("open_term: al_verleend", "open_term: al_verleent"),
        )
        .unwrap();
        let e = refuse_undeclared_terms(&service, &[typo], day()).unwrap_err();
        assert!(
            e.to_string().contains("'al_verleent' of wet_fictief#1")
                && e.to_string().contains("does not declare"),
            "{e}"
        );
    }

    #[test]
    fn a_reduction_without_a_readable_valid_from_is_refused() {
        for from in ["valid_from: '2026-13-01'\n", ""] {
            let yaml = reduction("reductie_a")
                .yaml
                .replace("valid_from: '2026-01-01'\n", from);
            let e = Reduction::parse(yaml).unwrap_err();
            assert!(e.to_string().contains("valid_from"), "{e}");
        }
    }

    #[test]
    fn rulework_other_than_a_reduction_is_not_one() {
        let e = Reduction::parse(DECIDING).unwrap_err();
        assert!(e.to_string().contains("not a REDUCTIE"), "{e}");
    }

    #[test]
    fn a_reduction_that_fills_no_open_term_is_refused() {
        let yaml = reduction("reductie_a").yaml;
        let start = yaml.find("      implements:").unwrap();
        let end = yaml.find("      execution:").unwrap();
        let without = format!("{}{}", &yaml[..start], &yaml[end..]);
        let e = Reduction::parse(without).unwrap_err();
        assert!(e.to_string().contains("fills no open term"), "{e}");
    }

    #[test]
    fn an_error_or_a_panic_after_load_leaves_nothing_behind() {
        let reductions = [reduction("reductie_a")];
        let mut service = service();
        let early = |service: &mut LawExecutionService| -> Result<()> {
            let _loaded = load(service, ACTOR, &reductions)?;
            Err(setup("the decision failed"))
        };
        assert!(early(&mut service).is_err());
        assert!(!service.has_law("reductie_a"));

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _loaded = load(&mut service, ACTOR, &reductions).unwrap();
            panic!("the decision panicked");
        }));
        assert!(panicked.is_err());
        assert!(!service.has_law("reductie_a"));
    }
}
