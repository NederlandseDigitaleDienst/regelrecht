//! Checks on a law document at load time that the schema cannot express.
//!
//! An Unknown (RFC-036) is a fact nobody has, and only resolution produces it:
//! a `source: {}` input without data, an optional parameter the caller left
//! out. Its serialized form `{"__unknown": true, "missing": [...]}` exists so
//! results and traces round-trip, and `Value`'s deserializer therefore builds
//! the variant from any document that carries the sentinel, a law's YAML
//! included. A law that wrote one would put an invented provenance into an
//! outcome, so the loader walks every literal in the definitions and the
//! actions and refuses the document.
//!
//! An article that produces several things (schema v0.8.1) lists them, one
//! item per type, each with its own parameters and outputs. The loader
//! refuses a list with two items of one type, an output given to two items
//! or to none, a parameter or output the article lacks, or a submission that
//! requests a decision this law does not take (RFC-047).
//!
//! A reduction (`regulatory_layer: REDUCTIE`, schema v0.8.2) is no
//! regulation: a cell's reading of its own chronicle (RFC-048), which the
//! cell gives as a named answer. Only a reduction reads the decisions taken
//! under an article (`source.decisions`) or the recorded facts an article
//! requires (`source.facts`); policy and law never do. A reduction does not
//! act on law: it fills no open term (`implements`), hooks onto nothing
//! (`hooks`), replaces no output (`overrides`), takes no decision
//! (`execution.produces`) and delegates nothing (`open_terms`). It names no
//! holder (`competent_authority`): it sits with its cell, or with none. Schema
//! v0.8.5 to v0.8.8 allowed `implements` on a reduction; a file that names
//! one of them is held to that schema, and the engine gives the declaration
//! no meaning ([`crate::resolver`]).
//!
//! The policy of the authority that decides binds an open term to an answer
//! (schema v0.8.9, RFC-048): a policy article that fills the term
//! (`implements`) and takes the answer as an input, `source: {reduction:
//! <answer>, parameters: {...}}`. Only such an article takes one, in a
//! policy that names its holder (`competent_authority`), whose cell the
//! engine asks where the term names no competent authorities. The answer
//! stands apart from every other way to a value (`regulation`, `output`,
//! `endpoint`, `decisions`, `facts`, `article`).
//!
//! The loader refuses any of these the other way round, and a `decisions`
//! beside another way to the value.
//!
//! A call to an article (`source.article`, schema v0.8.4, RFC-050) stands
//! apart from `regulation`, `decisions` and `endpoint`, and only such a call
//! takes `parameters_from`.
//!
//! Who may answer an open term, or establish a fact, sits on that term or
//! fact (`competent_authority`, schema v0.8.8, RFC-052); on an article the
//! field keeps its meaning of RFC-002, who decides, and names one authority.

use crate::answer::ChronicleRead;
use crate::article::ArticleBasedLaw;
use crate::error::{EngineError, Result};
use regelrecht_law_model::{
    parse_schema_version, ActionOperation, ActionValue, CompetentAuthority, MachineReadable,
    RegulatoryLayer,
};

/// The schema versions in which a `REDUCTIE` may have `implements`: from
/// v0.8.5, until v0.8.9 made the reduction a named answer.
const REDUCTION_IMPLEMENTS: (u32, u32, u32) = (0, 8, 5);
const REDUCTION_ANSWERS: (u32, u32, u32) = (0, 8, 9);

