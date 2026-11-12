//! Data types for note resolution.
//!
//! These mirror the W3C Web Annotation `TextQuoteSelector` and the
//! `regelrecht:hint` performance extension defined in RFC-005.

use serde::{Deserialize, Serialize};

/// A W3C Web Annotation `TextQuoteSelector`.
///
/// Selects text by an exact quote plus optional surrounding context. The
/// prefix/suffix disambiguate when the exact text occurs more than once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextQuoteSelector {
    /// The exact text to locate.
    pub exact: String,
    /// Text expected immediately before `exact` (for disambiguation).
    #[serde(default)]
    pub prefix: String,
    /// Text expected immediately after `exact` (for disambiguation).
    #[serde(default)]
    pub suffix: String,
    /// Optional, non-authoritative performance hint (`regelrecht:hint`).
    #[serde(default, rename = "regelrecht:hint")]
    pub hint: Option<SelectorHint>,
}

/// Performance hint: where to look first.
///
/// Parsed from a `regelrecht:hint` CssSelector (`article[number='N']`)
/// optionally refined by a TextPositionSelector. The hint is never
/// authoritative: if the text is not found at the hinted location, the whole
/// law is searched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "HintWire")]
pub struct SelectorHint {
    /// Article number to search first (e.g. "2", "4a").
    pub article_number: String,
    /// Optional character offset where the match should begin (article-relative).
    pub start: Option<usize>,
    /// Optional character offset where the match should end (article-relative).
    pub end: Option<usize>,
}

/// Wire format of a `regelrecht:hint` as it appears in YAML/JSON.
///
/// Deserialised then flattened into [`SelectorHint`]. The article number is
/// extracted from a `CssSelector` value of the form `article[number='N']`.
#[derive(Debug, Clone, Deserialize)]
struct HintWire {
    #[serde(default)]
    value: String,
    #[serde(default, rename = "refinedBy")]
    refined_by: Option<RefinedBy>,
}

#[derive(Debug, Clone, Deserialize)]
struct RefinedBy {
    #[serde(default)]
    start: Option<usize>,
    #[serde(default)]
    end: Option<usize>,
}

impl From<HintWire> for SelectorHint {
    fn from(wire: HintWire) -> Self {
        let article_number = parse_article_number(&wire.value).unwrap_or_default();
        let (start, end) = wire
            .refined_by
            .map(|r| (r.start, r.end))
            .unwrap_or((None, None));
        SelectorHint {
            article_number,
            start,
            end,
        }
    }
}

/// Extract `N` from a CssSelector value like `article[number='N']`.
fn parse_article_number(css_value: &str) -> Option<String> {
    let after = css_value.split("number=").nth(1)?;
    let trimmed = after.trim_start_matches(['\'', '"']);
    let end = trimmed.find(['\'', '"'])?;
    Some(trimmed[..end].to_string())
}

/// Whether a selector could be located, and how unambiguously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchStatus {
    /// Exactly one match (exact, or a clearly-best fuzzy match).
    Found,
    /// No match above the fuzzy threshold; the note is orphaned.
    Orphaned,
    /// Multiple equally-good matches; the note is ambiguous.
    Ambiguous,
    /// The fuzzy search was skipped or cut short by a resource bound
    /// (see [`SkipReason`]). Unlike [`Orphaned`](Self::Orphaned) this does
    /// **not** assert the text is absent, and unlike
    /// [`Found`](Self::Found) it claims no uniqueness: any candidates found
    /// before the cut-off ride along in `matches`, but text beyond the
    /// cut-off was never searched.
    Skipped,
}

/// Why a fuzzy search was skipped or cut short (the bounds live in
/// [`crate::config`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// The quote exceeds `MAX_FUZZY_QUOTE_CHARS`; the fuzzy scan never ran.
    /// Shortening the quote is the fix.
    QuoteTooLong,
    /// The scan or scoring budget ran out partway through the law; the text
    /// beyond the cut-off was not searched. The quote length is not the
    /// cause, so "shorten the quote" would be misdirected advice here.
    SearchBudget,
}

