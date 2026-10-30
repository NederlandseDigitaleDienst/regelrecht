//! Rule resolver for cross-law lookups
//!
//! Provides indexing and lookup functionality for laws, including:
//! - Law registry by ID with multi-version support
//! - Output index for fast article lookup by output name
//! - Implements index for IoC open term resolution
//! - Version selection based on reference_date
//!
//! # Multi-version Support
//!
//! Laws can have multiple versions with different `valid_from` dates. When looking up
//! a law, you can optionally provide a `reference_date` to select the appropriate version:
//! - Versions where `valid_from <= reference_date` are considered valid
//! - The version with the most recent `valid_from` among valid versions is selected
//! - If no `reference_date` is provided, the most recent version is used
//!
//! # Security
//!
//! The resolver enforces a maximum number of loaded laws (see [`crate::config::MAX_LOADED_LAWS`])
//! to prevent memory exhaustion attacks.

use crate::article::{
    Article, ArticleBasedLaw, HookFilter, HookPoint, LawLoad, OpenTerm, ProcedureDefinition,
};
use crate::config;
use crate::error::{EngineError, Result};
use crate::priority::{self, Candidate};
use crate::types::{RegulatoryLayer, Value};
use chrono::NaiveDate;
use std::collections::{BTreeSet, HashMap, HashSet};

/// Why a law version could not be selected for a reference date.
///
/// Used for honest diagnostics (RFC-019 §3): the engine states the *data fact*,
/// never a legal verdict like "geen grondslag" — eerbiedigende werking, a
/// statische verwijzing, or an alternative grondslag may keep the law alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionReason {
    /// No law with this id is loaded.
    NotFound,
    /// The law exists, but no version is in force yet on the reference date
    /// (every version has `valid_from` after it).
    NotYetInForce,
    /// The most recent applicable version ended before the reference date.
    /// Carries the `valid_to` date that was last in force.
    EndedOn(NaiveDate),
    /// A version's `valid_from` is an internal reference (RFC-001) instead of a
    /// date, so whether it was in force on the reference date cannot be
    /// determined from the file. Carries the reference as written.
    ///
    /// A document state and not a defect: the law says its commencement is
    /// fixed elsewhere, and this engine does not resolve that. It is the known
    /// gap of RFC-026, and someone can close it by harvesting what does fix it.
    UndeterminedStart(String),
    /// A version's `valid_from` is neither a date nor an internal reference.
    ///
    /// Loading refuses this, so reaching it means the value arrived another
    /// way. Kept apart from `UndeterminedStart` because the two are not the
    /// same thing: one is a law that says its start lies elsewhere, the other
    /// is a file that says nothing at all. Answering both with "we cannot
    /// determine it" reads as a known gap and hides the corruption.
    UnreadableStart(String),
}

impl SelectionReason {
    /// The data fact behind the failure, as one clause that reads after a law
    /// name: "…: no version of it is in force yet on this date".
    ///
    /// Deliberately never a legal verdict, for the reason on the enum itself.
    /// [`crate::LawExecutionService`] renders the same facts as errors where a
    /// failed selection aborts the execution; this renders them where the
    /// execution goes on without what could not be selected.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::NotFound => "no law with this id is loaded".to_string(),
            Self::NotYetInForce => "no version of it is in force yet on this date".to_string(),
            Self::EndedOn(valid_to) => format!(
                "its last applicable version ended on {}",
                valid_to.format("%Y-%m-%d")
            ),
            Self::UndeterminedStart(reference) => format!(
                "its valid_from is the internal reference '{reference}', so whether it was in \
                 force on this date cannot be determined from the regulation"
            ),
            Self::UnreadableStart(raw) => format!(
                "its valid_from is '{raw}', which is neither a date nor an internal reference, \
                 so this version cannot be placed in time at all"
            ),
        }
    }
}

/// What kind of declaration an index offered.
///
/// The three share a shape: the indexes are built from the newest version of
/// each law, while execution selects the version in force on the reference
/// date. What the index offers can therefore be a law that has no version to
/// select for this date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationKind {
    /// A hook article that would have fired at a hook point.
    Hook,
    /// A lex specialis that would have replaced or voided an output.
    Override,
    /// A regulation that would have filled an open term.
    Implementation,
}

impl DeclarationKind {
    /// The word this kind is called by in the trace and on the receipt.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hook => "hook",
            Self::Override => "override",
            Self::Implementation => "implementation",
        }
    }
}

/// A hook, override or implementation the indexes offered, that did not apply
/// on the reference date: its law has no version the engine could select for
/// that date, or the version it selected does not carry the indexed article.
/// The index is built from the newest version, so an article inserted later
/// leaves the reference pointing at nothing on an earlier date,
/// which is the calendar at work. A renumbering does the same, and that one is
/// a limitation of the undated index rather than a regulation out of force:
/// the reason then names the article that carries the declaration instead, and
/// [`DeclarationsFromOtherVersion`] records that the indexes read another
/// version.
///
/// Where the regulation is not in force, skipping it is right: it does not
/// apply. But the skip is invisible in the outcome — the general rule computes a value,
/// the hook does not fire, the open term falls through to its default — and
/// then "no lex specialis" reads as a finding while it is an answer about a
/// date. Recorded so an absence with a ground stays distinguishable from an
/// absence nobody asked about, the same reason [`DelegationRefusal`] is
/// recorded rather than dropped.
///
/// Deliberately not a [`DelegationRefusal`]: that one is a defect in the
/// corpus, where someone wrote a regulation that arrogates what the law does
/// not delegate. This one is usually no defect at all but the calendar doing
/// its work, so it must not read as an accusation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeclarationNotInForce {
    /// What the declaration would have done.
    pub kind: DeclarationKind,
    /// The law that declares it.
    pub law_id: String,
    /// The article of that law that carries the declaration.
    pub article: String,
    /// What it would have applied to, in the terms of this execution: the open
    /// term it fills, the output it overrides, the hook point it fires on.
    pub subject: String,
    /// Why it does not apply, as a data fact: why no version could be selected
    /// ([`SelectionReason::describe`]), or which version lacks the article.
    pub reason: String,
}

impl DeclarationNotInForce {
    /// One sentence naming the declaration, what it would have applied to, and
    /// why it did not. Used verbatim in the trace, so the receipt reads as prose.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "Not applied: {} {} article {} would have applied to {}, but {}",
            self.kind.as_str(),
            self.law_id,
            self.article,
            self.subject,
            self.reason,
        )
    }
}

/// The reason for a [`DeclarationNotInForce`] whose law *is* in force, but
/// whose version for the reference date has no article with the indexed number.
///
/// The indexes are built from the newest version, so this is what an article
/// inserted by a later version looks like on an earlier date: the calendar at
/// work. A renumbering looks the same, and that one is a limitation of the
/// undated index rather than a regulation out of force. The reason then names
/// the article of this version that carries an equivalent declaration
/// (`redeclares`), so the skip cannot pass for a rule that does not exist yet.
/// `what` names that declaration ("an override of the same output"). A caller
/// leaves out of `redeclares` the articles the index itself offers, since
/// those were tried on their own and are no renumbering. The check compares
/// the declaration as this execution sees it, so a renumbering of the target
/// article as well goes unnamed; the reason then under-reports, never
/// over-reports.
///
/// Worded as a data fact in the register of [`SelectionReason::describe`],
/// naming the law and its version so a reader can look it up: the subject of
/// the note is another law, so "it" would be ambiguous.
pub(crate) fn missing_article_reason(
    law: &ArticleBasedLaw,
    article_number: &str,
    what: &str,
    redeclares: impl Fn(&Article) -> bool,
) -> String {
    let version = match law.valid_from.as_deref() {
        Some(valid_from) => format!(" (valid_from {valid_from})"),
        None => String::new(),
    };
    let mut reason = format!(
        "the version of {} in force on this date{version} has no article {article_number}",
        law.id
    );
    let redeclared: Vec<&str> = law
        .articles
        .iter()
        .filter(|candidate| redeclares(candidate))
        .map(|candidate| candidate.number.as_str())
        .collect();
    if !redeclared.is_empty() {
        reason.push_str(&format!(
            "; article {} of that version declares {what}",
            redeclared.join(", ")
        ));
    }
    reason
}

/// A regulation that declared it fills an open term, but was refused because
/// its regulatory layer is not the layer the declaring article delegates to.
///
/// A refusal is a defect in the corpus, not a property of the case: someone
/// wrote a regulation that arrogates what the law does not delegate to it. It
/// is therefore recorded rather than dropped, so a citizen contesting the
/// decision can read that a filling was offered and why it did not count.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DelegationRefusal {
    /// The law whose article declares the open term.
    pub declaring_law: String,
    /// The article number that declares the open term.
    pub declaring_article: String,
    /// The open term that was to be filled.
    pub open_term: String,
    /// The regulatory layer that article reserves the term for.
    pub required_layer: String,
    /// The regulation that offered to fill it.
    pub refused_law: String,
    /// The article of that regulation that declared the `implements`.
    pub refused_article: String,
    /// That regulation's own regulatory layer.
    pub refused_layer: String,
}

impl DelegationRefusal {
    /// One sentence naming both regulations, the layer required and the layer
    /// offered. Used verbatim in the trace, so the receipt reads as prose.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "Refused: {} article {} is a {} and may not fill open term '{}', \
             which {} article {} reserves for a {}",
            self.refused_law,
            self.refused_article,
            self.refused_layer,
            self.open_term,
            self.declaring_law,
            self.declaring_article,
            self.required_layer,
        )
    }
}

/// The outcome of an implements-index lookup for one open term.
#[derive(Debug, Default)]
pub struct ImplementationLookup<'a> {
    /// Competent implementations, winner first.
    pub implementations: Vec<(&'a ArticleBasedLaw, &'a Article)>,
    /// Candidates refused by the delegation gate, in index order.
    pub refusals: Vec<DelegationRefusal>,
    /// Candidates whose law has no version in force on the reference date, in
    /// index order. Not competent for this date, and not silent about it.
    pub not_in_force: Vec<DeclarationNotInForce>,
}

/// Why no procedure definition was found (RFC-008).
///
/// Only the first variant means "this article has no lifecycle"; the other two
/// mean the corpus asked for one the engine could not find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcedureMiss {
    /// No procedure is defined for this legal character in any loaded law.
    /// An ordinary fact: the article is executed in one go.
    NoneForCharacter,
    /// The article names a procedure that no loaded law defines.
    NamedNotFound(String),
    /// A default procedure is registered for this legal character, but its
    /// definition is missing from the index — the two disagree.
    DefaultDangling(String),
}

/// A law whose hooks, overrides and procedures were read from a version other
/// than the one in force on the reference date.
///
/// Not a finding about the law, but about the answer: for this execution,
/// "no hook, no override, no procedure" came from the wrong version and cannot
/// be relied on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeclarationsFromOtherVersion {
    /// The law whose declarations were consulted.
    pub law_id: String,
    /// `valid_from` of the version the indexes were built from (the newest).
    pub indexed_version: Option<String>,
    /// `valid_from` of the version actually in force on the reference date.
    pub in_force_version: Option<String>,
}

impl DeclarationsFromOtherVersion {
    /// One sentence for the trace and the receipt.
    #[must_use]
    pub fn message(&self) -> String {
        let named = |v: &Option<String>| v.clone().unwrap_or_else(|| "no valid_from".to_string());
        format!(
            "Hooks, overrides and procedures of {} were read from the version of {}, \
             while the version of {} is in force on this date: absence of any of them \
             is not established here",
            self.law_id,
            named(&self.indexed_version),
            named(&self.in_force_version),
        )
    }
}

/// What a version declares beyond its own articles' execution: the material the
/// hooks, overrides and procedure indexes are built from. Two versions with the
/// same fingerprint index identically, so which one was indexed cannot matter.
fn declaration_fingerprint(law: &ArticleBasedLaw) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(procedures) = &law.procedure {
        for proc_def in procedures {
            parts.push(format!(
                "procedure\0{}\0{}\0{}",
                proc_def.id,
                proc_def.applies_to.legal_character,
                proc_def.default.unwrap_or(false)
            ));
        }
    }
    for article in &law.articles {
        if let Some(hooks) = article.get_hooks() {
            for decl in hooks {
                parts.push(format!(
                    "hook\0{}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}",
                    article.number,
                    decl.hook_point,
                    decl.applies_to.legal_character,
                    decl.applies_to.decision_type,
                    decl.applies_to.stage,
                    decl.applies_to.submission,
                    decl.applies_to.established_by
                ));
            }
        }
        if let Some(overrides) = article.get_overrides() {
            for decl in overrides {
                parts.push(format!(
                    "override\0{}\0{}\0{}\0{}",
                    article.number, decl.law, decl.article, decl.output
                ));
            }
        }
    }
    parts.sort();
    parts
}

/// A reference to a law article, used in implements and overrides indexes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LawArticleRef {
    pub(crate) law_id: String,
    pub(crate) article_number: String,
}

/// A hook index entry linking a hook declaration to the law and article that defined it.
pub struct HookEntry {
    pub(crate) law_id: String,
    pub(crate) article_number: String,
    filter: HookFilter,
}

impl HookEntry {
    /// The law that declares the hook.
    pub fn law_id(&self) -> &str {
        &self.law_id
    }

    /// The article that fires as the hook.
    pub fn article_number(&self) -> &str {
        &self.article_number
    }

    /// What the hook applies to.
    pub fn filter(&self) -> &HookFilter {
        &self.filter
    }
}

/// What a hook is filed under in the hooks index: the legal character of a
/// decision (RFC-007) or the kind of a submission (RFC-045).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum HookTarget {
    Decision(String),
    Submission(String),
}

/// Resolves cross-law references and provides law registry functionality.
///
/// The resolver maintains several indexes for efficient lookups:
/// - **Law registry**: All loaded laws by ID, supporting multiple versions per ID
/// - **Output index**: Maps (law_id, output_name) to article number
/// - **Implements index**: Maps (law_id, article, open_term_id) to implementing regulations
///
/// # Multi-version Support
///
/// Multiple versions of the same law (same `$id`) can be loaded. Each version
/// has a `valid_from` date. When querying, provide a `reference_date` to get
/// the appropriate version:
///
/// ```ignore
/// // Load two versions of the same law
/// resolver.load_from_yaml(law_v1_yaml)?; // valid_from: 2024-01-01
/// resolver.load_from_yaml(law_v2_yaml)?; // valid_from: 2025-01-01
///
/// // Get version for a specific date
/// let law = resolver.get_law_for_date("my_law", Some(date!(2024, 6, 1)));
/// // Returns v1 (valid_from 2024-01-01)
/// ```
///
/// # Example
///
/// ```ignore
/// use regelrecht_engine::RuleResolver;
///
/// let mut resolver = RuleResolver::new();
/// resolver.load_from_yaml(yaml_str)?;
///
/// // Find article by output
/// let article = resolver.get_article_by_output("wet_op_de_zorgtoeslag", "standaardpremie", None);
/// ```
pub struct RuleResolver {
    /// Registry of loaded laws by ID, supporting multiple versions per law ID.
    /// Each law ID maps to a list of versions, sorted by valid_from date (newest first).
    law_versions: HashMap<String, Vec<ArticleBasedLaw>>,
    /// Index: "law_id\0output_name" -> article_number
    /// Note: This index uses the most recent version of each law.
    /// Uses a flat string key (null-separated) to avoid two allocations per lookup.
    output_index: HashMap<String, String>,
    /// IoC index: (law_id, article, open_term_id) -> list of implementing articles
    implements_index: HashMap<(String, String, String), Vec<LawArticleRef>>,
    /// Hook index: (hook_point, legal character or submission kind) -> list of
    /// (law_id, article_number, filter). Enables O(1) lookup of hooks that
    /// should fire for a given lifecycle event or submission.
    hooks_index: HashMap<(HookPoint, HookTarget), Vec<HookEntry>>,
    /// Override index: (target_law, target_article, output) -> list of overriding articles
    /// Enables O(1) lookup of lex specialis overrides for a given output.
    overrides_index: HashMap<(String, String, String), Vec<LawArticleRef>>,
    /// Procedure index: (legal_character, procedure_id) -> (procedure definition, defining_law_id)
    /// Loaded from laws that define `procedure:` (typically the AWB).
    procedure_index: HashMap<(String, String), (ProcedureDefinition, String)>,
    /// Maps legal_character -> default procedure_id for that character.
    procedure_defaults: HashMap<String, String>,
    /// Law ids that have more than one loaded version where the declarations
    /// differ between versions (hooks, overrides, procedures). Kept so
    /// [`Self::declarations_from_another_version`] can answer in O(size of this
    /// set) instead of scanning every loaded law on every execution. Almost
    /// always empty.
    versioned_declarers: HashSet<String>,
}