/// Refuse a reading of a chronicle (`source.decisions`, `source.facts`)
/// outside a `REDUCTIE`, the keys that act on law and a holder
/// (`competent_authority`) in a `REDUCTIE`, and a misplaced answer of a
/// cell (`source.reduction`, RFC-048). A `REDUCTIE`
/// may have `implements` only under a `$schema` from v0.8.5 to v0.8.8. An
/// answer is taken only in a `UITVOERINGSBELEID` that names its holder, by
/// an article that fills an open term, from schema v0.8.9; it stands apart
/// from every other way to a value.
pub(crate) fn reject_misplaced_reduction(law: &ArticleBasedLaw) -> Result<()> {
    let refuse = |why: String| Err(EngineError::LoadError(format!("law '{}' {why}", law.id)));
    let reduction = law.regulatory_layer == RegulatoryLayer::Reductie;
    let policy = law.regulatory_layer == RegulatoryLayer::Uitvoeringsbeleid;
    let version = law.schema_version().and_then(parse_schema_version);
    // A file without `$schema` is held to the current version.
    let implements_allowed =
        version.is_some_and(|v| (REDUCTION_IMPLEMENTS..REDUCTION_ANSWERS).contains(&v));
    let answers_allowed = version.is_none_or(|v| v >= REDUCTION_ANSWERS);
    for (where_, section) in sections(law) {
        if reduction {
            if section.implements.is_some() && !implements_allowed {
                return refuse(format!(
                    "{where_}: a REDUCTIE fills no open term (implements); its cell gives it as \
                     a named answer, which a policy binds to the term (RFC-048)"
                ));
            }
            if let Some(key) = acts_on_law(section) {
                return refuse(format!(
                    "{where_}: a REDUCTIE is no regulation and does not act on law ({key})"
                ));
            }
        }
        for input in inputs(section) {
            let Some(source) = input.source.as_ref() else {
                continue;
            };
            if let Some(answer) = &source.reduction {
                let beside = [
                    ("regulation", source.regulation.is_some()),
                    ("output", source.output.is_some()),
                    ("endpoint", source.endpoint.is_some()),
                    ("decisions", source.decisions.is_some()),
                    ("facts", source.facts.is_some()),
                    ("article", source.article.is_some()),
                ];
                if let Some((key, _)) = beside.iter().find(|(_, present)| *present) {
                    return refuse(format!(
                        "{where_}: input '{}' takes the answer '{answer}' of a cell, which no \
                         {key} can stand beside",
                        input.name
                    ));
                }
                if !policy {
                    return refuse(format!(
                        "{where_}: input '{}' takes the answer '{answer}' of a cell; only the \
                         UITVOERINGSBELEID of the authority that decides binds a term to an \
                         answer (RFC-048)",
                        input.name
                    ));
                }
                if !answers_allowed {
                    return refuse(format!(
                        "{where_}: input '{}' takes the answer '{answer}' of a cell, which a \
                         policy does only from schema v0.8.9",
                        input.name
                    ));
                }
                if section.implements.is_none() {
                    return refuse(format!(
                        "{where_}: input '{}' takes the answer '{answer}' of a cell, but the \
                         article fills no open term (implements); a policy binds a term to an \
                         answer (RFC-048)",
                        input.name
                    ));
                }
                if !matches!(
                    law.competent_authority,
                    Some(CompetentAuthority::Structured(_))
                ) {
                    return refuse(format!(
                        "{where_}: input '{}' takes the answer '{answer}' of a cell, but the \
                         policy names no holder by name (competent_authority, one authority), \
                         whose cell gives it (RFC-048)",
                        input.name
                    ));
                }
            }
            let Some(read) = chronicle_read(source) else {
                continue;
            };
            if source.decisions.is_some() && source.facts.is_some() {
                return refuse(format!(
                    "{where_}: input '{}' reads {read} and facts besides; an input reads one \
                     of them",
                    input.name
                ));
            }
            if source.regulation.is_some()
                || source.output.is_some()
                || source.parameters.is_some()
                || source.endpoint.is_some()
            {
                return refuse(format!(
                    "{where_}: input '{}' reads {read}, which no regulation, output, parameters \
                     or endpoint can narrow",
                    input.name
                ));
            }
            if !reduction {
                return refuse(format!(
                    "{where_}: input '{}' reads {read}; only a REDUCTIE reads decisions or \
                     facts (RFC-048, RFC-051)",
                    input.name
                ));
            }
            // The article whose decisions it reads may be the value of a
            // parameter (schema v0.8.11): one the article declares itself.
            if let Some(name) = source
                .decisions
                .as_deref()
                .and_then(|d| d.strip_prefix('$'))
            {
                let declared = section
                    .execution
                    .as_ref()
                    .and_then(|e| e.parameters.as_ref())
                    .is_some_and(|ps| ps.iter().any(|p| p.name == name));
                if !declared {
                    return refuse(format!(
                        "{where_}: input '{}' reads {read}, but '{name}' is no parameter of the \
                         article; the decisions are named by a parameter it declares",
                        input.name
                    ));
                }
            }
        }
    }
    if reduction && law.competent_authority.is_some() {
        return refuse(
            "is a REDUCTIE and names no competent_authority: it sits with its cell, or with \
             none (schema v0.8.12)"
                .to_string(),
        );
    }
    Ok(())
}

/// The first key of `section` that acts on law: a hook, an override, a
/// decision or a delegated term.
fn acts_on_law(section: &MachineReadable) -> Option<&'static str> {
    [
        ("hooks", section.hooks.is_some()),
        ("overrides", section.overrides.is_some()),
        (
            "produces",
            section
                .execution
                .as_ref()
                .is_some_and(|e| e.produces.is_some()),
        ),
        ("open_terms", section.open_terms.is_some()),
    ]
    .into_iter()
    .find(|(_, present)| *present)
    .map(|(key, _)| key)
}

/// What an input reads from the chronicle of a cell: the decisions taken
/// under an article (RFC-048), named in the law or by a parameter (schema
/// v0.8.11), or the recorded instances of a fact an article requires
/// (schema v0.8.7, RFC-051). `None` for an input that reads neither.
pub(crate) fn chronicle_read(source: &regelrecht_law_model::Source) -> Option<ChronicleRead<'_>> {
    match (&source.decisions, &source.facts) {
        (Some(decisions), _) => Some(ChronicleRead::Decisions(decisions)),
        (None, Some(fact)) => Some(ChronicleRead::Facts(fact)),
        (None, None) => None,
    }
}

