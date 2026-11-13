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
//! requests a decision this law does not take (RFC-045 §1).
//!
//! A reduction (`regulatory_layer: REDUCTIE`, schema v0.8.2) is no
//! regulation: a cell's reading of its own chronicle (RFC-048), which the
//! cell gives as a named answer. Only a reduction reads the decisions taken
//! under an article (`source.decisions`) or the recorded facts an article
//! requires (`source.facts`); policy and law never do. A reduction does not
//! act on law: it fills no open term (`implements`), hooks onto nothing
//! (`hooks`), replaces no output (`overrides`), takes no decision
//! (`execution.produces`) and delegates nothing (`open_terms`). It names no
//! holder (`competent_authority`): it sits with its cell, or with none.
//!
//! The engine has no behavior per schema version: every file runs on one code
//! path. Not every version was issued for use, and the engine does not
//! support a construct that was later rejected. Such a construct is refused
//! with its replacement named:
//! `source.executograms` (v0.8.6), which v0.8.7 replaced by `source.facts`;
//! `implements` on a reduction (v0.8.5 to v0.8.8), which v0.8.9 replaced by
//! a named answer that a policy binds with `source.reduction`; and `combine`
//! on an open term (v0.8.8 to v0.8.12) or in a `source` (v0.8.9 to v0.8.12),
//! which v0.8.13 removed, since one question goes to one cell.
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
//! A call to an article (`source.article`, schema v0.8.4, RFC-045 §6) stands
//! apart from `regulation`, `decisions` and `endpoint`, and only such a call
//! takes `parameters_from`.
//!
//! Who may answer an open term, or establish a fact, sits on that term or
//! fact (`competent_authority`, schema v0.8.8, RFC-051); on an article the
//! field keeps its meaning of RFC-002, who decides, and names one authority.

use crate::answer::ChronicleRead;
use crate::article::ArticleBasedLaw;
use crate::error::{EngineError, Result};
use regelrecht_law_model::{
    ActionOperation, ActionValue, Column, CompetentAuthority, MachineReadable, ParameterType,
    RegulatoryLayer, StateProperty, names_fact,
};

/// Refuse a construct that a later schema version removed, naming what
/// replaced it. The engine runs every file on one code path (see the module
/// docs), so it cannot run such a construct; dropping it silently would run
/// the file without it. So far there are three: `source.executograms`
/// (v0.8.6), which v0.8.7 replaced by `source.facts` (RFC-051); `implements`
/// on a `REDUCTIE` (v0.8.5 to v0.8.8), which v0.8.9 replaced by a named
/// answer of the cell that a policy binds to the term (`source.reduction`,
/// RFC-048); and `combine` on an open term (v0.8.8 to v0.8.12) or in a
/// `source` (v0.8.9 to v0.8.12), which v0.8.13 removed: one question goes to
/// one cell, and an article makes one outcome of the answers of different
/// cells (RFC-048 §4).
pub(crate) fn reject_removed_constructs(law: &ArticleBasedLaw) -> Result<()> {
    let refuse = |why: String| Err(EngineError::LoadError(format!("law '{}' {why}", law.id)));
    let reduction = law.regulatory_layer == RegulatoryLayer::Reductie;
    for (where_, section) in sections(law) {
        if reduction && section.implements.is_some() {
            return refuse(format!(
                "{where_}: a REDUCTIE fills no open term (implements), which schema v0.8.9 \
                 removed; its cell gives it as a named answer, which a policy binds to the term \
                 with source.reduction (RFC-048)"
            ));
        }
        for term in section.open_terms.iter().flatten() {
            if term.combine.is_some() {
                return refuse(format!(
                    "{where_}: open term '{}' has combine, which schema v0.8.13 removed; one \
                     question goes to one cell, and an article makes one outcome of the answers \
                     of different cells (RFC-048 §4)",
                    term.id
                ));
            }
        }
        for input in inputs(section) {
            if input
                .source
                .as_ref()
                .is_some_and(|source| source.combine.is_some())
            {
                return refuse(format!(
                    "{where_}: input '{}' has source.combine, which schema v0.8.13 removed; one \
                     question goes to one cell, and an article makes one outcome of the answers \
                     of different cells (RFC-048 §4)",
                    input.name
                ));
            }
            if input
                .source
                .as_ref()
                .is_some_and(|source| source.executograms.is_some())
            {
                return refuse(format!(
                    "{where_}: input '{}' reads source.executograms, which schema v0.8.7 \
                     replaced; use source.facts (RFC-051)",
                    input.name
                ));
            }
        }
    }
    Ok(())
}