impl Default for RuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// `1 finding` or `n findings`, for a load error.
fn count_findings(count: usize) -> String {
    format!("{count} finding{}", if count == 1 { "" } else { "s" })
}

/// The findings of a load error, one per line, indented.
fn list_findings(findings: &[crate::typecheck::Finding]) -> String {
    findings
        .iter()
        .map(|finding| format!("  {finding}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `law` reads `law_id`: an input that takes one of its outputs or
/// calls one of its articles by a fixed reference (RFC-045 §6), or an article
/// that implements one of its open terms. These are the places where the
/// type check consults another law (RFC-037, N5, T5).
fn references_law(law: &ArticleBasedLaw, law_id: &str) -> bool {
    law.articles.iter().any(|article| {
        article.get_inputs().iter().any(|input| {
            input.source.as_ref().is_some_and(|source| {
                source.regulation.as_deref() == Some(law_id)
                    || source
                        .article
                        .as_deref()
                        .and_then(|target| target.split_once('#'))
                        .is_some_and(|(regulation, _)| regulation == law_id)
            })
        }) || article
            .get_implements()
            .is_some_and(|implements| implements.iter().any(|i| i.law == law_id))
    })
}

impl RuleResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self {
            law_versions: HashMap::new(),
            output_index: HashMap::new(),
            implements_index: HashMap::new(),
            hooks_index: HashMap::new(),
            overrides_index: HashMap::new(),
            procedure_index: HashMap::new(),
            procedure_defaults: HashMap::new(),
            versioned_declarers: HashSet::new(),
        }
    }

    /// Load a law into the resolver.
    ///
    /// If a law with the same ID and valid_from already exists, it will be replaced.
    /// Otherwise, the new version is added to the version list.
    ///
    /// # Arguments
    /// * `law` - The law to load
    ///
    /// # Returns
    /// `Ok(())` on success, `Err` if the maximum number of laws would be exceeded.
    ///
    /// # Security
    ///
    /// Enforces [`config::MAX_LOADED_LAWS`] to prevent memory exhaustion.
    pub fn load_law(&mut self, law: ArticleBasedLaw) -> Result<()> {
        // RFC-036: an unknown is produced by resolution, never written by a
        // law. Refuse a document that carries the serialized sentinel.
        crate::load_check::reject_unknown_literals(&law)?;
        // RFC-045: a law that produces several things in one article divides
        // its outputs over them without ambiguity.
        crate::load_check::reject_ambiguous_produces(&law)?;
        // RFC-051: a fact an article requires holds parameters it declares.
        crate::load_check::reject_malformed_facts(&law)?;

        // RFC-036/RFC-037: hold the law to its own declarations before it can
        // run, so the editor sees the same findings as `just validate`. The
        // cross-law rule (N5) is checked against the laws already loaded, with
        // this law standing in for its own id.
        let lookup = |id: &str| {
            if id == law.id {
                Some(&law)
            } else {
                self.get_law(id)
            }
        };
        let findings = crate::typecheck::check_law(&law, &lookup);
        if !findings.is_empty() {
            return Err(EngineError::LoadError(format!(
                "law '{}' fails the type check ({}):\n{}",
                law.id,
                count_findings(findings.len()),
                list_findings(&findings)
            )));
        }

        // RFC-037: the loaded set is well-typed whatever the order the laws
        // arrived in. A law already loaded that reads this one (an input with
        // `source.regulation`, or an `implements` of one of its terms) could
        // not be held against it before; it is checked again now, and a
        // finding refuses this law and names the consumer.
        let mut consumer_findings = Vec::new();
        for consumer in self
            .all_law_versions()
            .filter(|consumer| consumer.id != law.id && references_law(consumer, &law.id))
        {
            consumer_findings.extend(crate::typecheck::check_law(consumer, &lookup));
        }
        if !consumer_findings.is_empty() {
            let consumers: BTreeSet<&str> = consumer_findings
                .iter()
                .map(|finding| finding.law_id.as_str())
                .collect();
            return Err(EngineError::LoadError(format!(
                "loading law '{}' makes {} ill-typed ({}):\n{}",
                law.id,
                consumers
                    .iter()
                    .map(|id| format!("'{id}'"))
                    .collect::<Vec<_>>()
                    .join(", "),
                count_findings(consumer_findings.len()),
                list_findings(&consumer_findings)
            )));
        }

        self.insert_law(law)
    }

    /// [`Self::load_law`] without the type check: the run-time boundary checks
    /// of RFC-036 stay in place as defence in depth for what a well-typed set
    /// can no longer reach, and their tests need a set the loader would
    /// refuse.
    #[cfg(test)]
    pub(crate) fn load_law_unchecked(&mut self, yaml: &str) -> Result<String> {
        let law = ArticleBasedLaw::from_yaml_str(yaml)?;
        let law_id = law.id.clone();
        self.insert_law(law)?;
        Ok(law_id)
    }

    /// Put a checked law into the version list and the indexes.
    fn insert_law(&mut self, law: ArticleBasedLaw) -> Result<()> {
        let law_id = law.id.clone();
        let valid_from = law.valid_from.clone();

        // RFC-019: valid_to is static version-selection metadata. An unparseable
        // value (e.g. the format-valid but calendar-invalid '2023-02-30') would
        // silently skip the expiry check in select_in and keep an ended law in
        // force forever - reject it at load time instead. valid_from keeps its
        // lenient behaviour (it may be a '#'-reference per RFC-001).
        if let Some(valid_to) = &law.valid_to {
            parse_date(valid_to).map_err(|_| {
                EngineError::LoadError(format!(
                    "law '{law_id}': valid_to '{valid_to}' is not a valid date (expected YYYY-MM-DD)"
                ))
            })?;
        }

        // Same reasoning for the lower bound, with the one form the schema
        // allows besides a date: a `#`-reference (RFC-001). Anything else is a
        // typo, and a typo here reads in `select_in` as "this version has no
        // start date", which puts it in force on every date there is.
        if let Some(valid_from) = &law.valid_from {
            if !is_internal_reference(valid_from) && parse_date(valid_from).is_err() {
                return Err(EngineError::LoadError(format!(
                    "law '{law_id}': valid_from '{valid_from}' is neither a valid date \
                     (YYYY-MM-DD) nor an internal reference (#...)"
                )));
            }
        }

        Self::check_hook_filters(&law)?;

        // Count total laws across all versions
        let total_laws: usize = self.law_versions.values().map(|v| v.len()).sum();

        // Check if we're replacing an existing version (which doesn't increase count)
        let is_replacement = self
            .law_versions
            .get(&law_id)
            .is_some_and(|versions| versions.iter().any(|v| v.valid_from == valid_from));

        // Enforce law count limit (applies to all new versions, not just new law IDs)
        if !is_replacement && total_laws >= config::MAX_LOADED_LAWS {
            tracing::warn!(
                current = total_laws,
                max = config::MAX_LOADED_LAWS,
                law_id = %law_id,
                "Maximum law count exceeded"
            );
            return Err(EngineError::LoadError(format!(
                "Maximum number of laws exceeded ({} laws)",
                config::MAX_LOADED_LAWS
            )));
        }

        // Get or create the version list for this law ID
        let versions = self.law_versions.entry(law_id.clone()).or_default();

        // Check if we're replacing an existing version (same valid_from)
        let existing_idx = versions.iter().position(|v| v.valid_from == valid_from);
        if let Some(idx) = existing_idx {
            tracing::debug!(law_id = %law_id, valid_from = ?valid_from, "Replacing existing version");
            versions[idx] = law;
        } else {
            tracing::debug!(law_id = %law_id, valid_from = ?valid_from, "Adding new version");
            versions.push(law);
        }

        // Sort versions by valid_from date (newest first)
        // Use sort_by_cached_key to parse dates once instead of per-comparison
        versions.sort_by_cached_key(|v| {
            std::cmp::Reverse(v.valid_from.as_ref().and_then(|s| parse_date(s).ok()))
        });

        // Rebuild indexes using the most recent version
        self.rebuild_indexes_for_law(&law_id);

        let total_laws: usize = self.law_versions.values().map(|v| v.len()).sum();
        tracing::debug!(law_id = %law_id, total = total_laws, "Law loaded");
        Ok(())
    }

    /// Refuse a hook that does not say which legal character or which kind of
    /// submission it applies to, that says both, or that narrows itself with
    /// a field of the other kind (RFC-045).
    ///
    /// The hooks index is keyed by (hook_point, legal_character) or, for a
    /// hook on a submission, by (hook_point, kind), so a hook without either
    /// has no key. Dropping it while loading the rest of the law is
    /// the worst of the three options: the article declares that it fires on an
    /// event, the engine keeps that article, and the hook never fires — a
    /// motiveringsplicht or a bezwaartermijn that quietly does not happen.
    ///
    /// Reading it as "applies to every legal character" is the other tempting
    /// option, and it is guesswork: since schema v0.7.0 `applies_to` requires
    /// `legal_character` (or, since v0.8.0, `submission`), so nothing in the
    /// corpus says what an absent one would mean. The model is more
    /// permissive than the schema, which is exactly why this check exists
    /// here. Like the unknown `delegation_type`
    /// in [`Self::matches_delegation`], it is a safety net a schema-conformant
    /// corpus never trips.
    fn check_hook_filters(law: &ArticleBasedLaw) -> Result<()> {
        for article in &law.articles {
            let Some(hooks) = article.get_hooks() else {
                continue;
            };
            for decl in hooks {
                let f = &decl.applies_to;
                let problem = match (&f.legal_character, &f.submission) {
                    (None, None) => Some(
                        "hook declares neither applies_to.legal_character nor \
                         applies_to.submission, so it can never fire",
                    ),
                    (Some(_), Some(_)) => Some(
                        "hook declares both applies_to.legal_character and applies_to.submission; \
                         a hook applies to a decision or to a submission (RFC-045)",
                    ),
                    (Some(_), None) if f.established_by.is_some() => {
                        Some("applies_to.established_by only narrows a hook on a submission")
                    }
                    (None, Some(_)) if f.stage.is_some() || f.decision_type.is_some() => Some(
                        "a hook on a submission takes no stage or decision_type; \
                         established_by narrows it",
                    ),
                    _ => None,
                };
                if let Some(problem) = problem {
                    return Err(EngineError::LoadError(format!(
                        "law '{}' article {}: {problem}",
                        law.id, article.number
                    )));
                }
            }
        }
        Ok(())
    }

    /// Load a law from YAML string.
    ///
    /// # Arguments
    /// * `yaml` - YAML content of the law
    ///
    /// # Returns
    /// The law ID on success.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - YAML parsing fails
    /// - Maximum number of laws would be exceeded
    pub fn load_from_yaml(&mut self, yaml: &str) -> Result<String> {
        let law = ArticleBasedLaw::from_yaml_str(yaml)?;
        let law_id = law.id.clone();
        self.load_law(law)?;
        Ok(law_id)
    }

    /// Get a law by ID (returns the most recent version).
    ///
    /// This is a convenience method that returns the most recent version.
    /// For version-aware lookups, use [`Self::get_law_for_date`].
    pub fn get_law(&self, law_id: &str) -> Option<&ArticleBasedLaw> {
        self.law_versions
            .get(law_id)
            .and_then(|versions| versions.first())
    }

    /// Get a law by ID for a specific reference date.
    ///
    /// Selects the appropriate version based on the reference date:
    /// - Versions where `valid_from <= reference_date` are considered valid
    /// - The version with the most recent `valid_from` among valid versions is returned
    /// - If `reference_date` is None, returns the most recent version
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    /// * `reference_date` - Optional date to select the appropriate version
    ///
    /// # Returns
    /// Reference to the selected version, or None if no valid version exists.
    pub fn get_law_for_date(
        &self,
        law_id: &str,
        reference_date: Option<NaiveDate>,
    ) -> Option<&ArticleBasedLaw> {
        let versions = self.law_versions.get(law_id)?;

        match reference_date {
            None => versions.first(), // Return most recent
            Some(ref_date) => self.select_version_for_date(versions, ref_date),
        }
    }

    /// Like [`Self::get_law_for_date`], but reports *why* selection failed.
    ///
    /// On failure returns a [`SelectionReason`] so callers can produce an honest
    /// diagnostic (RFC-019 §3) without inventing a legal verdict. A required
    /// `reference_date` is taken: the "no date → latest version" shortcut of
    /// [`Self::get_law_for_date`] does not apply here.
    pub fn get_law_for_date_result(
        &self,
        law_id: &str,
        reference_date: NaiveDate,
    ) -> std::result::Result<&ArticleBasedLaw, SelectionReason> {
        let versions = self
            .law_versions
            .get(law_id)
            .ok_or(SelectionReason::NotFound)?;
        Self::select_in(versions, reference_date)
    }

    /// Version-aware lookup reporting *why* selection failed, tolerating an
    /// absent reference date.
    ///
    /// With `Some(date)` this is [`Self::get_law_for_date_result`]; with `None`
    /// it keeps the "no date → latest version" behaviour of
    /// [`Self::get_law_for_date`], failing only when the law id is unknown.
    ///
    /// NOTE: `None` deliberately means "latest version regardless of validity
    /// window" - the `valid_to` upper bound is NOT applied, so an ended law's
    /// final version is returned. Execution paths always pass `Some(date)`
    /// (malformed calculation dates are rejected up front); only pass `None`
    /// for display/listing-style lookups. Revisit when RFC-020 threads
    /// `as_of` dates through every resolution.
    pub fn get_law_for_date_reported(
        &self,
        law_id: &str,
        reference_date: Option<NaiveDate>,
    ) -> std::result::Result<&ArticleBasedLaw, SelectionReason> {
        match reference_date {
            Some(date) => self.get_law_for_date_result(law_id, date),
            None => self
                .law_versions
                .get(law_id)
                .and_then(|versions| versions.first())
                .ok_or(SelectionReason::NotFound),
        }
    }

    /// Select the appropriate version for a reference date.
    ///
    /// # Selection Logic
    /// 1. Among versions with `valid_from <= reference_date`, take the most recent
    ///    (`valid_from`-less versions match any date). Versions are sorted newest-first.
    /// 2. If that version has a `valid_to` and `reference_date > valid_to`, the law is
    ///    no longer in force → return `None`. Do **not** fall through to an older
    ///    version: a repealed law does not resurrect a prior version (RFC-019 §2).
    fn select_version_for_date<'a>(
        &self,
        versions: &'a [ArticleBasedLaw],
        reference_date: NaiveDate,
    ) -> Option<&'a ArticleBasedLaw> {
        Self::select_in(versions, reference_date).ok()
    }

    /// Shared selection core over a version slice (sorted newest-first), reporting
    /// the reason on failure. See [`Self::select_version_for_date`] for the rule.
    fn select_in(
        versions: &[ArticleBasedLaw],
        reference_date: NaiveDate,
    ) -> std::result::Result<&ArticleBasedLaw, SelectionReason> {
        // Walk newest-first and take the first version that can be the answer.
        //
        // A version whose commencement date the engine cannot read is not a
        // version without a lower bound. `load_law` has already refused a
        // malformed date, so what reaches this point is a `#`-reference
        // (RFC-001): the law says its commencement is fixed elsewhere, and the
        // engine does not resolve that. Treating it as "no start date" put such
        // a version in force on every date there is, including dates before it
        // existed. Reaching one here means no younger version applied and this
        // one cannot be ruled in or out, so the honest answer is that we do not
        // know. Versions sort with the unparseable ones last, so a `#`-reference
        // on some old version never blocks a date a real version covers.
        let mut candidate = None;
        for version in versions {
            match version.valid_from.as_deref() {
                Some(raw) if is_internal_reference(raw) => {
                    return Err(SelectionReason::UndeterminedStart(raw.to_string()));
                }
                Some(raw) => {
                    // Unparseable dates are refused at load; treat a stray one
                    // as unreadable rather than as no lower bound.
                    let Ok(valid_from) = parse_date(raw) else {
                        return Err(SelectionReason::UnreadableStart(raw.to_string()));
                    };
                    if valid_from <= reference_date {
                        candidate = Some(version);
                        break;
                    }
                }
                None => {
                    candidate = Some(version);
                    break;
                }
            }
        }
        let candidate = candidate.ok_or(SelectionReason::NotYetInForce)?;

        // Upper bound (inclusive): in force iff reference_date <= valid_to.
        if let Some(valid_to) = candidate.valid_to.as_ref().and_then(|s| parse_date(s).ok()) {
            if reference_date > valid_to {
                return Err(SelectionReason::EndedOn(valid_to));
            }
        }
        Ok(candidate)
    }

    /// Get an article by law ID and output name.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    /// * `output` - The output name to find
    /// * `reference_date` - Optional date to select the appropriate law version
    ///
    /// # Returns
    /// Reference to the article if found.
    pub fn get_article_by_output(
        &self,
        law_id: &str,
        output: &str,
        reference_date: Option<NaiveDate>,
    ) -> Option<&Article> {
        let law = self.get_law_for_date(law_id, reference_date)?;
        // Try indexed lookup first (O(1)), fall back to linear scan
        let index_key = format!("{}\0{}", law_id, output);
        if let Some(article_number) = self.output_index.get(&index_key) {
            if let Some(article) = law.find_article_by_number(article_number) {
                // Verify the article in this version actually has the output
                if article.has_output(output) {
                    return Some(article);
                }
            }
        }
        // Fallback: linear scan (handles version-specific differences)
        law.find_article_by_output(output)
    }

    /// Whether two scoped laws hold for the same territory: both carry scope
    /// fields, and the same ones with the same values. Two national laws do
    /// not count; for them `matches_scope` already holds.
    fn shares_territory(a: &ArticleBasedLaw, b: &ArticleBasedLaw) -> bool {
        let fields = |law: &ArticleBasedLaw| {
            (
                law.gemeente_code.clone(),
                law.provincie_code.clone(),
                law.waterschap_code.clone(),
            )
        };
        let territory = fields(a);
        territory != (None, None, None) && territory == fields(b)
    }

    /// Find all implementations of an open term, resolved by priority.
    ///
    /// Check if a law's scope fields match the execution scope.
    ///
    /// Scope fields are law-level metadata that limit territorial applicability
    /// (e.g., `gemeente_code`, `waterschap_code`). A law with no scope fields
    /// is considered national and always matches. A law with scope fields only
    /// matches if every scope field has a matching value in the execution scope.
    fn matches_scope(law: &ArticleBasedLaw, scope: &HashMap<String, Value>) -> bool {
        if let Some(ref law_gemeente) = law.gemeente_code {
            let scope_value = scope.get("gemeente_code").and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            match scope_value {
                Some(sg) if sg == law_gemeente => {}
                _ => return false,
            }
        }
        if let Some(ref law_provincie) = law.provincie_code {
            let scope_value = scope.get("provincie_code").and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            match scope_value {
                Some(sp) if sp == law_provincie => {}
                _ => return false,
            }
        }
        if let Some(ref law_waterschap) = law.waterschap_code {
            let scope_value = scope.get("waterschap_code").and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });
            match scope_value {
                Some(sw) if sw == law_waterschap => {}
                _ => return false,
            }
        }
        true
    }

    /// Whether `law` may fill an open term that requires `delegation_type`.
    ///
    /// Delegation is a question of competence, not of precedence: an article
    /// that reserves a term for a ministeriële regeling has not authorised a
    /// beleidsregel to fill it, and a beleidsregel that does so anyway is not a
    /// weaker implementation but no implementation at all. So the gate sits
    /// *before* [`priority::resolve_candidate`], which ranks implementations
    /// that are all competent.
    ///
    /// A mismatching layer is skipped rather than fatal, so that a competent
    /// candidate lower in the ranking can still win and so that "no
    /// implementation found" — which the caller handles via the open term's
    /// `default` or its `required` flag — stays reachable. Skipping is not the
    /// same as staying silent: unlike [`Self::matches_scope`], which rejects a
    /// regulation from another municipality because it simply does not apply to
    /// this case, a wrong layer is a defect in the corpus. Someone wrote a
    /// regulation that arrogates what the law does not delegate. The refusal is
    /// therefore reported back to the caller as a [`DelegationRefusal`], which
    /// records it in the trace and on the receipt.
    ///
    /// A `delegation_type` that names no known regulatory layer is a different
    /// matter and is an error. Comparing an unknown string against every layer
    /// would silently reject every candidate, so the engine would answer "no
    /// implementation" to a question it did not understand. That is the same
    /// reason `compare_law_priority` errors on an unresolvable collision rather
    /// than guessing.
    ///
    /// Since v0.7.0 the schema declares `delegation_type` as an enum of the
    /// thirteen layers, so a typo is caught when the file is written. That does
    /// not make this check dead: the schema binds only v0.7.0 files, older
    /// versions arrive unvalidated, and the engine loads YAML without running
    /// the schema over it. The error stays as a safety net that a
    /// schema-conformant corpus never trips.
    fn matches_delegation(
        law: &ArticleBasedLaw,
        declaring_law_id: &str,
        open_term_id: &str,
        delegation_type: &str,
    ) -> Result<bool> {
        if RegulatoryLayer::from_yaml_str(delegation_type).is_none() {
            return Err(EngineError::ResolutionError(format!(
                "Open term '{open_term_id}' on {declaring_law_id} declares delegation_type \
                 '{delegation_type}', which is not a known regulatory layer"
            )));
        }
        Ok(law.regulatory_layer.as_str() == delegation_type)
    }

    /// Looks up the implements index for regulations that declare they fill
    /// the given open term. Optionally filters by temporal validity.
    ///
    /// Returns the competent candidates sorted by priority (winner first),
    /// together with every candidate refused by the delegation gate and every
    /// candidate that has no version in force on the reference date.
    ///
    /// The open term is passed in by the caller rather than looked up again
    /// here. A second, independent lookup of the declaring law, article and
    /// term would fail open: for an older version of the declaring law the
    /// article may carry another number, or the term may not exist yet, and
    /// every one of those misses would read as "this law demands no particular
    /// layer" — opening the gate for the very case the gate exists for.
    ///
    /// # Arguments
    /// * `law_id` - The law that declares the open term
    /// * `article` - The article number that declares the open term
    /// * `open_term` - The open term as declared by that article
    /// * `reference_date` - Optional date to filter by temporal validity
    pub fn find_implementations(
        &self,
        law_id: &str,
        article: &str,
        open_term: &OpenTerm,
        reference_date: Option<NaiveDate>,
        scope: &HashMap<String, Value>,
    ) -> Result<ImplementationLookup<'_>> {
        let open_term_id = open_term.id.as_str();
        let key = (
            law_id.to_string(),
            article.to_string(),
            open_term_id.to_string(),
        );
        let candidate_entries = match self.implements_index.get(&key) {
            Some(entries) => entries,
            None => return Ok(ImplementationLookup::default()),
        };

        tracing::debug!(
            law_id = %law_id,
            article = %article,
            open_term_id = %open_term_id,
            candidates = candidate_entries.len(),
            "Finding implementations for open term"
        );

        // What layer the declaring article demands of an implementation. Absent
        // means the law demands nothing, and then every layer may fill the term.
        let delegation_type = open_term.delegation_type.as_deref();

        // Resolve each candidate to actual (law, article) references
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut lookup = ImplementationLookup::default();
        // The declaring law, to recognise candidates of its own territory.
        let declarer = self.get_law_for_date(law_id, reference_date);

        for entry in candidate_entries {
            // Version filter. A regulation that is not in force on this date
            // cannot fill the term, but its disappearance would be silent: the
            // caller sees zero candidates and reports "no implementation, using
            // the default" — the same sentence it prints when nobody ever wrote
            // one. The two filters below say why they drop a candidate, and
            // this one now does too, through the same lookup so the record
            // reaches trace and receipt rather than the log alone.
            let law = match self.get_law_for_date_reported(&entry.law_id, reference_date) {
                Ok(law) => law,
                Err(reason) => {
                    tracing::debug!(
                        candidate = %entry.law_id,
                        reason = %reason.describe(),
                        "Skipping: no version of this candidate is in force on the reference date"
                    );
                    lookup.not_in_force.push(DeclarationNotInForce {
                        kind: DeclarationKind::Implementation,
                        law_id: entry.law_id.clone(),
                        article: entry.article_number.clone(),
                        subject: format!(
                            "open term '{open_term_id}' of {law_id} article {article}"
                        ),
                        reason: reason.describe(),
                    });
                    continue;
                }
            };

            // Scope filtering: check all scope fields on the candidate law against
            // the execution parameters. A scoped regulation (e.g., with gemeente_code
            // or provincie_code) only matches when the execution scope contains the
            // same value. Unscoped regulations (national) always match. The
            // regulation that declares the term matches too: the scope picks
            // which regulation applies to a case, and that one already does.
            // So does a regulation scoped to the same territory as the one that
            // declares the term, such as the college's nadere regels under a
            // verordening of the same municipality.
            let of_the_declarers_territory =
                declarer.is_some_and(|declarer| Self::shares_territory(declarer, law));
            if law.id != law_id && !of_the_declarers_territory && !Self::matches_scope(law, scope) {
                tracing::debug!(
                    candidate = %entry.law_id,
                    "Skipping: scope fields do not match execution parameters"
                );
                continue;
            }

            // Delegation gate: a layer the declaring article did not authorise
            // is not a weaker candidate but no candidate at all.
            if let Some(expected) = delegation_type {
                if !Self::matches_delegation(law, law_id, open_term_id, expected)? {
                    tracing::debug!(
                        candidate = %entry.law_id,
                        candidate_layer = %law.regulatory_layer.as_str(),
                        delegation_type = %expected,
                        "Skipping: regulatory_layer is not the layer the open term delegates to"
                    );
                    lookup.refusals.push(DelegationRefusal {
                        declaring_law: law_id.to_string(),
                        declaring_article: article.to_string(),
                        open_term: open_term_id.to_string(),
                        required_layer: expected.to_string(),
                        refused_law: law.id.clone(),
                        refused_article: entry.article_number.clone(),
                        refused_layer: law.regulatory_layer.as_str().to_string(),
                    });
                    continue;
                }
            }

            let Some(art) = law
                .articles
                .iter()
                .find(|a| a.number == entry.article_number)
            else {
                // The index is built from the newest version of each law, but
                // the candidate was resolved for the reference date. An article
                // number that the newest version has and this one does not is a
                // version mismatch, not an absent implementation — say so
                // rather than letting the candidate disappear without a word.
                // The log alone did not: the caller still reported "no
                // implementation, using the default", so the record goes the
                // same way as the not-in-force one above, to trace and receipt.
                lookup.not_in_force.push(DeclarationNotInForce {
                    kind: DeclarationKind::Implementation,
                    law_id: entry.law_id.clone(),
                    article: entry.article_number.clone(),
                    subject: format!("open term '{open_term_id}' of {law_id} article {article}"),
                    reason: missing_article_reason(
                        law,
                        &entry.article_number,
                        "an implementation of the same open term",
                        |candidate| {
                            let offered = candidate_entries.iter().any(|e| {
                                e.law_id == entry.law_id && e.article_number == candidate.number
                            });
                            !offered
                                && candidate.get_implements().is_some_and(|decls| {
                                    decls.iter().any(|d| {
                                        d.law == law_id
                                            && d.article == article
                                            && d.open_term == open_term_id
                                    })
                                })
                        },
                    ),
                });
                continue;
            };

            candidates.push(Candidate {
                law,
                article_number: entry.article_number.clone(),
            });
            lookup.implementations.push((law, art));
        }

        if candidates.is_empty() {
            return Ok(lookup);
        }

        // Use priority resolution to sort — return winner first
        if let Some((winner_law, reason)) = priority::resolve_candidate(&candidates)? {
            tracing::debug!(
                winner = %winner_law.id,
                reason = %reason,
                "Open term implementation resolved"
            );

            // Put winner first, then the rest
            let winner_idx = lookup
                .implementations
                .iter()
                .position(|(law, _)| law.id == winner_law.id);
            if let Some(idx) = winner_idx {
                if idx != 0 {
                    lookup.implementations.swap(0, idx);
                }
            }
        }

        Ok(lookup)
    }

    /// Get the number of entries in the implements index.
    #[cfg(test)]
    pub fn implements_count(&self) -> usize {
        self.implements_index.values().map(|v| v.len()).sum()
    }

    /// Get the number of entries in the output index.
    ///
    /// This counts the total number of (law_id, output_name) pairs across all laws.
    pub fn output_count(&self) -> usize {
        self.output_index.len()
    }

    /// List all (law_id, output_name) pairs from the output index.
    pub fn list_all_outputs(&self) -> Vec<(&str, &str)> {
        let mut outputs: Vec<(&str, &str)> = self
            .output_index
            .keys()
            .filter_map(|key| key.split_once('\0'))
            .collect();
        outputs.sort();
        outputs
    }

    /// Load all YAML law files from a directory (recursively).
    ///
    /// Scans the given directory for `.yaml` files and loads each one.
    /// Files that fail to parse are logged as warnings and skipped.
    ///
    /// # Arguments
    /// * `dir` - Path to the directory to scan
    ///
    /// # Returns
    /// Number of successfully loaded law files.
    ///
    /// # Errors
    /// Returns error if the directory cannot be read.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_directory(&mut self, dir: &std::path::Path) -> Result<usize> {
        let mut count = 0;
        self.load_from_directory_recursive(dir, &mut count)?;
        Ok(count)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_directory_recursive(
        &mut self,
        dir: &std::path::Path,
        count: &mut usize,
    ) -> Result<()> {
        use std::fs;

        let entries = fs::read_dir(dir).map_err(|e| {
            EngineError::LoadError(format!(
                "Failed to read directory '{}': {}",
                dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                EngineError::LoadError(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.is_dir() {
                self.load_from_directory_recursive(&path, count)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                match ArticleBasedLaw::from_yaml_file(&path) {
                    Ok(law) => match self.load_law(law) {
                        Ok(()) => {
                            *count += 1;
                        }
                        Err(e) => {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "Failed to register law from file"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to parse YAML law file"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// List all loaded law IDs (unique, not including versions).
    pub fn list_laws(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.law_versions.keys().map(|s| s.as_str()).collect();
        ids.sort();
        ids
    }

    /// Get the number of unique law IDs (not counting versions).
    pub fn law_count(&self) -> usize {
        self.law_versions.len()
    }

    /// Get the total number of loaded law versions.
    pub fn version_count(&self) -> usize {
        self.law_versions.values().map(|v| v.len()).sum()
    }

    /// Get the number of versions for a specific law.
    pub fn version_count_for_law(&self, law_id: &str) -> usize {
        self.law_versions.get(law_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Check if a law is loaded (any version).
    pub fn has_law(&self, law_id: &str) -> bool {
        self.law_versions.contains_key(law_id)
    }

    /// Iterate over all loaded law versions (all IDs, all versions).
    pub fn all_law_versions(&self) -> impl Iterator<Item = &ArticleBasedLaw> {
        self.law_versions.values().flat_map(|v| v.iter())
    }

    /// Unload all versions of a law from the resolver.
    ///
    /// Removes all versions of the law and all its indexes.
    ///
    /// # Returns
    /// `true` if the law was removed, `false` if it didn't exist.
    pub fn unload_law(&mut self, law_id: &str) -> bool {
        if self.law_versions.remove(law_id).is_some() {
            self.remove_indexes_for_law(law_id);
            true
        } else {
            false
        }
    }

    /// Unload a specific version of a law.
    ///
    /// # Arguments
    /// * `law_id` - The law identifier
    /// * `valid_from` - The valid_from date of the version to remove
    ///
    /// # Returns
    /// `true` if the version was removed, `false` if it didn't exist.
    pub fn unload_law_version(&mut self, law_id: &str, valid_from: Option<&str>) -> bool {
        let Some(versions) = self.law_versions.get_mut(law_id) else {
            return false;
        };

        let original_len = versions.len();
        versions.retain(|v| v.valid_from.as_deref() != valid_from);

        if versions.len() < original_len {
            if versions.is_empty() {
                self.law_versions.remove(law_id);
                self.remove_indexes_for_law(law_id);
            } else {
                // Rebuild indexes with the new most recent version
                self.rebuild_indexes_for_law(law_id);
            }
            true
        } else {
            false
        }
    }

    /// Rebuild output, implements, hook, override, and procedure indexes for a specific law.
    fn rebuild_indexes_for_law(&mut self, law_id: &str) {
        // Record whether the newest version speaks for the older ones here.
        self.note_versioned_declarations(law_id);

        // Remove old output index entries
        self.output_index
            .retain(|key, _| key.split_once('\0').is_none_or(|(id, _)| id != law_id));

        // Remove old implements index entries where this law is an implementor
        for candidates in self.implements_index.values_mut() {
            candidates.retain(|r| r.law_id != law_id);
        }
        self.implements_index.retain(|_, v| !v.is_empty());

        // Remove old hook index entries for this law
        for entries in self.hooks_index.values_mut() {
            entries.retain(|entry| entry.law_id != law_id);
        }
        self.hooks_index.retain(|_, v| !v.is_empty());

        // Remove old override index entries for this law
        for entries in self.overrides_index.values_mut() {
            entries.retain(|r| r.law_id != law_id);
        }
        self.overrides_index.retain(|_, v| !v.is_empty());

        // Remove old procedure index entries defined by this law
        self.procedure_index
            .retain(|_, (_, defining_law)| defining_law != law_id);
        // Clean up defaults whose backing procedure no longer exists
        let proc_index = &self.procedure_index;
        self.procedure_defaults
            .retain(|lc, proc_id| proc_index.contains_key(&(lc.clone(), proc_id.clone())));

        // Add new index entries from the most recent version
        // Access law_versions directly to avoid borrowing self through get_law()
        if let Some(versions) = self.law_versions.get(law_id) {
            if let Some(law) = versions.first() {
                // Procedure index (top-level)
                if let Some(procedures) = &law.procedure {
                    for proc_def in procedures {
                        let key = (
                            proc_def.applies_to.legal_character.clone(),
                            proc_def.id.clone(),
                        );
                        self.procedure_index
                            .insert(key, (proc_def.clone(), law_id.to_string()));

                        if proc_def.default.unwrap_or(false) {
                            self.procedure_defaults.insert(
                                proc_def.applies_to.legal_character.clone(),
                                proc_def.id.clone(),
                            );
                        }
                    }
                }

                for article in &law.articles {
                    // Output index
                    if let Some(exec) = article.get_execution_spec() {
                        if let Some(outputs) = &exec.output {
                            for output in outputs {
                                self.output_index.insert(
                                    format!("{}\0{}", law_id, output.name),
                                    article.number.clone(),
                                );
                            }
                        }
                    }

                    // Implements index (IoC). A reduction fills no term: its
                    // cell gives it as a named answer (RFC-048), and the
                    // loader refuses `implements` on one
                    // (`reject_removed_constructs`).
                    let reduction = law.regulatory_layer == RegulatoryLayer::Reductie;
                    if let Some(impl_decls) = article.get_implements().filter(|_| !reduction) {
                        for decl in impl_decls {
                            let key = (
                                decl.law.clone(),
                                decl.article.clone(),
                                decl.open_term.clone(),
                            );
                            let entry = LawArticleRef {
                                law_id: law_id.to_string(),
                                article_number: article.number.clone(),
                            };
                            let candidates = self.implements_index.entry(key).or_default();
                            if !candidates.contains(&entry) {
                                candidates.push(entry);
                            }
                        }
                    }

                    // Hooks index. A hook with neither a legal_character nor
                    // a submission never gets here: `load_law` refuses the
                    // law, because there is no honest key to file it under
                    // (see `check_hook_filters`).
                    if let Some(hook_decls) = article.get_hooks() {
                        for decl in hook_decls {
                            let f = &decl.applies_to;
                            let target = match (&f.legal_character, &f.submission) {
                                (Some(lc), _) => HookTarget::Decision(lc.clone()),
                                (None, Some(kind)) => HookTarget::Submission(kind.clone()),
                                (None, None) => continue,
                            };
                            let entry = HookEntry {
                                law_id: law_id.to_string(),
                                article_number: article.number.clone(),
                                filter: f.clone(),
                            };
                            self.hooks_index
                                .entry((decl.hook_point, target))
                                .or_default()
                                .push(entry);
                        }
                    }

                    // Overrides index
                    if let Some(ovr_decls) = article.get_overrides() {
                        for decl in ovr_decls {
                            let key = (decl.law.clone(), decl.article.clone(), decl.output.clone());
                            let entry = LawArticleRef {
                                law_id: law_id.to_string(),
                                article_number: article.number.clone(),
                            };
                            let candidates = self.overrides_index.entry(key).or_default();
                            if !candidates.contains(&entry) {
                                candidates.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Remove all indexes for a law.
    fn remove_indexes_for_law(&mut self, law_id: &str) {
        self.versioned_declarers.remove(law_id);

        // Remove output index entries
        self.output_index
            .retain(|key, _| key.split_once('\0').is_none_or(|(id, _)| id != law_id));

        // Remove from implements index (this law as implementor)
        for candidates in self.implements_index.values_mut() {
            candidates.retain(|r| r.law_id != law_id);
        }
        self.implements_index.retain(|_, v| !v.is_empty());

        // Remove hook index entries for this law
        for entries in self.hooks_index.values_mut() {
            entries.retain(|entry| entry.law_id != law_id);
        }
        self.hooks_index.retain(|_, v| !v.is_empty());

        // Remove override index entries for this law
        for entries in self.overrides_index.values_mut() {
            entries.retain(|r| r.law_id != law_id);
        }
        self.overrides_index.retain(|_, v| !v.is_empty());

        // Remove procedure index entries defined by this law
        self.procedure_index
            .retain(|_, (_, defining_law)| defining_law != law_id);
        let proc_index = &self.procedure_index;
        self.procedure_defaults
            .retain(|lc, proc_id| proc_index.contains_key(&(lc.clone(), proc_id.clone())));
    }

    /// Record whether this law's declarations vary across its versions.
    ///
    /// The hooks, overrides and procedure indexes are built from the newest
    /// version only. That is invisible to [`Self::find_hooks`],
    /// [`Self::find_overrides`] and [`Self::find_procedure`], which take no
    /// reference date and cannot take one without threading a date through
    /// every index (see `declarations_from_another_version`). Until then, at
    /// least know when it matters: only a law whose declarations actually
    /// differ between versions can give a wrong answer for an older date.
    fn note_versioned_declarations(&mut self, law_id: &str) {
        let Some(versions) = self.law_versions.get(law_id) else {
            self.versioned_declarers.remove(law_id);
            return;
        };
        let Some(indexed) = versions.first() else {
            self.versioned_declarers.remove(law_id);
            return;
        };
        let varies = versions
            .iter()
            .skip(1)
            .any(|other| declaration_fingerprint(other) != declaration_fingerprint(indexed));
        if varies {
            tracing::warn!(
                law_id = %law_id,
                indexed_version = ?indexed.valid_from,
                "Law declares different hooks/overrides/procedures per version; \
                 the indexes only carry the newest version"
            );
            self.versioned_declarers.insert(law_id.to_string());
        } else {
            self.versioned_declarers.remove(law_id);
        }
    }

    /// Laws whose hooks, overrides and procedures were answered from a version
    /// other than the one in force on `reference_date`.
    ///
    /// A caller that gets "no hook, no override, no procedure" for such a law
    /// has not been told that none exist; it has been told what the newest
    /// version declares. That difference decides whether a motiveringsplicht or
    /// a bezwaartermijn fires, so it must not read as a finding. Callers put
    /// this on the trace and on the receipt.
    pub fn declarations_from_another_version(
        &self,
        reference_date: Option<NaiveDate>,
    ) -> Vec<DeclarationsFromOtherVersion> {
        let mut notes = Vec::new();
        for law_id in &self.versioned_declarers {
            let Some(versions) = self.law_versions.get(law_id) else {
                continue;
            };
            let Some(indexed) = versions.first() else {
                continue;
            };
            let in_force = match reference_date {
                None => Some(indexed),
                Some(date) => Self::select_in(versions, date).ok(),
            };
            let Some(in_force) = in_force else {
                continue;
            };
            if !std::ptr::eq(indexed, in_force) {
                notes.push(DeclarationsFromOtherVersion {
                    law_id: law_id.clone(),
                    indexed_version: indexed.valid_from.clone(),
                    in_force_version: in_force.valid_from.clone(),
                });
            }
        }
        notes.sort_by(|a, b| a.law_id.cmp(&b.law_id));
        notes
    }

    /// Find hooks that match a given lifecycle event.
    ///
    /// Returns matching (law_id, article_number, filter) entries.
    /// Filters by stage: if the hook has a stage, it must match; if not, it defaults to "BESLUIT".
    pub(crate) fn find_hooks(
        &self,
        hook_point: HookPoint,
        legal_character: &str,
        decision_type: Option<&str>,
        stage: &str,
    ) -> Vec<&HookEntry> {
        let key = (
            hook_point,
            HookTarget::Decision(legal_character.to_string()),
        );
        let Some(entries) = self.hooks_index.get(&key) else {
            return Vec::new();
        };

        entries
            .iter()
            .filter(|entry| hook_filter_admits(&entry.filter, decision_type, stage))
            .collect()
    }

    /// Find the hooks at `hook_point` on a submission of `kind` that article
    /// `article_number` of `law_id` establishes (RFC-045), such as Awb 4:2 on
    /// every `AANVRAAG`. A hook with `established_by` only fires on the
    /// submission of that one article. Public so that a runtime can ask which
    /// articles take part in a submission by the same rule the engine fires
    /// them. Each hook article comes back once.
    pub fn find_submission_hooks(
        &self,
        hook_point: HookPoint,
        kind: &str,
        law_id: &str,
        article_number: &str,
    ) -> Vec<&HookEntry> {
        let key = (hook_point, HookTarget::Submission(kind.to_string()));
        let Some(entries) = self.hooks_index.get(&key) else {
            return Vec::new();
        };
        let mut hooks: Vec<&HookEntry> = Vec::new();
        for e in entries
            .iter()
            .filter(|e| submission_filter_admits(&e.filter, law_id, article_number))
        {
            // An article with two matching hooks at this point (one on every
            // application, one on this article's) fires once.
            if !hooks
                .iter()
                .any(|h| h.law_id == e.law_id && h.article_number == e.article_number)
            {
                hooks.push(e);
            }
        }
        hooks
    }

    /// Find overrides for a specific article output.
    ///
    /// Returns matching (overriding_law_id, overriding_article_number) entries.
    pub(crate) fn find_overrides(
        &self,
        target_law: &str,
        target_article: &str,
        output: &str,
    ) -> &[LawArticleRef] {
        let key = (
            target_law.to_string(),
            target_article.to_string(),
            output.to_string(),
        );
        self.overrides_index
            .get(&key)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find a procedure definition for a given legal character and optional procedure ID.
    ///
    /// If procedure_id is None, returns the default procedure for the legal character.
    ///
    /// Prefer [`Self::find_procedure_reported`] on execution paths: this method
    /// answers `None` to two very different questions, and a caller that cannot
    /// tell them apart executes an article without its procedure.
    pub fn find_procedure(
        &self,
        legal_character: &str,
        procedure_id: Option<&str>,
    ) -> Option<&ProcedureDefinition> {
        self.find_procedure_reported(legal_character, procedure_id)
            .ok()
    }

    /// Like [`Self::find_procedure`], but says *why* no procedure was found.
    ///
    /// The distinction is the whole point. "This legal character has no
    /// procedure anywhere in the corpus" is an ordinary fact: a law executed
    /// without the AWB loaded produces a BESCHIKKING and simply has no
    /// lifecycle, so the caller runs the article once and is right to do so.
    /// "This article asks for procedure X and no loaded law defines X" is a
    /// defect, and answering it with the same `None` drops the stages the
    /// procedure would have imposed — the hearing, the notification, the
    /// objection period. Those are the rights of the person the decision is
    /// about, and they must not fall away because a lookup missed.
    pub fn find_procedure_reported(
        &self,
        legal_character: &str,
        procedure_id: Option<&str>,
    ) -> std::result::Result<&ProcedureDefinition, ProcedureMiss> {
        let (proc_id, named) = match procedure_id {
            Some(id) => (id.to_string(), true),
            None => match self.procedure_defaults.get(legal_character) {
                Some(default_id) => (default_id.clone(), false),
                None => return Err(ProcedureMiss::NoneForCharacter),
            },
        };
        let key = (legal_character.to_string(), proc_id.clone());
        match self.procedure_index.get(&key) {
            Some((def, _)) => Ok(def),
            None if named => Err(ProcedureMiss::NamedNotFound(proc_id)),
            // The default was registered from a procedure definition, so its
            // absence here means the two indexes disagree.
            None => Err(ProcedureMiss::DefaultDangling(proc_id)),
        }
    }

    /// Validate that all override targets exist in loaded laws.
    ///
    /// Returns a list of validation errors for overrides that reference
    /// non-existent laws, articles, or outputs.
    pub fn validate_override_targets(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for ((target_law, target_article, target_output), overriders) in &self.overrides_index {
            // Check target law exists
            let Some(law) = self.get_law(target_law) else {
                for ovr in overriders {
                    errors.push(format!(
                        "Override {}:{} targets non-existent law '{target_law}'",
                        ovr.law_id, ovr.article_number
                    ));
                }
                continue;
            };

            // Check target article exists
            let Some(article) = law.find_article_by_number(target_article) else {
                for ovr in overriders {
                    errors.push(format!(
                        "Override {}:{} targets non-existent article '{target_article}' in '{target_law}'",
                        ovr.law_id, ovr.article_number
                    ));
                }
                continue;
            };

            // Check target output exists
            if !article.has_output(target_output) {
                for ovr in overriders {
                    errors.push(format!(
                        "Override {}:{} targets non-existent output '{target_output}' on '{target_law}:{target_article}'",
                        ovr.law_id, ovr.article_number
                    ));
                }
            }
        }
        errors
    }
}

/// Whether a hook filter admits a decision at this stage, apart from its legal
/// character (which the hooks index is keyed on).
///
/// An absent stage means BESLUIT (backward compatibility per RFC-008); an
/// absent decision type admits every decision type.
pub(crate) fn hook_filter_admits(
    filter: &HookFilter,
    decision_type: Option<&str>,
    stage: &str,
) -> bool {
    if filter.stage.as_deref().unwrap_or("BESLUIT") != stage {
        return false;
    }
    match filter.decision_type.as_deref() {
        Some(filter_dt) => decision_type == Some(filter_dt),
        None => true,
    }
}

/// Whether a hook filter on a submission admits the submission that article
/// `article_number` of `law_id` establishes, apart from its kind (which the
/// hooks index is keyed on): without `established_by` every submission of the
/// kind, with it only the submission of that article (RFC-045).
pub(crate) fn submission_filter_admits(
    filter: &HookFilter,
    law_id: &str,
    article_number: &str,
) -> bool {
    filter.established_by.as_deref().is_none_or(|r| {
        r.split_once('#')
            .is_some_and(|(law, article)| law == law_id && article == article_number)
    })
}

/// Whether a `valid_from` is an internal reference rather than a date.
///
/// RFC-001 allows `valid_from` to point at an article instead of naming a date,
/// for a law whose commencement is fixed elsewhere ("treedt in werking op een
/// bij koninklijk besluit te bepalen tijdstip"). The schema pins the two forms:
/// `^([0-9]{4}-[0-9]{2}-[0-9]{2}|#.+)$`.
fn is_internal_reference(valid_from: &str) -> bool {
    valid_from.starts_with('#') && valid_from.len() > 1
}

/// Parse a date string in ISO 8601 format (YYYY-MM-DD).
fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| EngineError::InvalidOperation(format!("Failed to parse date '{}': {}", s, e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_law() -> &'static str {
        r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: 42
"#
    }

    fn make_test_law_with_valid_from(valid_from: &str, value: i32) -> String {
        format!(
            r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '{valid_from}'
articles:
  - number: '1'
    text: Test article version {valid_from}
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: {value}
"#
        )
    }

    fn make_test_law_with_validity(valid_from: &str, valid_to: &str, value: i32) -> String {
        format!(
            r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '{valid_from}'
valid_to: '{valid_to}'
articles:
  - number: '1'
    text: Test article {valid_from} t/m {valid_to}
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: {value}
"#
        )
    }

    #[test]
    fn test_resolver_basic() {
        let mut resolver = RuleResolver::new();

        let law_id = resolver.load_from_yaml(make_test_law()).unwrap();
        assert_eq!(law_id, "test_law");

        assert!(resolver.has_law("test_law"));
        assert!(!resolver.has_law("nonexistent"));
        assert_eq!(resolver.law_count(), 1);
    }

    #[test]
    fn test_resolver_get_law() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        let law = resolver.get_law("test_law").unwrap();
        assert_eq!(law.id, "test_law");
        assert_eq!(law.articles.len(), 1);
    }

    #[test]
    fn test_resolver_output_index() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        let article = resolver
            .get_article_by_output("test_law", "test_output", None)
            .unwrap();
        assert_eq!(article.number, "1");

        // Non-existent output
        assert!(resolver
            .get_article_by_output("test_law", "nonexistent", None)
            .is_none());

        // Non-existent law
        assert!(resolver
            .get_article_by_output("nonexistent", "test_output", None)
            .is_none());
    }

    #[test]
    fn test_resolver_list_laws() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_test_law()).unwrap();

        let laws = resolver.list_laws();
        assert_eq!(laws.len(), 1);
        assert_eq!(laws, vec!["test_law"]);
    }

    #[test]
    fn test_resolver_unload() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        assert!(resolver.has_law("test_law"));
        assert!(resolver.unload_law("test_law"));
        assert!(!resolver.has_law("test_law"));
        assert!(!resolver.unload_law("test_law")); // Already removed

        // Output index should also be removed
        assert!(resolver
            .get_article_by_output("test_law", "test_output", None)
            .is_none());
    }

    #[test]
    fn test_resolver_replace_law() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();

        // Load a different version of the same law (same valid_from = None)
        let updated_yaml = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '2'
    text: Updated article
    machine_readable:
      execution:
        output:
          - name: new_output
            type: number
        actions:
          - output: new_output
            value: 100
"#;
        resolver.load_from_yaml(updated_yaml).unwrap();

        // Should have the new article (replacement since same valid_from)
        let law = resolver.get_law("test_law").unwrap();
        assert_eq!(law.articles[0].number, "2");

        // Old output should be gone
        assert!(resolver
            .get_article_by_output("test_law", "test_output", None)
            .is_none());

        // New output should exist
        assert!(resolver
            .get_article_by_output("test_law", "new_output", None)
            .is_some());
    }

    #[test]
    fn test_resolver_load_law_returns_result() {
        // Test that load_law now returns a Result
        let mut resolver = RuleResolver::new();
        let law = ArticleBasedLaw::from_yaml_str(make_test_law()).unwrap();

        // First load should succeed
        assert!(resolver.load_law(law.clone()).is_ok());
        assert_eq!(resolver.law_count(), 1);

        // Replacement should also succeed
        assert!(resolver.load_law(law).is_ok());
        assert_eq!(resolver.law_count(), 1); // Still 1 - replacement
    }

    // -------------------------------------------------------------------------
    // Multi-version Support Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolver_multi_version_basic() {
        let mut resolver = RuleResolver::new();

        // Load two versions of the same law
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();

        // Should have 1 law with 2 versions
        assert_eq!(resolver.law_count(), 1);
        assert_eq!(resolver.version_count(), 2);
        assert_eq!(resolver.version_count_for_law("test_law"), 2);

        // get_law returns the most recent version
        let law = resolver.get_law("test_law").unwrap();
        assert_eq!(law.valid_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_resolver_get_law_for_date() {
        let mut resolver = RuleResolver::new();

        // Load three versions
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2023-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-06-01", 200))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 300))
            .unwrap();

        // Query for different dates
        let date_2023 = NaiveDate::from_ymd_opt(2023, 6, 1).unwrap();
        let date_2024 = NaiveDate::from_ymd_opt(2024, 12, 1).unwrap();
        let date_2025 = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();

        // 2023-06-01: Should get 2023-01-01 version
        let law = resolver
            .get_law_for_date("test_law", Some(date_2023))
            .unwrap();
        assert_eq!(law.valid_from, Some("2023-01-01".to_string()));

        // 2024-12-01: Should get 2024-06-01 version (most recent valid)
        let law = resolver
            .get_law_for_date("test_law", Some(date_2024))
            .unwrap();
        assert_eq!(law.valid_from, Some("2024-06-01".to_string()));

        // 2025-06-01: Should get 2025-01-01 version
        let law = resolver
            .get_law_for_date("test_law", Some(date_2025))
            .unwrap();
        assert_eq!(law.valid_from, Some("2025-01-01".to_string()));

        // None: Should get most recent version
        let law = resolver.get_law_for_date("test_law", None).unwrap();
        assert_eq!(law.valid_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_resolver_get_law_for_date_no_valid_version() {
        let mut resolver = RuleResolver::new();

        // Load a version valid from 2025
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 100))
            .unwrap();

        // Query for a date before any version is valid
        let date_2024 = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let law = resolver.get_law_for_date("test_law", Some(date_2024));
        assert!(law.is_none());
    }

    #[test]
    fn test_resolver_valid_to_upper_bound() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_validity(
                "2023-01-01",
                "2023-12-31",
                100,
            ))
            .unwrap();

        // In force before and on the (inclusive) end date.
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2023, 6, 1).unwrap())
            )
            .is_some());
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap())
            )
            .is_some());

        // No longer in force the day after valid_to.
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            )
            .is_none());
    }

    #[test]
    fn test_resolver_valid_to_no_fall_through() {
        // An older open-ended version (100) and a newer version (200) that ends 2024-12-31.
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2023-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_validity(
                "2024-01-01",
                "2024-12-31",
                200,
            ))
            .unwrap();

        // Before the newer version: the older one applies.
        let law = resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2023, 6, 1).unwrap()),
            )
            .unwrap();
        assert_eq!(law.valid_from, Some("2023-01-01".to_string()));

        // After the newer version has ended: the law is no longer in force.
        // It must NOT resurrect the older open-ended version (RFC-019 §2).
        assert!(resolver
            .get_law_for_date(
                "test_law",
                Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap())
            )
            .is_none());
    }

    #[test]
    fn test_resolver_selection_reason() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_validity(
                "2023-01-01",
                "2023-12-31",
                100,
            ))
            .unwrap();

        // Unknown law id.
        assert_eq!(
            resolver.get_law_for_date_result("nope", NaiveDate::from_ymd_opt(2023, 6, 1).unwrap()),
            Err(SelectionReason::NotFound)
        );
        // Before it is in force.
        assert_eq!(
            resolver
                .get_law_for_date_result("test_law", NaiveDate::from_ymd_opt(2022, 1, 1).unwrap()),
            Err(SelectionReason::NotYetInForce)
        );
        // After it has ended — states the data fact, not a verdict.
        assert_eq!(
            resolver
                .get_law_for_date_result("test_law", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            Err(SelectionReason::EndedOn(
                NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
            ))
        );
        // In force.
        assert!(resolver
            .get_law_for_date_result("test_law", NaiveDate::from_ymd_opt(2023, 6, 1).unwrap())
            .is_ok());
    }

    #[test]
    fn test_resolver_rejects_calendar_invalid_valid_to() {
        // Format-valid but calendar-invalid (passes the schema regex): without
        // the load-time check this would silently skip the expiry check and
        // keep the law in force forever (RFC-019).
        let mut resolver = RuleResolver::new();
        let result = resolver.load_from_yaml(&make_test_law_with_validity(
            "2023-01-01",
            "2023-02-30",
            100,
        ));
        assert!(matches!(result, Err(EngineError::LoadError(_))));
    }

    #[test]
    fn test_resolver_rejects_a_law_that_fails_the_type_check() {
        // The loader runs the static type check (RFC-036/RFC-037), so a law
        // the validator refuses cannot be loaded into the editor either. The
        // error lists every finding.
        let mut resolver = RuleResolver::new();
        let law = r#"
$id: getypeerd
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        input:
          - name: huur
            type: amount
            source: {}
        output:
          - name: geen_huur
            type: boolean
          - name: klasse
            type: string
        actions:
          - output: geen_huur
            value:
              operation: EQUALS
              subject: $huur
              value: null
          - output: klasse
            value:
              operation: IF
              cases:
                - when: true
                  then: 5
"#;
        let err = resolver.load_from_yaml(law).unwrap_err();
        let message = err.to_string();
        assert!(matches!(err, EngineError::LoadError(_)), "{err:?}");
        assert!(
            message.contains("fails the type check (2 findings)"),
            "{message}"
        );
        assert!(message.contains("[N1]"), "{message}");
        assert!(message.contains("[T4]"), "{message}");
        assert!(!resolver.has_law("getypeerd"));

        // Declared as the law behaves, it loads.
        let fixed = law
            .replace(
                "type: amount\n",
                "type: amount\n            nullable: true\n",
            )
            .replace("then: 5", "then: vijf");
        resolver.load_from_yaml(&fixed).unwrap();
        assert!(resolver.has_law("getypeerd"));
        // One finding is one finding.
        let mut resolver = RuleResolver::new();
        let one = law.replace("then: 5", "then: vijf");
        let message = resolver.load_from_yaml(&one).unwrap_err().to_string();
        assert!(message.contains("(1 finding):"), "{message}");
    }

    /// A law whose input `k` takes `klasse` from `doel`, declared with the
    /// given attribute lines, and `doel` with a nullable `klasse`.
    fn consumer_and_target(input_attributes: &str) -> (String, &'static str) {
        let consumer = format!(
            r#"
$id: afnemer
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: k
            type: string
            {input_attributes}
            source:
              regulation: doel
              output: klasse
              parameters:
                bsn: $bsn
        output:
          - name: y
            type: boolean
        actions:
          - output: y
            value:
              operation: EQUALS
              subject: $k
              value: a
"#
        );
        let target = r#"
$id: doel
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: klasse
            type: string
            nullable: true
        actions:
          - output: klasse
            value: null
"#;
        (consumer, target)
    }

    #[test]
    fn test_the_loaded_set_is_well_typed_whatever_the_load_order() {
        // Target first: the consumer is refused for taking a nullable output
        // into a non-nullable input (N5). Consumer first: the target is
        // refused for making the consumer ill-typed, and the error names the
        // consumer. Either way the set that stays loaded is well-typed.
        let (consumer, target) = consumer_and_target("");
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(target).unwrap();
        let err = resolver.load_from_yaml(&consumer).unwrap_err().to_string();
        assert!(err.contains("law 'afnemer' fails the type check"), "{err}");
        assert!(!resolver.has_law("afnemer"));

        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(&consumer).unwrap();
        let err = resolver.load_from_yaml(target).unwrap_err().to_string();
        assert!(
            err.contains("loading law 'doel' makes 'afnemer' ill-typed (1 finding):"),
            "{err}"
        );
        assert!(
            err.contains(
                "afnemer article 1 input 'k': [N5] 'k' takes 'klasse' from doel, which may be null"
            ),
            "{err}"
        );
        assert!(!resolver.has_law("doel"));
        assert!(resolver.has_law("afnemer"));

        // With the input declared nullable both orders load.
        let (consumer, target) = consumer_and_target("nullable: true");
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(&consumer).unwrap();
        resolver.load_from_yaml(target).unwrap();
        assert!(resolver.has_law("doel") && resolver.has_law("afnemer"));

        // A consumer that does not reference the new law is not the new law's
        // concern: an unrelated ill-typed law never gets loaded, and a
        // well-typed one is not re-checked into a finding.
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(&consumer).unwrap();
        resolver
            .load_from_yaml(
                &consumer
                    .replace("$id: afnemer", "$id: ander")
                    .replace("regulation: doel", "regulation: elders"),
            )
            .unwrap();
        resolver.load_from_yaml(target).unwrap();
    }

    /// A law that references nothing and fails its own type check (N1: an
    /// absence test on a non-nullable field). Only the unchecked loader takes
    /// it, which is the point: it is a law the checked loader would never have
    /// let in.
    const ILL_TYPED_STRANGER: &str = r#"
