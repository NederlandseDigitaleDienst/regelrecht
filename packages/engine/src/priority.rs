//! Priority resolution for competing implementations.
//!
//! When multiple regulations implement the same open term, the engine
//! must pick a winner. This module provides the resolution logic:
//!
//! 1. **Lex superior**: Higher regulatory layers take precedence
//! 2. **Lex posterior**: Among equal layers, later effective dates win
//!
//! ## TODO: Resolve `#` internal references at load time
//!
//! Laws may declare `valid_from: '#datum_inwerkingtreding'` — an internal
//! reference to an article output. Currently, priority comparison rejects
//! these with an error. The proper fix is to resolve `#` references when
//! loading laws into the resolver, so that `valid_from` always contains a
//! concrete date by the time priority comparison runs.

use crate::article::ArticleBasedLaw;
use crate::error::EngineError;
use crate::types::RegulatoryLayer;

type Result<T> = std::result::Result<T, EngineError>;

/// Returns the priority rank for a regulatory layer (lower = higher authority).
///
/// The hierarchy follows the Dutch legal system:
/// - International/EU law at the top
/// - Constitution
/// - Formal law
/// - Delegated legislation (AMvB, ministerial regulation)
/// - Policy rules and local ordinances at the bottom
#[must_use]
pub fn layer_rank(layer: &RegulatoryLayer) -> u8 {
    match layer {
        RegulatoryLayer::Verdrag => 0,
        RegulatoryLayer::EuVerordening => 1,
        RegulatoryLayer::EuRichtlijn => 2,
        RegulatoryLayer::Grondwet => 3,
        RegulatoryLayer::Wet => 4,
        RegulatoryLayer::KoninklijkBesluit => 5,
        RegulatoryLayer::Amvb => 6,
        RegulatoryLayer::MinisterieleRegeling => 7,
        RegulatoryLayer::ProvincialeVerordening => 8,
        // Waterschappen and gemeenten have coequal constitutional status (Grondwet ch. 7)
        // but govern non-overlapping domains. Scope filtering (waterschap_code vs
        // gemeente_code) prevents lex superior conflicts between them in practice.
        RegulatoryLayer::WaterschapsVerordening => 9,
        RegulatoryLayer::GemeentelijkeVerordening => 10,
        RegulatoryLayer::Beleidsregel => 11,
        RegulatoryLayer::Uitvoeringsbeleid => 12,
        // No regulation (RFC-048): a reduction fills no open term, the loader
        // refuses one that tries, so it never meets a candidate to rank.
        RegulatoryLayer::Reductie => 13,
    }
}

/// A candidate implementation with its source law and article number.
pub struct Candidate<'a> {
    pub law: &'a ArticleBasedLaw,
    pub article_number: String,
}

/// Validate that a law's `valid_from` is a concrete YYYY-MM-DD date suitable
/// for lex posterior comparison. Rejects missing dates and unresolved `#`
/// internal references.
fn validate_date_for_comparison<'a>(
    law_id: &str,
    valid_from: &'a Option<String>,
) -> Result<&'a str> {
    let date = valid_from.as_deref().ok_or_else(|| {
        EngineError::ResolutionError(format!(
            "Cannot resolve priority: law '{law_id}' has no valid_from date — \
             lex posterior comparison requires valid_from on all candidates"
        ))
    })?;

    if date.starts_with('#') {
        return Err(EngineError::ResolutionError(format!(
            "Cannot resolve priority: law '{law_id}' has unresolved valid_from \
             reference '{date}' — internal references must be resolved before \
             priority comparison"
        )));
    }

    Ok(date)
}

/// Compare two laws by priority: lex superior (layer rank), then lex posterior (valid_from).
///
/// Returns `Ordering::Greater` if `a` outranks `b`, `Ordering::Less` if `b` outranks `a`.
/// Returns `Err` if both have the same layer and date (ambiguous).
pub fn compare_law_priority(
    a: &ArticleBasedLaw,
    b: &ArticleBasedLaw,
) -> Result<std::cmp::Ordering> {
    let a_rank = layer_rank(&a.regulatory_layer);
    let b_rank = layer_rank(&b.regulatory_layer);

    if a_rank < b_rank {
        return Ok(std::cmp::Ordering::Greater); // a has higher authority
    }
    if a_rank > b_rank {
        return Ok(std::cmp::Ordering::Less); // b has higher authority
    }

    // Same layer: compare valid_from dates (lex posterior)
    let a_date = validate_date_for_comparison(&a.id, &a.valid_from)?;
    let b_date = validate_date_for_comparison(&b.id, &b.valid_from)?;

    if a_date > b_date {
        Ok(std::cmp::Ordering::Greater)
    } else if a_date < b_date {
        Ok(std::cmp::Ordering::Less)
    } else {
        Err(EngineError::ResolutionError(format!(
            "Ambiguous priority: '{}' and '{}' both have regulatory layer {:?} and valid_from '{}' \
             — cannot determine winner",
            a.id, b.id, a.regulatory_layer, a_date
        )))
    }
}

