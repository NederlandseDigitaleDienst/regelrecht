//! The answers of cells that the caller supplies (RFC-048 §2).
//!
//! Who runs an execution may know a value the engine does not: a keeper of
//! records that reads them back with rulework of its own, a cell with its
//! reductions. Such a cell gives named answers. The caller passes it with
//! the call ([`Call`]), as the answerer of the call and as the other
//! answerers of the sphere ([`Call::sphere`]).
//!
//! The law asks no cell. The policy of the authority that decides binds an
//! open term of the law to an answer: an article of that policy fills the
//! term (`implements`) and takes the answer as an input
//! (`source: {reduction: <answer>, parameters: {...}}`, schema v0.8.9). The
//! engine resolves that input by asking the answerers of the call for the
//! answer by its name ([`OpenTermAnswerer::answer`]), with the parameters the
//! binding passes. It asks those the filled term names in its
//! `competent_authority` (RFC-048 §4), and else those the policy names as its
//! holder. It compares names; what an answerer is, it does not know.
//! Several answers that are not null make one value with the `combine` of
//! the term, or else that of the binding; an answerer that gives no answer
//! of that name, or a null one, is silent.
//!
//! Without an answerer that answers, the input is null: the binding then
//! fills nothing, so the term is what it is without a filling regulation,
//! such as the finding of the deciding authority.
//!
//! The engine does not see how an answer came about. It records who gave it
//! and from what ([`AnswerProvenance`]) in the report of the execution
//! (`ArticleResult::open_terms`, with the binding article) and as one node
//! per answer in the trace. Explaining the answer is the answerer's own
//! business.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::article::{ArticleBasedLaw, AuthorityType, NamedAuthority};
use crate::error::Result;
use crate::service::LawExecutionService;
use crate::types::Value;

/// A named answer the engine asks an answerer for.
#[derive(Debug, Clone, Copy)]
pub struct AnswerRequest<'r> {
    /// The name of the answer (`source.reduction`).
    pub name: &'r str,
    /// The values the binding passes (`source.parameters`), under the names
    /// the answer takes them by.
    pub parameters: &'r BTreeMap<String, Value>,
    /// The date of the execution (YYYY-MM-DD).
    pub calculation_date: &'r str,
}

/// An answer to an [`AnswerRequest`]: the value, and who gave it from what.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub value: Value,
    pub provenance: AnswerProvenance,
}

/// Who gave a named answer and from what, as the answerer says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerProvenance {
    /// Who answered, as it names itself.
    pub answerer: String,
    /// The name of the answer it gave.
    pub answer: String,
    /// The rulework whose result the answer is, by `$id`, the article in it
    /// that gave the answer, and the `valid_from` of its version. For a cell
    /// that is its reduction and the reduction's article.
    pub law_id: String,
    pub article: String,
    pub valid_from: String,
    /// How far the answerer read its own records for the answer: how many,
    /// and the id of the last. `None` when it read none.
    pub records_read: usize,
    pub last_record: Option<String>,
}

impl std::fmt::Display for AnswerProvenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "answer '{}' of {} with {} article {} (valid from {}), over {} records",
            self.answer,
            self.answerer,
            self.law_id,
            self.article,
            self.valid_from,
            self.records_read
        )?;
        if let Some(last) = &self.last_record {
            write!(f, " up to '{last}'")?;
        }
        Ok(())
    }
}

/// Who an answerer is, as the law can name it (RFC-051): its name, and the
/// category of authorities it belongs to, if any. The engine matches this
/// against a `competent_authority`: an `INSTANCE` by `name`, a `CATEGORY`
/// by `category`, as exact text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authority {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl Authority {
    /// Whether one of `named` names this authority: an `INSTANCE` by its
    /// name, a `CATEGORY` by its category.
    pub fn is_named_by(&self, named: &[NamedAuthority]) -> bool {
        named.iter().any(|authority| match authority.kind() {
            AuthorityType::Instance => authority.name == self.name,
            AuthorityType::Category => self.category.as_deref() == Some(authority.name.as_str()),
        })
    }
}

/// What gives named answers for the caller of an execution: a cell.
pub trait OpenTermAnswerer {
    /// Who this answerer is, for the engine to tell whether a binding asks
    /// it ([`Authority`]).
    fn authority(&self) -> &Authority;

    /// Give the answer `request` names, or `None` when this answerer gives
    /// no answer of that name. `service` is the service that runs the
    /// execution: an answerer may run rulework of its own on it, in an
    /// execution of its own. An error ends the execution, such as a
    /// parameter the answer takes that the request lacks.
    fn answer(
        &self,
        service: &LawExecutionService,
        request: &AnswerRequest<'_>,
    ) -> Result<Option<Answer>>;
}

/// What a reduction reads from a chronicle (RFC-048): the decisions taken
/// under an article (`source.decisions`, `<regulation>#<article>`), or the
/// recorded instances of a fact an article requires (`source.facts`,
/// `<regulation>#<article>#<fact>`, RFC-051).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChronicleRead<'r> {
    Decisions(&'r str),
    Facts(&'r str),
}

impl std::fmt::Display for ChronicleRead<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decisions(grondslag) => write!(f, "the decisions taken under {grondslag}"),
            Self::Facts(fact) => write!(f, "the recorded facts {fact}"),
        }
    }
}

/// The chronicle a reduction is applied to: that of the cell that runs it
/// (RFC-048). A reduction reads it through its inputs that read decisions
/// or facts, and so does every reduction it calls: a reduction without a
/// holder, such as that of the Awb, has no chronicle of its own and reads
/// the chronicle of the executing cell.
pub trait Chronicle {
    /// Who keeps the chronicle, as the trace names the provider of the rows.
    fn keeper(&self) -> &str;

    /// The rows `read` gives for an input of `law`, on `calculation_date`
    /// (YYYY-MM-DD): one per decision or recorded instance, in the order of
    /// the chronicle. `None` when
    /// this chronicle does not answer it; the input is then an error, never
    /// an empty list. An error ends the execution, such as a reduction of
    /// another holder than the keeper.
    fn rows(
        &self,
        service: &LawExecutionService,
        law: &ArticleBasedLaw,
        read: ChronicleRead<'_>,
        calculation_date: &str,
    ) -> Result<Option<Value>>;
}

/// What a caller gives an execution apart from its parameters: the
/// findings for the case (RFC-045), the answerer of the call and the other
/// answerers of the sphere (RFC-048 §4), and the chronicle a reduction is
/// applied to.
#[derive(Default)]
pub struct Call<'a> {
    pub findings: BTreeMap<String, Value>,
    /// The cell that runs the execution: asked for the answers a binding of
    /// its policy takes (RFC-048).
    pub answerer: Option<&'a dyn OpenTermAnswerer>,
    /// The chronicle of the cell that runs a reduction (RFC-048): it gives
    /// the rows every reduction in the execution reads. Only a cell that
    /// runs one of its reductions passes it; a decision is taken without.
    pub chronicle: Option<&'a dyn Chronicle>,
    /// The other cells of the sphere, asked with [`Self::answerer`] where a
    /// binding fills a term that names them in `competent_authority`
    /// (RFC-048 §4), or where the binding's policy names them as its holder.
    pub sphere: Vec<&'a dyn OpenTermAnswerer>,
}

impl Call<'_> {
    /// A call with `findings` and no answerer.
    pub fn with_findings(findings: BTreeMap<String, Value>) -> Self {
        Self {
            findings,
            ..Self::default()
        }
    }
}