$id: vreemde
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        input:
          - name: huur
            type: amount
            source: {}
        output:
          - name: geen_huur
            type: boolean
        actions:
          - output: geen_huur
            value:
              operation: EQUALS
              subject: $huur
              value: null
"#;

    #[test]
    fn test_a_law_that_does_not_reference_the_new_one_is_not_re_checked() {
        // The re-check on load is scoped to the laws that actually read the
        // arriving law. Scoping it is not only about work: a law already in the
        // resolver that the new one has nothing to do with must not be dragged
        // into the new law's load. Here `vreemde` is ill-typed and references
        // nothing, so re-checking it would produce findings and refuse `doel`
        // for a defect `doel` neither caused nor could fix — with the error
        // saying that loading 'doel' makes 'vreemde' ill-typed, which is false.
        let (_, target) = consumer_and_target("");
        let mut resolver = RuleResolver::new();
        assert!(resolver.load_from_yaml(ILL_TYPED_STRANGER).is_err());
        resolver.load_law_unchecked(ILL_TYPED_STRANGER).unwrap();
        resolver.load_from_yaml(target).unwrap();
        assert!(resolver.has_law("doel"));
    }

    #[test]
    fn test_an_earlier_version_of_the_arriving_law_itself_is_not_a_consumer() {
        // A law that reads its own id is a consumer of every law but itself.
        // The arriving version already stands in for its own id in `check_law`
        // above, so its own older versions stay out of the consumer sweep: an
        // ill-typed v1 is superseded by v2, not something to hold v2 against.
        // Sweeping it in would make a law that repairs its own defect
        // unloadable, leaving only the broken version in the resolver.
        //
        // The self-reference is what makes this a test and not a tautology: it
        // makes v1 look like a consumer of `zelf` to everything except the
        // check on the id.
        let self_reader = r#"
$id: zelf
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: klasse
            type: string
            nullable: true
        actions:
          - output: klasse
            value: null
  - number: '2'
    text: t
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: k
            type: string
            source:
              regulation: zelf
              output: klasse
              parameters:
                bsn: $bsn
        output:
          - name: y
            type: boolean
        actions:
          - output: y
            value:
              operation: EQUALS
              subject: $k
              value: a
"#;
        // v1 takes its own nullable output into a non-nullable input (N5), so
        // only the unchecked loader seats it.
        let mut resolver = RuleResolver::new();
        assert!(resolver.load_from_yaml(self_reader).is_err());
        resolver.load_law_unchecked(self_reader).unwrap();
        // v2 declares that input nullable and is well-typed. It carries the
        // same id, so the broken v1 is not one of its consumers.
        let v2 = self_reader
            .replace(
                "publication_date: '2025-01-01'\nvalid_from: '2025-01-01'",
                "publication_date: '2026-01-01'\nvalid_from: '2026-01-01'",
            )
            .replace(
                "          - name: k\n            type: string\n",
                "          - name: k\n            type: string\n            nullable: true\n",
            );
        assert_ne!(v2, self_reader);
        resolver.load_from_yaml(&v2).unwrap();
        assert_eq!(resolver.version_count(), 2);
    }

    #[test]
    fn test_a_new_version_of_a_law_is_held_against_its_consumers() {
        // The consumer was fine against version 1 (non-nullable output). A
        // version 2 that makes the output nullable would make the consumer
        // ill-typed, and is refused.
        let (consumer, target) = consumer_and_target("");
        let v1 = target
            .replace("            nullable: true\n", "")
            .replace("value: null", "value: a")
            .replace(
                "publication_date: '2025-01-01'",
                "publication_date: '2025-01-01'\nvalid_from: '2025-01-01'",
            );
        let v2 = target.replace(
            "publication_date: '2025-01-01'",
            "publication_date: '2026-01-01'\nvalid_from: '2026-01-01'",
        );
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(&v1).unwrap();
        resolver.load_from_yaml(&consumer).unwrap();
        let err = resolver.load_from_yaml(&v2).unwrap_err().to_string();
        assert!(err.contains("makes 'afnemer' ill-typed"), "{err}");
        assert_eq!(resolver.version_count(), 2);
    }

    #[test]
    fn test_a_delegating_law_is_held_against_its_implementations() {
        // The implementation, loaded first, declares its output nullable. The
        // delegating law then arrives with the term required and without a
        // default: loading it would leave an implementation that can hand the
        // term a null, so it is refused and the implementation is named.
        let implementing = r#"
$id: regeling
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: t
    machine_readable:
      implements:
        - law: wet
          article: '4'
          open_term: premie
      execution:
        output:
          - name: premie
            type: number
            nullable: true
        actions:
          - output: premie
            value: null
"#;
        let delegating = r#"
$id: wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: t
    machine_readable:
      open_terms:
        - id: premie
          type: number
          required: true
      execution:
        output:
          - name: premie
            type: number
        actions:
          - output: premie
            value: $premie
"#;
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(implementing).unwrap();
        let err = resolver.load_from_yaml(delegating).unwrap_err().to_string();
        assert!(
            err.contains("loading law 'wet' makes 'regeling' ill-typed"),
            "{err}"
        );
        assert!(
            err.contains("[N5] 'premie' implements the required term 'premie' of wet article 4"),
            "{err}"
        );
        // The other way round the implementation is refused itself.
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(delegating).unwrap();
        let err = resolver
            .load_from_yaml(implementing)
            .unwrap_err()
            .to_string();
        assert!(err.contains("law 'regeling' fails the type check"), "{err}");
    }

    #[test]
    fn test_load_law_unchecked_skips_the_type_check() {
        // The test-only loader exists for the run-time boundary tests; it must
        // load what the checked loader refuses.
        let (consumer, target) = consumer_and_target("");
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(target).unwrap();
        assert!(resolver.load_from_yaml(&consumer).is_err());
        assert_eq!(resolver.load_law_unchecked(&consumer).unwrap(), "afnemer");
        assert!(resolver.has_law("afnemer"));
    }

    #[test]
    fn test_resolver_version_replacement() {
        let mut resolver = RuleResolver::new();

        // Load a version
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        assert_eq!(resolver.version_count(), 1);

        // Load the same version again (same valid_from) - should replace
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 200))
            .unwrap();
        assert_eq!(resolver.version_count(), 1); // Still 1, replaced

        // Load a different version - should add
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 300))
            .unwrap();
        assert_eq!(resolver.version_count(), 2);
    }

    #[test]
    fn test_resolver_unload_version() {
        let mut resolver = RuleResolver::new();

        // Load two versions
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();
        assert_eq!(resolver.version_count(), 2);

        // Unload one version
        assert!(resolver.unload_law_version("test_law", Some("2024-01-01")));
        assert_eq!(resolver.version_count(), 1);
        assert!(resolver.has_law("test_law"));

        // Unload remaining version
        assert!(resolver.unload_law_version("test_law", Some("2025-01-01")));
        assert_eq!(resolver.version_count(), 0);
        assert!(!resolver.has_law("test_law"));
    }

    #[test]
    fn test_resolver_article_by_output_with_date() {
        let mut resolver = RuleResolver::new();

        // Load two versions with different article numbers
        let v1 = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Article v1
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: 100
"#;
        let v2 = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '2'
    text: Article v2
    machine_readable:
      execution:
        output:
          - name: test_output
            type: number
        actions:
          - output: test_output
            value: 200
"#;
        resolver.load_from_yaml(v1).unwrap();
        resolver.load_from_yaml(v2).unwrap();

        let date_2024 = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let date_2025 = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();

        // Get article for 2024 - should get v1 article
        let article = resolver
            .get_article_by_output("test_law", "test_output", Some(date_2024))
            .unwrap();
        assert_eq!(article.number, "1");

        // Get article for 2025 - should get v2 article
        let article = resolver
            .get_article_by_output("test_law", "test_output", Some(date_2025))
            .unwrap();
        assert_eq!(article.number, "2");
    }

    #[test]
    fn test_resolver_mixed_valid_from() {
        // Test mixing laws with and without valid_from
        let mut resolver = RuleResolver::new();

        // Load version without valid_from
        resolver.load_from_yaml(make_test_law()).unwrap();

        // Load version with valid_from
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();

        assert_eq!(resolver.version_count(), 2);

        // The version with valid_from should be sorted first (has a date)
        // Version without valid_from should match any date
        let date_2024 = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let law = resolver.get_law_for_date("test_law", Some(date_2024));
        assert!(law.is_some()); // The None valid_from version should match
    }

    // -------------------------------------------------------------------------
    // New Method Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolver_output_count() {
        let mut resolver = RuleResolver::new();
        assert_eq!(resolver.output_count(), 0);

        resolver.load_from_yaml(make_test_law()).unwrap();
        assert_eq!(resolver.output_count(), 1);
    }

    #[test]
    fn test_resolver_list_all_outputs() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_test_law()).unwrap();

        let outputs = resolver.list_all_outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs.contains(&("test_law", "test_output")));
    }

    // -------------------------------------------------------------------------
    // Implements Index (IoC) Tests
    // -------------------------------------------------------------------------

    fn make_law_with_open_term() -> &'static str {
        r#"
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 0
"#
    }

    fn make_implementing_regulation() -> &'static str {
        r#"