/// Pick the winning candidate from a list of implementations.
///
/// Resolution rules:
/// 1. Lex superior: the candidate from the highest regulatory layer wins
/// 2. Lex posterior: among candidates at the same layer, the one with the
///    latest `valid_from` date wins
///
/// Returns `None` if the candidate list is empty.
/// Returns `Some((winner, reason))` with a human-readable reason string.
/// Returns `Err` if two candidates have the same layer and date (ambiguous).
pub fn resolve_candidate<'a>(
    candidates: &[Candidate<'a>],
) -> Result<Option<(&'a ArticleBasedLaw, String)>> {
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut best = &candidates[0];
    let mut reason = format!("only candidate ({})", best.law.id);

    for candidate in &candidates[1..] {
        match compare_law_priority(candidate.law, best.law)? {
            std::cmp::Ordering::Greater => {
                // Build the reason before moving `best`: it must describe the
                // loser's layer and date, not repeat the winner's.
                let best_rank = layer_rank(&best.law.regulatory_layer);
                let cand_rank = layer_rank(&candidate.law.regulatory_layer);
                reason = if cand_rank < best_rank {
                    format!(
                        "lex superior: {} ({:?}) outranks {} ({:?})",
                        candidate.law.id,
                        candidate.law.regulatory_layer,
                        best.law.id,
                        best.law.regulatory_layer,
                    )
                } else {
                    format!(
                        "lex posterior: {} (valid_from {}) is newer than {} (valid_from {})",
                        candidate.law.id,
                        candidate.law.valid_from.as_deref().unwrap_or("?"),
                        best.law.id,
                        best.law.valid_from.as_deref().unwrap_or("?"),
                    )
                };
                best = candidate;
            }
            std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
                // Equal is unreachable: compare_law_priority returns Err for ambiguous
            }
        }
    }

    Ok(Some((best.law, reason)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::article::LawLoad;

    #[test]
    fn test_layer_priority_ordering() {
        // International > Constitution > Law > AMvB > Ministerial > Policy
        assert!(layer_rank(&RegulatoryLayer::Verdrag) < layer_rank(&RegulatoryLayer::Grondwet));
        assert!(layer_rank(&RegulatoryLayer::Grondwet) < layer_rank(&RegulatoryLayer::Wet));
        assert!(layer_rank(&RegulatoryLayer::Wet) < layer_rank(&RegulatoryLayer::Amvb));
        assert!(
            layer_rank(&RegulatoryLayer::Amvb) < layer_rank(&RegulatoryLayer::MinisterieleRegeling)
        );
        assert!(
            layer_rank(&RegulatoryLayer::MinisterieleRegeling)
                < layer_rank(&RegulatoryLayer::Beleidsregel)
        );
        assert!(
            layer_rank(&RegulatoryLayer::ProvincialeVerordening)
                < layer_rank(&RegulatoryLayer::WaterschapsVerordening)
        );
        assert!(
            layer_rank(&RegulatoryLayer::WaterschapsVerordening)
                < layer_rank(&RegulatoryLayer::GemeentelijkeVerordening)
        );
    }

    #[test]
    fn test_eu_law_outranks_national() {
        assert!(layer_rank(&RegulatoryLayer::EuVerordening) < layer_rank(&RegulatoryLayer::Wet));
        assert!(layer_rank(&RegulatoryLayer::EuRichtlijn) < layer_rank(&RegulatoryLayer::Wet));
    }

    /// A one-article law; only the fields priority looks at vary.
    fn law(id: &str, layer: &str, valid_from: Option<&str>) -> ArticleBasedLaw {
        let valid_from = valid_from.map_or(String::new(), |d| format!("valid_from: '{d}'\n"));
        ArticleBasedLaw::from_yaml_str(&format!(
            "$id: {id}\nregulatory_layer: {layer}\npublication_date: '2025-01-01'\n\
             {valid_from}articles:\n  - number: '1'\n    text: Test\n"
        ))
        .unwrap()
    }

    fn candidates<'a>(laws: &[&'a ArticleBasedLaw]) -> Vec<Candidate<'a>> {
        laws.iter()
            .map(|law| Candidate {
                law,
                article_number: "1".to_string(),
            })
            .collect()
    }

    const REGELING: &str = "MINISTERIELE_REGELING";

    #[test]
    fn test_resolve_candidate_empty() {
        let candidates: Vec<Candidate> = vec![];
        assert!(resolve_candidate(&candidates).unwrap().is_none());
    }

    #[test]
    fn test_resolve_candidate_single() {
        let law = law("test_regulation", REGELING, None);

        let (winner, reason) = resolve_candidate(&candidates(&[&law])).unwrap().unwrap();
        assert_eq!(winner.id, "test_regulation");
        assert_eq!(reason, "only candidate (test_regulation)");
    }

    #[test]
    fn test_resolve_candidate_lex_superior() {
        let wet = law("higher_law", "WET", None);
        let regeling = law("lower_regulation", REGELING, None);

        let (winner, reason) = resolve_candidate(&candidates(&[&regeling, &wet]))
            .unwrap()
            .unwrap();
        assert_eq!(winner.id, "higher_law");
        // The full sentence matters: the loser's layer must be the loser's,
        // not a repeat of the winner's — that distinction is what lex
        // superior is about.
        assert_eq!(
            reason,
            "lex superior: higher_law (Wet) outranks lower_regulation (MinisterieleRegeling)"
        );
    }

    #[test]
    fn test_resolve_candidate_lex_posterior() {
        let older = law("older_regulation", REGELING, Some("2024-01-01"));
        let newer = law("newer_regulation", REGELING, Some("2025-01-01"));

        let (winner, reason) = resolve_candidate(&candidates(&[&older, &newer]))
            .unwrap()
            .unwrap();
        assert_eq!(winner.id, "newer_regulation");
        // Full-sentence assert: the loser's date must be the loser's own
        // valid_from, not a repeat of the winner's.
        assert_eq!(
            reason,
            "lex posterior: newer_regulation (valid_from 2025-01-01) is newer than \
             older_regulation (valid_from 2024-01-01)"
        );
    }

    /// Lex superior beats lex posterior: a lower layer loses even when its
    /// `valid_from` is later than that of the higher layer.
    #[test]
    fn test_compare_lower_layer_loses_despite_newer_date() {
        let wet = law("higher_law", "WET", Some("2024-01-01"));
        let regeling = law("lower_regulation", REGELING, Some("2025-01-01"));

        assert_eq!(
            compare_law_priority(&regeling, &wet).unwrap(),
            std::cmp::Ordering::Less,
            "the ministerial regulation must lose to the formal law, even though it is newer"
        );
        assert_eq!(
            compare_law_priority(&wet, &regeling).unwrap(),
            std::cmp::Ordering::Greater,
            "the formal law must win regardless of argument order"
        );
    }

    /// Lex posterior in both directions: the older law loses to the newer one.
    #[test]
    fn test_compare_older_date_loses_at_same_layer() {
        let older = law("older_regulation", REGELING, Some("2024-01-01"));
        let newer = law("newer_regulation", REGELING, Some("2025-01-01"));

        assert_eq!(
            compare_law_priority(&older, &newer).unwrap(),
            std::cmp::Ordering::Less,
            "the older regulation must lose"
        );
        assert_eq!(
            compare_law_priority(&newer, &older).unwrap(),
            std::cmp::Ordering::Greater,
            "the newer regulation must win"
        );
    }

    /// Same layer plus same `valid_from` has no winner: the engine must refuse
    /// to pick one rather than silently preferring an argument position.
    #[test]
    fn test_compare_same_layer_and_date_is_ambiguous() {
        let one = law("regulation_one", REGELING, Some("2025-01-01"));
        let two = law("regulation_two", REGELING, Some("2025-01-01"));

        for (a, b) in [(&one, &two), (&two, &one)] {
            let err = compare_law_priority(a, b).expect_err(
                "two regulations at the same layer with the same valid_from must not resolve",
            );
            assert!(
                err.to_string().contains("Ambiguous priority"),
                "Expected an ambiguity error, got: {err}"
            );
        }
    }

    /// The same ambiguity must surface through `resolve_candidate`, so a caller
    /// never gets an arbitrary winner from an unresolvable pair.
    #[test]
    fn test_resolve_candidate_ambiguous_is_error() {
        let one = law("regulation_one", REGELING, Some("2025-01-01"));
        let two = law("regulation_two", REGELING, Some("2025-01-01"));

        let err = resolve_candidate(&candidates(&[&one, &two])).unwrap_err();
        assert!(
            err.to_string().contains("Ambiguous priority"),
            "Expected an ambiguity error, got: {err}"
        );
    }

    /// The winner must not depend on candidate order: the newer regulation wins
    /// even when it is already the incumbent best.
    #[test]
    fn test_resolve_candidate_lex_posterior_newest_first() {
        let newer = law("newer_regulation", REGELING, Some("2025-01-01"));
        let older = law("older_regulation", REGELING, Some("2024-01-01"));

        let (winner, reason) = resolve_candidate(&candidates(&[&newer, &older]))
            .unwrap()
            .unwrap();
        assert_eq!(winner.id, "newer_regulation");
        // The incumbent kept its seat, so no comparison sentence was written.
        assert_eq!(reason, "only candidate (newer_regulation)");
    }

    #[test]
    fn test_resolve_candidate_missing_valid_from_is_error() {
        let without_date = law("no_date_regulation", REGELING, None);
        let with_date = law("dated_regulation", REGELING, Some("2025-01-01"));

        let err = resolve_candidate(&candidates(&[&without_date, &with_date])).unwrap_err();
        assert!(
            err.to_string().contains("no valid_from date"),
            "Expected error about missing valid_from, got: {}",
            err
        );
    }

    #[test]
    fn test_resolve_candidate_unresolved_reference_is_error() {
        let with_ref = law("ref_regulation", REGELING, Some("#datum_inwerkingtreding"));
        let with_date = law("dated_regulation", REGELING, Some("2025-01-01"));

        let err = resolve_candidate(&candidates(&[&with_ref, &with_date])).unwrap_err();
        assert!(
            err.to_string().contains("unresolved valid_from reference"),
            "Expected error about unresolved reference, got: {}",
            err
        );
    }
}
