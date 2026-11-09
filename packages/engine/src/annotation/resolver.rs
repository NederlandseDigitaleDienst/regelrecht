//! TextQuoteSelector resolution algorithm (RFC-005, RFC-018).
//!
//! Resolution order:
//! 1. Exact match: locate `prefix + exact + suffix` as a substring, with
//!    whitespace-tolerant prefix/suffix checks.
//! 2. Fuzzy match: a sliding window over the text scored
//!    `exact*0.5 + prefix*0.25 + suffix*0.25`, keeping candidates at or above
//!    the threshold (0.7).
//! 3. One match (or a clear winner) is [`MatchStatus::Found`], several
//!    equally-good are [`MatchStatus::Ambiguous`], none is
//!    [`MatchStatus::Orphaned`].
//! 4. The fuzzy scan is budgeted (see [`FuzzyBudget`] and the
//!    `MAX_FUZZY_*` limits in [`crate::config`]): it is cubic in the quote
//!    length and runs synchronously on the browser main thread via WASM, so
//!    an unbounded scan freezes the editor for minutes. When a bound cuts the
//!    search short before anything was found the result is
//!    [`MatchStatus::Skipped`] — "this was not searched" — never a silent
//!    `Orphaned`.
//!
//! [`MatchStatus::Found`]: crate::annotation::MatchStatus::Found
//! [`MatchStatus::Ambiguous`]: crate::annotation::MatchStatus::Ambiguous
//! [`MatchStatus::Orphaned`]: crate::annotation::MatchStatus::Orphaned
//! [`MatchStatus::Skipped`]: crate::annotation::MatchStatus::Skipped
//!
//! A `regelrecht:hint` on the selector does not participate in resolution.
//! RFC-018 is explicit that article numbers are non-authoritative and that
//! the note follows the *text*; any hint-driven shortcut can hide a competing
//! occurrence elsewhere in the law, silently turning a genuine
//! [`MatchStatus::Ambiguous`] (which the editor escalates to a human) into a
//! confident [`MatchStatus::Found`] on a possibly renumbered article. Ruling
//! out that competitor requires the full scan anyway, so the hint cannot
//! shorten the search without being allowed to override it.
//!
//! Structurally ported from the Python proof-of-concept on the
//! `feature/annotation-resolver` branch (resolution order, dedup, tiebreak
//! margin; the PoC's hint fast path was dropped, see above). The scoring
//! function differs deliberately: the
//! PoC used `difflib.SequenceMatcher.ratio()` (Ratcliff-Obershelp); this uses
//! normalised Levenshtein per RFC-018, which is harsher on block moves. The
//! two disagree near the 0.7 threshold, so a boundary BDD scenario guards the
//! behaviour. The Rust port is the single source of truth (it also runs in
//! the browser via WASM).
//!
//! All offsets ([`TextMatch::start`]/`end`, [`SelectorHint`] positions) are
//! **`char` offsets** (Unicode scalar values), not byte or UTF-16 code-unit
//! offsets. JS consumers indexing the law text must account for this; see the
//! WASM binding docs.

use crate::annotation::types::{MatchResult, SkipReason, TextMatch, TextQuoteSelector};
use crate::article::Article;
use crate::config::{MAX_FUZZY_QUOTE_CHARS, MAX_FUZZY_SCAN_CHARS, MAX_FUZZY_SCORED_WINDOWS};
use std::collections::HashSet;

/// Default minimum weighted score for a fuzzy match to count.
pub const DEFAULT_FUZZY_THRESHOLD: f64 = 0.7;

/// Allowed window-length variation around `exact.len()` when scanning for
/// fuzzy candidates (30%).
const WINDOW_TOLERANCE: f64 = 0.3;

/// Margin by which the best fuzzy match must beat the second-best to be
/// treated as unambiguous.
const TIEBREAK_MARGIN: f64 = 0.1;

/// Work budget for one resolve call's fuzzy scanning.
///
/// The sliding-window scan is cubic in the quote length and runs
/// synchronously on the browser main thread via WASM, so it needs a hard
/// upper bound like every other scan in this engine. The limits live in
/// [`crate::config`] next to the YAML/array/recursion budgets; the exact-match
/// pass is linear and stays unbudgeted. Tests construct smaller budgets to
/// exercise the truncation paths.
#[derive(Debug, Clone, Copy)]
struct FuzzyBudget {
    /// Quote length (chars) above which fuzzy matching does not run at all.
    max_quote_chars: usize,
    /// Law text (chars) this resolve may still fuzzily scan.
    scan_chars_left: usize,
    /// Candidate windows this resolve may still score (three Levenshtein
    /// computations each).
    scored_windows_left: usize,
    /// Set (with the bound that was hit) when any part of the search was
    /// skipped; turns the outcome into [`MatchResult::skipped`] so "not
    /// (fully) searched" stays distinguishable from "searched and absent".
    /// The first cause wins: a later, different bound does not rewrite why
    /// the search degraded.
    skipped: Option<SkipReason>,
}

impl Default for FuzzyBudget {
    fn default() -> Self {
        Self {
            max_quote_chars: MAX_FUZZY_QUOTE_CHARS,
            scan_chars_left: MAX_FUZZY_SCAN_CHARS,
            scored_windows_left: MAX_FUZZY_SCORED_WINDOWS,
            skipped: None,
        }
    }
}

impl FuzzyBudget {
    /// Record that a bound was hit, keeping the first cause.
    fn mark_skipped(&mut self, reason: SkipReason) {
        self.skipped.get_or_insert(reason);
    }
}

/// Resolve `selector` against the articles of a law.
///
/// Article numbers on the returned matches identify where the text was found.
/// A present hint never overrides the full-text search: the result is the
/// same with or without it (see the module docs).
pub fn resolve(selector: &TextQuoteSelector, articles: &[Article]) -> MatchResult {
    resolve_with_threshold(selector, articles, DEFAULT_FUZZY_THRESHOLD)
}

/// [`resolve`] with an explicit fuzzy threshold (used by tests).
pub fn resolve_with_threshold(
    selector: &TextQuoteSelector,
    articles: &[Article],
    threshold: f64,
) -> MatchResult {
    // Exact match across all articles.
    let mut exact: Vec<TextMatch> = Vec::new();
    for article in articles {
        for mut m in find_exact_matches(&article.text, selector) {
            m.article_number = article.number.clone();
            exact.push(m);
        }
    }
    if !exact.is_empty() {
        return if exact.len() == 1 {
            MatchResult::found(exact)
        } else {
            MatchResult::ambiguous(exact)
        };
    }

    // Fuzzy match across all articles, within one shared work budget.
    let mut budget = FuzzyBudget::default();
    let mut fuzzy: Vec<TextMatch> = Vec::new();
    for article in articles {
        for mut m in find_fuzzy_matches(&article.text, selector, threshold, &mut budget) {
            m.article_number = article.number.clone();
            fuzzy.push(m);
        }
    }
    finalize_fuzzy(fuzzy, budget.skipped)
}