$id: regeling_standaardpremie
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1928
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1928
"#
    }

    fn make_implementing_regulation_older() -> &'static str {
        r#"
$id: regeling_standaardpremie_2024
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 1889
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1889
"#
    }

    /// The open term as the declaring article declares it — the same value the
    /// service hands to `find_implementations`, rather than one built in the
    /// test, so the tests cannot drift from what the corpus actually says.
    fn open_term<'a>(
        resolver: &'a RuleResolver,
        law_id: &str,
        article: &str,
        term_id: &str,
    ) -> &'a OpenTerm {
        resolver
            .get_law(law_id)
            .and_then(|law| law.find_article_by_number(article))
            .and_then(|a| a.get_open_terms())
            .and_then(|terms| terms.iter().find(|t| t.id == term_id))
            .unwrap_or_else(|| {
                panic!("{law_id} article {article} should declare open term '{term_id}'")
            })
    }

    #[test]
    fn test_implements_index_populated() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        // Index should be populated
        assert_eq!(resolver.implements_count(), 1);

        // Look up
        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
        assert_eq!(results[0].1.number, "1");
    }

    #[test]
    fn test_implements_index_no_match() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        // No implementing regulation loaded

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert!(results.is_empty());
    }

    #[test]
    fn test_implements_index_priority_lex_posterior() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation_older())
            .unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        assert_eq!(resolver.implements_count(), 2);

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert_eq!(results.len(), 2);
        // Winner (newest) should be first
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
        assert_eq!(results[1].0.id, "regeling_standaardpremie_2024");
    }

    // -------------------------------------------------------------------------
    // Delegation gate: who the law authorises to fill an open term
    // -------------------------------------------------------------------------

    /// A beleidsregel claiming an open term that the law reserves for a
    /// ministeriële regeling. Uitvoeringsbeleid is not an algemeen verbindend
    /// voorschrift, so it cannot fill a delegated term however recent it is.
    fn make_beleidsregel_claiming_standaardpremie() -> &'static str {
        r#"