/// Refuse a call to an article (`source.article`, schema v0.8.4, RFC-050)
/// beside another way to the value (`regulation`, `decisions`, `endpoint`),
/// and `parameters_from` without `article`, which nothing would receive.
pub(crate) fn reject_misplaced_article_call(law: &ArticleBasedLaw) -> Result<()> {
    for (where_, section) in sections(law) {
        for input in inputs(section) {
            let Some(source) = input.source.as_ref() else {
                continue;
            };
            let Some(target) = &source.article else {
                if source.parameters_from.is_some() {
                    return Err(EngineError::LoadError(format!(
                        "law '{}' {where_}: input '{}' has parameters_from without an article \
                         to pass them to",
                        law.id, input.name
                    )));
                }
                continue;
            };
            let beside = [
                ("regulation", source.regulation.is_some()),
                ("decisions", source.decisions.is_some()),
                ("facts", source.facts.is_some()),
                ("endpoint", source.endpoint.is_some()),
            ];
            if let Some((key, _)) = beside.iter().find(|(_, present)| *present) {
                return Err(EngineError::LoadError(format!(
                    "law '{}' {where_}: input '{}' calls the article {target}, which no {key} \
                     can stand beside",
                    law.id, input.name
                )));
            }
        }
    }
    Ok(())
}

/// Refuse competence where it does not belong (RFC-052):
///
/// - an article names one competent authority (RFC-002), never a list;
/// - an open term or a fact names its competent authorities by name, one or
///   a list of at least one, never a `#` reference;
/// - `competent_authority` on a `requires` entry only with `fact`.
pub(crate) fn reject_misplaced_competence(law: &ArticleBasedLaw) -> Result<()> {
    for (where_, section) in sections(law) {
        let refuse = |why: String| {
            Err(EngineError::LoadError(format!(
                "law '{}' {where_}: {why} (RFC-052)",
                law.id
            )))
        };
        if matches!(
            section.competent_authority,
            Some(CompetentAuthority::List(_))
        ) {
            return refuse(
                "competent_authority on an article names who decides (RFC-002), one \
                 authority; a list of those who may answer belongs on an open term or a fact"
                    .to_string(),
            );
        }
        let named = |authority: &Option<CompetentAuthority>, what: &str| -> Result<bool> {
            match authority {
                None => Ok(false),
                Some(CompetentAuthority::String(reference)) => refuse(format!(
                    "{what} names its competent_authority as '{reference}'; name the \
                     authorities themselves"
                ))
                .map(|()| false),
                Some(CompetentAuthority::List(all)) if all.is_empty() => {
                    refuse(format!("{what} has an empty competent_authority list")).map(|()| false)
                }
                Some(_) => Ok(true),
            }
        };
        for requirement in section.requires.iter().flatten() {
            match &requirement.fact {
                Some(fact) => {
                    named(&requirement.competent_authority, &format!("fact '{fact}'"))?;
                }
                None if requirement.competent_authority.is_some() => {
                    return refuse(
                        "a requires entry has a competent_authority but no fact".to_string(),
                    )
                }
                None => {}
            }
        }
        for term in section.open_terms.iter().flatten() {
            named(
                &term.competent_authority,
                &format!("open term '{}'", term.id),
            )?;
        }
    }
    Ok(())
}

/// Every machine-readable section of `law`, the preamble's first, with how
/// a message names it.
fn sections(law: &ArticleBasedLaw) -> impl Iterator<Item = (String, &MachineReadable)> {
    law.preamble
        .iter()
        .filter_map(|p| {
            p.machine_readable
                .as_ref()
                .map(|m| ("the preamble".to_string(), m))
        })
        .chain(law.articles.iter().filter_map(|a| {
            a.machine_readable
                .as_ref()
                .map(|m| (format!("article {}", a.number), m))
        }))
}

fn inputs(section: &MachineReadable) -> &[regelrecht_law_model::Input] {
    section
        .execution
        .as_ref()
        .and_then(|e| e.input.as_deref())
        .unwrap_or(&[])
}

/// Refuse a law that writes an Unknown value as a literal anywhere in its
/// definitions or actions (RFC-036).
pub(crate) fn reject_unknown_literals(law: &ArticleBasedLaw) -> Result<()> {
    for article in &law.articles {
        if let Some(definitions) = article.get_definitions() {
            for (name, definition) in definitions {
                if definition.value().contains_unknown() {
                    return Err(unknown_literal(
                        law,
                        &article.number,
                        format!("definition '{name}'"),
                    ));
                }
            }
        }
        let Some(actions) = article
            .get_execution_spec()
            .and_then(|e| e.actions.as_ref())
        else {
            continue;
        };
        for (index, action) in actions.iter().enumerate() {
            let where_ = match &action.output {
                Some(output) => format!("action for output '{output}'"),
                None => format!("action {}", index + 1),
            };
            let operands = action.operands();
            for operand in operands {
                if action_value_contains_unknown(operand) {
                    return Err(unknown_literal(law, &article.number, where_));
                }
            }
        }
    }
    Ok(())
}