/// Refuse a reading of a chronicle (`source.decisions`, `source.facts`)
/// outside a `REDUCTIE`, the keys that act on law and a holder
/// (`competent_authority`) in a `REDUCTIE`, and a misplaced answer of a
/// cell (`source.reduction`, RFC-048). An answer is taken only in a
/// `UITVOERINGSBELEID` that names its holder, by an article that fills an
/// open term; it stands apart from every other way to a value.
pub(crate) fn reject_misplaced_reduction(law: &ArticleBasedLaw) -> Result<()> {
    let refuse = |why: String| Err(EngineError::LoadError(format!("law '{}' {why}", law.id)));
    let reduction = law.regulatory_layer == RegulatoryLayer::Reductie;
    let policy = law.regulatory_layer == RegulatoryLayer::Uitvoeringsbeleid;
    for (where_, section) in sections(law) {
        if reduction && let Some(key) = acts_on_law(section) {
            return refuse(format!(
                "{where_}: a REDUCTIE is no regulation and does not act on law ({key})"
            ));
        }
        for input in inputs(section) {
            let Some(source) = input.source.as_ref() else {
                continue;
            };
            if let Err(why) = check_projections_read(
                input,
                source,
                section,
                reduction,
                from_version(law, (0, 8, 19)),
            ) {
                return refuse(format!("{where_}: {why}"));
            }
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
            // From schema v0.8.17 a reduction reads no decisions: a row about
            // a decision comes from an aggregate (`source.projections`).
            if source.decisions.is_some() && from_version(law, (0, 8, 17)) {
                return refuse(format!(
                    "{where_}: input '{}' reads {read}; since schema v0.8.17 a REDUCTIE reads \
                     the aggregates of the decisions (source.projections), not the decisions \
                     themselves (source.decisions)",
                    input.name
                ));
            }
            // From schema v0.8.19 a reduction reads no recorded facts either:
            // an aggregate plays them (`projection.per`), and a read model
            // reads that aggregate.
            if source.facts.is_some() && from_version(law, (0, 8, 19)) {
                return refuse(format!(
                    "{where_}: input '{}' reads {read}; since schema v0.8.19 a REDUCTIE reads \
                     the aggregates of the facts (source.projections), not the facts \
                     themselves (source.facts)",
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
        // Checked after the inputs, so that a reading of the chronicle that
        // the version refuses names itself first.
        if reduction
            && from_version(law, (0, 8, 19))
            && let Some(why) = neither_aggregate_nor_read_model(section)
        {
            return refuse(format!("{where_}: {why}"));
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

/// Since schema v0.8.19 each article of a `REDUCTIE` with a
/// `machine_readable` takes one of two forms: an aggregate (a projection,
/// which plays the chronicle) or a read model, whose execution reads
/// aggregates (`source.projections`). One file may hold both kinds. Why
/// `section` is neither, or `None` when it is one of them; an article with
/// only text has no `machine_readable` and is not asked. An article that is
/// both, a projection with an execution, is refused with the projection.
fn neither_aggregate_nor_read_model(section: &MachineReadable) -> Option<String> {
    if section.projection.is_some() {
        return None;
    }
    let reads_aggregates = inputs(section)
        .iter()
        .any(|i| i.source.as_ref().is_some_and(|s| s.projections.is_some()));
    (!reads_aggregates).then(|| {
        "reads no aggregates (source.projections) and plays none (projection); since schema \
         v0.8.19 a REDUCTIE is an aggregate or a read model over one"
            .to_string()
    })
}

/// Since schema v0.8.19 an input that reads aggregates names an article that
/// plays them (`<regulation>#<article>`): an article of the same file, or of
/// a reduction the loader holds. A reduction the loader does not hold yet is
/// checked when the aggregates are played, because the two load in any
/// order.
fn check_projections_targets<'l>(
    law: &ArticleBasedLaw,
    lookup: &dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
) -> std::result::Result<(), String> {
    for (where_, section) in sections(law) {
        for input in inputs(section) {
            let Some(reference) = input.source.as_ref().and_then(|s| s.projections.as_deref())
            else {
                continue;
            };
            let (target, article) = regelrecht_law_model::split_projections(reference);
            // `lookup` gives this law for its own id.
            if let Some(held) = lookup(target) {
                crate::projection::aggregate_projection(held, article)
                    .map_err(|why| format!("{where_}: input '{}': {why}", input.name))?;
            }
        }
    }
    Ok(())
}

/// An input that reads the aggregates of a reduction (`source.projections`,
/// schema v0.8.17) does so in a `REDUCTIE`, and stands alone: no other way
/// to a value beside it. `where` names the roots by values the reading
/// article declares as parameters, and stands only beside `projections`.
fn check_projections_read(
    input: &regelrecht_law_model::Input,
    source: &regelrecht_law_model::Source,
    section: &MachineReadable,
    reduction: bool,
    since_v0_8_19: bool,
) -> std::result::Result<(), String> {
    let name = &input.name;
    let Some(of) = &source.projections else {
        return match source.where_ {
            Some(_) => Err(format!(
                "input '{name}' has where without projections; where picks the aggregates a \
                 reduction reads"
            )),
            None => Ok(()),
        };
    };
    let beside = [
        ("regulation", source.regulation.is_some()),
        ("output", source.output.is_some()),
        ("parameters", source.parameters.is_some()),
        ("endpoint", source.endpoint.is_some()),
        ("decisions", source.decisions.is_some()),
        ("facts", source.facts.is_some()),
        ("reduction", source.reduction.is_some()),
        ("article", source.article.is_some()),
    ];
    if let Some((key, _)) = beside.iter().find(|(_, present)| *present) {
        return Err(format!(
            "input '{name}' reads the aggregates of {of}, which no {key} can stand beside"
        ));
    }
    if !reduction {
        return Err(format!(
            "input '{name}' reads the aggregates of {of}; only a REDUCTIE reads a chronicle \
             (RFC-048)"
        ));
    }
    let declared: Vec<&str> = section
        .execution
        .as_ref()
        .and_then(|e| e.parameters.as_deref())
        .unwrap_or_default()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    // From schema v0.8.19 the reference names the article of the aggregate,
    // also in the reading file itself; up to v0.8.18 the reduction alone.
    if since_v0_8_19 && !regelrecht_law_model::is_article_reference(of) {
        return Err(format!(
            "input '{name}' reads the aggregates of {of}; since schema v0.8.19 \
             source.projections names the article of the aggregate, as {of}#<article>"
        ));
    }
    for (field, reference) in source.where_.iter().flatten() {
        let parameter = reference.strip_prefix('$').filter(|p| declared.contains(p));
        if parameter.is_none() {
            return Err(format!(
                "input '{name}' picks the aggregates of {of} where '{field}' is {reference}, \
                 which is no $-reference to a parameter of the article"
            ));
        }
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

/// Refuse a call to an article (`source.article`, schema v0.8.4, RFC-045 §6)
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

/// Refuse competence where it does not belong (RFC-051):
///
/// - an article names one competent authority (RFC-002), never a list;
/// - an open term or a fact names its competent authorities by name, one or
///   a list of at least one, never a `#` reference;
/// - `competent_authority` on a `requires` entry only with `fact`.
pub(crate) fn reject_misplaced_competence(law: &ArticleBasedLaw) -> Result<()> {
    for (where_, section) in sections(law) {
        let refuse = |why: String| {
            Err(EngineError::LoadError(format!(
                "law '{}' {where_}: {why} (RFC-051)",
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
                    );
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
                if requirement.effects.is_some() {
                    return refuse("lists effects on a requirement that names no fact".to_string());
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
            // An effect of a fact (schema v0.8.15) revokes the record the fact
            // refers to (`revokes: refers_to`), so which record it is about
            // has one source. Its `when` is a boolean the fact holds, not
            // nullable.
            let held = |name: &str| {
                parameters
                    .iter()
                    .any(|p| p == name)
                    .then(|| article.get_parameters().iter().find(|p| p.name == name))
                    .flatten()
            };
            for effect in requirement.effects() {
                let check = |name: &str, role: &str, types: &[ParameterType]| {
                    let what = |why: String| {
                        Err(format!(
                            "requires the fact '{fact}' with an effect that names '{name}' as \
                             {role}, which {why}"
                        ))
                    };
                    match held(name) {
                        None => what("the fact does not hold".to_string()),
                        Some(p) if !types.contains(&p.param_type) => what(format!(
                            "is a {:?}, not a {}",
                            p.param_type,
                            types
                                .iter()
                                .map(|t| format!("{t:?}"))
                                .collect::<Vec<_>>()
                                .join(" or ")
                        )),
                        // A record without a target or a condition is refused
                        // when the chronicle is played; refuse it here first.
                        Some(p) if p.is_nullable() => what("may be null".to_string()),
                        Some(_) => Ok(()),
                    }
                };
                if effect.parameter != regelrecht_law_model::RecordEffect::REFERS_TO {
                    return refuse(format!(
                        "requires the fact '{fact}' with an effect that revokes '{}'; a fact \
                         revokes the record it refers to: write `revokes: {}`",
                        effect.parameter,
                        regelrecht_law_model::RecordEffect::REFERS_TO
                    ));
                }
                let named = effect
                    .when
                    .as_deref()
                    .map(|w| (w, "its condition", &[ParameterType::Boolean][..]));
                if let Some((name, role, types)) = named
                    && let Err(why) = check(name, role, types)
                {
                    return refuse(why);
                }
            }
        }
    }
    Ok(())
}

/// Whether `law` declares schema `version` or later, or no version at all.
/// A rule a released version did not have applies from the version that
/// introduces it, so a file valid under an earlier version still loads.
pub(crate) fn from_version(law: &ArticleBasedLaw, version: (u32, u32, u32)) -> bool {
    law.schema_version()
        .and_then(regelrecht_law_model::parse_schema_version)
        .is_none_or(|declared| declared >= version)
}

/// Refuse an article whose `produces` list (schema v0.8.1, RFC-045 §1) has two
/// items of one type, gives an output to two items (as one of its
/// `outputs`, or as data `derived` for the decision, RFC-045 §1), lists
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
/// (RFC-045 §1).
pub(crate) fn reject_ambiguous_produces(law: &ArticleBasedLaw) -> Result<()> {
    for article in law.articles.iter().filter(|a| a.produces_list()) {
        let produced = article.get_produced();
        let refuse = |why: String| {
            Err(EngineError::LoadError(format!(
                "law '{}': article {} {why} (RFC-045 §1)",
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
            let Some(target) = item.requests() else {
                continue;
            };
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
                        "requests the decision of '{target}', but this law has no article {number}"
                    ));
                }
                Some(a) if a.get_decision().is_none() => {
                    return refuse(format!(
                        "requests the decision of '{target}', which takes no decision"
                    ));
                }
                Some(_) => {}
            }
        }
        if let Some(item) = produced
            .iter()
            .find(|p| p.legal_character().is_none() && p.listed_derived().is_some())
        {
            return refuse(format!(
                "lists derived outputs on its {}; only a decision has derived data (RFC-045 §1)",
                item.submission_kind().unwrap_or("item")
            ));
        }
        // Without a decision, an output no submission lists would be read by
        // nothing the article produces: refused, so a forgotten or misspelt
        // output fails. Beside a decision that lists its outputs, it belongs
        // to no item (RFC-045 §1).
        if article.get_decision().is_none()
            && let Some(name) = article
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
            if item.legal_character().is_none() && !item.effects().is_empty() {
                return refuse(format!(
                    "lists effects on its {}; only a decision has effects on another decision",
                    item.submission_kind().unwrap_or("item")
                ));
            }
            for effect in item.effects() {
                let name = &effect.parameter;
                let Some(parameter) = article.get_parameters().iter().find(|p| &p.name == name)
                else {
                    return refuse(format!(
                        "names parameter '{name}' in an effect under produces, but declares no such parameter"
                    ));
                };
                // The parameter names the record it revokes: a string that
                // holds its id, or a decision, which the cell keeps by its id
                // in the chronicle (`bestreden_besluit` in Awb 7:11). Never
                // null: a decision without the record it revokes is refused
                // when the chronicle is played; refuse it here first. From
                // schema v0.8.15 on: a file valid under v0.8.14 still loads.
                let names_a_record = matches!(
                    parameter.param_type,
                    ParameterType::String | ParameterType::Decision
                );
                if !names_a_record && from_version(law, (0, 8, 15)) {
                    return refuse(format!(
                        "names parameter '{name}' in an effect under produces, which is a {:?}; \
                         the record an effect revokes is a string that holds its id, or a \
                         decision",
                        parameter.param_type
                    ));
                }
                if parameter.is_nullable() && from_version(law, (0, 8, 15)) {
                    return refuse(format!(
                        "names parameter '{name}' in an effect under produces, which may be null"
                    ));
                }
                // `when` reads an outcome of this decision: one of its own
                // outputs, and a boolean, so the effect holds or it does not.
                if let Some(when) = &effect.when {
                    if !article.outputs_of(item).contains(&when.as_str()) {
                        return refuse(format!(
                            "makes an effect depend on '{when}', which is no output of its decision"
                        ));
                    }
                    let boolean = article.get_outputs().iter().any(|o| {
                        &o.name == when
                            && o.output_type == regelrecht_law_model::ParameterType::Boolean
                    });
                    if !boolean {
                        return refuse(format!(
                            "makes an effect depend on '{when}', which is not a boolean"
                        ));
                    }
                }
            }
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

/// A projection (schema v0.8.14) sits only in a reduction, names each
/// property of its state once, and its steps read and set only those
/// properties.
pub(crate) fn reject_malformed_projection(law: &ArticleBasedLaw) -> Result<()> {
    let refuse = |where_: String, why: String| {
        Err(EngineError::LoadError(format!(
            "law '{}' {where_}: {why}",
            law.id
        )))
    };
    for (where_, section) in sections(law) {
        let Some(projection) = &section.projection else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Reductie {
            return refuse(
                where_,
                "has a projection; only a reduction plays a chronicle forward".to_string(),
            );
        }
        if projection.state.is_empty() {
            return refuse(where_, "has a projection without a state".to_string());
        }
        // From schema v0.8.17 a projection plays one aggregate per record
        // (`per`); up to v0.8.16 it played the records of one `key`.
        if from_version(law, (0, 8, 17)) {
            if let Some(key) = &projection.key {
                return refuse(
                    where_,
                    format!(
                        "plays the records of one '{key}' (key); since schema v0.8.17 a \
                         projection plays one aggregate per record (per), read with \
                         source.projections"
                    ),
                );
            }
            if projection.per.is_none() {
                return refuse(
                    where_,
                    "has a projection without per; since schema v0.8.17 a projection plays \
                     one aggregate per record"
                        .to_string(),
                );
            }
        }
        // A record address may name a fact since schema v0.8.18.
        if !from_version(law, (0, 8, 18)) {
            let named = projection
                .per_basis_and_kind()
                .into_iter()
                .chain(projection.steps.iter().map(|s| s.basis_and_kind()))
                .find_map(|(_, segment)| segment.filter(|s| names_fact(s)));
            if let Some(fact) = named {
                return refuse(
                    where_,
                    format!(
                        "names the fact '{fact}'; a fact name came with schema v0.8.18, \
                         before it a record address names a kind"
                    ),
                );
            }
        }
        if let Some(per) = &projection.per {
            if projection.key.is_some() {
                return refuse(
                    where_,
                    format!(
                        "has a projection with a key and per {per}; it plays the records of \
                         one key or one aggregate per record, not both"
                    ),
                );
            }
            if section.execution.is_some() {
                return refuse(
                    where_,
                    format!(
                        "has a projection per {per} and an execution; it answers no question \
                         of its own, a reduction reads its aggregates with source.projections"
                    ),
                );
            }
            // A row of the aggregates holds the id of its root under `id`.
            if projection
                .state
                .iter()
                .any(|p| p.name == crate::projection::ROOT_ID)
            {
                return refuse(
                    where_,
                    format!(
                        "has a projection per {per} with a state property '{}'; a row of its \
                         aggregates holds the id of its root under that name",
                        crate::projection::ROOT_ID
                    ),
                );
            }
        }
        let parameters = section
            .execution
            .as_ref()
            .and_then(|e| e.parameters.as_deref())
            .unwrap_or_default();
        if let Some(key) = &projection.key
            && !parameters.iter().any(|p| &p.name == key)
        {
            return refuse(
                where_,
                format!("plays the records of one '{key}', which is no parameter it declares"),
            );
        }
        let declares_state = section
            .execution
            .as_ref()
            .and_then(|e| e.parameters.as_ref())
            .is_some_and(|ps| ps.iter().any(|p| p.name == crate::projection::STATE));
        if declares_state {
            return refuse(
                where_,
                format!(
                    "declares a parameter '{}', which its projection gives",
                    crate::projection::STATE
                ),
            );
        }
        let mut names: Vec<&str> = Vec::new();
        let since_v0_8_20 = from_version(law, (0, 8, 20));
        for property in &projection.state {
            if let Err(why) = check_state_property(property, since_v0_8_20) {
                return refuse(where_, why);
            }
            if names.contains(&property.name.as_str()) {
                return refuse(
                    where_,
                    format!("names state property '{}' twice", property.name),
                );
            }
            names.push(&property.name);
        }
        for (i, step) in projection.steps.iter().enumerate() {
            // Two steps for one record would leave which one applies to the
            // order they are written in.
            let (basis, kind) = step.basis_and_kind();
            // A step on a fact (schema v0.8.18) and one on EXECUTOGRAM on
            // the same article both apply to an instance of that fact.
            let overlap = |a: Option<&str>, b: Option<&str>| match (a, b) {
                (None, _) | (_, None) => true,
                (Some(a), Some(b)) => {
                    a == b
                        || (names_fact(a) && b == crate::projection::EXECUTOGRAM)
                        || (a == crate::projection::EXECUTOGRAM && names_fact(b))
                }
            };
            if projection.steps[..i].iter().any(|earlier| {
                let (earlier_basis, earlier_kind) = earlier.basis_and_kind();
                earlier_basis == basis && overlap(earlier_kind, kind)
            }) {
                return refuse(
                    where_,
                    format!("has two steps for the same records under {basis}"),
                );
            }
            // Since schema v0.8.17 a step passes no parameter of the
            // question: the fold depends on the chronicle alone. A file
            // under an earlier version keeps what that version allowed.
            let question: Option<Vec<&str>> =
                passes_question(law).then(|| parameters.iter().map(|p| p.name.as_str()).collect());
            if let Err(why) = check_step_shape(step, &names, question.as_deref()) {
                return refuse(
                    where_,
                    format!("the step for records under {} {why}", step.record),
                );
            }
        }
    }
    Ok(())
}

/// Whether `ty` is a scalar: the type of a column of an array of rows, and
/// of every property of a projection's state but such an array.
fn is_scalar(ty: ParameterType) -> bool {
    matches!(
        ty,
        ParameterType::Boolean
            | ParameterType::Number
            | ParameterType::Amount
            | ParameterType::String
            | ParameterType::Date
    )
}

/// A property of a projection's state is a scalar without columns, or since
/// schema v0.8.20 (`since_v0_8_20`) an array with its columns
/// ([`check_columns`]). `nullable` also came with v0.8.20, and an array in
/// the state is never null. Before v0.8.20 an array in the state is refused as
/// any other type a state cannot have, unless it declares columns, which
/// the message then names.
fn check_state_property(
    property: &StateProperty,
    since_v0_8_20: bool,
) -> std::result::Result<(), String> {
    let name = &property.name;
    let ty = property.property_type;
    if property.nullable.is_some() && !since_v0_8_20 {
        return Err(format!(
            "declares state property '{name}' nullable, which came with schema v0.8.20"
        ));
    }
    if property.nullable == Some(true) && ty == ParameterType::Array {
        return Err(format!(
            "declares state property '{name}' nullable; an array in the state is never null, \
             it starts as an empty list"
        ));
    }
    match property.columns.as_deref() {
        Some(_) if ty == ParameterType::Array && !since_v0_8_20 => Err(format!(
            "gives state property '{name}' the type array with columns, which came with \
             schema v0.8.20"
        )),
        Some(columns) if ty == ParameterType::Array => {
            check_columns(columns).map_err(|why| format!("state property '{name}' {why}"))
        }
        None if ty == ParameterType::Array && since_v0_8_20 => Err(format!(
            "gives state property '{name}' the type array without columns; a state holds an \
             array only with its columns"
        )),
        _ if !is_scalar(ty) => Err(format!(
            "gives state property '{name}' the type {ty:?}, which a state cannot have"
        )),
        Some(_) => Err(format!(
            "declares columns for state property '{name}', which is no array"
        )),
        None => Ok(()),
    }
}

/// The columns of an array of rows (schema v0.8.20): at least one, each
/// named once and of a scalar type.
fn check_columns(columns: &[Column]) -> std::result::Result<(), String> {
    if columns.is_empty() {
        return Err("declares no columns".to_string());
    }
    for (i, column) in columns.iter().enumerate() {
        if columns[..i].iter().any(|c| c.name == column.name) {
            return Err(format!("names column '{}' twice", column.name));
        }
        if !is_scalar(column.column_type) {
            return Err(format!(
                "gives column '{}' the type {:?}; a column holds a boolean, number, string, \
                 amount or date",
                column.name, column.column_type
            ));
        }
    }
    Ok(())
}

/// Refuse a parameter that declares columns (schema v0.8.20) but is no
/// array, or whose columns [`check_columns`] refuses. This holds for a file
/// under any version: a projection compares the columns of the fact it reads
/// whatever version that fact's law declares, and no file before v0.8.20
/// has `columns` on a parameter.
pub(crate) fn reject_malformed_columns(law: &ArticleBasedLaw) -> Result<()> {
    for (where_, section) in sections(law) {
        let parameters = section
            .execution
            .as_ref()
            .and_then(|e| e.parameters.as_deref())
            .unwrap_or_default();
        for parameter in parameters {
            let Some(columns) = parameter.columns.as_deref() else {
                continue;
            };
            let why = if parameter.param_type == ParameterType::Array {
                check_columns(columns).err()
            } else {
                Some(format!(
                    "declares columns, but is a {:?}; only an array has columns",
                    parameter.param_type
                ))
            };
            if let Some(why) = why {
                return Err(EngineError::LoadError(format!(
                    "law '{}' {where_}: parameter '{}' {why}",
                    law.id, parameter.name
                )));
            }
        }
    }
    Ok(())
}

/// Whether a step of `law` may pass a parameter of the question: from schema
/// v0.8.14 up to v0.8.16, which allowed it. A file that names no version is held to
/// the newest.
fn passes_question(law: &ArticleBasedLaw) -> bool {
    !from_version(law, (0, 8, 17))
}

/// The shape of a step, apart from the laws it calls (which are in other
/// regelwerken, so the engine checks them when it plays the projection).
/// Its inputs each take an output of a law, `source: {regulation, output,
/// parameters}` and nothing else, with parameters `$record.<field>` or
/// `$state.<property>`. It sets at least one property of its state, each to
/// a bare reference: one of its inputs, a field of the record or a property
/// of the state, through actions with an output and a value. A step
/// computes nothing itself.
fn check_step_shape(
    step: &regelrecht_law_model::ProjectionStep,
    names: &[&str],
    question: Option<&[&str]>,
) -> std::result::Result<(), String> {
    use regelrecht_law_model::StepSource;
    let own = |source: &StepSource| match source {
        StepSource::State(p) if !names.contains(&p.as_str()) => {
            Err(format!("reads {source}, which is no property of its state"))
        }
        StepSource::Input(i) if !step.input.iter().any(|input| &input.name == i) => {
            Err(format!("reads {source}, which is no input of the step"))
        }
        _ => Ok(()),
    };
    for (n, input) in step.input.iter().enumerate() {
        if step.input[..n]
            .iter()
            .any(|earlier| earlier.name == input.name)
        {
            return Err(format!("names input '{}' twice", input.name));
        }
        let allowed = ["regulation", "output", "parameters", "description"];
        let source = input
            .source
            .as_ref()
            .map(|s| serde_json::to_value(s).unwrap_or_default());
        let keys: Vec<String> = source
            .as_ref()
            .and_then(|s| s.as_object())
            .map(|o| {
                o.iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, _)| k.clone())
                    .collect()
            })
            .unwrap_or_default();
        if !keys.iter().any(|k| k == "regulation") || !keys.iter().any(|k| k == "output") {
            return Err(format!(
                "has input '{}' that takes no output of a regulation; a step takes only \
                 source.regulation with an output",
                input.name
            ));
        }
        if let Some(other) = keys.iter().find(|k| !allowed.contains(&k.as_str())) {
            return Err(format!(
                "has input '{}' with source.{other}; a step takes only source.regulation with \
                 an output and parameters",
                input.name
            ));
        }
        let parameters = input.source.as_ref().and_then(|s| s.parameters.as_ref());
        for (name, value) in parameters.into_iter().flatten() {
            match StepSource::parse(value) {
                Some(source @ (StepSource::Record(_) | StepSource::State(_))) => own(&source)?,
                // A parameter of the question the reduction answers.
                Some(StepSource::Input(asked))
                    if question.is_some_and(|q| q.contains(&asked.as_str())) => {}
                _ if question.is_none() => {
                    return Err(format!(
                        "has input '{}' that passes '{name}' as '{value}'; a parameter of a step \
                         is $record.<field> or $state.<property>, and since schema v0.8.17 no \
                         parameter of the question",
                        input.name
                    ));
                }
                _ => {
                    return Err(format!(
                        "has input '{}' that passes '{name}' as '{value}'; a parameter of a step \
                         is $record.<field>, $state.<property> or a parameter of the question",
                        input.name
                    ));
                }
            }
        }
    }
    if step.actions.is_empty() {
        return Err("sets nothing".to_string());
    }
    let mut given: Vec<&str> = Vec::new();
    for action in &step.actions {
        // An action of a step names a property and gives it a value; it
        // computes nothing, so it has no operation of its own.
        let fields = serde_json::to_value(action).unwrap_or_default();
        let extra = fields.as_object().and_then(|o| {
            o.iter()
                .find(|(k, v)| {
                    !v.is_null() && !["output", "value", "legal_basis"].contains(&k.as_str())
                })
                .map(|(k, _)| k.clone())
        });
        let Some(property) = action.output.as_deref() else {
            return Err("has an action without an output".to_string());
        };
        if let Some(extra) = extra {
            return Err(format!(
                "sets '{property}' with {extra}; an action of a step has an output and a value, \
                 because a step computes nothing itself"
            ));
        }
        if !names.contains(&property) {
            return Err(format!(
                "sets '{property}', which is no property of its state"
            ));
        }
        if given.contains(&property) {
            return Err(format!("sets '{property}' twice"));
        }
        given.push(property);
        let source = match &action.value {
            Some(ActionValue::Literal(regelrecht_law_model::Value::String(s))) => {
                StepSource::parse(s)
            }
            _ => None,
        };
        let Some(source) = source else {
            return Err(format!(
                "sets '{property}' to something that is no bare reference; write $<input>, \
                 $record.<field> or $state.<property>, because a step computes nothing itself"
            ));
        };
        own(&source)?;
    }
    Ok(())
}

/// A projection fits the laws its steps name (schema v0.8.16), in the
/// versions `lookup` gives: the law each step's records rest on and each law
/// an input calls ([`crate::projection::check_step`]). The reduction and the
/// law are separate regelwerken that load in any order, so a law `lookup`
/// does not give is not checked here. A law that arrives later loads
/// whatever a reduction expects of it, so the law never depends on the
/// reduction; the engine checks the versions in force when it plays the
/// projection. The error names the article and the step.
///
/// The check at load came with schema v0.8.16. A reduction on v0.8.15 or
/// earlier is held to its steps only when it plays, as the released versions
/// were, so a law version that is not yet in force cannot refuse it at load.
pub(crate) fn check_projection_wiring<'l>(
    law: &ArticleBasedLaw,
    lookup: &dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
) -> std::result::Result<(), String> {
    if !from_version(law, (0, 8, 16)) {
        return Ok(());
    }
    if from_version(law, (0, 8, 19)) {
        check_projections_targets(law, lookup)?;
    }
    for (where_, section) in sections(law) {
        let Some(projection) = &section.projection else {
            continue;
        };
        let question = section
            .execution
            .as_ref()
            .and_then(|e| e.parameters.as_deref())
            .unwrap_or_default();
        crate::projection::check_per(
            projection,
            lookup,
            crate::projection::Versions::Loaded,
            from_version(law, (0, 8, 18)),
        )
        .map_err(|why| {
            format!(
                "{where_}: per {}: {why}",
                projection.per.as_deref().unwrap_or_default()
            )
        })?;
        for step in &projection.steps {
            crate::projection::check_step(
                projection,
                step,
                question,
                lookup,
                crate::projection::Versions::Loaded,
                crate::projection::StepRules {
                    since_v0_8_16: true,
                    // `$record.id` is the record's own id from v0.8.19, in a
                    // projection with `per`, as when the projection plays.
                    own_id: projection.per.is_some() && from_version(law, (0, 8, 19)),
                    since_v0_8_20: from_version(law, (0, 8, 20)),
                },
            )
            .map_err(|why| format!("{where_}: the step on {}: {why}", step.record))?;
        }
    }
    Ok(())
}

/// `$state` is the state a projection gives (schema v0.8.14). An article
/// with a projection reads it only as `$state.<property>`, and the property
/// must be one its state declares: a misspelt name would otherwise load and
/// fail only when a chronicle is played. Reading the state as a whole is
/// refused too, because an answer names what it reads. An article without a
/// projection has no state, so it cannot read `$state` unless it declares a
/// variable of that name itself.
pub(crate) fn reject_unknown_state(law: &ArticleBasedLaw) -> Result<()> {
    use crate::projection::STATE;
    for (where_, section) in sections(law) {
        let Some(execution) = section.execution.as_ref() else {
            continue;
        };
        let from_actions = execution
            .actions
            .iter()
            .flatten()
            .flat_map(crate::demand::referenced_paths);
        let from_inputs = execution
            .input
            .iter()
            .flatten()
            .filter_map(|input| input.source.as_ref()?.parameters.as_ref())
            .flat_map(|parameters| parameters.values())
            .filter_map(|value| value.strip_prefix('$').map(str::to_string));
        let reads: std::collections::BTreeSet<String> = from_actions
            .chain(from_inputs)
            .filter(|path| path == STATE || path.starts_with(&format!("{STATE}.")))
            .collect();
        let Some(first) = reads.iter().next() else {
            continue;
        };
        let refuse = |why: String| {
            Err(EngineError::LoadError(format!(
                "law '{}' {where_}: {why}",
                law.id
            )))
        };
        let Some(projection) = &section.projection else {
            let declares = execution
                .parameters
                .iter()
                .flatten()
                .any(|p| p.name == STATE)
                || execution.input.iter().flatten().any(|i| i.name == STATE)
                || execution.output.iter().flatten().any(|o| o.name == STATE);
            if declares {
                continue;
            }
            return refuse(format!(
                "reads ${first}, but only a projection gives a state, and it has none"
            ));
        };
        let declared: Vec<&str> = projection.state.iter().map(|p| p.name.as_str()).collect();
        for path in &reads {
            let property = path
                .strip_prefix(&format!("{STATE}."))
                .map(|rest| rest.split('.').next().unwrap_or(rest));
            match property {
                None => {
                    return refuse(format!(
                        "reads $state as a whole; name a property of its state: {}",
                        declared.join(", ")
                    ));
                }
                Some(name) if !declared.contains(&name) => {
                    return refuse(format!(
                        "reads $state.{name}, which is no property of its state: {}",
                        declared.join(", ")
                    ));
                }
                Some(_) => {}
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
    /// (RFC-045 §1): the article loads, and it is no output of the decision,
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
    /// item (RFC-045 §1): the article loads, and the output is neither an
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
                "          - type: AANVRAAG\n            parameters: [p]",
                "declares output 'x', but nothing it produces yields it",
            ),
            (
                "          - type: AANVRAAG\n            outputs: [x]\n          - type: BESCHIKKING\n            outputs: [x]",
                "gives output 'x' to two things",
            ),
            (
                "          - type: AANVRAAG\n            parameters: [p]\n            effects:\n              - revokes: p\n          - type: BESCHIKKING",
                "lists effects on its AANVRAAG",
            ),
            (
                "          - type: BESCHIKKING\n            effects:\n              - revokes: q",
                "names parameter 'q' in an effect",
            ),
            (
                "          - type: BESCHIKKING\n            effects:\n              - revokes: p\n                when: y",
                "depend on 'y', which is no output of its decision",
            ),
            (
                "          - type: AANVRAAG\n            outputs: [x]\n          - type: BESCHIKKING\n            effects:\n              - revokes: p\n                when: x",
                "depend on 'x', which is no output of its decision",
            ),
            (
                "          - type: BESCHIKKING\n            derived: [x]\n            effects:\n              - revokes: p\n                when: x",
                "depend on 'x', which is no output of its decision",
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

    /// A decision's effects name a parameter of the article, and may depend
    /// on a boolean output of the decision (schema v0.8.14). There is one
    /// kind of effect: `replaces` is no effect.
    #[test]
    fn a_decision_may_revoke_by_a_parameter() {
        let replaces =
            "          - type: BESCHIKKING\n            effects:\n              - replaces: p";
        let err = LawExecutionService::new()
            .load_law(&law_producing(replaces))
            .expect_err("replaces is no effect");
        assert!(
            err.to_string().contains("unknown field `replaces`"),
            "{err}"
        );
        for effect in ["revokes: p", "revokes: p\n                when: x"] {
            let items = format!(
                "          - type: BESCHIKKING\n            effects:\n              - {effect}"
            );
            LawExecutionService::new()
                .load_law(&law_producing(&items))
                .unwrap_or_else(|e| panic!("{effect}: {e}"));
        }
    }

    /// `when` names an outcome that holds or does not: a boolean output.
    #[test]
    fn an_effect_depends_on_a_boolean_output() {
        let items = "          - type: BESCHIKKING\n            effects:\n              - revokes: p\n                when: x";
        let law = law_producing(items)
            .replace("            type: boolean", "            type: string")
            .replace("            value: true", "            value: ja");
        let err = LawExecutionService::new()
            .load_law(&law)
            .expect_err("a string output cannot decide an effect");
        assert!(
            err.to_string()
                .contains("depend on 'x', which is not a boolean"),
            "{err}"
        );
    }

    /// `law_producing` with a date parameter `vanaf`, a boolean `b`, a
    /// decision `d`, and the nullable `q` (a string) and `c` (a boolean).
    fn law_with_kinds(items: &str) -> String {
        law_producing(items).replace(
            "        parameters:\n          - name: p\n            type: string\n",
            "        parameters:\n          - name: p\n            type: string\n          - {name: vanaf, type: date}\n          - {name: b, type: boolean}\n          - {name: d, type: decision}\n          - {name: q, type: string, nullable: true}\n          - {name: c, type: boolean, nullable: true}\n",
        )
    }

    /// `law_with_kinds` that declares schema version `v` (`v0.8.14`).
    fn law_with_kinds_on(items: &str, v: &str) -> String {
        law_with_kinds(items).replacen(
            "$id: lijst",
            &format!(
                "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{v}/schema/{v}/schema.json\n$id: lijst"
            ),
            1,
        )
    }

    /// An effect revokes from the start; it holds for no window of days
    /// (schema v0.8.15 has no `from` or `until`), and the record it revokes
    /// is never null.
    #[test]
    fn a_decision_revokes_by_a_parameter_that_is_never_null() {
        for (effect, why) in [
            ("{revokes: p, from: vanaf}", "unknown field `from`"),
            ("{revokes: p, until: vanaf}", "unknown field `until`"),
            (
                "{revokes: q}",
                "names parameter 'q' in an effect under produces, which may be null",
            ),
        ] {
            let err = LawExecutionService::new()
                .load_law(&law_with_kinds(&format!(
                    "          - type: BESCHIKKING\n            effects:\n              - {effect}"
                )))
                .expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
    }
    /// The refusal of a nullable `revokes` parameter came with schema
    /// v0.8.15: a law that declares v0.8.14, which the released schema
    /// allows, still loads.
    #[test]
    fn a_nullable_revokes_still_loads_on_v0_8_14() {
        let items =
            "          - type: BESCHIKKING\n            effects:\n              - {revokes: q}";
        let on = |v: &str| law_with_kinds_on(items, v);
        LawExecutionService::new()
            .load_law(&on("v0.8.14"))
            .expect("valid under v0.8.14");
        let err = LawExecutionService::new()
            .load_law(&on("v0.8.15"))
            .expect_err("refused from v0.8.15");
        assert!(
            err.to_string()
                .contains("names parameter 'q' in an effect under produces, which may be null"),
            "{err}"
        );
    }

    /// A decision names the record it revokes by a string that holds its
    /// id, or by a decision, which the cell keeps by its id. From schema
    /// v0.8.15 the loader refuses another type; a law that declares v0.8.14
    /// still loads.
    #[test]
    fn a_decision_revokes_by_a_string_or_a_decision_from_v0_8_15() {
        let items = |parameter: &str| {
            format!(
                "          - type: BESCHIKKING\n            effects:\n              - {{revokes: {parameter}}}"
            )
        };
        for parameter in ["p", "d"] {
            LawExecutionService::new()
                .load_law(&law_with_kinds_on(&items(parameter), "v0.8.15"))
                .unwrap_or_else(|e| panic!("{parameter} names a record: {e}"));
        }
        for (parameter, kind) in [("b", "Boolean"), ("vanaf", "Date")] {
            LawExecutionService::new()
                .load_law(&law_with_kinds_on(&items(parameter), "v0.8.14"))
                .unwrap_or_else(|e| panic!("{parameter} loads on v0.8.14: {e}"));
            let err = LawExecutionService::new()
                .load_law(&law_with_kinds_on(&items(parameter), "v0.8.15"))
                .expect_err(parameter);
            assert!(
                err.to_string().contains(&format!(
                    "names parameter '{parameter}' in an effect under produces, which is a {kind}; \
                     the record an effect revokes is a string that holds its id, or a decision"
                )),
                "{err}"
            );
        }
    }

    /// A fact may revoke an earlier record (schema v0.8.15): the record it
    /// refers to, `revokes: refers_to`, so which record a fact is about has
    /// one source. Its condition is a boolean the fact holds, not nullable.
    #[test]
    fn a_fact_revokes_by_what_it_holds() {
        let fact = |parameters: &str, effect: &str| {
            law_with_kinds("          - type: BESCHIKKING").replace(
                "    machine_readable:\n      execution:\n",
                &format!(
                    "    machine_readable:\n      requires:\n        - fact: f\n          parameters: [{parameters}]\n          effects: [{{{effect}}}]\n      execution:\n"
                ),
            )
        };
        for (parameters, effect) in [
            ("p", "revokes: refers_to"),
            ("p, b", "revokes: refers_to, when: b"),
        ] {
            LawExecutionService::new()
                .load_law(&fact(parameters, effect))
                .unwrap_or_else(|e| panic!("{effect}: {e}"));
        }
        for (parameters, effect, why) in [
            (
                "p",
                "revokes: p",
                "with an effect that revokes 'p'; a fact revokes the record it refers to: write `revokes: refers_to`",
            ),
            (
                "p",
                "revokes: refers_to, when: b",
                "with an effect that names 'b' as its condition, which the fact does not hold",
            ),
            (
                "p, vanaf",
                "revokes: refers_to, when: vanaf",
                "with an effect that names 'vanaf' as its condition, which is a Date, not a Boolean",
            ),
            (
                "p, c",
                "revokes: refers_to, when: c",
                "with an effect that names 'c' as its condition, which may be null",
            ),
        ] {
            let err = LawExecutionService::new()
                .load_law(&fact(parameters, effect))
                .expect_err(why);
            assert!(
                err.to_string()
                    .contains(&format!("requires the fact 'f' {why}")),
                "{why}: {err}"
            );
        }
        let err = LawExecutionService::new()
            .load_law(&law_requiring(
                "        - article: '2'\n          effects: [{revokes: p}]",
            ))
            .expect_err("effects without a fact");
        assert!(
            err.to_string()
                .contains("lists effects on a requirement that names no fact"),
            "{err}"
        );
    }

    /// A rulework under schema v0.8.14, which allowed `key`, an `execution`
    /// beside the projection and a parameter of the question in a step.
    fn law_with_projection(layer: &str, projection: &str) -> String {
        format!(
            r#"
$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.14/schema/v0.8.14/schema.json
$id: projectie
regulatory_layer: {layer}
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      projection:
{projection}
      execution:
        output:
          - name: x
            type: boolean
        actions:
          - output: x
            value: $state.a
"#
        )
    }

    /// A projection sits in a reduction, names each property once with a
    /// type a state can have, and its steps set only those (schema v0.8.14).
    #[test]
    fn a_projection_keeps_to_its_own_state() {
        let state = "        state:\n          - {name: a, type: boolean}\n";
        let ok = format!(
            "{state}        steps:\n          - {{record: wet#1, actions: [{{output: a, value: $record.v}}]}}"
        );
        LawExecutionService::new()
            .load_law(&law_with_projection("REDUCTIE", &ok))
            .unwrap();
        // Steps for different kinds of record under one article, and a
        // string that a step sets to null.
        let kinds = "        state:\n          - {name: a, type: boolean}\n          - {name: s, type: string}\n        steps:\n          - {record: wet#1#AANVRAAG, actions: [{output: a, value: $record.v}]}\n          - {record: wet#1#BESCHIKKING, input: [{name: x, type: boolean, source: {regulation: wet, output: o, parameters: {p: $state.s, q: $record.k}}}], actions: [{output: a, value: $x}, {output: s, value: $state.s}]}";
        LawExecutionService::new()
            .load_law(&law_with_projection("REDUCTIE", kinds))
            .unwrap();
        // A step may pass a parameter of the question the reduction answers.
        let asks = format!(
            "{state}        steps:\n          - {{record: wet#1, input: [{{name: x, type: boolean, source: {{regulation: wet, output: o, parameters: {{p: $peildatum}}}}}}], actions: [{{output: a, value: $x}}]}}"
        );
        let with_question = law_with_projection("REDUCTIE", &asks).replace(
            "      execution:\n",
            "      execution:\n        parameters:\n          - {name: peildatum, type: date}\n",
        );
        LawExecutionService::new().load_law(&with_question).unwrap();
        // Without that parameter, `$peildatum` is nothing the step can pass.
        let err = LawExecutionService::new()
            .load_law(&law_with_projection("REDUCTIE", &asks))
            .expect_err("no parameter peildatum");
        assert!(
            err.to_string().contains(
                "has input 'x' that passes 'p' as '$peildatum'; a parameter of a step is \
                 $record.<field>, $state.<property> or a parameter of the question"
            ),
            "{err}"
        );
        let declares_state = law_with_projection("REDUCTIE", &ok).replace(
            "      execution:\n",
            "      execution:\n        parameters:\n          - {name: state, type: object}\n",
        );
        let err = LawExecutionService::new()
            .load_law(&declares_state)
            .expect_err("a declared state");
        assert!(
            err.to_string().contains("declares a parameter 'state'"),
            "{err}"
        );
        for (layer, projection, why) in [
            (
                "WET",
                ok.clone(),
                "only a reduction plays a chronicle forward",
            ),
            (
                "REDUCTIE",
                format!(
                    "{state}        steps:\n          - {{record: wet#1, actions: [{{output: b, value: $record.v}}]}}"
                ),
                "the step for records under wet#1 sets 'b', which is no property of its state",
            ),
            (
                "REDUCTIE",
                format!("{state}          - {{name: a, type: number}}\n        steps: []"),
                "names state property 'a' twice",
            ),
            (
                "REDUCTIE",
                "        state:\n          - {name: a, type: array}\n        steps: []".to_string(),
                "the type Array, which a state cannot have",
            ),
            (
                "REDUCTIE",
                "        state: []\n        steps: []".to_string(),
                "has a projection without a state",
            ),
            (
                "REDUCTIE",
                format!("        key: aanvrager\n{state}        steps: []"),
                "plays the records of one 'aanvrager', which is no parameter it declares",
            ),
            (
                "REDUCTIE",
                format!(
                    "{state}        steps:\n          - {{record: wet#1, actions: [{{output: a, value: $record.v}}]}}\n          - {{record: wet#1#BESCHIKKING, actions: [{{output: a, value: $record.v}}]}}"
                ),
                "has two steps for the same records under wet#1",
            ),
            (
                "REDUCTIE",
                format!(
                    "{state}        steps:\n          - {{record: wet#1#BESCHIKKING, actions: [{{output: a, value: $record.v}}]}}\n          - {{record: wet#1#BESCHIKKING, actions: [{{output: a, value: $record.v}}]}}"
                ),
                "has two steps for the same records under wet#1",
            ),
        ] {
            let err = LawExecutionService::new()
                .load_law(&law_with_projection(layer, &projection))
                .expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
        // A step takes outputs of a law and sets properties to bare
        // references; it computes nothing itself.
        let input = |source: &str| format!("[{{name: x, type: boolean, source: {source}}}]");
        let law_input = input("{regulation: wet, output: o, parameters: {p: $state.a}}");
        for (inputs, set, why) in [
            (law_input.clone(), "[{output: a, value: {operation: NOT, value: $x}}]", "sets 'a' to something that is no bare reference"),
            (law_input.clone(), "[{output: a, value: true}]", "sets 'a' to something that is no bare reference"),
            (law_input.clone(), "[{output: a, value: $y}]", "reads $y, which is no input of the step"),
            (law_input.clone(), "[{output: a, value: $state.b}]", "reads $state.b, which is no property of its state"),
            (law_input.clone(), "[]", "the step for records under wet#1 sets nothing"),
            (law_input.clone(), "[{output: a, value: $x}, {output: a, value: $state.a}]", "sets 'a' twice"),
            (law_input.clone(), "[{output: a, operation: NOT, value: $x}]", "sets 'a' with operation"),
            (input("{reduction: antwoord}"), "[{output: a, value: $x}]", "has input 'x' that takes no output of a regulation"),
            (input("{regulation: wet}"), "[{output: a, value: $x}]", "has input 'x' that takes no output of a regulation"),
            (input("{output: o}"), "[{output: a, value: $x}]", "has input 'x' that takes no output of a regulation"),
            (input("{regulation: wet, output: o, decisions: 'wet#1'}"), "[{output: a, value: $x}]", "has input 'x' with source.decisions"),
            (input("{regulation: wet, output: o, parameters: {p: true}}"), "[{output: a, value: $x}]", "passes 'p' as 'true'"),
            (input("{regulation: wet, output: o, parameters: {p: $x}}"), "[{output: a, value: $x}]", "passes 'p' as '$x'"),
            (input("{regulation: wet, output: o, parameters: {p: $state.b}}"), "[{output: a, value: $x}]", "reads $state.b, which is no property of its state"),
            (
                "[{name: x, type: boolean, source: {regulation: wet, output: o}}, {name: x, type: boolean, source: {regulation: wet, output: o}}]".to_string(),
                "[{output: a, value: $x}]",
                "names input 'x' twice",
            ),
        ] {
            let projection = format!("{state}        steps:\n          - {{record: wet#1, input: {inputs}, actions: {set}}}");
            let err = LawExecutionService::new()
                .load_law(&law_with_projection("REDUCTIE", &projection))
                .expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
        // A kind is in capitals after a third `#`, a fact name (schema
        // v0.8.18) in lower case.
        for record in ["wet#1#Beschikking", "wet#1#"] {
            let projection = format!(
                "{state}        steps:\n          - {{record: '{record}', actions: [{{output: a, value: $record.v}}]}}"
            );
            let err = LawExecutionService::new()
                .load_law(&law_with_projection("REDUCTIE", &projection))
                .expect_err(record);
            assert!(
                err.to_string().contains(&format!(
                    "'{record}' is not <regulation>#<article>, optionally with #<KIND> or #<fact>"
                )),
                "{record}: {err}"
            );
        }
        // A fact name in a file on an earlier version.
        let projection = format!(
            "{state}        steps:\n          - {{record: 'wet#1#beschikking', actions: [{{output: a, value: $record.v}}]}}"
        );
        let err = LawExecutionService::new()
            .load_law(&law_with_projection("REDUCTIE", &projection))
            .expect_err("a fact name before v0.8.18");
        assert!(
            err.to_string()
                .contains("names the fact 'beschikking'; a fact name came with schema v0.8.18"),
            "{err}"
        );
    }

    /// A projection plays one aggregate per record (`per`, schema v0.8.17)
    /// or the records of one key, not both, and answers no question of its
    /// own.
    #[test]
    fn a_projection_per_record_answers_no_question() {
        let state = "        state:\n          - {name: a, type: boolean}\n        steps:\n          - {record: wet#1, actions: [{output: a, value: $record.v}]}";
        let per = law_with_projection(
            "REDUCTIE",
            &format!("        per: wet#1#BESCHIKKING\n{state}"),
        );
        let without_execution = |law: String| {
            let i = law.find("      execution:\n").unwrap();
            law[..i].to_string()
        };
        LawExecutionService::new()
            .load_law(&without_execution(per.clone()))
            .unwrap();
        for (law, why) in [
            (
                per.clone(),
                "has a projection per wet#1#BESCHIKKING and an execution; it answers no question of its own",
            ),
            (
                without_execution(per.replace("        per:", "        key: x\n        per:")),
                "has a projection with a key and per wet#1#BESCHIKKING",
            ),
            (
                without_execution(
                    per.replace("{name: a, type: boolean}", "{name: id, type: string}")
                        .replace("value: $record.v}", "value: $record.w}")
                        .replace("output: a,", "output: id,"),
                ),
                "has a projection per wet#1#BESCHIKKING with a state property 'id'; a row of its aggregates holds the id of its root under that name",
            ),
            (
                without_execution(
                    per.replace("per: wet#1#BESCHIKKING", "per: 'wet#1#Beschikking'"),
                ),
                "'wet#1#Beschikking' is not <regulation>#<article>, optionally with #<KIND> or #<fact>",
            ),
        ] {
            let err = LawExecutionService::new().load_law(&law).expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
    }

    /// Since schema v0.8.17 a step passes no parameter of the question and a
    /// projection plays one aggregate per record (`per`), not the records of
    /// one `key`. A file under v0.8.16 or earlier keeps both; a file that names no
    /// version is held to the newest.
    #[test]
    fn since_v0_8_17_a_projection_has_per_and_its_steps_ask_no_question() {
        const V0_8_14: &str = "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.14/schema/v0.8.14/schema.json\n";
        const V0_8_17: &str = "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-v0.8.17/schema/v0.8.17/schema.json\n";
        let projection = "        key: peildatum\n        state:\n          - {name: a, type: boolean}\n        steps:\n          - {record: wet#1, input: [{name: x, type: boolean, source: {regulation: wet, output: o, parameters: {p: $peildatum}}}], actions: [{output: a, value: $x}]}";
        let keyed = law_with_projection("REDUCTIE", projection).replace(
            "      execution:\n",
            "      execution:\n        parameters:\n          - {name: peildatum, type: date}\n",
        );
        LawExecutionService::new().load_law(&keyed).unwrap();
        let without_execution = |law: &str| {
            let i = law.find("      execution:\n").unwrap();
            law[..i].to_string()
        };
        let per =
            without_execution(&keyed.replace("        key: peildatum\n", "        per: wet#1\n"));
        for (law, why) in [
            (
                keyed.replace(V0_8_14, V0_8_17),
                "plays the records of one 'peildatum' (key); since schema v0.8.17 a projection plays one aggregate per record (per)",
            ),
            (
                keyed.replace(V0_8_14, ""),
                "plays the records of one 'peildatum' (key)",
            ),
            (
                keyed
                    .replace(V0_8_14, V0_8_17)
                    .replace("        key: peildatum\n", ""),
                "has a projection without per; since schema v0.8.17 a projection plays one aggregate per record",
            ),
            (
                per.replace(V0_8_14, V0_8_17),
                "has input 'x' that passes 'p' as '$peildatum'; a parameter of a step is $record.<field> or $state.<property>, and since schema v0.8.17 no parameter of the question",
            ),
            (
                per.replace(V0_8_14, ""),
                "and since schema v0.8.17 no parameter of the question",
            ),
        ] {
            let err = LawExecutionService::new().load_law(&law).expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
    }

    /// An input that reads the aggregates of a reduction stands alone, in a
    /// REDUCTIE, and picks its roots by parameters of its article.
    #[test]
    fn the_aggregates_are_read_alone_in_a_reduction() {
        let reader = |layer: &str, source: &str| {
            format!(
                r#"
$id: lezer
regulatory_layer: {layer}
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        parameters: [{{name: wie, type: string}}]
        input:
          - {{name: rijen, type: array, source: {source}}}
        output: [{{name: n, type: number}}]
        actions:
          - {{output: n, value: 0}}
"#
            )
        };
        LawExecutionService::new()
            .load_law(&reader(
                "REDUCTIE",
                "{projections: per_besluit#1, where: {aanvrager: $wie}}",
            ))
            .unwrap();
        LawExecutionService::new()
            .load_law(&reader("REDUCTIE", "{projections: per_besluit#1}"))
            .unwrap();
        for (layer, source, why) in [
            (
                "WET",
                "{projections: per_besluit#1}",
                "input 'rijen' reads the aggregates of per_besluit#1; only a REDUCTIE reads a chronicle",
            ),
            (
                "REDUCTIE",
                "{projections: per_besluit#1, regulation: wet, output: o}",
                "input 'rijen' reads the aggregates of per_besluit#1, which no regulation can stand beside",
            ),
            (
                "REDUCTIE",
                "{projections: per_besluit#1, decisions: 'wet#1'}",
                "input 'rijen' reads the aggregates of per_besluit#1, which no decisions can stand beside",
            ),
            (
                "REDUCTIE",
                "{projections: per_besluit#1, where: {aanvrager: $niemand}}",
                "input 'rijen' picks the aggregates of per_besluit#1 where 'aanvrager' is $niemand, which is no $-reference to a parameter of the article",
            ),
            (
                "REDUCTIE",
                "{projections: per_besluit#1, where: {aanvrager: wie}}",
                "where 'aanvrager' is wie, which is no $-reference",
            ),
            (
                "REDUCTIE",
                "{regulation: wet, output: o, where: {aanvrager: $wie}}",
                "input 'rijen' has where without projections",
            ),
        ] {
            let err = LawExecutionService::new()
                .load_law(&reader(layer, source))
                .expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
    }

    /// From schema v0.8.17 a reduction reads the aggregates of decisions,
    /// not the decisions themselves; a reduction on v0.8.16 or earlier still
    /// reads them (`source.decisions`, schema v0.8.2).
    #[test]
    fn since_v0_8_17_a_reduction_reads_no_decisions() {
        let reduction = |version: &str| {
            let schema = if version.is_empty() {
                String::new()
            } else {
                format!(
                    "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{version}/schema/{version}/schema.json\n"
                )
            };
            format!(
                r#"
{schema}$id: lezer
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        input:
          - {{name: rijen, type: array, source: {{decisions: 'wet#1'}}}}
        output: [{{name: n, type: number}}]
        actions:
          - {{output: n, value: 0}}
"#
            )
        };
        LawExecutionService::new()
            .load_law(&reduction("v0.8.16"))
            .unwrap();
        let why = "input 'rijen' reads the decisions taken under wet#1; since schema v0.8.17 a REDUCTIE reads the aggregates of the decisions (source.projections), not the decisions themselves (source.decisions)";
        for version in ["v0.8.17", ""] {
            let err = LawExecutionService::new()
                .load_law(&reduction(version))
                .expect_err(why);
            assert!(err.to_string().contains(why), "{version}: {err}");
        }
    }

    /// The `$schema` line for `version`, or none, which counts as the latest.
    fn schema_line(version: &str) -> String {
        if version.is_empty() {
            String::new()
        } else {
            format!(
                "$schema: https://raw.githubusercontent.com/NederlandseDigitaleDienst/regelrecht/refs/tags/schema-{version}/schema/{version}/schema.json\n"
            )
        }
    }

    /// From schema v0.8.19 a reduction reads the aggregates of recorded
    /// facts, not the facts themselves (`source.facts`).
    #[test]
    fn since_v0_8_19_a_reduction_reads_no_facts() {
        let reduction = |version: &str| {
            let schema = schema_line(version);
            format!(
                r#"
{schema}$id: lezer
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        input:
          - {{name: rijen, type: array, source: {{facts: 'wet#1#feit'}}}}
        output: [{{name: n, type: number}}]
        actions:
          - {{output: n, value: 0}}
"#
            )
        };
        LawExecutionService::new()
            .load_law(&reduction("v0.8.18"))
            .unwrap();
        let why = "input 'rijen' reads the recorded facts wet#1#feit; since schema v0.8.19 a REDUCTIE reads the aggregates of the facts (source.projections), not the facts themselves (source.facts)";
        for version in ["v0.8.19", ""] {
            let err = LawExecutionService::new()
                .load_law(&reduction(version))
                .expect_err(why);
            assert!(err.to_string().contains(why), "{version}: {err}");
        }
    }

    /// From schema v0.8.19 a reduction is an aggregate (a projection) or a
    /// read model, whose every article with an execution reads aggregates
    /// (`source.projections`). An article without an execution is neither
    /// and is no reason to refuse.
    #[test]
    fn since_v0_8_19_a_reduction_is_an_aggregate_or_a_read_model() {
        let reduction = |version: &str, input: &str| {
            let schema = schema_line(version);
            format!(
                r#"
{schema}$id: lezer
regulatory_layer: REDUCTIE
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Alleen tekst.
  - number: '2'
    text: t
    machine_readable:
      execution:
        parameters: [{{name: wie, type: string}}]
        input:
{input}        output: [{{name: n, type: number}}]
        actions:
          - {{output: n, value: 0}}
"#
            )
        };
        let call = "          - {name: x, type: number, source: {regulation: wet, output: x}}\n";
        let read = "          - {name: rijen, type: array, source: {projections: aggregaat#1, where: {wie: $wie}}}\n";
        let load = |law: String| LawExecutionService::new().load_law(&law);
        // Up to v0.8.18 a reduction may only call other rulework.
        load(reduction("v0.8.18", call)).unwrap();
        // A read model may call other rulework beside the aggregates it reads.
        load(reduction("v0.8.19", &format!("{call}{read}"))).unwrap();
        load(reduction("", read)).unwrap();
        let why = "article 2: reads no aggregates (source.projections) and plays none (projection); since schema v0.8.19 a REDUCTIE is an aggregate or a read model over one";
        for version in ["v0.8.19", ""] {
            let err = load(reduction(version, call)).expect_err(why);
            assert!(err.to_string().contains(why), "{version}: {err}");
        }
        // Another layer is not held to it.
        load(
            reduction("v0.8.19", call)
                .replace("REDUCTIE", "UITVOERINGSBELEID")
                .replace(
                    "$id: lezer",
                    "$id: lezer\ncompetent_authority: {name: Iemand}",
                ),
        )
        .unwrap();
    }

    /// An answer reads `$state.<property>` of a property its state declares;
    /// a misspelt property, the state as a whole, or `$state` in an article
    /// without a projection fails to load (schema v0.8.14).
    #[test]
    fn an_answer_reads_only_declared_state() {
        let state = "        state:\n          - {name: a, type: boolean}\n        steps: []";
        let load = |law: String| LawExecutionService::new().load_law(&law);
        // A declared property, also inside a condition.
        load(law_with_projection("REDUCTIE", state).replace(
            "            value: $state.a\n",
            "            value: {operation: AND, conditions: [$state.a, true]}\n",
        ))
        .unwrap();
        for (law, why) in [
            (
                law_with_projection("REDUCTIE", state).replace("$state.a", "$state.b"),
                "reads $state.b, which is no property of its state: a",
            ),
            (
                law_with_projection("REDUCTIE", state).replace(
                    "            value: $state.a\n",
                    "            value: {operation: NOT, value: $state.vergunnig}\n",
                ),
                "reads $state.vergunnig, which is no property of its state: a",
            ),
            (
                law_with_projection("REDUCTIE", state).replace("$state.a", "$state"),
                "reads $state as a whole; name a property of its state: a",
            ),
            (
                law_with_projection("REDUCTIE", state).replace(
                    "      execution:\n",
                    "      execution:\n        input:\n          - {name: i, type: boolean, source: {regulation: elders, output: o, parameters: {p: $state.c}}}\n",
                ),
                "reads $state.c, which is no property of its state: a",
            ),
            (
                law_with_projection("REDUCTIE", state)
                    .replace("      projection:\n", "")
                    .replace(state, ""),
                "reads $state.a, but only a projection gives a state, and it has none",
            ),
            (
                law_with_projection("WET", state)
                    .replace("      projection:\n", "")
                    .replace(state, ""),
                "reads $state.a, but only a projection gives a state, and it has none",
            ),
        ] {
            let err = load(law).expect_err(why);
            assert!(err.to_string().contains(why), "{why}: {err}");
        }
        // Without a projection, an article may read a `state` it declares:
        // a parameter, an input or an output of that name.
        let without = law_with_projection("WET", state)
            .replace("      projection:\n", "")
            .replace(state, "");
        for declares in [
            "      execution:\n        parameters:\n          - {name: state, type: object}\n",
            "      execution:\n        input:\n          - {name: state, type: object, source: {regulation: elders, output: o}}\n",
        ] {
            load(without.replace("      execution:\n", declares)).unwrap();
        }
        load(
            without
                .replace(
                    "        output:\n",
                    "        output:\n          - name: state\n            type: object\n",
                )
                .replace(
                    "        actions:\n",
                    "        actions:\n          - output: state\n            value: {a: true}\n",
                ),
        )
        .unwrap();
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