$id: beleidsregel_standaardpremie
regulatory_layer: BELEIDSREGEL
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 2500
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 2500
"#
    }

    #[test]
    fn test_find_implementations_rejects_wrong_delegation_layer() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_beleidsregel_claiming_standaardpremie())
            .unwrap();

        // The declaration is indexed — the gate is about competence, not indexing.
        assert_eq!(resolver.implements_count(), 1);

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert!(
            results.is_empty(),
            "a BELEIDSREGEL may not fill a term delegated to a MINISTERIELE_REGELING"
        );
    }

    /// An unauthorised candidate on a higher layer must not shadow the
    /// authorised one. When the gate ran after priority, a WET claiming a term
    /// reserved for a ministeriële regeling won on lex superior and was then
    /// refused, which left the valid regeling unreachable (RFC-036 draft, §7).
    /// The gate filters first, so the regeling wins and the WET is on record.
    #[test]
    fn test_find_implementations_refused_higher_layer_does_not_shadow_the_authorised_one() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: wet_die_de_premie_claimt
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 9999
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 9999
"#,
            )
            .unwrap();

        let lookup = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap();

        let ids: Vec<&str> = lookup
            .implementations
            .iter()
            .map(|(law, _)| law.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["regeling_standaardpremie"],
            "only the authorised regeling is a candidate"
        );
        assert_eq!(lookup.refusals.len(), 1);
        assert_eq!(lookup.refusals[0].refused_law, "wet_die_de_premie_claimt");
        assert_eq!(lookup.refusals[0].refused_layer, "WET");
    }

    /// A refused candidate is skipped, but not without a word: the lookup
    /// hands the caller what was offered, by whom, and which layer the law
    /// reserves the term for.
    #[test]
    fn test_find_implementations_reports_the_refused_candidate() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_beleidsregel_claiming_standaardpremie())
            .unwrap();

        let lookup = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap();

        assert_eq!(
            lookup.refusals,
            vec![DelegationRefusal {
                declaring_law: "wet_op_de_zorgtoeslag".to_string(),
                declaring_article: "4".to_string(),
                open_term: "standaardpremie".to_string(),
                required_layer: "MINISTERIELE_REGELING".to_string(),
                refused_law: "beleidsregel_standaardpremie".to_string(),
                refused_article: "1".to_string(),
                refused_layer: "BELEIDSREGEL".to_string(),
            }],
            "a refused implementation must be reported, not dropped"
        );

        let message = lookup.refusals[0].message();
        for part in [
            "beleidsregel_standaardpremie",
            "BELEIDSREGEL",
            "standaardpremie",
            "wet_op_de_zorgtoeslag",
            "MINISTERIELE_REGELING",
        ] {
            assert!(
                message.contains(part),
                "the refusal message must name {part}: {message}"
            );
        }
    }

    /// A candidate rejected for scope is not a refusal: a verordening from
    /// another municipality does not apply to this case, which is a property of
    /// the case and not a defect in the corpus.
    #[test]
    fn test_find_implementations_scope_mismatch_is_not_a_refusal() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(make_law_with_open_term_scoped())
            .unwrap();
        resolver
            .load_from_yaml(&make_scoped_implementation(
                "verordening_g0363",
                "gemeente_code: 'GM0363'",
                7,
            ))
            .unwrap();

        let lookup = resolver
            .find_implementations(
                "kaderwet",
                "1",
                open_term(&resolver, "kaderwet", "1", "tarief"),
                None,
                &scope_of("gemeente_code", "GM0599"),
            )
            .unwrap();

        assert!(lookup.implementations.is_empty());
        assert!(
            lookup.refusals.is_empty(),
            "another municipality's verordening is silently inapplicable, not refused"
        );
    }

    /// The gate must not merely demote an unauthorised candidate: the
    /// ministeriële regeling wins even though the beleidsregel is newer, and
    /// the beleidsregel does not appear among the results at all.
    #[test]
    fn test_find_implementations_wrong_layer_does_not_shadow_authorised_one() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_beleidsregel_claiming_standaardpremie())
            .unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
    }

    /// The gate must not depend on finding the declaring term a second time.
    ///
    /// Here the term is declared on article '4' of the version in force in
    /// 2026, while the version in force in 2025 numbers the same provision '3'.
    /// A gate that re-derived `delegation_type` from the law selected for the
    /// reference date would look for article '4' in the 2025 version, find
    /// nothing, and read that miss as "this law demands no particular layer" —
    /// letting a beleidsregel fill a term reserved for a ministeriële regeling.
    /// Passing the term in makes that whole class impossible; this test holds
    /// the door shut.
    #[test]
    fn test_find_implementations_gate_holds_when_the_older_version_renumbered() {
        let mut resolver = RuleResolver::new();

        // The version in force in 2025: same provision, article '3'.
        resolver
            .load_from_yaml(
                r#"
$id: wet_hernummerd
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '3'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegation_type: MINISTERIELE_REGELING
"#,
            )
            .unwrap();
        // The version in force in 2026: the provision is renumbered to '4'.
        resolver
            .load_from_yaml(
                r#"
$id: wet_hernummerd
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2026-01-01'
articles:
  - number: '4'
    text: De standaardpremie wordt vastgesteld bij ministeriele regeling
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegation_type: MINISTERIELE_REGELING
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: beleidsregel_hernummerd
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: De standaardpremie bedraagt 2500
    machine_readable:
      implements:
        - law: wet_hernummerd
          article: '4'
          open_term: standaardpremie
      execution:
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 2500
"#,
            )
            .unwrap();

        // Article '4' as the 2026 version declares it, evaluated against a
        // reference date on which the 2025 version is in force.
        let term = open_term(&resolver, "wet_hernummerd", "4", "standaardpremie");
        let lookup = resolver
            .find_implementations(
                "wet_hernummerd",
                "4",
                term,
                Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
                &HashMap::new(),
            )
            .unwrap();

        assert!(
            lookup.implementations.is_empty(),
            "a BELEIDSREGEL may not fill this term, whichever version of the \
             declaring law the reference date selects"
        );
        assert_eq!(
            lookup.refusals.len(),
            1,
            "and the refusal is reported: {:?}",
            lookup.refusals
        );
    }

    /// An open term without a `delegation_type` demands nothing of the layer,
    /// so uitvoeringsbeleid may fill it.
    #[test]
    fn test_find_implementations_without_delegation_type_accepts_any_layer() {
        let mut resolver = RuleResolver::new();

        resolver
            .load_from_yaml(
                r#"
$id: wet_zonder_delegatie
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bestuursorgaan handelt redelijkerwijs
    machine_readable:
      open_terms:
        - id: redelijke_termijn_dagen
          type: number
          required: true
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: beleid_redelijke_termijn
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Een redelijke termijn is acht weken
    machine_readable:
      implements:
        - law: wet_zonder_delegatie
          article: '1'
          open_term: redelijke_termijn_dagen
      execution:
        output:
          - name: redelijke_termijn_dagen
            type: number
        actions:
          - output: redelijke_termijn_dagen
            value: 56
"#,
            )
            .unwrap();

        let results = resolver
            .find_implementations(
                "wet_zonder_delegatie",
                "1",
                open_term(
                    &resolver,
                    "wet_zonder_delegatie",
                    "1",
                    "redelijke_termijn_dagen",
                ),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "beleid_redelijke_termijn");
    }

    /// A `delegation_type` naming no known layer is an error, not a silent
    /// rejection of everything: the engine must not answer "no implementation"
    /// to a question it did not understand.
    #[test]
    fn test_find_implementations_unknown_delegation_type_is_error() {
        let mut resolver = RuleResolver::new();

        resolver
            .load_from_yaml(
                r#"
$id: wet_met_typo
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Bij ministeriele regeling wordt het bedrag vastgesteld
    machine_readable:
      open_terms:
        - id: bedrag
          type: amount
          required: true
          delegation_type: MINISTERIELE_REGELIN
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: regeling_bedrag
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag bedraagt 100
    machine_readable:
      implements:
        - law: wet_met_typo
          article: '1'
          open_term: bedrag
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 100
"#,
            )
            .unwrap();

        let err = resolver
            .find_implementations(
                "wet_met_typo",
                "1",
                open_term(&resolver, "wet_met_typo", "1", "bedrag"),
                None,
                &HashMap::new(),
            )
            .unwrap_err();
        assert!(
            matches!(err, EngineError::ResolutionError(ref m)
                if m.contains("MINISTERIELE_REGELIN") && m.contains("not a known regulatory layer")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_implements_index_unload() {
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();

        assert_eq!(resolver.implements_count(), 1);

        resolver.unload_law("regeling_standaardpremie");
        assert_eq!(resolver.implements_count(), 0);

        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert!(results.is_empty());
    }

    #[test]
    fn test_implements_index_backward_compat() {
        // Laws without implements should still load fine
        let mut resolver = RuleResolver::new();

        resolver.load_from_yaml(make_test_law()).unwrap();

        assert_eq!(resolver.implements_count(), 0);
        assert_eq!(resolver.law_count(), 1);
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

    #[test]
    fn test_resolver_load_from_directory() {
        let regulation_path = get_regulation_path().join("nl");

        let mut resolver = RuleResolver::new();
        let count = resolver.load_from_directory(&regulation_path).unwrap();

        assert!(
            count >= 10,
            "Expected at least 10 laws from corpus/regulation/nl, got {}",
            count
        );
        assert!(resolver.has_law("wet_op_de_zorgtoeslag"));
        assert!(resolver.has_law("regeling_standaardpremie"));
        assert!(resolver.has_law("participatiewet"));
    }

    // -------------------------------------------------------------------------
    // Law count limit (memory-exhaustion guard)
    // -------------------------------------------------------------------------

    /// A minimal law with its own `$id`, so the registry can be filled to the limit.
    fn make_numbered_law(n: usize, valid_from: Option<&str>) -> String {
        let valid_from_line = match valid_from {
            Some(d) => format!("valid_from: '{d}'\n"),
            None => String::new(),
        };
        format!(
            r#"
$id: filler_law_{n}
regulatory_layer: WET
publication_date: '2025-01-01'
{valid_from_line}articles:
  - number: '1'
    text: Filler article {n}
    machine_readable:
      execution:
        output:
          - name: filler_output
            type: number
        actions:
          - output: filler_output
            value: {n}
"#
        )
    }

    fn resolver_at_law_limit() -> RuleResolver {
        let mut resolver = RuleResolver::new();
        for n in 0..config::MAX_LOADED_LAWS {
            resolver
                .load_from_yaml(&make_numbered_law(n, None))
                .unwrap();
        }
        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
        resolver
    }

    #[test]
    fn test_resolver_limit_rejects_a_new_law() {
        let mut resolver = resolver_at_law_limit();

        let result = resolver.load_from_yaml(&make_numbered_law(config::MAX_LOADED_LAWS, None));

        assert!(
            matches!(result, Err(EngineError::LoadError(_))),
            "a new law at the limit must be rejected, got {result:?}"
        );
        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
        assert!(!resolver.has_law(&format!("filler_law_{}", config::MAX_LOADED_LAWS)));
    }

    #[test]
    fn test_resolver_limit_rejects_new_version_of_existing_law() {
        // A second version of an already-loaded law is a *new* law in the
        // registry: it adds memory, so it must hit the limit like any other.
        let mut resolver = resolver_at_law_limit();

        let result = resolver.load_from_yaml(&make_numbered_law(0, Some("2030-01-01")));

        assert!(
            matches!(result, Err(EngineError::LoadError(_))),
            "adding a new version at the limit must be rejected, got {result:?}"
        );
        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
        assert_eq!(resolver.version_count_for_law("filler_law_0"), 1);
    }

    #[test]
    fn test_resolver_limit_allows_replacing_existing_version() {
        // Replacing a version (same law id, same valid_from) does not grow the
        // registry, so the limit must not block it — otherwise a full resolver
        // could never be corrected.
        let mut resolver = resolver_at_law_limit();

        resolver
            .load_from_yaml(&make_numbered_law(0, None))
            .expect("replacing an existing version at the limit must be allowed");

        assert_eq!(resolver.version_count(), config::MAX_LOADED_LAWS);
    }

    #[test]
    fn test_resolver_law_count_counts_unique_ids() {
        let mut resolver = RuleResolver::new();
        assert_eq!(resolver.law_count(), 0);

        resolver
            .load_from_yaml(&make_numbered_law(1, None))
            .unwrap();
        resolver
            .load_from_yaml(&make_numbered_law(2, None))
            .unwrap();
        assert_eq!(resolver.law_count(), 2);

        // A second version of law 1 adds a version, not a law id.
        resolver
            .load_from_yaml(&make_numbered_law(1, Some("2030-01-01")))
            .unwrap();
        assert_eq!(resolver.law_count(), 2);
        assert_eq!(resolver.version_count(), 3);
    }

    #[test]
    fn test_resolver_all_law_versions_yields_every_version() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();
        resolver
            .load_from_yaml(&make_numbered_law(7, None))
            .unwrap();

        let mut seen: Vec<(&str, Option<&str>)> = resolver
            .all_law_versions()
            .map(|law| (law.id.as_str(), law.valid_from.as_deref()))
            .collect();
        seen.sort();

        assert_eq!(
            seen,
            vec![
                ("filler_law_7", None),
                ("test_law", Some("2024-01-01")),
                ("test_law", Some("2025-01-01")),
            ]
        );
    }

    // -------------------------------------------------------------------------
    // Version unloading
    // -------------------------------------------------------------------------

    #[test]
    fn test_resolver_unload_version_removes_only_the_named_version() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2025-01-01", 200))
            .unwrap();

        assert!(resolver.unload_law_version("test_law", Some("2024-01-01")));

        // The version that was *not* named must survive.
        assert_eq!(resolver.version_count(), 1);
        let remaining = resolver.get_law("test_law").unwrap();
        assert_eq!(remaining.valid_from, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_resolver_unload_unknown_version_is_a_no_op() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_test_law_with_valid_from("2024-01-01", 100))
            .unwrap();

        // Nothing matches, so nothing was removed — the caller must be told so.
        assert!(!resolver.unload_law_version("test_law", Some("1999-01-01")));
        assert!(!resolver.unload_law_version("test_law", None));
        assert_eq!(resolver.version_count(), 1);
        assert!(resolver.has_law("test_law"));

        // Unknown law id likewise.
        assert!(!resolver.unload_law_version("nonexistent", Some("2024-01-01")));
    }

    // -------------------------------------------------------------------------
    // Scope filtering (gemeentelijke / waterschaps-verordeningen)
    // -------------------------------------------------------------------------

    fn make_law_with_open_term_scoped() -> &'static str {
        r#"
$id: kaderwet
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief wordt vastgesteld bij verordening
    machine_readable:
      open_terms:
        - id: tarief
          type: amount
          required: true
          delegated_to: waterschap
          delegation_type: WATERSCHAPS_VERORDENING
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: 0
"#
    }

    fn make_scoped_implementation(id: &str, scope_line: &str, value: i32) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: WATERSCHAPS_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
{scope_line}
articles:
  - number: '1'
    text: Het tarief bedraagt {value}
    machine_readable:
      implements:
        - law: kaderwet
          article: '1'
          open_term: tarief
          gelet_op: "Gelet op artikel 1 van de Kaderwet"
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: {value}
"#
        )
    }

    fn scope_of(field: &str, code: &str) -> HashMap<String, Value> {
        let mut scope = HashMap::new();
        scope.insert(field.to_string(), Value::String(code.to_string()));
        scope
    }

    /// A scoped regulation fills an open term of its own whatever the scope:
    /// the scope picks which municipality's regulation applies to a case,
    /// and a regulation whose article declares the term already applies. A
    /// hardship clause in a verordening that sets aside one of its own
    /// articles is such a case.
    #[test]
    fn test_find_implementations_a_scoped_law_fills_its_own_term() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: verordening_met_hardheidsclausule
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
gemeente_code: 'GM0363'
articles:
  - number: '1'
    text: Het college weigert, tenzij artikel 2 anders bepaalt.
    machine_readable:
      open_terms:
        - id: buiten_toepassing
          type: boolean
          required: true
      execution:
        output:
          - name: geweigerd
            type: boolean
        actions:
          - output: geweigerd
            value:
              operation: NOT
              value: $buiten_toepassing
  - number: '2'
    text: Het college kan artikel 1 buiten toepassing laten.
    machine_readable:
      implements:
        - law: verordening_met_hardheidsclausule
          article: '1'
          open_term: buiten_toepassing
      execution:
        output:
          - name: buiten_toepassing
            type: boolean
        actions:
          - output: buiten_toepassing
            value: true