/// Refuse a fact an article requires (`requires` with `fact`, schema v0.8.7,
/// RFC-051) that names a provision besides, holds no parameter or one the
/// article does not declare, or whose name another fact of the article has.
/// The law says what the fact holds; a reduction reads it by that shape.
pub(crate) fn reject_malformed_facts(law: &ArticleBasedLaw) -> Result<()> {
    for article in &law.articles {
        let refuse = |why: String| {
            Err(EngineError::LoadError(format!(
                "law '{}': article {} {why} (RFC-051)",
                law.id, article.number
            )))
        };
        let mut names: Vec<&str> = Vec::new();
        for requirement in article.get_requires() {
            let Some(fact) = requirement.fact.as_deref() else {
                if requirement.parameters.is_some() {
                    return refuse(
                        "lists parameters on a requirement that names no fact".to_string(),
                    );
                }
                continue;
            };
            if requirement.article.is_some()
                || requirement.law.is_some()
                || requirement.regeling.is_some()
                || requirement.koninklijk_besluit.is_some()
                || requirement.values.is_some()
            {
                return refuse(format!(
                    "requires the fact '{fact}' and names another provision in the same entry"
                ));
            }
            if names.contains(&fact) {
                return refuse(format!("requires the fact '{fact}' twice"));
            }
            names.push(fact);
            let parameters = requirement.parameters.as_deref().unwrap_or_default();
            if parameters.is_empty() {
                return refuse(format!(
                    "requires the fact '{fact}', which holds no parameters"
                ));
            }
            for name in parameters {
                if !article.get_parameters().iter().any(|p| &p.name == name) {
                    return refuse(format!(
                        "requires the fact '{fact}' holding '{name}', but declares no such parameter"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Refuse an article whose `produces` list (schema v0.8.1, RFC-047) has two
/// items of one type, gives an output to two items (as one of its
/// `outputs`, or as data `derived` for the decision, RFC-050), lists
/// `derived` on an item that is no decision, names a
/// parameter or output the article lacks, or has a submission that requests
/// a decision this law does not take. One item per type makes article and
/// type name an item, so nothing in the list needs a name of its own; a
/// staged execution also picks the item by kind (the stage named after the
/// submission, every other stage the decision). A submission that requests a
/// decision of another law is checked when it is used, since that law may be
/// loaded later.
///
/// An output that no item lists, beside a decision that lists its own,
/// belongs to no item: it is an output of the article that another article
/// may read, not part of the decision, and only computed when read
/// (RFC-052).
pub(crate) fn reject_ambiguous_produces(law: &ArticleBasedLaw) -> Result<()> {
    for article in law.articles.iter().filter(|a| a.produces_list()) {
        let produced = article.get_produced();
        let refuse = |why: String| {
            Err(EngineError::LoadError(format!(
                "law '{}': article {} {why} (RFC-047)",
                law.id, article.number
            )))
        };
        let decisions = produced
            .iter()
            .filter(|p| p.legal_character().is_some())
            .count();
        if decisions > 1 {
            return refuse(format!(
                "produces {decisions} decisions; it may produce one"
            ));
        }
        let mut types: Vec<&str> = Vec::new();
        for item in &produced {
            let ty = item
                .legal_character()
                .or(item.submission_kind())
                .unwrap_or("");
            if types.contains(&ty) {
                return refuse(format!(
                    "produces two items of type {ty}; it may produce one"
                ));
            }
            types.push(ty);
        }
        for item in &produced {
            let also = item.also_requests();
            if !also.is_empty() && item.requests().is_none() {
                return refuse(
                    "lists also_requests without requests; a submission requests its own \
                     decision first"
                        .to_string(),
                );
            }
            if let Some(twice) = also.iter().enumerate().find_map(|(i, t)| {
                (Some(t.as_str()) == item.requests() || also[..i].contains(t)).then_some(t)
            }) {
                return refuse(format!("requests '{twice}' under also_requests as well"));
            }
            if let Some(no) = also
                .iter()
                .find(|t| !regelrecht_law_model::is_article_reference(t))
            {
                return refuse(format!("also_requests names '{no}', no article reference"));
            }
            let targets = item
                .requests()
                .into_iter()
                .chain(also.iter().map(String::as_str));
            for target in targets {
                if item.submission_kind().is_none() {
                    return refuse(format!(
                        "requests '{target}' on a decision; only a submission requests a decision"
                    ));
                }
                let Some((regulation, number)) = target.split_once('#') else {
                    continue;
                };
                if regulation != law.id {
                    continue;
                }
                match law.find_article_by_number(number) {
                    None => {
                        return refuse(format!(
                            "requests the decision of '{target}', but this law has no article \
                             {number}"
                        ))
                    }
                    Some(a) if a.get_decision().is_none() => {
                        return refuse(format!(
                            "requests the decision of '{target}', which takes no decision"
                        ))
                    }
                    Some(_) => {}
                }
            }
        }
        if let Some(item) = produced
            .iter()
            .find(|p| p.legal_character().is_none() && p.listed_derived().is_some())
        {
            return refuse(format!(
                "lists derived outputs on its {}; only a decision has derived data (RFC-050)",
                item.submission_kind().unwrap_or("item")
            ));
        }
        // Without a decision, an output no submission lists would be read by
        // nothing the article produces: refused, so a forgotten or misspelt
        // output fails. Beside a decision that lists its outputs, it belongs
        // to no item (RFC-052).
        if article.get_decision().is_none() {
            if let Some(name) = article
                .get_outputs()
                .iter()
                .map(|o| o.name.as_str())
                .find(|name| {
                    !produced
                        .iter()
                        .any(|&p| article.outputs_of(p).contains(name))
                })
            {
                return refuse(format!(
                    "declares output '{name}', but nothing it produces yields it; \
                     a submission yields only the outputs it lists"
                ));
            }
        }
        let mut listed: Vec<&str> = Vec::new();
        let named = produced
            .iter()
            .flat_map(|p| [p.listed_outputs(), p.listed_derived()])
            .flatten()
            .flatten();
        for name in named {
            if !article.has_output(name) {
                return refuse(format!(
                    "lists output '{name}' under produces, but declares no such output"
                ));
            }
            if listed.contains(&name.as_str()) {
                return refuse(format!("gives output '{name}' to two things it produces"));
            }
            listed.push(name);
        }
        for &item in &produced {
            for name in article.parameters_of(item) {
                if !article.get_parameters().iter().any(|p| p.name == name) {
                    return refuse(format!(
                        "lists parameter '{name}' under produces, but declares no such parameter"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn unknown_literal(law: &ArticleBasedLaw, article: &str, where_: String) -> EngineError {
    EngineError::LoadError(format!(
        "law '{}': article {article} writes an unknown value as a literal ({where_}); \
         an unknown cannot be written in a law, only resolution produces it (RFC-036)",
        law.id
    ))
}

/// Whether an Unknown literal sits anywhere in an action value: a literal at
/// this level, or one nested in an operation's operands.
fn action_value_contains_unknown(value: &ActionValue) -> bool {
    match value {
        ActionValue::Literal(literal) => literal.contains_unknown(),
        ActionValue::Operation(op) => operation_contains_unknown(op),
    }
}

/// Through [`ActionOperation::operands`], which is exhaustive over the
/// operation variants, so a new operation cannot carry a literal past this
/// check unnoticed.
fn operation_contains_unknown(op: &ActionOperation) -> bool {
    op.operands().into_iter().any(action_value_contains_unknown)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::service::LawExecutionService;
    use crate::types::Value;

    /// The review's `literal-unknown.json` law: an Unknown sentinel written
    /// straight into an `AND`.
    const LITERAL_IN_ACTION: &str = r#"
$id: lit
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: boolean
        actions:
          - output: x
            value:
              operation: AND
              conditions:
                - true
                - {"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}
"#;

    const LITERAL_IN_DEFINITION: &str = r#"
$id: lit_def
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: t
    machine_readable:
      definitions:
        GRENS:
          - 1
          - {"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}
      execution:
        output:
          - name: x
            type: number
        actions:
          - output: x
            value: $GRENS
"#;

    const LITERAL_DEEP_IN_IF: &str = r#"
$id: lit_if
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '3'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: number
        actions:
          - output: x
            value:
              operation: IF
              cases:
                - when: true
                  then:
                    operation: FOREACH
                    collection:
                      operation: LIST
                      items:
                        - 1
                    as: i
                    body:
                      operation: ADD
                      values:
                        - $i
                        - {"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}
"#;

    /// The sentinel as a law would write it, for splicing into a template.
    const SENTINEL: &str = r#"{"__unknown": true, "missing": [{"law": "verzonnen", "name": "iets", "kind": "no_data"}]}"#;

    /// An `IN` whose three operand positions are filled from the caller, so
    /// each position can carry the sentinel on its own.
    fn law_with_in(subject: &str, value: Option<&str>, values: Option<&str>) -> String {
        let mut op = format!("              operation: IN\n              subject: {subject}\n");
        if let Some(value) = value {
            op.push_str(&format!("              value: {value}\n"));
        }
        if let Some(values) = values {
            op.push_str(&format!(
                "              values:\n                - {values}\n"
            ));
        }
        format!(
            r#"
$id: lit_in
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: boolean
        actions:
          - output: x
            value:
{op}"#
        )
    }

    /// A `DATE_ADD` with one date and one optional part, likewise fillable per
    /// position.
    fn law_with_date_add(date: &str, part: &str, part_value: &str) -> String {
        format!(
            r#"
$id: lit_date_add
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: string
        actions:
          - output: x
            value:
              operation: DATE_ADD
              date: {date}
              {part}: {part_value}
"#
        )
    }

    fn assert_rejected(yaml: &str, case: &str) {
        let mut service = LawExecutionService::new();
        let err = service.load_law(yaml).err().unwrap_or_else(|| {
            panic!("{case}: a law with an Unknown literal must not load");
        });
        assert!(
            matches!(err, EngineError::LoadError(_)) && err.to_string().contains("RFC-036"),
            "{case}: got {err:?}"
        );
    }

    /// An article with `produces` as a list, with `items` spliced in.
    fn law_producing(items: &str) -> String {
        format!(
            r#"
$id: lijst
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        produces:
{items}
        parameters:
          - name: p
            type: string
        output:
          - name: x
            type: boolean
        actions:
          - output: x
            value: true
"#
        )
    }

    /// `law_producing` with the decision only and `requires` spliced in.
    fn law_requiring(requires: &str) -> String {
        law_producing("          - type: BESCHIKKING").replace(
            "    machine_readable:\n      execution:\n",
            &format!("    machine_readable:\n      requires:\n{requires}\n      execution:\n"),
        )
    }

    /// A fact the article requires (schema v0.8.7, RFC-051) holds parameters
    /// the article declares, has a name of its own within the article, and
    /// names no other provision in the same entry.
    #[test]
    fn a_required_fact_holds_parameters_of_the_article() {
        let yaml = law_requiring("        - fact: onderzoek\n          parameters: [p]");
        let mut service = LawExecutionService::new();
        service.load_law(&yaml).expect("a well-formed fact loads");
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(&yaml).expect("parses");
        let fact = law.articles[0]
            .required_fact("onderzoek")
            .expect("the fact");
        assert_eq!(fact.parameters.as_deref(), Some(&["p".to_string()][..]));
        assert!(law.articles[0].required_fact("ander").is_none());

        for (requires, why) in [
            (
                "        - fact: onderzoek\n          parameters: [q]",
                "holding 'q', but declares no such parameter",
            ),
            (
                "        - fact: onderzoek\n          parameters: [p]\n        - fact: onderzoek\n          parameters: [p]",
                "requires the fact 'onderzoek' twice",
            ),
            (
                "        - fact: onderzoek\n          parameters: [p]\n          law: ander",
                "names another provision in the same entry",
            ),
            (
                "        - fact: onderzoek\n          parameters: [p]\n          article: '2'",
                "names another provision in the same entry",
            ),
            (
                "        - fact: onderzoek\n          parameters: [p]\n          regeling: ander",
                "names another provision in the same entry",
            ),
            (
                "        - fact: onderzoek\n          parameters: [p]\n          koninklijk_besluit: ander",
                "names another provision in the same entry",
            ),
            (
                "        - fact: onderzoek\n          parameters: [p]\n          values: [x]",
                "names another provision in the same entry",
            ),
            ("        - fact: onderzoek", "which holds no parameters"),
            (
                "        - article: '2'\n          parameters: [p]",
                "lists parameters on a requirement that names no fact",
            ),
        ] {
            let mut service = LawExecutionService::new();
            let err = service
                .load_law(&law_requiring(requires))
                .expect_err("a malformed fact must not load");
            assert!(
                matches!(err, EngineError::LoadError(_)) && err.to_string().contains(why),
                "{why}: got {err:?}"
            );
        }
    }

    #[test]
    fn a_produces_list_that_divides_the_outputs_loads() {
        let yaml = law_producing(
            "          - type: AANVRAAG\n            requests: lijst#1\n            parameters: [p]\n          - type: BESCHIKKING\n            outputs: [x]",
        );
        let mut service = LawExecutionService::new();
        service.load_law(&yaml).expect("a well-divided list loads");
    }

    /// A submission without `outputs` yields none, so the decision without
    /// `outputs` takes every output and the list is not ambiguous.
    #[test]
    fn a_submission_without_outputs_leaves_every_output_to_the_decision() {
        let yaml = law_producing(
            "          - type: AANVRAAG\n            parameters: [p]\n          - type: BESCHIKKING",
        );
        let mut service = LawExecutionService::new();
        service
            .load_law(&yaml)
            .expect("the decision takes the outputs no item lists");
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(&yaml).expect("parses");
        let article = &law.articles[0];
        let submission = article.get_submission().expect("a submission");
        let decision = article.get_decision().expect("a decision");
        assert!(article.outputs_of(submission).is_empty());
        assert_eq!(article.outputs_of(decision), ["x"]);
    }

    /// `law_producing` with a second output `afgeleid` and `items` spliced in.
    fn law_with_two_outputs(items: &str) -> String {
        law_producing(items)
        .replace(
            "        output:\n          - name: x\n            type: boolean\n",
            "        output:\n          - name: x\n            type: boolean\n          - name: afgeleid\n            type: boolean\n",
        )
        .replace(
            "          - output: x\n            value: true\n",
            "          - output: x\n            value: true\n          - output: afgeleid\n            value: false\n",
        )
    }

    /// An output the decision lists under `derived` is data derived for it
    /// (RFC-050): the article loads, and it is no output of the decision,
    /// also when the decision lists no `outputs` of its own.
    #[test]
    fn an_output_the_decision_derives_is_no_output_of_it() {
        for decision in [
            "          - type: BESCHIKKING\n            outputs: [x]\n            derived: [afgeleid]",
            "          - type: BESCHIKKING\n            derived: [afgeleid]",
        ] {
            let yaml = law_with_two_outputs(&format!(
                "          - type: AANVRAAG\n            parameters: [p]\n{decision}"
            ));
            let mut service = LawExecutionService::new();
            service.load_law(&yaml).expect("derived data loads");
            let law: ArticleBasedLaw = serde_yaml_ng::from_str(&yaml).expect("parses");
            let article = &law.articles[0];
            let decision = article.get_decision().expect("a decision");
            assert_eq!(article.outputs_of(decision), ["x"]);
            assert_eq!(article.derived_outputs(), ["afgeleid"]);
        }
    }

    /// An output beside a decision that lists its own outputs belongs to no
    /// item (RFC-052): the article loads, and the output is neither an
    /// output of the decision nor derived data for it.
    #[test]
    fn an_output_beside_the_decision_belongs_to_no_item() {
        let yaml = law_with_two_outputs(
            "          - type: AANVRAAG\n            parameters: [p]\n          - type: BESCHIKKING\n            outputs: [x]",
        );
        let mut service = LawExecutionService::new();
        service
            .load_law(&yaml)
            .expect("an output beside the decision loads");
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(&yaml).expect("parses");
        let article = &law.articles[0];
        let decision = article.get_decision().expect("a decision");
        assert_eq!(article.outputs_of(decision), ["x"]);
        assert!(article.derived_outputs().is_empty());
    }

    /// Derived data is listed, never inferred: `derived` names an output,
    /// and only a decision has it.
    #[test]
    fn derived_data_is_listed_on_the_decision() {
        for (items, why) in [
            (
                "          - type: BESCHIKKING\n            outputs: [x]\n            derived: [afgeleidd]",
                "lists output 'afgeleidd' under produces",
            ),
            (
                "          - type: BESCHIKKING\n            outputs: [x, afgeleid]\n            derived: [afgeleid]",
                "gives output 'afgeleid' to two things",
            ),
            (
                "          - type: AANVRAAG\n            outputs: [x]\n            derived: [afgeleid]",
                "lists derived outputs on its AANVRAAG",
            ),
        ] {
            let mut service = LawExecutionService::new();
            let err = service
                .load_law(&law_with_two_outputs(items))
                .expect_err("an unlisted or misplaced derived output must not load");
            assert!(
                matches!(err, EngineError::LoadError(_)) && err.to_string().contains(why),
                "{why}: got {err:?}"
            );
        }
    }

    #[test]
    fn a_produces_list_must_divide_the_outputs_unambiguously() {
        for (items, why) in [
            (
                "          - type: BESCHIKKING\n            outputs: [y]",
                "lists output 'y' under produces",
            ),
            (
                "          - type: AANVRAAG\n            outputs: [x]\n          - type: BESCHIKKING\n            outputs: [x]",
                "gives output 'x' to two things",
            ),
            (
                "          - type: BESCHIKKING\n          - type: TOETS",
                "produces 2 decisions",
            ),
            (
                "          - type: AANVRAAG\n            parameters: [q]\n          - type: BESCHIKKING",
                "lists parameter 'q' under produces",
            ),
            (
                "          - type: AANVRAAG\n          - type: AANVRAAG",
                "two items of type AANVRAAG",
            ),
            (
                "          - type: BESCHIKKING\n            requests: lijst#1",
                "only a submission requests a decision",
            ),
            (
                "          - type: AANVRAAG\n            requests: lijst#2\n          - type: BESCHIKKING",
                "this law has no article 2",
            ),
            (
                "          - type: AANVRAAG\n            requests: lijst#1",
                "which takes no decision",
            ),
            (
                "          - type: AANVRAAG\n            requests: lijst#1\n            also_requests: [lijst#2]\n          - type: BESCHIKKING",
                "this law has no article 2",
            ),
            (
                "          - type: AANVRAAG\n            also_requests: [lijst#1]\n          - type: BESCHIKKING",
                "without requests",
            ),
            (
                "          - type: AANVRAAG\n            requests: lijst#1\n            also_requests: [lijst#1]\n          - type: BESCHIKKING",
                "under also_requests as well",
            ),
            (
                "          - type: AANVRAAG\n            requests: lijst#1\n            also_requests: [ander#1, ander#1]\n          - type: BESCHIKKING",
                "under also_requests as well",
            ),
            (
                "          - type: AANVRAAG\n            requests: lijst#1\n            also_requests: [geen]\n          - type: BESCHIKKING",
                "no article reference",
            ),
            (
                "          - type: AANVRAAG\n            parameters: [p]",
                "declares output 'x', but nothing it produces yields it",
            ),
            (
                "          - type: AANVRAAG\n            outputs: [x]\n          - type: BESCHIKKING\n            outputs: [x]",
                "gives output 'x' to two things",
            ),
        ] {
            let mut service = LawExecutionService::new();
            let err = service
                .load_law(&law_producing(items))
                .expect_err("an ambiguous list must not load");
            assert!(
                matches!(err, EngineError::LoadError(_)) && err.to_string().contains(why),
                "{why}: got {err:?}"
            );
        }
    }

    /// Not an Unknown: a sentinel without a usable `missing` list stays a
    /// plain object, and a law may write whatever objects it likes.
    const MALFORMED_SENTINEL: &str = r#"
$id: obj
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        output:
          - name: x
            type: object
        actions:
          - output: x
            value: {"__unknown": true, "missing": []}
"#;

    #[test]
    fn a_law_cannot_write_an_unknown_literal() {
        for (yaml, where_) in [
            (
                LITERAL_IN_ACTION,
                "article 1 writes an unknown value as a literal (action for output 'x')",
            ),
            (
                LITERAL_IN_DEFINITION,
                "article 2 writes an unknown value as a literal (definition 'GRENS')",
            ),
            (
                LITERAL_DEEP_IN_IF,
                "article 3 writes an unknown value as a literal (action for output 'x')",
            ),
        ] {
            let mut service = LawExecutionService::new();
            let err = service
                .load_law(yaml)
                .expect_err("a law with an Unknown literal must not load");
            let message = err.to_string();
            assert!(
                matches!(err, EngineError::LoadError(_)) && message.contains(where_),
                "got {err:?}"
            );
            assert!(message.contains("RFC-036"), "got {message}");
        }
    }

    /// The recursion has to look at every operand of an operation, not at some
    /// of them: a law that hides an Unknown in one position is as invalid as
    /// one that writes it in all of them. So each position is walked alone,
    /// with the others holding ordinary values.
    #[test]
    fn an_unknown_in_any_single_operand_of_in_is_found() {
        for (case, yaml) in [
            (
                "subject only",
                law_with_in(SENTINEL, Some("'a'"), Some("'b'")),
            ),
            (
                "value only",
                law_with_in("'a'", Some(SENTINEL), Some("'b'")),
            ),
            (
                "values only",
                law_with_in("'a'", Some("'b'"), Some(SENTINEL)),
            ),
            // The optional operands are absent one at a time as well: an
            // operand that is not there must not mask the one that is.
            ("subject, no value", law_with_in(SENTINEL, None, None)),
            ("value, no values", law_with_in("'a'", Some(SENTINEL), None)),
            ("values, no value", law_with_in("'a'", None, Some(SENTINEL))),
        ] {
            assert_rejected(&yaml, case);
        }
    }

    /// Same for DATE_ADD, whose date sits next to four optional parts.
    #[test]
    fn an_unknown_in_any_single_operand_of_date_add_is_found() {
        for (case, yaml) in [
            ("date only", law_with_date_add(SENTINEL, "years", "1")),
            (
                "years only",
                law_with_date_add("'2025-01-01'", "years", SENTINEL),
            ),
            (
                "months only",
                law_with_date_add("'2025-01-01'", "months", SENTINEL),
            ),
            (
                "weeks only",
                law_with_date_add("'2025-01-01'", "weeks", SENTINEL),
            ),
            (
                "days only",
                law_with_date_add("'2025-01-01'", "days", SENTINEL),
            ),
        ] {
            assert_rejected(&yaml, case);
        }
    }

    #[test]
    fn a_malformed_sentinel_is_an_ordinary_object_and_loads() {
        let mut service = LawExecutionService::new();
        service.load_law(MALFORMED_SENTINEL).unwrap();
        let result = service
            .evaluate_law_output("obj", "x", Default::default(), "2025-01-01")
            .unwrap();
        assert!(matches!(result.outputs.get("x"), Some(Value::Object(_))));
    }
}