/// A single located span in the law text.
///
/// `start`/`end` are **`char` offsets** (Unicode scalar values) into the
/// article text, article-relative, not byte offsets and not UTF-16 code
/// units. A JS consumer slicing the text (e.g. to build a DOM Range) must map
/// these to UTF-16 indices itself; they will differ for any text containing
/// non-BMP characters, and the convention must match the offsets the editor
/// writes into a `regelrecht:hint`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMatch {
    /// Article the match was found in (empty when resolving raw text).
    #[serde(default)]
    pub article_number: String,
    /// `char` offset (article-relative) where the match begins.
    pub start: usize,
    /// `char` offset (article-relative) where the match ends.
    pub end: usize,
    /// Confidence: `1.0` for an exact match, `< 1.0` for a fuzzy match.
    pub confidence: f64,
    /// The actual text that was matched.
    pub matched_text: String,
}

/// The outcome of resolving a [`TextQuoteSelector`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchResult {
    /// Overall resolution status.
    pub status: MatchStatus,
    /// Located spans. One element when `Found`, several when `Ambiguous`,
    /// empty when `Orphaned`. When `Skipped`: any candidates found before
    /// the search was cut short — never a claim of uniqueness.
    pub matches: Vec<TextMatch>,
    /// Why the search was skipped; present exactly when `status` is
    /// `Skipped`. On the wire: `"quote_too_long"` or `"search_budget"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<SkipReason>,
}

impl MatchResult {
    pub(crate) fn found(matches: Vec<TextMatch>) -> Self {
        Self {
            status: MatchStatus::Found,
            matches,
            skip_reason: None,
        }
    }

    pub(crate) fn orphaned() -> Self {
        Self {
            status: MatchStatus::Orphaned,
            matches: Vec::new(),
            skip_reason: None,
        }
    }

    pub(crate) fn ambiguous(matches: Vec<TextMatch>) -> Self {
        Self {
            status: MatchStatus::Ambiguous,
            matches,
            skip_reason: None,
        }
    }

    /// A search cut short by `reason`; `matches` carries whatever candidates
    /// were found before the cut-off (possibly none).
    pub(crate) fn skipped(reason: SkipReason, matches: Vec<TextMatch>) -> Self {
        Self {
            status: MatchStatus::Skipped,
            matches,
            skip_reason: Some(reason),
        }
    }

    /// True when exactly one location was found.
    pub fn is_found(&self) -> bool {
        self.status == MatchStatus::Found
    }

    /// True when no location was found.
    pub fn is_orphaned(&self) -> bool {
        self.status == MatchStatus::Orphaned
    }

    /// True when multiple equally-good locations were found.
    pub fn is_ambiguous(&self) -> bool {
        self.status == MatchStatus::Ambiguous
    }

    /// True when the fuzzy search was skipped or truncated by a resource
    /// bound before any match was found.
    pub fn is_skipped(&self) -> bool {
        self.status == MatchStatus::Skipped
    }