"#,
            )
            .unwrap();
        for scope in [HashMap::new(), scope_of("gemeente_code", "GM0599")] {
            let found = resolver
                .find_implementations(
                    "verordening_met_hardheidsclausule",
                    "1",
                    open_term(
                        &resolver,
                        "verordening_met_hardheidsclausule",
                        "1",
                        "buiten_toepassing",
                    ),
                    None,
                    &scope,
                )
                .unwrap()
                .implementations;
            assert_eq!(found.len(), 1, "scope {scope:?}");
            assert_eq!(found[0].1.number, "2");
        }
    }

    /// A regulation of the same municipality fills a term that its
    /// verordening declares, also without a scope, like the verordening
    /// itself (the college's nadere regels); one of another municipality does
    /// not.
    #[test]
    fn test_find_implementations_a_law_of_the_same_territory_fills_the_term() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: verordening_gm0363
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
gemeente_code: 'GM0363'
articles:
  - number: '1'
    text: Het college stelt het tarief vast.
    machine_readable:
      open_terms:
        - id: tarief
          type: number
          required: true
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: $tarief
"#,
            )
            .unwrap();
        for (id, code) in [
            ("nadere_regels_gm0363", "GM0363"),
            ("nadere_regels_gm0599", "GM0599"),
        ] {
            resolver
                .load_from_yaml(&format!(
                    r#"
$id: {id}
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
gemeente_code: '{code}'
articles:
  - number: '1'
    text: Het tarief is 7.
    machine_readable:
      implements:
        - law: verordening_gm0363
          article: '1'
          open_term: tarief
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: 7
"#
                ))
                .unwrap();
        }
        // A run without a gemeente_code, as a cell makes one.
        let found = resolver
            .find_implementations(
                "verordening_gm0363",
                "1",
                open_term(&resolver, "verordening_gm0363", "1", "tarief"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        let ids: Vec<&str> = found.iter().map(|(law, _)| law.id.as_str()).collect();
        assert_eq!(ids, ["nadere_regels_gm0363"]);
    }

    /// Two laws share a territory only when both carry scope fields and those
    /// fields are equal; two national laws do not.
    #[test]
    fn test_shares_territory() {
        let law = |id: &str, scope: &str| {
            ArticleBasedLaw::from_yaml_str(&format!(
                r#"
$id: {id}
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
{scope}
articles:
  - number: '1'
    text: Tekst.
"#
            ))
            .unwrap()
        };
        let national = law("a", "");
        let gm = law("b", "gemeente_code: 'GM0363'");
        let gm_other = law("c", "gemeente_code: 'GM0599'");
        let pv = law("d", "provincie_code: 'PV27'");
        let ws = law("e", "waterschap_code: 'WS0653'");
        let ws_other = law("f", "waterschap_code: 'WS0155'");
        let shares = RuleResolver::shares_territory;
        assert!(!shares(&national, &national));
        assert!(!shares(&national, &gm));
        assert!(!shares(&gm, &national));
        assert!(shares(&gm, &gm));
        assert!(!shares(&gm, &gm_other));
        assert!(shares(&pv, &pv));
        assert!(!shares(&pv, &gm));
        assert!(shares(&ws, &ws));
        assert!(!shares(&ws, &ws_other));
    }

    /// A waterschapsverordening only implements an open term for its own
    /// waterschap; another waterschap (or a national run) must not inherit it.
    #[test]
    fn test_find_implementations_filters_on_waterschap_code() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(make_law_with_open_term_scoped())
            .unwrap();
        resolver
            .load_from_yaml(&make_scoped_implementation(
                "keur_ws0653",
                "waterschap_code: 'WS0653'",
                42,
            ))
            .unwrap();

        let find = |scope: &HashMap<String, Value>| {
            resolver
                .find_implementations(
                    "kaderwet",
                    "1",
                    open_term(&resolver, "kaderwet", "1", "tarief"),
                    None,
                    scope,
                )
                .unwrap()
                .implementations
        };

        // Own waterschap: applies.
        let matching = find(&scope_of("waterschap_code", "WS0653"));
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].0.id, "keur_ws0653");

        // Different waterschap: does not apply.
        assert!(find(&scope_of("waterschap_code", "WS0999")).is_empty());

        // No waterschap in scope at all: does not apply.
        assert!(find(&HashMap::new()).is_empty());

        // A gemeente in scope does not satisfy a waterschap requirement.
        assert!(find(&scope_of("gemeente_code", "WS0653")).is_empty());
    }

    /// The gemeente branch of the same rule, so both scope fields are pinned.
    #[test]
    fn test_find_implementations_filters_on_gemeente_code() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(make_law_with_open_term_scoped())
            .unwrap();
        resolver
            .load_from_yaml(&make_scoped_implementation(
                "verordening_g0363",
                "gemeente_code: 'GM0363'",
                7,
            ))
            .unwrap();

        let find = |scope: &HashMap<String, Value>| {
            resolver
                .find_implementations(
                    "kaderwet",
                    "1",
                    open_term(&resolver, "kaderwet", "1", "tarief"),
                    None,
                    scope,
                )
                .unwrap()
                .implementations
        };

        assert_eq!(find(&scope_of("gemeente_code", "GM0363")).len(), 1);
        assert!(find(&scope_of("gemeente_code", "GM0599")).is_empty());
        assert!(find(&HashMap::new()).is_empty());
    }

    /// The provincie branch, which the comment on `matches_scope` already
    /// promised while the code did not have it: a provinciale verordening was
    /// national as far as the resolver was concerned.
    #[test]
    fn test_find_implementations_filters_on_provincie_code() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: kaderwet_provinciaal
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief wordt vastgesteld bij provinciale verordening
    machine_readable:
      open_terms:
        - id: tarief
          type: amount
          required: true
          delegated_to: provinciale staten
          delegation_type: PROVINCIALE_VERORDENING
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: verordening_pv27
regulatory_layer: PROVINCIALE_VERORDENING
publication_date: '2025-01-01'
valid_from: '2025-01-01'
provincie_code: PV27
articles:
  - number: '1'
    text: Het tarief bedraagt 12
    machine_readable:
      implements:
        - law: kaderwet_provinciaal
          article: '1'
          open_term: tarief
      execution:
        output:
          - name: tarief
            type: number
        actions:
          - output: tarief
            value: 12