/// Resolve a selector against a single raw text body (no article context).
pub fn resolve_in_text(selector: &TextQuoteSelector, text: &str, threshold: f64) -> MatchResult {
    let exact = find_exact_matches(text, selector);
    if !exact.is_empty() {
        return if exact.len() == 1 {
            MatchResult::found(exact)
        } else {
            MatchResult::ambiguous(exact)
        };
    }
    let mut budget = FuzzyBudget::default();
    let fuzzy = find_fuzzy_matches(text, selector, threshold, &mut budget);
    finalize_fuzzy(fuzzy, budget.skipped)
}

/// Collapse fuzzy candidates into a final [`MatchResult`].
///
/// A cut-short search (`skipped` is set) is always `Skipped`, whatever was
/// found by then: `Found` would claim uniqueness over text that was never
/// searched, and an empty `Orphaned` would claim absence that was never
/// established. Candidates found before the cut-off ride along in the
/// result's `matches`. A complete search collapses as before: overlapping
/// spans deduplicated keeping the highest confidence; a single survivor, or
/// a clear winner (more than [`TIEBREAK_MARGIN`] ahead of the runner-up), is
/// `Found`; several equally-good are `Ambiguous`; none is `Orphaned`.
fn finalize_fuzzy(matches: Vec<TextMatch>, skipped: Option<SkipReason>) -> MatchResult {
    if let Some(reason) = skipped {
        return MatchResult::skipped(reason, deduplicate_overlapping(matches));
    }
    if matches.is_empty() {
        return MatchResult::orphaned();
    }
    let deduped = deduplicate_overlapping(matches);
    match clear_winner(&deduped) {
        Some(winner) => MatchResult::found(vec![winner]),
        None => MatchResult::ambiguous(deduped),
    }
}

/// The one candidate that stands out, if there is one.
///
/// A lone candidate wins by default; with several, the best has to beat the
/// runner-up by *more* than [`TIEBREAK_MARGIN`]. `None` means the field is
/// tied, which the caller reports as ambiguous. Candidates arrive sorted by
/// confidence descending (the caller dedupes first, which sorts).
fn clear_winner(deduped: &[TextMatch]) -> Option<TextMatch> {
    match deduped {
        [only] => Some(only.clone()),
        [best, second, ..] if best.confidence - second.confidence > TIEBREAK_MARGIN => {
            Some(best.clone())
        }
        _ => None,
    }
}

/// All exact occurrences of `exact` whose (whitespace-normalised) prefix and
/// suffix match. Offsets are in `char`s, not bytes.
fn find_exact_matches(text: &str, selector: &TextQuoteSelector) -> Vec<TextMatch> {
    let chars: Vec<char> = text.chars().collect();
    let exact: Vec<char> = selector.exact.chars().collect();
    if exact.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let mut from = 0usize;
    while from + exact.len() <= chars.len() {
        let Some(rel) = find_subslice(&chars[from..], &exact) else {
            break;
        };
        let pos = from + rel;
        let end = pos + exact.len();

        // Window is exactly `len + 1` chars flush against `exact`, so after
        // trimming it must *equal* the prefix/suffix. Equality (not
        // ends_with/starts_with) rejects word-internal false positives, e.g.
        // prefix "op een" must not be satisfied by "rop een". The +1 char of
        // slack plus trim() absorbs a single whitespace difference.
        let prefix_ok = selector.prefix.is_empty() || {
            let p_len = selector.prefix.chars().count();
            let p_start = pos.saturating_sub(p_len + 1);
            let actual: String = chars[p_start..pos].iter().collect();
            actual.trim() == selector.prefix.trim()
        };
        let suffix_ok = selector.suffix.is_empty() || {
            let s_len = selector.suffix.chars().count();
            let s_end = (end + s_len + 1).min(chars.len());
            let actual: String = chars[end..s_end].iter().collect();
            actual.trim() == selector.suffix.trim()
        };

        if prefix_ok && suffix_ok {
            matches.push(TextMatch {
                article_number: String::new(),
                start: pos,
                end,
                confidence: 1.0,
                matched_text: selector.exact.clone(),
            });
        }
        from = pos + 1;
    }
    matches
}