    /// The single match, when [`is_found`](Self::is_found).
    pub fn single(&self) -> Option<&TextMatch> {
        if self.is_found() {
            self.matches.first()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_article_number_from_css_selector() {
        assert_eq!(
            parse_article_number("article[number='4a']").as_deref(),
            Some("4a")
        );
        assert_eq!(
            parse_article_number("article[number=\"2\"]").as_deref(),
            Some("2")
        );
        assert_eq!(parse_article_number("article").as_deref(), None);
    }

    #[test]
    fn deserialises_hint_from_w3c_shape() {
        let yaml = r#"
exact: zorgtoeslag
prefix: "op een "
suffix: " ter grootte"
regelrecht:hint:
  type: CssSelector
  value: "article[number='2']"
  refinedBy:
    type: TextPositionSelector
    start: 45
    end: 56
"#;
        let sel: TextQuoteSelector = serde_yaml_ng::from_str(yaml).unwrap();
        let hint = sel.hint.expect("hint present");
        assert_eq!(hint.article_number, "2");
        assert_eq!(hint.start, Some(45));
        assert_eq!(hint.end, Some(56));
    }

    fn a_match() -> TextMatch {
        TextMatch {
            article_number: "2".to_string(),
            start: 0,
            end: 4,
            confidence: 1.0,
            matched_text: "test".to_string(),
        }
    }

    /// The three predicates classify a result into exactly one bucket: a
    /// consumer branching on `is_orphaned()`/`is_ambiguous()` (as
    /// `validate_annotations` does) must not report a resolved note as
    /// unresolvable.
    #[test]
    fn status_predicates_are_mutually_exclusive() {
        let found = MatchResult::found(vec![a_match()]);
        assert!(found.is_found());
        assert!(!found.is_orphaned());
        assert!(!found.is_ambiguous());

        let orphaned = MatchResult::orphaned();
        assert!(orphaned.is_orphaned());
        assert!(!orphaned.is_found());
        assert!(!orphaned.is_ambiguous());

        let ambiguous = MatchResult::ambiguous(vec![a_match(), a_match()]);
        assert!(ambiguous.is_ambiguous());
        assert!(!ambiguous.is_found());
        assert!(!ambiguous.is_orphaned());
        assert!(!ambiguous.is_skipped());

        let skipped = MatchResult::skipped(SkipReason::QuoteTooLong, Vec::new());
        assert!(skipped.is_skipped());
        assert!(!skipped.is_found());
        assert!(
            !skipped.is_orphaned(),
            "a skipped search must not present itself as 'searched and absent'"
        );
        assert!(!skipped.is_ambiguous());
    }

    /// The wire values the frontend branches on: `status: 'skipped'` plus a
    /// `skip_reason` naming the bound that was hit.
    #[test]
    fn skipped_serialises_status_and_reason() {
        let json =
            serde_json::to_string(&MatchResult::skipped(SkipReason::QuoteTooLong, Vec::new()))
                .unwrap();
        assert!(json.contains("\"status\":\"skipped\""), "{json}");
        assert!(
            json.contains("\"skip_reason\":\"quote_too_long\""),
            "{json}"
        );

        let json =
            serde_json::to_string(&MatchResult::skipped(SkipReason::SearchBudget, Vec::new()))
                .unwrap();
        assert!(json.contains("\"skip_reason\":\"search_budget\""), "{json}");

        // Other statuses carry no reason field at all: absence must mean
        // "was not skipped", not "reason unknown".
        let json = serde_json::to_string(&MatchResult::orphaned()).unwrap();
        assert!(!json.contains("skip_reason"), "{json}");
    }

    /// A skipped search may carry the candidates found before the cut-off,
    /// but a consumer asking for "the single match" must not get one: a
    /// candidate is not a claim of uniqueness over text never searched.
    #[test]
    fn a_skipped_result_hands_out_no_single_match() {
        let skipped = MatchResult::skipped(SkipReason::SearchBudget, vec![a_match()]);
        assert_eq!(skipped.matches.len(), 1);
        assert!(skipped.single().is_none());
    }

    /// `single()` is only meaningful for a `Found` result; an ambiguous result
    /// carries several candidates and must not hand out the first one as if it
    /// were the answer.
    #[test]
    fn single_only_yields_a_match_when_found() {
        assert_eq!(
            MatchResult::found(vec![a_match()]).single(),
            Some(&a_match())
        );
        assert!(MatchResult::orphaned().single().is_none());
        assert!(
            MatchResult::ambiguous(vec![a_match(), a_match()])
                .single()
                .is_none()
        );
    }

    #[test]
    fn selector_without_hint() {
        let yaml = r#"
exact: verzekerde
prefix: "de "
"#;
        let sel: TextQuoteSelector = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(sel.exact, "verzekerde");
        assert_eq!(sel.prefix, "de ");
        assert_eq!(sel.suffix, "");
        assert!(sel.hint.is_none());
    }
}