"#,
            )
            .unwrap();

        let find = |scope: &HashMap<String, Value>| {
            resolver
                .find_implementations(
                    "kaderwet_provinciaal",
                    "1",
                    open_term(&resolver, "kaderwet_provinciaal", "1", "tarief"),
                    None,
                    scope,
                )
                .unwrap()
                .implementations
        };

        assert_eq!(find(&scope_of("provincie_code", "PV27")).len(), 1);
        assert!(find(&scope_of("provincie_code", "PV26")).is_empty());
        assert!(find(&HashMap::new()).is_empty());
        // A gemeente in scope does not satisfy a provincie requirement.
        assert!(find(&scope_of("gemeente_code", "PV27")).is_empty());
    }

    // -------------------------------------------------------------------------
    // Index bookkeeping: hooks, overrides, procedures
    // -------------------------------------------------------------------------

    fn make_law_with_hook(id: &str, article: &str, decision_type: Option<&str>) -> String {
        let decision_type_line = match decision_type {
            Some(dt) => format!("            decision_type: {dt}\n"),
            None => String::new(),
        };
        format!(
            r#"
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '{article}'
    text: Hook article
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
{decision_type_line}      execution:
        output:
          - name: hook_output_{id}
            type: number
        actions:
          - output: hook_output_{id}
            value: 1
"#
        )
    }

    fn make_law_with_override(id: &str, output: &str) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '69'
    text: In afwijking van artikel 1 van de doelwet
    machine_readable:
      overrides:
        - law: doelwet
          article: '1'
          output: {output}
      execution:
        output:
          - name: {output}
            type: number
        actions:
          - output: {output}
            value: 4
"#
        )
    }

    fn make_override_target() -> &'static str {
        r#"
$id: doelwet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De termijn bedraagt zes weken
    machine_readable:
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 6
"#
    }

    fn make_law_with_procedure(id: &str, legal_character: &str, proc_id: &str) -> String {
        format!(
            r#"
$id: {id}
regulatory_layer: WET
publication_date: '2025-01-01'
procedure:
  - id: {proc_id}
    default: true
    applies_to:
      legal_character: {legal_character}
    stages:
      - name: AANVRAAG
        description: Belanghebbende dient aanvraag in
      - name: BESLUIT
        description: Bestuursorgaan neemt besluit
articles:
  - number: '1'
    text: Procedure-artikel
    machine_readable:
      execution:
        output:
          - name: proc_output_{proc_id}
            type: number
        actions:
          - output: proc_output_{proc_id}
            value: 1
"#
        )
    }

    /// Loading an unrelated law rebuilds only *its own* index entries. It must
    /// not evict the hooks, overrides and procedures other laws registered.
    #[test]
    fn test_loading_a_law_keeps_other_laws_indexes() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "bezwaartermijn_weken",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet", "3:46", None))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();

        // Now load something entirely unrelated.
        resolver.load_from_yaml(make_test_law()).unwrap();

        let overrides = resolver.find_overrides("doelwet", "1", "bezwaartermijn_weken");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].law_id, "afwijkingswet");

        let hooks = resolver.find_hooks(HookPoint::PostActions, "BESCHIKKING", None, "BESLUIT");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].law_id, "hookwet");

        assert!(resolver.find_procedure("BESCHIKKING", None).is_some());
        assert!(resolver
            .find_procedure("BESCHIKKING", Some("beschikking"))
            .is_some());
    }

    /// Unloading one law must remove exactly that law's index entries and leave
    /// every other law's entries intact.
    #[test]
    fn test_unloading_a_law_leaves_other_laws_indexes_intact() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet_a",
                "bezwaartermijn_weken",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet_b",
                "bezwaartermijn_weken",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet_a", "3:46", None))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet_b", "3:47", None))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet_a",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet_b",
                "ALGEMEEN_VERBINDEND_VOORSCHRIFT",
                "avv",
            ))
            .unwrap();

        assert!(resolver.unload_law("afwijkingswet_a"));
        assert!(resolver.unload_law("hookwet_a"));
        assert!(resolver.unload_law("procedurewet_a"));

        // Overrides: only b survives, and it is really b.
        let overrides = resolver.find_overrides("doelwet", "1", "bezwaartermijn_weken");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].law_id, "afwijkingswet_b");

        // Hooks: only b survives, and it is really b.
        let hooks = resolver.find_hooks(HookPoint::PostActions, "BESCHIKKING", None, "BESLUIT");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].law_id, "hookwet_b");

        // Procedures: the unloaded law's procedure is gone, the other remains.
        assert!(resolver.find_procedure("BESCHIKKING", None).is_none());
        assert!(resolver
            .find_procedure("ALGEMEEN_VERBINDEND_VOORSCHRIFT", None)
            .is_some());
    }

    // -------------------------------------------------------------------------
    // Fail-open: what cannot be determined must not read as "no requirement"
    // -------------------------------------------------------------------------

    /// A `valid_from` that is neither a date nor an internal reference is a
    /// typo, and a typo used to read as "this version has no start date", which
    /// put it in force on every date there is. Refused at load, like `valid_to`.
    #[test]
    fn test_a_malformed_valid_from_is_refused_at_load() {
        let mut resolver = RuleResolver::new();
        let err = resolver
            .load_from_yaml(
                r#"
$id: wet_met_kapotte_datum
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2023-02-30'
articles: []
"#,
            )
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("valid_from") && err.contains("2023-02-30"),
            "the error must name the field and the value: {err}"
        );
        assert_eq!(resolver.law_count(), 0, "the law must not be half-loaded");
    }

    /// A `#`-reference is the one non-date form the schema allows: the law says
    /// its commencement is fixed elsewhere. It loads, but selection refuses to
    /// pretend it knows the answer.
    #[test]
    fn test_an_internal_reference_valid_from_loads_but_does_not_select() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: wet_bij_kb
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '#artikel_12'
articles:
  - number: '1'
    text: Deze wet treedt in werking op een bij koninklijk besluit te bepalen tijdstip
    machine_readable:
      execution:
        output:
          - name: bedrag
            type: number
        actions:
          - output: bedrag
            value: 1