/// Fuzzy candidates at or above `threshold`, sorted by confidence descending.
///
/// Mirrors the Python proof-of-concept: collect sliding windows of
/// `len(exact) ± 30%` that share a significant word with `exact`, then score
/// each by weighted Levenshtein similarity. The scan draws on `budget`: a
/// quote over the length cap, an article that no longer fits the scan budget,
/// or running out of scoring budget all mark the budget skipped with the
/// bound that was hit (and the last one stops the scan), so the caller
/// reports the search as skipped rather than silently incomplete.
fn find_fuzzy_matches(
    text: &str,
    selector: &TextQuoteSelector,
    threshold: f64,
    budget: &mut FuzzyBudget,
) -> Vec<TextMatch> {
    let chars: Vec<char> = text.chars().collect();
    let exact_len = selector.exact.chars().count();
    if exact_len == 0 || chars.is_empty() {
        return Vec::new();
    }
    if exact_len > budget.max_quote_chars {
        budget.mark_skipped(SkipReason::QuoteTooLong);
        return Vec::new();
    }
    if chars.len() > budget.scan_chars_left {
        budget.mark_skipped(SkipReason::SearchBudget);
        return Vec::new();
    }
    budget.scan_chars_left -= chars.len();

    let tolerance = ((exact_len as f64) * WINDOW_TOLERANCE) as usize;
    let min_w = exact_len.saturating_sub(tolerance).max(1);
    let max_w = exact_len + tolerance;

    // Constant for the whole scan: compute once, not per window position.
    let index = WordIndex::new(&chars, &selector.exact);
    let prefix_len = selector.prefix.chars().count();
    let suffix_len = selector.suffix.chars().count();

    let mut matches: Vec<TextMatch> = Vec::new();
    'scan: for window in min_w..=max_w {
        if window > chars.len() {
            break;
        }
        let mut cursor = index.cursor(window);
        for i in 0..=(chars.len() - window) {
            if !cursor.shares_significant_content(i) {
                continue;
            }
            if budget.scored_windows_left == 0 {
                budget.mark_skipped(SkipReason::SearchBudget);
                break 'scan;
            }
            budget.scored_windows_left -= 1;

            let candidate: String = chars[i..i + window].iter().collect();
            let p_start = i.saturating_sub(prefix_len);
            let actual_prefix: String = chars[p_start..i].iter().collect();
            let s_end = (i + window + suffix_len).min(chars.len());
            let actual_suffix: String = chars[i + window..s_end].iter().collect();

            let exact_score = similarity(&selector.exact, &candidate);
            let prefix_score = if selector.prefix.is_empty() {
                1.0
            } else {
                similarity(&selector.prefix, &actual_prefix)
            };
            let suffix_score = if selector.suffix.is_empty() {
                1.0
            } else {
                similarity(&selector.suffix, &actual_suffix)
            };

            let weighted = exact_score * 0.5 + prefix_score * 0.25 + suffix_score * 0.25;
            if weighted >= threshold {
                matches.push(TextMatch {
                    article_number: String::new(),
                    start: i,
                    end: i + window,
                    confidence: weighted,
                    matched_text: candidate,
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches
}

/// Normalised Levenshtein similarity in `[0.0, 1.0]` (RFC-018).
fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    strsim::normalized_levenshtein(a, b)
}

/// Significant words (longer than 3 chars, lowercased) of a string.
///
/// Computed once for `exact` before the sliding-window scan; "significant"
/// excludes short function words (articles, prepositions) so the pre-filter
/// keys on content words. Words are kept as `Vec<char>` so the scan can look
/// up text slices without allocating.
fn significant_words(s: &str) -> HashSet<Vec<char>> {
    s.split_whitespace()
        .filter(|w| w.chars().count() > 3)
        .map(|w| w.chars().map(lower_char).collect())
        .collect()
}

/// Per-`char` lowercase (first scalar of the mapping), keeping offsets
/// aligned with the original text. Differs from `str::to_lowercase` only for
/// exotic expanding mappings (e.g. 'İ' → "i̇"), which do not occur in Dutch
/// legal text and would merely make the pre-filter slightly stricter there.
fn lower_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Pre-computed word index over one article's text for the fuzzy pre-filter:
/// does a window share a significant word with the quote?
///
/// Replaces a per-window `String` build plus `to_lowercase()`, whose
/// allocation floor alone was seconds per article even with every window
/// rejected. This lowercases the text once, records the non-whitespace runs
/// (words) with a per-run flag "this whole word is a significant quote word",
/// and then answers each window in O(1) amortised: interior words via prefix
/// sums over that flag, plus at most two hash lookups for the words truncated
/// at the window edges — the same tokens the old
/// `candidate.split_whitespace()` produced, so the filter's semantics are
/// unchanged.
struct WordIndex {
    /// The text, lowercased per char (offset-aligned with the original).
    lower: Vec<char>,
    /// Non-whitespace runs as `(start, end)`; sorted, non-overlapping.
    runs: Vec<(usize, usize)>,
    /// `match_prefix[k]` = number of runs in `runs[..k]` that are themselves
    /// a significant quote word.
    match_prefix: Vec<usize>,
    /// Significant (>3 chars, lowercased) words of the quote.
    exact_words: HashSet<Vec<char>>,
}

impl WordIndex {
    fn new(chars: &[char], exact: &str) -> Self {
        let lower: Vec<char> = chars.iter().map(|&c| lower_char(c)).collect();
        let exact_words = significant_words(exact);

        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut start: Option<usize> = None;
        for (i, &c) in chars.iter().enumerate() {
            match (c.is_whitespace(), start) {
                (false, None) => start = Some(i),
                (true, Some(s)) => {
                    runs.push((s, i));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            runs.push((s, chars.len()));
        }

        let mut match_prefix = Vec::with_capacity(runs.len() + 1);
        let mut total = 0usize;
        match_prefix.push(total);
        for &(s, e) in &runs {
            let hit = e - s > 3 && exact_words.contains(&lower[s..e]);
            total += usize::from(hit);
            match_prefix.push(total);
        }

        Self {
            lower,
            runs,
            match_prefix,
            exact_words,
        }
    }

    /// A cursor for scanning windows of one fixed size at non-decreasing
    /// positions (the shape of the sliding-window loop).
    fn cursor(&self, window: usize) -> WindowCursor<'_> {
        WindowCursor {
            index: self,
            window,
            a: 0,
            b: 0,
        }
    }

    /// Does the (possibly truncated) word at `lower[ts..te]` count as shared
    /// significant content? Same rule as the old per-window tokenisation.
    /// The >3-chars significance threshold needs no separate check here:
    /// `exact_words` only holds words longer than 3 chars, so a shorter
    /// token can never be contained in it.
    fn edge_token_matches(&self, ts: usize, te: usize) -> bool {
        debug_assert!(te > ts, "callers only pass non-empty edge tokens");
        self.exact_words.contains(&self.lower[ts..te])
    }
}

/// Sliding-window view over a [`WordIndex`] for one window size.
///
/// `a` is the first run starting at or after the window start, `b` the first
/// run ending past the window end; both only ever move forward, so a whole
/// pass over the text costs O(text + runs), not O(text × words-per-window).
struct WindowCursor<'a> {
    index: &'a WordIndex,
    window: usize,
    a: usize,
    b: usize,
}

impl WindowCursor<'_> {
    /// Cheap pre-filter: does the window starting at `i` share a significant
    /// word with the quote? Positions must be queried in non-decreasing
    /// order. Allocates nothing.
    fn shares_significant_content(&mut self, i: usize) -> bool {
        let j = i + self.window;
        let runs = &self.index.runs;
        while self.a < runs.len() && runs[self.a].0 < i {
            self.a += 1;
        }
        while self.b < runs.len() && runs[self.b].1 <= j {
            self.b += 1;
        }

        // Words lying entirely inside the window (`runs[a..b]`), counted via
        // prefix sums. When one run spans the whole window `b` sits before
        // `a`; the sums are monotone, so the comparison is false on its own
        // and needs no separate `b > a` guard.
        if self.index.match_prefix[self.b] > self.index.match_prefix[self.a] {
            return true;
        }

        // Word truncated at the left window edge: the run containing `i`. At
        // most one run can contain `i`, and by the pointer invariant it is
        // `runs[a-1]` (every run at or after `a` starts inside the window).
        if self.a > 0 {
            let (s, e) = runs[self.a - 1];
            debug_assert!(s < i, "runs[a-1] starts before the window");
            if e > i && self.index.edge_token_matches(i, e.min(j)) {
                return true;
            }
        }
        // Word truncated at the right window edge. When one run spans the
        // whole window it is the same run as above; don't test it twice.
        if self.b < runs.len() && (self.a == 0 || self.b != self.a - 1) {
            let (s, e) = runs[self.b];
            debug_assert!(e > j, "runs[b] ends past the window");
            if s < j && self.index.edge_token_matches(s.max(i), j) {
                return true;
            }
        }
        false
    }
}

/// Keep only the highest-confidence match for each overlapping region.
fn deduplicate_overlapping(mut matches: Vec<TextMatch>) -> Vec<TextMatch> {
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<TextMatch> = Vec::new();
    for m in matches {
        let overlaps = kept
            .iter()
            .any(|k| m.article_number == k.article_number && m.start < k.end && k.start < m.end);
        if !overlaps {
            kept.push(m);
        }
    }
    kept
}

/// First index of `needle` within `haystack`, comparing `char`s.
fn find_subslice(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::types::SelectorHint;

    fn article(number: &str, text: &str) -> Article {
        Article {
            number: number.to_string(),
            text: text.to_string(),
            url: None,
            placement: None,
            machine_readable: None,
            references: None,
        }
    }

    fn selector(exact: &str, prefix: &str, suffix: &str) -> TextQuoteSelector {
        TextQuoteSelector {
            exact: exact.to_string(),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
            hint: None,
        }
    }

    fn hinted(
        exact: &str,
        prefix: &str,
        suffix: &str,
        article_number: &str,
        span: Option<(usize, usize)>,
    ) -> TextQuoteSelector {
        let mut sel = selector(exact, prefix, suffix);
        sel.hint = Some(SelectorHint {
            article_number: article_number.to_string(),
            start: span.map(|(s, _)| s),
            end: span.map(|(_, e)| e),
        });
        sel
    }

    /// A candidate as the fuzzy scan produces them, for the collapsing helpers.
    fn candidate(article_number: &str, start: usize, end: usize, confidence: f64) -> TextMatch {
        TextMatch {
            article_number: article_number.to_string(),
            start,
            end,
            confidence,
            matched_text: String::new(),
        }
    }

    #[test]
    fn exact_match_single() {
        let arts = vec![article(
            "2",
            "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil",
        )];
        let sel = selector("zorgtoeslag", "op een ", " ter grootte");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "2");
        assert_eq!(r.single().unwrap().confidence, 1.0);
    }

    #[test]
    fn article_renumbered_still_resolves() {
        // Same text, different article number: content-addressed lookup wins.
        let arts = vec![
            article("1a", "Een nieuw ingevoegd artikel."),
            article(
                "4a",
                "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil",
            ),
        ];
        let sel = selector("zorgtoeslag", "op een ", " ter grootte");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "4a");
    }

    #[test]
    fn ambiguous_without_context() {
        let arts = vec![article(
            "2",
            "de verzekerde en de verzekerde en nog een verzekerde",
        )];
        let sel = selector("verzekerde", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous());
        assert!(r.matches.len() >= 2);
    }

    #[test]
    fn unique_with_context() {
        let arts = vec![article(
            "2",
            "de verzekerde betaalt; de verzekerde ontvangt; heeft de verzekerde aanspraak op zorgtoeslag",
        )];
        let sel = selector("verzekerde", "heeft de ", " aanspraak");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
    }

    #[test]
    fn prefix_rejects_word_internal_false_positive() {
        // "op een" must not be satisfied by the word-internal "...rop een"
        // (the bug an `ends_with` check would let through). Only the genuine
        // occurrence preceded by exactly "op een" should match.
        let text = "de stroop een zorgtoeslag; recht op een zorgtoeslag";
        let arts = vec![article("2", text)];
        let sel = selector("zorgtoeslag", "op een ", "");
        let r = resolve(&sel, &arts);
        assert!(
            r.is_found(),
            "expected only the genuine 'op een ' occurrence"
        );
        let m = r.single().unwrap();
        // The second "zorgtoeslag", not the one anchored via "stroop een".
        assert_eq!(m.start, text.rfind("zorgtoeslag").unwrap());
    }

    #[test]
    fn orphaned_when_text_removed() {
        let arts = vec![article("2", "Geheel andere tekst zonder de gezochte zin.")];
        let sel = selector(
            "zorgtoeslag ter grootte van dat verschil",
            "aanspraak op een ",
            "",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_orphaned());
    }

    #[test]
    fn fuzzy_match_on_minor_change() {
        // "aanspraak op een" -> "recht op een"; suffix slightly changed too.
        let arts = vec![article(
            "2",
            "heeft de verzekerde recht op een zorgtoeslag ter grootte van het verschil",
        )];
        let sel = selector(
            "aanspraak op een zorgtoeslag",
            "heeft de verzekerde ",
            " ter grootte van dat verschil",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "expected fuzzy match, got {:?}", r.status);
        assert!(r.single().unwrap().confidence < 1.0);
    }

    #[test]
    fn a_hint_does_not_change_a_unique_match() {
        let arts = vec![
            article("1", "Onbelangrijke tekst."),
            article("2", "heeft de verzekerde aanspraak op een zorgtoeslag hier"),
        ];
        let mut sel = selector("zorgtoeslag", "op een ", " hier");
        sel.hint = Some(SelectorHint {
            article_number: "2".to_string(),
            start: None,
            end: None,
        });
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "2");
    }

    // === Exact matching: every occurrence, and the context that filters them ===

    #[test]
    fn every_occurrence_is_reported_when_the_context_does_not_disambiguate() {
        // Without prefix/suffix all three occurrences are equally good. They
        // must all be reported: the number of candidates is what tells the
        // editor how ambiguous the anchor really is.
        let arts = vec![article(
            "2",
            "de verzekerde en de verzekerde en nog een verzekerde",
        )];
        let sel = selector("verzekerde", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous());
        let starts: Vec<usize> = r.matches.iter().map(|m| m.start).collect();
        assert_eq!(starts, vec![3, 20, 42]);
    }

    #[test]
    fn without_context_a_quote_also_matches_inside_a_longer_word() {
        // An empty prefix and suffix impose no context at all, so "toeslag"
        // also matches inside "zorgtoeslagen". That is exactly why an anchor
        // without context comes back ambiguous instead of found.
        let arts = vec![article("2", "de zorgtoeslagen en de aanvullende toeslag")];
        let sel = selector("toeslag", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous());
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn context_matches_across_a_single_whitespace_difference() {
        // Prefix and suffix are stored without the whitespace that separates
        // them from the quote in the law text. One character of slack plus
        // trimming absorbs that, so this stays an exact hit (confidence 1.0)
        // instead of degrading to a fuzzy one.
        let arts = vec![article(
            "2",
            "heeft recht op een zorgtoeslag ter grootte van dat verschil",
        )];
        let sel = selector("zorgtoeslag", "op een", "ter grootte");
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        let m = r.single().unwrap();
        assert_eq!(m.start, 19);
        assert_eq!(
            m.confidence, 1.0,
            "a hit on the literal text must not degrade to a fuzzy match"
        );
    }

    #[test]
    fn both_prefix_and_suffix_have_to_match() {
        // The first occurrence has the right prefix but the wrong suffix. Only
        // the occurrence that matches both sides is the anchor.
        let text =
            "recht op een zorgtoeslag krachtens deze wet en recht op een zorgtoeslag van rechtswege";
        let arts = vec![article("2", text)];
        let sel = selector("zorgtoeslag", "op een", "van rechtswege");
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        let m = r.single().unwrap();
        assert!(text[m.end..].starts_with(" van rechtswege"));
    }

    // === Hints: recorded but never authoritative ===

    #[test]
    fn a_hint_does_not_remove_a_cross_article_ambiguity() {
        // Both articles carry the quote verbatim. After a renumbering the
        // recorded article number may point at the wrong one, so the hint may
        // not decide: the ambiguity goes to a human, exactly as it would
        // without a hint.
        let arts = vec![
            article("2", "Deze verplichting vloeit voort uit de wet."),
            article("3", "De inspecteur handelt overeenkomstig de wet."),
        ];
        let sel = hinted("de wet", "", "", "3", None);
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous(), "got {:?}", r.status);
        let articles_hit: Vec<&str> = r
            .matches
            .iter()
            .map(|m| m.article_number.as_str())
            .collect();
        assert_eq!(articles_hit, vec!["2", "3"]);
    }

    #[test]
    fn a_position_hint_does_not_pick_between_identical_occurrences() {
        // The same phrase occurs twice with identical context, so prefix and
        // suffix cannot tell them apart. The recorded position is as stale-
        // prone as the article number (any edit above it shifts the offsets),
        // so it may not silently pick one: both occurrences are reported.
        let text = "recht op een zorgtoeslag van rechtswege en recht op een zorgtoeslag van rechtswege, aldus de toelichting";
        let arts = vec![article("2", text)];
        let start = text.rfind("zorgtoeslag").unwrap();
        let sel = hinted(
            "zorgtoeslag",
            "op een",
            "van rechtswege",
            "2",
            Some((start, start + "zorgtoeslag".len())),
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_ambiguous(), "got {:?}", r.status);
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn a_hinted_fuzzy_match_loses_to_an_exact_match_elsewhere() {
        // The hinted article only fuzzily resembles the quote; another
        // article carries it verbatim with the right context. The full
        // search is authoritative, so the exact occurrence wins even though
        // the hint points elsewhere.
        let arts = vec![
            // "groote" (sic): one letter off, a clear fuzzy hit but not exact.
            article("2", "recht op een zorgtoeslag ter groote van dat verschil"),
            article(
                "7",
                "heeft de verzekerde aanspraak op een zorgtoeslag ter grootte van dat verschil",
            ),
        ];
        let sel = hinted("zorgtoeslag", "op een ", " ter grootte", "2", None);
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        let m = r.single().unwrap();
        assert_eq!(m.article_number, "7");
        assert_eq!(m.confidence, 1.0);
    }

    #[test]
    fn a_position_hint_with_the_wrong_prefix_is_rejected() {
        // The article was rewritten and the recorded offsets now land on
        // another occurrence: same words, different context. The hint is
        // non-authoritative, so the text search decides.
        let text =
            "een voorschot zorgtoeslag van rechtswege; recht op een zorgtoeslag van rechtswege";
        let arts = vec![article("2", text)];
        let stale = text.find("zorgtoeslag").unwrap();
        let sel = hinted(
            "zorgtoeslag",
            "op een",
            "van rechtswege",
            "2",
            Some((stale, stale + "zorgtoeslag".len())),
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(
            r.single().unwrap().start,
            text.rfind("zorgtoeslag").unwrap(),
            "the occurrence with the wrong prefix must not win"
        );
    }

    #[test]
    fn a_position_hint_with_the_wrong_suffix_is_rejected() {
        let text =
            "recht op een zorgtoeslag krachtens deze wet en recht op een zorgtoeslag van rechtswege";
        let arts = vec![article("2", text)];
        let stale = text.find("zorgtoeslag").unwrap();
        let sel = hinted(
            "zorgtoeslag",
            "op een",
            "van rechtswege",
            "2",
            Some((stale, stale + "zorgtoeslag".len())),
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(
            r.single().unwrap().start,
            text.rfind("zorgtoeslag").unwrap(),
            "the occurrence with the wrong suffix must not win"
        );
    }

    #[test]
    fn a_position_hint_past_the_end_of_the_article_is_ignored() {
        // The article was shortened; the recorded offsets now point beyond it.
        // That must not panic and must not stop the search.
        let arts = vec![article("2", "heeft recht op een zorgtoeslag hier")];
        let sel = hinted("zorgtoeslag", "op een", "hier", "2", Some((120, 131)));
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().start, 19);
    }

    // === Fuzzy matching: which windows, which span, which threshold ===

    #[test]
    fn a_fuzzy_anchor_covers_the_changed_phrase() {
        // The note quotes "aanspraak op een zorgtoeslag"; the article now says
        // "recht op een zorgtoeslag". The anchor has to cover that whole
        // phrase and nothing else: it is the span the editor highlights.
        let arts = vec![article(
            "2",
            "heeft de verzekerde recht op een zorgtoeslag ter grootte van het verschil",
        )];
        let sel = selector(
            "aanspraak op een zorgtoeslag",
            "heeft de verzekerde",
            "ter grootte",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "expected a fuzzy match, got {:?}", r.status);
        let m = r.single().unwrap();
        assert_eq!(
            m.matched_text.trim(),
            "recht op een zorgtoeslag",
            "the anchor must cover the changed phrase, not an arbitrary window of about the right size"
        );
        assert!(m.confidence < 1.0);
    }

    #[test]
    fn fuzzy_windows_stay_within_the_length_tolerance() {
        // Candidates are windows of the quote's length ±30%. A window far off
        // that length is a different phrase, not a changed one, and scoring it
        // only invites the resolver to anchor on the wrong text.
        let sel = selector("zorgtoeslag", "", "");
        let text = "de zorgtoeslag wordt jaarlijks vastgesteld door de Belastingdienst";
        let matches = find_fuzzy_matches(
            text,
            &sel,
            DEFAULT_FUZZY_THRESHOLD,
            &mut FuzzyBudget::default(),
        );
        assert!(!matches.is_empty());
        for m in &matches {
            let len = m.end - m.start;
            assert!(
                (8..=14).contains(&len),
                "window of {len} chars for an 11-char quote: {m:?}"
            );
        }
    }

    #[test]
    fn a_fuzzy_window_may_span_the_whole_article() {
        // The article text is the changed quote and nothing else, so the only
        // window that covers it is the full text.
        let arts = vec![article("2", "recht op een zorgtoeslag")];
        let sel = selector("recht op de zorgtoeslag", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(r.single().unwrap().matched_text, "recht op een zorgtoeslag");
    }

    #[test]
    fn a_rewrite_that_only_shares_a_word_does_not_anchor() {
        // "belanghebbende" gets these windows past the pre-filter, but none of
        // them scores at the threshold. The note must orphan rather than
        // anchor onto text it is not about.
        let arts = vec![article(
            "2",
            "de belanghebbende dient de aanvraag in bij de Belastingdienst",
        )];
        let sel = selector(
            "stelt het verzamelinkomen van de belanghebbende ambtshalve vast",
            "de inspecteur",
            "voor het jaar",
        );
        let r = resolve(&sel, &arts);
        assert!(r.is_orphaned(), "got {:?} at {:?}", r.status, r.matches);
    }

    #[test]
    fn a_quote_of_only_short_words_does_not_fuzzily_anchor() {
        // "in de wet" has no word longer than three characters, so there is
        // nothing to key a fuzzy match on. Once the phrase is replaced the note
        // orphans instead of drifting onto text that merely looks like it.
        let arts = vec![article("2", "Deze bevoegdheid is opgenomen in het besluit")];
        let sel = selector("in de wet", "", "");
        let r = resolve(&sel, &arts);
        assert!(r.is_orphaned(), "got {:?} at {:?}", r.status, r.matches);
    }

    // === Budgets: the scan has a ceiling, and hitting it is visible ===

    /// An article of ~230 chars whose text a long quote can be sliced from.
    fn long_sentence() -> &'static str {
        "De verzekerde heeft tegenover de zorgverzekeraar aanspraak op vergoeding van de kosten \
         van zorg zoals verzekerd krachtens de zorgverzekering, voor zover de verzekerde daarop \
         naar inhoud en omvang redelijkerwijs is aangewezen."
    }

    #[test]
    fn a_quote_over_the_length_cap_is_skipped_not_orphaned() {
        let text = long_sentence();
        let arts = vec![article("2", text)];
        // The quote is the article with one word changed: fuzzy matching
        // would find it, but only by scanning windows around a >120-char
        // quote — exactly the cubic blow-up the cap exists for.
        let quote = text.replace("aanspraak", "recht");
        let sel = selector(&quote, "", "");

        // Premises: the quote really is over the cap, has no exact
        // occurrence, and would fuzzily match were the cap not there.
        assert!(quote.chars().count() > MAX_FUZZY_QUOTE_CHARS);
        assert!(!text.contains(&quote));
        let mut roomy = FuzzyBudget {
            max_quote_chars: usize::MAX,
            ..FuzzyBudget::default()
        };
        assert!(
            !find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut roomy).is_empty(),
            "without the cap this quote fuzzily matches; otherwise this test proves nothing"
        );

        let r = resolve(&sel, &arts);
        assert!(
            r.is_skipped(),
            "a quote too long to search must say so, not report 'not found': {:?}",
            r.status
        );
        assert_eq!(r.skip_reason, Some(SkipReason::QuoteTooLong));
        assert!(r.matches.is_empty());
    }

    #[test]
    fn a_long_quote_that_still_occurs_verbatim_resolves_exactly() {
        // The length cap bounds only the fuzzy scan; the linear exact match
        // keeps resolving quotes of any length.
        let text = long_sentence();
        let quote: String = text.chars().skip(3).take(150).collect();
        assert!(quote.chars().count() > MAX_FUZZY_QUOTE_CHARS);
        let arts = vec![article("2", text)];
        let r = resolve(&selector(&quote, "", ""), &arts);
        assert!(r.is_found(), "got {:?}", r.status);
        assert_eq!(r.single().unwrap().confidence, 1.0);
    }

    #[test]
    fn a_hinted_long_quote_is_also_skipped() {
        // A hint does not participate in resolution, so it must not become a
        // side door around the quote-length cap either.
        let text = long_sentence();
        let quote = text.replace("aanspraak", "recht");
        assert!(quote.chars().count() > MAX_FUZZY_QUOTE_CHARS);
        let arts = vec![article("1", "Onbelangrijke tekst."), article("2", text)];
        let sel = hinted(&quote, "", "", "2", None);
        let r = resolve(&sel, &arts);
        assert!(r.is_skipped(), "got {:?}", r.status);
    }

    #[test]
    fn text_beyond_the_scan_budget_is_skipped_not_orphaned() {
        // Two articles that together exceed the scan budget; the changed
        // quote sits in the second one, past the cut-off. Reporting
        // "orphaned" would claim the whole law was searched — it was not.
        let filler = "vulwoord ".repeat(MAX_FUZZY_SCAN_CHARS / 9 / 2 + 100);
        let target = format!("{filler}en voorts recht op een zorgtoeslag van rechtswege");
        let sel = selector(
            "aanspraak op een zorgtoeslag",
            "en voorts ",
            " van rechtswege",
        );

        // Premises: the two articles together bust the budget, and the
        // second article on its own (which does fit) fuzzily matches.
        let total = filler.chars().count() + target.chars().count();
        assert!(total > MAX_FUZZY_SCAN_CHARS);
        let alone = resolve(&sel, &[article("2", &target)]);
        assert!(
            alone.is_found(),
            "the target must be fuzzily matchable on its own, got {:?}",
            alone.status
        );

        let arts = vec![article("1", &filler), article("2", &target)];
        let r = resolve(&sel, &arts);
        assert!(r.is_skipped(), "got {:?}", r.status);
        assert_eq!(
            r.skip_reason,
            Some(SkipReason::SearchBudget),
            "the quote is fine; the law was too large to finish searching"
        );
    }

    #[test]
    fn exhausting_the_scoring_budget_is_skipped_not_orphaned() {
        // Noise made of the quote's own key word: every window passes the
        // pre-filter and gets scored (below threshold — the context words
        // differ), draining the scoring budget before the real target at the
        // end is ever reached.
        // Enough noise positions that the budget runs dry within the first
        // window size, long before any window reaches the target.
        let noise = "zorgtoeslag ".repeat(MAX_FUZZY_SCORED_WINDOWS);
        let target = "de verzekerde heeft recht op een aanvullende zorgtoeslag per jaar";
        let text = format!("{noise}{target}");
        let sel = selector(
            "aanspraak op een aanvullende zorgtoeslag",
            "verzekerde heeft ",
            " per jaar",
        );

        // Premises: the target alone matches fuzzily, and the full text
        // genuinely exhausts the scoring budget.
        let alone = resolve(&sel, &[article("2", target)]);
        assert!(alone.is_found(), "got {:?}", alone.status);
        let mut budget = FuzzyBudget::default();
        find_fuzzy_matches(&text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut budget);
        assert_eq!(
            budget.scored_windows_left, 0,
            "the noise must drain the scoring budget; otherwise this test proves nothing"
        );

        let r = resolve(&sel, &[article("2", &text)]);
        assert!(r.is_skipped(), "got {:?}", r.status);
        assert_eq!(r.skip_reason, Some(SkipReason::SearchBudget));
    }

    #[test]
    fn a_better_candidate_past_the_cut_off_is_not_lost_behind_a_found() {
        // A weak-but-scoring candidate in the first article, then an article
        // whose noise drains the scoring budget before the better candidate
        // at its end is ever scored. Reporting the early candidate as `Found`
        // would anchor the note on the worse place and hide that the search
        // never got to the better one.
        let sel = selector(
            "aanspraak op een aanvullende zorgtoeslag",
            "verzekerde heeft ",
            " per jaar",
        );
        let weak = "de verzekerde heeft aanspraak op enige aanvullende toeslag per jaar";
        let better = "de verzekerde heeft aanspraak op een aanvullende zorgtoeslaag per jaar";
        let noise = "zorgtoeslag ".repeat(MAX_FUZZY_SCORED_WINDOWS);

        // Premises: both articles match on their own, the second one better.
        let weak_alone = resolve(&sel, &[article("1", weak)]);
        let better_alone = resolve(&sel, &[article("2", better)]);
        assert!(weak_alone.is_found(), "got {:?}", weak_alone.status);
        assert!(better_alone.is_found(), "got {:?}", better_alone.status);
        assert!(
            better_alone.single().unwrap().confidence > weak_alone.single().unwrap().confidence,
            "the second article must be the better candidate"
        );

        let arts = vec![
            article("1", weak),
            article("2", &format!("{noise}{better}")),
        ];
        let r = resolve(&sel, &arts);
        assert!(r.is_skipped(), "got {:?}", r.status);
        assert_eq!(r.skip_reason, Some(SkipReason::SearchBudget));
        assert!(
            r.single().is_none(),
            "the early candidate must not pass as the unique match"
        );
        assert_eq!(
            r.matches.first().map(|m| m.article_number.as_str()),
            Some("1"),
            "the candidate found before the cut-off stays visible"
        );
    }

    #[test]
    fn a_cut_short_search_never_claims_a_definitive_outcome() {
        let cut = Some(SkipReason::SearchBudget);
        assert!(finalize_fuzzy(Vec::new(), cut).is_skipped());
        assert!(finalize_fuzzy(Vec::new(), None).is_orphaned());

        // A candidate found before the cut-off must not become `Found`: that
        // would claim uniqueness over text that was never searched. It rides
        // along in the skipped result instead.
        let r = finalize_fuzzy(vec![candidate("2", 0, 10, 0.9)], cut);
        assert!(r.is_skipped(), "got {:?}", r.status);
        assert_eq!(r.matches.len(), 1, "the candidate stays visible");
        assert_eq!(r.skip_reason, Some(SkipReason::SearchBudget));
    }

    #[test]
    fn the_scan_budget_is_spent_per_article_and_carries_across_articles() {
        // First article fits and is scanned; the second no longer fits. The
        // budget must be drawn down by the first scan — a per-article reset
        // would defeat the bound.
        let sel = selector("aanspraak op een zorgtoeslag", "", "");
        let first = "tekst zonder relevante woorden hier";
        let second = "en dan recht op een zorgtoeslag";
        let mut budget = FuzzyBudget {
            scan_chars_left: first.chars().count() + 1,
            ..FuzzyBudget::default()
        };
        let m1 = find_fuzzy_matches(first, &sel, DEFAULT_FUZZY_THRESHOLD, &mut budget);
        assert!(m1.is_empty());
        assert!(
            budget.skipped.is_none(),
            "the first article fits the budget"
        );
        let m2 = find_fuzzy_matches(second, &sel, DEFAULT_FUZZY_THRESHOLD, &mut budget);
        assert!(m2.is_empty(), "the second article no longer fits");
        assert_eq!(budget.skipped, Some(SkipReason::SearchBudget));
    }

    #[test]
    fn an_article_that_exactly_fits_the_scan_budget_is_scanned() {
        // The bound is "does not fit", strictly: an article of exactly the
        // remaining budget is still scanned, and the scan spends the budget
        // down to zero (not up, not divided — the budget is a countdown).
        let sel = selector("aanspraak op een zorgtoeslag", "", "");
        let text = "en dan recht op een zorgtoeslag";
        let mut budget = FuzzyBudget {
            scan_chars_left: text.chars().count(),
            ..FuzzyBudget::default()
        };
        let matches = find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut budget);
        assert!(
            budget.skipped.is_none(),
            "an exact fit is within the budget"
        );
        assert!(!matches.is_empty(), "the exact-fit article was scanned");
        assert_eq!(budget.scan_chars_left, 0);
    }

    #[test]
    fn a_quote_exactly_at_the_length_cap_is_still_searched() {
        // The cap is "longer than", strictly: a quote of exactly the maximum
        // length still gets the fuzzy scan; one char less of headroom skips.
        let sel = selector("aanspraak op een zorgtoeslag", "", "");
        let quote_len = sel.exact.chars().count();
        let text = "en dan recht op een zorgtoeslag";

        let mut at_cap = FuzzyBudget {
            max_quote_chars: quote_len,
            ..FuzzyBudget::default()
        };
        let matches = find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut at_cap);
        assert!(
            at_cap.skipped.is_none(),
            "exactly at the cap is still searched"
        );
        assert!(!matches.is_empty());

        let mut over_cap = FuzzyBudget {
            max_quote_chars: quote_len - 1,
            ..FuzzyBudget::default()
        };
        let m2 = find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut over_cap);
        assert!(m2.is_empty(), "one char over the cap skips the scan");
        assert_eq!(over_cap.skipped, Some(SkipReason::QuoteTooLong));
    }

    #[test]
    fn the_scoring_budget_counts_each_scored_window_exactly_once() {
        // Measure how many windows this scan scores, then rerun with a
        // budget of exactly that number: it must complete untruncated with
        // the budget spent to zero. One less must truncate. This pins the
        // countdown itself (a budget that counts up or divides never hits
        // zero at the right moment).
        let sel = selector("aanspraak op een zorgtoeslag", "", "");
        let text = "en dan recht op een zorgtoeslag";
        let mut probe = FuzzyBudget::default();
        let matches = find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut probe);
        assert!(!matches.is_empty());
        let scored = MAX_FUZZY_SCORED_WINDOWS - probe.scored_windows_left;
        assert!(scored >= 1, "at least the matching window was scored");

        let mut exact_fit = FuzzyBudget {
            scored_windows_left: scored,
            ..FuzzyBudget::default()
        };
        let m2 = find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut exact_fit);
        assert_eq!(m2.len(), matches.len());
        assert!(
            exact_fit.skipped.is_none(),
            "exactly enough budget is enough"
        );
        assert_eq!(exact_fit.scored_windows_left, 0);

        let mut one_short = FuzzyBudget {
            scored_windows_left: scored - 1,
            ..FuzzyBudget::default()
        };
        find_fuzzy_matches(text, &sel, DEFAULT_FUZZY_THRESHOLD, &mut one_short);
        assert!(
            one_short.skipped.is_some(),
            "one window short truncates the scan"
        );
    }

    // === Scoring and collapsing helpers ===

    #[test]
    fn similarity_treats_a_missing_side_as_no_similarity() {
        // A window at the very start of an article has no text before it. That
        // scores zero against a non-empty prefix, not a free 1.0.
        assert_eq!(similarity("op een", ""), 0.0);
        assert_eq!(similarity("", "op een"), 0.0);
        assert_eq!(similarity("", ""), 1.0);
        assert_eq!(similarity("zorgtoeslag", "zorgtoeslag"), 1.0);
        assert!(similarity("aanspraak", "aanzoek") > 0.0);
    }

    /// Old-API shim: does the window `[i, i+window)` of `text` share a
    /// significant word with `exact`? (One-off query through the cursor.)
    fn window_shares(text: &str, exact: &str, i: usize, window: usize) -> bool {
        let chars: Vec<char> = text.chars().collect();
        let index = WordIndex::new(&chars, exact);
        index.cursor(window).shares_significant_content(i)
    }

    #[test]
    fn only_words_longer_than_three_characters_are_significant() {
        let words = significant_words("Recht op een zorgtoeslag van de wet");
        let as_chars = |s: &str| s.chars().collect::<Vec<char>>();
        assert!(words.contains(&as_chars("recht")), "lowercased: {words:?}");
        assert!(words.contains(&as_chars("zorgtoeslag")));
        assert!(
            !words.contains(&as_chars("wet")),
            "three characters is too common to key on"
        );
        assert!(!words.contains(&as_chars("op")));
    }

    #[test]
    fn the_prefilter_needs_a_word_shared_with_the_quote() {
        let quote = "aanspraak op zorgtoeslag";
        let shares = |text: &str| window_shares(text, quote, 0, text.chars().count());
        assert!(shares("recht op zorgtoeslag"));
        assert!(
            !shares("vastgesteld bij ministeriële regeling"),
            "long words that the quote does not use are not shared content"
        );
        assert!(!shares("op de wet"));
    }

    #[test]
    fn the_prefilter_matches_case_insensitively() {
        assert!(window_shares(
            "ZORGTOESLAG voor iedereen",
            "de zorgtoeslag",
            0,
            11
        ));
    }

    #[test]
    fn the_prefilter_sees_a_word_truncated_at_a_window_edge() {
        // The window cuts "zorgtoeslagen" down to "zorgtoeslag": the old
        // per-window tokenisation counted that truncated token as shared
        // content, and the indexed pre-filter must keep doing so.
        let text = "de zorgtoeslagen";
        assert!(window_shares(text, "zorgtoeslag", 0, 14));
        // A cut that leaves only "zorg" (long enough, but not a quote word)
        // or "zor" (too short to be significant) is not shared content.
        assert!(!window_shares(text, "zorgtoeslag", 0, 7));
        assert!(!window_shares(text, "zorgtoeslag", 0, 6));
    }

    #[test]
    fn the_prefilter_sees_interior_and_left_truncated_words() {
        let text = "aanspraak op een zorgtoeslag";
        // Interior word: window over " op een zorgtoeslag" fully contains
        // "zorgtoeslag".
        assert!(window_shares(text, "de zorgtoeslag", 9, 19));
        // Left-truncated: window starting inside "aanspraak" keeps "spraak",
        // which is not a quote word.
        assert!(!window_shares(text, "de aanspraak", 3, 10));
        // But a left cut that still leaves a full quote word elsewhere is
        // fine: "op een zorgtoeslag" contains "zorgtoeslag".
        assert!(window_shares(text, "de zorgtoeslag", 3, 25));
    }

    #[test]
    fn a_word_truncated_only_at_the_left_edge_is_seen() {
        // The window starts four chars into "zorgtoeslagen"; the remaining
        // "toeslagen" is a quote word. Only the left-edge check can see this.
        let text = "zorgtoeslagen x";
        assert!(window_shares(text, "de toeslagen", 4, 9));
        // Same left cut, but now the window also ends inside the run: the
        // token is clipped on both sides to "toeslag".
        assert!(window_shares(text, "de toeslag", 4, 7));
        // A window ending exactly where a run ends, and one starting exactly
        // where a run starts: adjacent runs are no edge tokens at all.
        assert!(!window_shares("de zorgtoeslag", "de zorgtoeslag", 2, 11));
        assert!(!window_shares("zorgtoeslag de", "zorgtoeslag x", 1, 11));
    }

    #[test]
    fn a_right_truncated_word_is_seen_from_any_window_start() {
        let text = "de zorgtoeslagen";
        // Window starting exactly at the run start (the pointer boundary):
        // token [3, 14) = "zorgtoeslag".
        assert!(window_shares(text, "zorgtoeslag", 3, 11));
        // Window starting inside the run: token [3, 10) = "zorgtoe".
        assert!(window_shares("de zorgtoeslagen mooi", "de zorgtoe", 3, 7));
        // A full run inside the window before the right-truncated word: the
        // truncated "zorgtoeslag" must still be found past the non-matching
        // "flop".
        assert!(window_shares(
            "de flop zorgtoeslagen",
            "de zorgtoeslag",
            3,
            16
        ));
    }

    #[test]
    fn a_cursor_answers_the_same_as_a_fresh_query_at_every_position() {
        // The cursor's two pointers only move forward; sliding it over the
        // text must give the same answers as querying each window cold.
        let text = "de verzekerde heeft aanspraak op een zorgtoeslag van de wet";
        let quote = "aanspraak op zorgtoeslag";
        let chars: Vec<char> = text.chars().collect();
        let index = WordIndex::new(&chars, quote);
        for window in [5, 11, 24] {
            let mut cursor = index.cursor(window);
            for i in 0..=(chars.len() - window) {
                assert_eq!(
                    cursor.shares_significant_content(i),
                    window_shares(text, quote, i, window),
                    "window {window} at {i}"
                );
            }
        }
    }

    #[test]
    fn touching_candidates_are_not_overlapping() {
        // Two spans that touch (one ends where the next begins) are separate
        // anchors; only genuinely overlapping spans collapse onto the best one.
        let kept = deduplicate_overlapping(vec![
            candidate("2", 0, 10, 0.80),
            candidate("2", 10, 20, 0.90),
            candidate("2", 20, 30, 0.85),
        ]);
        assert_eq!(kept.len(), 3, "kept {kept:?}");
    }

    #[test]
    fn overlapping_candidates_collapse_onto_the_best() {
        let kept = deduplicate_overlapping(vec![
            candidate("2", 0, 10, 0.80),
            candidate("2", 5, 15, 0.90),
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].confidence, 0.90);
    }

    #[test]
    fn the_same_span_in_another_article_is_a_separate_candidate() {
        let kept = deduplicate_overlapping(vec![
            candidate("2", 0, 10, 0.90),
            candidate("3", 0, 10, 0.80),
        ]);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn a_winner_has_to_beat_the_runner_up_by_more_than_the_margin() {
        assert_eq!(
            clear_winner(&[candidate("2", 0, 10, 0.80)]).map(|w| w.start),
            Some(0),
            "a lone candidate wins by default"
        );
        assert_eq!(
            clear_winner(&[candidate("2", 0, 10, 0.95), candidate("2", 20, 30, 0.80)])
                .map(|w| w.start),
            Some(0),
            "0.15 ahead is a clear winner"
        );

        // Exactly the margin is not "more than": still a tie. 0.2 and 0.1 are
        // chosen because their difference is exactly 0.1 in binary floating
        // point, which is what puts this case on the boundary.
        let (best, second) = (0.2, 0.1);
        assert_eq!(best - second, TIEBREAK_MARGIN, "fixture sits on the margin");
        assert!(
            clear_winner(&[candidate("2", 0, 10, best), candidate("2", 20, 30, second)]).is_none()
        );

        assert!(
            clear_winner(&[candidate("2", 0, 10, 0.75), candidate("2", 20, 30, 0.72)]).is_none(),
            "within the margin is a tie"
        );
        assert!(clear_winner(&[]).is_none());
    }

    #[test]
    fn find_subslice_reports_the_first_position() {
        let hay: Vec<char> = "zorgtoeslag".chars().collect();
        let needle = |s: &str| s.chars().collect::<Vec<char>>();
        assert_eq!(find_subslice(&hay, &needle("toeslag")), Some(4));
        assert_eq!(find_subslice(&hay, &needle("zorgtoeslag")), Some(0));
        assert_eq!(find_subslice(&hay, &needle("premie")), None);
        assert_eq!(
            find_subslice(&hay, &needle("zorgtoeslagen")),
            None,
            "a needle longer than the haystack cannot be in it"
        );
        assert_eq!(
            find_subslice(&hay, &[]),
            None,
            "an empty quote matches nothing"
        );
    }

    // === Resolving against raw text, without articles ===

    #[test]
    fn raw_text_resolves_a_single_exact_quote() {
        let sel = selector("de wet", "", "");
        let r = resolve_in_text(
            &sel,
            "Deze verplichting vloeit voort uit de wet.",
            DEFAULT_FUZZY_THRESHOLD,
        );
        assert!(r.is_found(), "got {:?}", r.status);
        let m = r.single().unwrap();
        assert_eq!(m.matched_text, "de wet");
        assert_eq!(m.confidence, 1.0);
        assert_eq!(m.article_number, "", "raw text has no article to name");
    }

    #[test]
    fn raw_text_with_a_repeated_quote_is_ambiguous() {
        let sel = selector("de wet", "", "");
        let r = resolve_in_text(
            &sel,
            "de wet verwijst naar de wet.",
            DEFAULT_FUZZY_THRESHOLD,
        );
        assert!(r.is_ambiguous(), "got {:?}", r.status);
        assert_eq!(r.matches.len(), 2);
    }

    #[test]
    fn outdated_hint_falls_back_to_full_search() {
        // Hint points at article 9 which does not contain the text;
        // resolver must still find it in article 2.
        let arts = vec![
            article("2", "heeft de verzekerde aanspraak op een zorgtoeslag hier"),
            article("9", "Niets relevants."),
        ];
        let mut sel = selector("zorgtoeslag", "op een ", " hier");
        sel.hint = Some(SelectorHint {
            article_number: "9".to_string(),
            start: Some(0),
            end: Some(5),
        });
        let r = resolve(&sel, &arts);
        assert!(r.is_found());
        assert_eq!(r.single().unwrap().article_number, "2");
    }
}