"#,
            )
            .unwrap();

        let reason = resolver
            .get_law_for_date_result("wet_bij_kb", NaiveDate::from_ymd_opt(2025, 6, 1).unwrap())
            .unwrap_err();
        assert_eq!(
            reason,
            SelectionReason::UndeterminedStart("#artikel_12".to_string()),
            "an undetermined commencement must be reported, not read as 'always in force'"
        );
    }

    /// A `#` with nothing behind it references nothing.
    ///
    /// The schema pins the two forms a `valid_from` may take, and the reference
    /// form is `#.+`: a bare `#` names no article, so it is a typo and not a
    /// commencement fixed elsewhere. Admitting it as a reference would carry a
    /// typo past the load-time refusal, which is the whole point of that check,
    /// and leave it to be re-read later as a start date the law does not give.
    #[test]
    fn test_a_bare_hash_valid_from_is_refused_at_load() {
        let mut resolver = RuleResolver::new();
        let err = resolver
            .load_from_yaml(
                r#"
$id: wet_met_kaal_hekje
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '#'
articles: []
"#,
            )
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("valid_from") && err.contains("internal reference"),
            "a bare '#' is neither a date nor a reference, and the error must say so: {err}"
        );
        assert_eq!(resolver.law_count(), 0);
    }

    /// The refusal is about this date, not about the law: a version with a real
    /// date that covers the reference date still answers, even when an older
    /// version leaves its commencement to a koninklijk besluit.
    #[test]
    fn test_an_undetermined_older_version_does_not_block_a_dated_one() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: wet_gemengd
regulatory_layer: WET
publication_date: '2020-01-01'
valid_from: '#artikel_12'
articles: []
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: wet_gemengd
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles: []
"#,
            )
            .unwrap();

        let selected = resolver
            .get_law_for_date_result("wet_gemengd", NaiveDate::from_ymd_opt(2025, 6, 1).unwrap())
            .expect("the dated version covers this date");
        assert_eq!(selected.valid_from.as_deref(), Some("2025-01-01"));

        // Before that version commenced, the only candidate left is the one
        // whose start date the file does not give.
        let reason = resolver
            .get_law_for_date_result("wet_gemengd", NaiveDate::from_ymd_opt(2021, 6, 1).unwrap())
            .unwrap_err();
        assert_eq!(
            reason,
            SelectionReason::UndeterminedStart("#artikel_12".to_string())
        );
    }

    /// A hook with no `legal_character` has no key to be filed under, so it
    /// would be dropped and never fire while the article claiming it stays.
    /// The law is refused instead.
    #[test]
    fn test_a_hook_without_legal_character_is_refused_at_load() {
        let mut resolver = RuleResolver::new();
        let err = resolver
            .load_from_yaml(
                r#"
$id: wet_met_stille_hook
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '3:46'
    text: Een besluit dient te berusten op een deugdelijke motivering
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            stage: BESLUIT
      execution:
        output:
          - name: motivering_vereist
            type: boolean
        actions:
          - output: motivering_vereist
            value: true
"#,
            )
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("3:46") && err.contains("legal_character"),
            "the error must name the article and what it lacks: {err}"
        );
        assert_eq!(resolver.law_count(), 0);
    }

    /// A procedure asked for by name and not found is a defect, and must be
    /// distinguishable from "this legal character has no procedure at all".
    #[test]
    fn test_procedure_lookup_distinguishes_absent_from_missing() {
        let mut resolver = RuleResolver::new();

        // Nothing loaded: no procedure exists for this character.
        assert_eq!(
            resolver.find_procedure_reported("BESCHIKKING", None),
            Err(ProcedureMiss::NoneForCharacter)
        );

        resolver
            .load_from_yaml(&make_law_with_procedure(
                "awb_test",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();

        // The default is found by character, and by its own name.
        assert!(resolver
            .find_procedure_reported("BESCHIKKING", None)
            .is_ok());
        assert!(resolver
            .find_procedure_reported("BESCHIKKING", Some("beschikking"))
            .is_ok());

        // A named procedure that no law defines is a different answer.
        assert_eq!(
            resolver.find_procedure_reported("BESCHIKKING", Some("beschikking_uov")),
            Err(ProcedureMiss::NamedNotFound("beschikking_uov".to_string())),
            "asking for a procedure by name and missing it must not read as 'no procedure'"
        );

        // And another character still has none at all.
        assert_eq!(
            resolver.find_procedure_reported("TOETS", None),
            Err(ProcedureMiss::NoneForCharacter)
        );
    }

    /// The third miss, and the one no law can produce: the default index names
    /// a procedure the definition index does not hold.
    ///
    /// It is reported apart from a named miss on purpose. A named miss is a law
    /// asking for something that was never written, which a corpus author fixes
    /// in the law; a dangling default is the resolver's own two indexes
    /// disagreeing, which no edit to a law can repair. Reading the second as
    /// the first sends whoever debugs it to the wrong file.
    ///
    /// The state is built by hand because loading cannot produce it — which is
    /// exactly why nothing else covers this arm.
    #[test]
    fn test_a_dangling_default_is_not_reported_as_a_named_miss() {
        let mut resolver = RuleResolver::new();
        resolver
            .procedure_defaults
            .insert("BESCHIKKING".to_string(), "beschikking".to_string());
        // procedure_index deliberately left empty: the indexes disagree.

        assert_eq!(
            resolver.find_procedure_reported("BESCHIKKING", None),
            Err(ProcedureMiss::DefaultDangling("beschikking".to_string())),
            "a default that resolves to nothing is an index defect, not a law asking for a \
             procedure by name"
        );

        // The same procedure id asked for by name is the other answer, so the
        // two are told apart by how the id was arrived at and not by the id.
        assert_eq!(
            resolver.find_procedure_reported("BESCHIKKING", Some("beschikking")),
            Err(ProcedureMiss::NamedNotFound("beschikking".to_string()))
        );
    }

    /// The hooks, overrides and procedure indexes carry only the newest version
    /// of a law. When an older version is in force on the reference date and it
    /// declares something different, the resolver says so, so that "no hook"
    /// cannot pass as a finding.
    #[test]
    fn test_declarations_from_another_version_are_reported() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(
                r#"
$id: wet_met_hook
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '2024-01-01'
articles:
  - number: '1'
    text: Oude versie met een hook
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: motivering_vereist
            type: boolean
        actions:
          - output: motivering_vereist
            value: true
"#,
            )
            .unwrap();
        resolver
            .load_from_yaml(
                r#"
$id: wet_met_hook
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: Nieuwe versie zonder hook
    machine_readable:
      execution:
        output:
          - name: motivering_vereist
            type: boolean
        actions:
          - output: motivering_vereist
            value: true
"#,
            )
            .unwrap();

        // On a date the newest version covers, the index speaks for it.
        assert!(resolver
            .declarations_from_another_version(Some(NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()))
            .is_empty());

        // On a date the older version covers, it does not.
        let notes = resolver
            .declarations_from_another_version(Some(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()));
        assert_eq!(notes.len(), 1, "expected one note, got {notes:?}");
        assert_eq!(notes[0].law_id, "wet_met_hook");
        assert_eq!(notes[0].indexed_version.as_deref(), Some("2025-01-01"));
        assert_eq!(notes[0].in_force_version.as_deref(), Some("2024-01-01"));
        assert!(
            notes[0].message().contains("not established"),
            "the note must say that absence was not established: {}",
            notes[0].message()
        );
    }

    /// Two versions that declare the same hooks, overrides and procedures index
    /// identically, so which one was indexed cannot change an answer. Saying
    /// otherwise on every multi-version law would make the note meaningless.
    #[test]
    fn test_identical_declarations_across_versions_are_not_reported() {
        let mut resolver = RuleResolver::new();
        for publication in ["2024-01-01", "2025-01-01"] {
            resolver
                .load_from_yaml(&format!(
                    r#"
$id: wet_gelijk
regulatory_layer: WET
publication_date: '{publication}'
valid_from: '{publication}'
articles:
  - number: '1'
    text: Beide versies verklaren hetzelfde
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
            stage: BESLUIT
      execution:
        output:
          - name: motivering_vereist
            type: boolean
        actions:
          - output: motivering_vereist
            value: true
"#
                ))
                .unwrap();
        }

        assert!(
            resolver
                .declarations_from_another_version(Some(
                    NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()
                ))
                .is_empty(),
            "versions that declare the same thing need no note"
        );
    }

    /// The output index must lose exactly the unloaded law's outputs.
    #[test]
    fn test_unloading_a_law_leaves_other_laws_outputs_indexed() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_test_law()).unwrap();
        resolver
            .load_from_yaml(&make_numbered_law(3, None))
            .unwrap();

        assert!(resolver.unload_law("test_law"));

        assert_eq!(
            resolver.list_all_outputs(),
            vec![("filler_law_3", "filler_output")]
        );
        assert_eq!(resolver.output_count(), 1);
    }

    /// Unloading one implementing regulation must not take its siblings with it.
    #[test]
    fn test_unloading_one_implementor_keeps_the_others() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_law_with_open_term()).unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation_older())
            .unwrap();
        resolver
            .load_from_yaml(make_implementing_regulation())
            .unwrap();
        assert_eq!(resolver.implements_count(), 2);

        resolver.unload_law("regeling_standaardpremie_2024");

        assert_eq!(resolver.implements_count(), 1);
        let results = resolver
            .find_implementations(
                "wet_op_de_zorgtoeslag",
                "4",
                open_term(&resolver, "wet_op_de_zorgtoeslag", "4", "standaardpremie"),
                None,
                &HashMap::new(),
            )
            .unwrap()
            .implementations;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "regeling_standaardpremie");
    }

    // -------------------------------------------------------------------------
    // Hook filtering
    // -------------------------------------------------------------------------

    /// A hook narrowed to one decision type fires for that type only — never for
    /// another type, and never for a decision without a type.
    #[test]
    fn test_find_hooks_filters_on_decision_type() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet", "3:46", Some("TOEKENNING")))
            .unwrap();

        let find = |dt: Option<&str>| {
            resolver
                .find_hooks(HookPoint::PostActions, "BESCHIKKING", dt, "BESLUIT")
                .len()
        };

        assert_eq!(find(Some("TOEKENNING")), 1);
        assert_eq!(find(Some("AFWIJZING")), 0);
        assert_eq!(find(None), 0);
    }

    /// A hook without a decision-type filter fires regardless of decision type.
    #[test]
    fn test_find_hooks_without_decision_type_filter_matches_all() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_law_with_hook("hookwet", "3:46", None))
            .unwrap();

        for dt in [Some("TOEKENNING"), Some("AFWIJZING"), None] {
            assert_eq!(
                resolver
                    .find_hooks(HookPoint::PostActions, "BESCHIKKING", dt, "BESLUIT")
                    .len(),
                1,
                "unfiltered hook should fire for decision_type {dt:?}"
            );
        }

        // Stage still filters.
        assert_eq!(
            resolver
                .find_hooks(HookPoint::PostActions, "BESCHIKKING", None, "BEKENDMAKING")
                .len(),
            0
        );
    }

    // -------------------------------------------------------------------------
    // Procedure lookup
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_procedure_by_default_and_by_id() {
        let mut resolver = RuleResolver::new();
        resolver
            .load_from_yaml(&make_law_with_procedure(
                "procedurewet",
                "BESCHIKKING",
                "beschikking",
            ))
            .unwrap();

        let by_default = resolver.find_procedure("BESCHIKKING", None).unwrap();
        assert_eq!(by_default.id, "beschikking");
        assert_eq!(by_default.stages.len(), 2);

        let by_id = resolver
            .find_procedure("BESCHIKKING", Some("beschikking"))
            .unwrap();
        assert_eq!(by_id.id, "beschikking");

        // Unknown procedure id, and unknown legal character.
        assert!(resolver
            .find_procedure("BESCHIKKING", Some("nope"))
            .is_none());
        assert!(resolver.find_procedure("ONBEKEND", None).is_none());
    }

    // -------------------------------------------------------------------------
    // Override target validation
    // -------------------------------------------------------------------------

    /// A law without a `valid_from` is in force on any date; the reason then
    /// names no version date rather than an empty one, and with nothing
    /// redeclared it adds no clause about another article.
    #[test]
    fn test_missing_article_reason_without_valid_from() {
        let law = ArticleBasedLaw::from_yaml_str(make_test_law()).unwrap();
        assert_eq!(
            missing_article_reason(&law, "9", "a hook on the same hook point", |_| false),
            "the version of test_law in force on this date has no article 9"
        );
        assert_eq!(
            missing_article_reason(&law, "9", "a hook on the same hook point", |_| true),
            "the version of test_law in force on this date has no article 9; article 1 of \
             that version declares a hook on the same hook point"
        );

        let two = ArticleBasedLaw::from_yaml_str(
            r#"
$id: twee_artikelen
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
articles:
  - number: '4'
    text: Een
  - number: '5'
    text: Twee
"#,
        )
        .unwrap();
        assert_eq!(
            missing_article_reason(&two, "1", "an override of the same output", |_| true),
            "the version of twee_artikelen in force on this date (valid_from 2025-01-01) \
             has no article 1; article 4, 5 of that version declares an override of the \
             same output"
        );
    }

    #[test]
    fn test_validate_override_targets_accepts_a_resolvable_override() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "bezwaartermijn_weken",
            ))
            .unwrap();

        assert_eq!(resolver.validate_override_targets(), Vec::<String>::new());
    }

    #[test]
    fn test_validate_override_targets_reports_missing_law() {
        let mut resolver = RuleResolver::new();
        // The target law 'doelwet' is never loaded.
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "bezwaartermijn_weken",
            ))
            .unwrap();

        let errors = resolver.validate_override_targets();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("non-existent law 'doelwet'"),
            "unexpected error: {}",
            errors[0]
        );
        assert!(errors[0].contains("afwijkingswet:69"));
    }

    #[test]
    fn test_validate_override_targets_reports_missing_article() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        // Overrides article '99', which 'doelwet' does not have.
        let yaml = r#"
$id: afwijkingswet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '69'
    text: In afwijking van artikel 99 van de doelwet
    machine_readable:
      overrides:
        - law: doelwet
          article: '99'
          output: bezwaartermijn_weken
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 4
"#;
        resolver.load_from_yaml(yaml).unwrap();

        let errors = resolver.validate_override_targets();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("non-existent article '99'"),
            "unexpected error: {}",
            errors[0]
        );
    }

    #[test]
    fn test_validate_override_targets_reports_missing_output() {
        let mut resolver = RuleResolver::new();
        resolver.load_from_yaml(make_override_target()).unwrap();
        // The article exists, but does not produce 'beroepstermijn_weken'.
        resolver
            .load_from_yaml(&make_law_with_override(
                "afwijkingswet",
                "beroepstermijn_weken",
            ))
            .unwrap();

        let errors = resolver.validate_override_targets();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("non-existent output 'beroepstermijn_weken'"),
            "unexpected error: {}",
            errors[0]
        );
        assert!(errors[0].contains("doelwet:1"));
    }

    /// The word is the whole point of the sentence. Blank it out and the
    /// receipt reads "Not applied:  regeling_x article 1 would have applied
    /// to ...", which no longer says whether a hook, a lex specialis or an
    /// implementation was passed over — the distinction the note exists to
    /// carry. Asserting on the enum alone leaves the word unpinned.
    #[test]
    fn every_declaration_kind_names_itself_in_the_sentence() {
        for (kind, word) in [
            (DeclarationKind::Hook, "hook"),
            (DeclarationKind::Override, "override"),
            (DeclarationKind::Implementation, "implementation"),
        ] {
            assert_eq!(kind.as_str(), word);
            let note = DeclarationNotInForce {
                kind,
                law_id: "regeling_x".to_string(),
                article: "1".to_string(),
                subject: "output 'bedrag' of wet_y".to_string(),
                reason: "no version of it is in force yet on this date".to_string(),
            };
            assert_eq!(
                note.message(),
                format!(
                    "Not applied: {word} regeling_x article 1 would have applied to \
                     output 'bedrag' of wet_y, but no version of it is in force yet on \
                     this date"
                )
            );
        }
    }
}
