//! Text wrapping and normalization utilities for YAML output.

use regex::Regex;
use std::sync::LazyLock;
use textwrap::{Options, fill};

use crate::config::TEXT_WRAP_WIDTH;

/// Regex pattern for reference-style links [text][refN].
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static REFERENCE_LINK_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]\[ref\d+\]").expect("valid regex"));

/// Regex pattern for missing space after comma before a word character.
/// Matches "word,word" but not "word, word" or "1,000".
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static MISSING_SPACE_AFTER_COMMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([a-zA-Z]),([a-zA-Z])").expect("valid regex"));

/// Regex pattern for markdown reference definition lines (`[label]: url`) at
/// line start. Matches any label, not just the `refN` labels the harvester
/// generates today — the fold invariant is "no line-oriented reference
/// definitions at all", and an unrecognized label folding into prose would
/// break the markdown link.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static REFERENCE_DEFINITION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\[[^\]]+\]: ").expect("valid regex"));

/// Check if text contains reference-style links that would be broken by wrapping.
fn contains_reference_link(text: &str) -> bool {
    REFERENCE_LINK_PATTERN.is_match(text)
}

/// Block scalar style for a multi-line text field in the emitted YAML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    /// Literal block scalar (`|-`): every newline in the emitted string is a
    /// real newline after loading.
    Literal,
    /// Folded block scalar (`>-`): wrap newlines fold back to spaces on load,
    /// so only paragraph breaks (`\n\n`) survive in the loaded string.
    Folded,
}

/// Decide the block scalar style for a text field.
///
/// `normalized` is the unwrapped source text (after [`normalize_text`]);
/// `emitted` is the (possibly wrapped) string that goes into the YAML struct.
/// Folding is only safe when every newline that wrapping introduced is
/// cosmetic — i.e. the source itself contains no deliberate hard line breaks
/// and nothing that folded-scalar semantics would corrupt. When in doubt this
/// returns [`TextStyle::Literal`], which keeps today's byte-identical output.
pub fn classify_text_style(normalized: &str, emitted: &str) -> TextStyle {
    // Single-line emission: serde writes a plain/quoted scalar, nothing to fold.
    if !emitted.contains('\n') {
        return TextStyle::Literal;
    }

    // Markdown reference definitions ([label]: url) are line-oriented: folding
    // would join consecutive definitions onto one line and break the markdown.
    if REFERENCE_DEFINITION_PATTERN.is_match(normalized) {
        return TextStyle::Literal;
    }

    // Every maximal newline run in the source must be exactly 2 (a paragraph
    // break). A run of 1 is a deliberate hard line break (e.g. a `- ` list
    // built by the splitter) that folding would join into prose; a run of 3+
    // would need 3+ blank lines in the folded block (yamllint empty-lines max).
    let mut run = 0usize;
    for ch in normalized.chars().chain(std::iter::once('\0')) {
        if ch == '\n' {
            run += 1;
        } else {
            if run != 0 && run != 2 {
                return TextStyle::Literal;
            }
            run = 0;
        }
    }

    // "More-indented" lines do not fold and switch folded-scalar semantics.
    if normalized
        .lines()
        .any(|l| l.starts_with(' ') || l.starts_with('\t'))
    {
        return TextStyle::Literal;
    }

    // Defensive re-checks on the emitted string. These should all follow from
    // the rules above plus textwrap behavior, but folding an unexpected shape
    // would corrupt legal text, so verify at runtime and fall back instead.
    let lines: Vec<&str> = emitted.lines().collect();
    let first_last_blank = lines
        .first()
        .zip(lines.last())
        .is_none_or(|(f, l)| f.trim().is_empty() || l.trim().is_empty());
    let unsafe_line = lines.iter().any(|l| {
        l.starts_with(' ') || l.starts_with('\t') || (!l.is_empty() && l.trim_end() != *l)
    });
    // Redundant with the newline-run check on `normalized` above; kept to
    // catch the hypothetical case where normalize_text/wrap_text_default
    // would inject back-to-back blank lines that were not in the source.
    let double_blank = lines.windows(2).any(|w| w[0].is_empty() && w[1].is_empty());
    if first_last_blank || unsafe_line || double_blank {
        return TextStyle::Literal;
    }

    TextStyle::Folded
}

/// Normalize common typographical issues in source text.
///
/// Fixes:
/// - Missing space after comma before a word (e.g., "lid,van" → "lid, van")
///
/// This is needed because some official source XML contains typographical errors.
pub fn normalize_text(text: &str) -> String {
    // Loop until no more replacements needed (handles overlapping cases like "a,b,c")
    let mut result = text.to_string();
    loop {
        let replaced = MISSING_SPACE_AFTER_COMMA
            .replace_all(&result, "$1, $2")
            .to_string();
        if replaced == result {
            break;
        }
        result = replaced;
    }
    result
}

/// Wrap text at specified width, preserving paragraph breaks and reference definitions.
///
/// Reference definitions (lines starting with [refN]:) are preserved as-is
/// to maintain valid markdown reference-style links.
pub fn wrap_text(text: &str, width: usize) -> String {
    // Separate reference definitions from main text
    let lines: Vec<&str> = text.lines().collect();
    let mut ref_lines: Vec<&str> = Vec::new();
    let mut content_lines: Vec<&str> = Vec::new();

    // Find where reference definitions start (from end)
    // Only include empty lines that are BETWEEN reference definitions
    let mut in_refs = false;
    let mut pending_empty: Vec<&str> = Vec::new();

    for line in lines.iter().rev() {
        if line.starts_with("[ref") && line.contains("]: ") {
            // Found a reference line - include any pending empty lines
            for empty in pending_empty.drain(..).rev() {
                ref_lines.insert(0, empty);
            }
            ref_lines.insert(0, line);
            in_refs = true;
        } else if in_refs && line.is_empty() {
            // Empty line while in refs - save for later
            // Only add if followed by another ref line
            pending_empty.push(line);
        } else {
            // Non-ref line - move pending empties to content and exit ref mode
            for empty in pending_empty.drain(..) {
                content_lines.insert(0, empty);
            }
            in_refs = false;
            content_lines.insert(0, line);
        }
    }

    // Any remaining pending empties go to content
    for empty in pending_empty {
        content_lines.insert(0, empty);
    }

    // Wrap content paragraphs, but skip paragraphs containing reference-style links
    let content_text = content_lines.join("\n");
    let paragraphs: Vec<&str> = content_text.split("\n\n").collect();

    let options = Options::new(width);
    let wrapped: Vec<String> = paragraphs
        .iter()
        .map(|p| {
            if contains_reference_link(p) {
                wrap_with_protected_refs(p, &options)
            } else {
                fill(p, &options)
            }
        })
        .collect();

    let wrapped_content = wrapped.join("\n\n");

    // Append reference definitions unchanged
    if !ref_lines.is_empty() {
        format!("{}\n\n{}", wrapped_content, ref_lines.join("\n"))
    } else {
        wrapped_content
    }
}

/// Wrap text while protecting reference-style links from being split.
///
/// Replaces `[text][refN]` tokens with single-word placeholders before wrapping,
/// then restores them after. This allows textwrap to break lines around the links
/// without breaking inside them.
///
/// The placeholder is deliberately as wide as the link it replaces, so textwrap
/// computes the same line breaks it would for the real text. A link can therefore
/// be wider than the wrap width — long law titles regularly are — and textwrap's
/// default `break_words` would then split the placeholder across two lines, after
/// which it can no longer be matched back and the marker leaks into the published
/// law text. Word breaking is disabled for this pass to keep every placeholder in
/// one piece; such a link simply overflows its line, exactly as it does after
/// restoration anyway.
fn wrap_with_protected_refs(text: &str, options: &Options<'_>) -> String {
    let mut replacements: Vec<(String, String)> = Vec::new();
    let mut protected = text.to_string();

    // Replace each reference link with a same-width non-breakable placeholder.
    // Width must match so textwrap calculates line breaks correctly.
    for mat in REFERENCE_LINK_PATTERN.find_iter(text) {
        let original = mat.as_str();
        let tag = format!("__REF{:03}__", replacements.len());
        let placeholder = format!(
            "{}{}",
            tag,
            "_".repeat(original.len().saturating_sub(tag.len()))
        );
        replacements.push((placeholder.clone(), original.to_string()));
        protected = protected.replacen(original, &placeholder, 1);
    }

    // Keep placeholders intact: a placeholder wider than the wrap width must
    // stay one word, otherwise it cannot be restored below.
    let protect_options = options.clone().break_words(false);
    let wrapped = fill(&protected, &protect_options);

    // Restore original reference links
    let mut result = wrapped;
    for (placeholder, original) in &replacements {
        result = result.replacen(placeholder.as_str(), original, 1);
    }

    result
}

/// Check if text should be wrapped for readability.
pub fn should_wrap_text(text: &str) -> bool {
    let has_markdown_links = text.contains('[') && text.contains("](");
    text.len() > 80 || has_markdown_links
}

/// Wrap text with default width.
pub fn wrap_text_default(text: &str) -> String {
    wrap_text(text, TEXT_WRAP_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text_simple() {
        let text = "This is a simple text that should be wrapped when it exceeds the specified width limit.";
        let wrapped = wrap_text(text, 40);
        assert!(wrapped.contains('\n'));
    }

    #[test]
    fn test_wrap_text_preserves_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.";
        let wrapped = wrap_text(text, 100);
        assert!(wrapped.contains("\n\n"));
    }

    #[test]
    fn test_wrap_text_preserves_references() {
        let text = "Some text with a [link][ref1].\n\n[ref1]: https://example.com";
        let wrapped = wrap_text(text, 100);
        assert!(wrapped.contains("[ref1]: https://example.com"));
    }

    #[test]
    fn test_should_wrap_text_long() {
        let long_text = "A".repeat(100);
        assert!(should_wrap_text(&long_text));
    }

    #[test]
    fn test_should_wrap_text_short() {
        let short_text = "Short text";
        assert!(!should_wrap_text(short_text));
    }

    #[test]
    fn test_should_wrap_text_with_links() {
        let text = "Text with [link](url)";
        assert!(should_wrap_text(text));
    }

    #[test]
    fn test_contains_reference_link() {
        assert!(contains_reference_link(
            "See [article 4][ref1] for details."
        ));
        assert!(contains_reference_link(
            "Multiple [ref][ref1] and [other][ref2] links."
        ));
        assert!(!contains_reference_link("No reference links here."));
        assert!(!contains_reference_link(
            "[link](url) is inline, not reference."
        ));
    }

    #[test]
    fn test_wrap_text_wraps_around_reference_links() {
        // Long text with reference links should be wrapped, but links stay intact
        let text = "This is a very long paragraph that contains a reference link like [article 4 of the Zorgverzekeringswet][ref1] which should not be broken across lines because that would invalidate the markdown.";
        let wrapped = wrap_text(text, 60);
        // The reference link should still be intact (not split across lines)
        assert!(wrapped.contains("[article 4 of the Zorgverzekeringswet][ref1]"));
        // But the text should be wrapped (contains newlines)
        assert!(wrapped.contains('\n'), "Text should be wrapped");
        // And no line should exceed the width by much (allowing for unbreakable ref links)
        for line in wrapped.lines() {
            assert!(
                line.len() <= 110,
                "Line too long ({} chars): {}",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn test_reference_link_placeholder_keeps_line_breaks_of_plain_text() {
        // The placeholder that protects a reference link is deliberately as wide
        // as the link itself, so the lines break where they would for plain
        // prose of the same length. A narrower or wider placeholder would
        // silently reflow every harvested law, so pin the invariant here.
        let link = "[artikel 4 van de Zorgverzekeringswet][ref1]";
        let same_width_word = "x".repeat(link.chars().count());
        let sentence = "De verzekerde die op grond van {} recht heeft op zorg, meldt dit onverwijld aan de zorgverzekeraar.";

        let line_widths = |text: &str| -> Vec<usize> {
            wrap_text(text, 60)
                .lines()
                .map(|l| l.chars().count())
                .collect()
        };

        assert_eq!(
            line_widths(&sentence.replace("{}", link)),
            line_widths(&sentence.replace("{}", &same_width_word)),
            "protecting a reference link changed the line breaks"
        );
    }

    #[test]
    fn test_wrap_text_restores_reference_link_longer_than_width() {
        // A reference link can be longer than the wrap width on its own (long
        // law titles are common). The placeholder that protects it must not be
        // broken across lines, otherwise it can no longer be restored and a
        // `__REF000___` marker ends up in the published law text.
        let link = "[artikel 1 van de Wet zorg en dwang psychogeriatrische en verstandelijk gehandicapte clienten, zoals dat luidde op 1 januari 2025][ref1]";
        assert!(
            link.len() > TEXT_WRAP_WIDTH,
            "test premise: link ({} chars) must exceed the wrap width ({})",
            link.len(),
            TEXT_WRAP_WIDTH
        );

        let text = format!(
            "De zorgaanbieder verleent zorg als bedoeld in {link} aan de verzekerde die daarop is aangewezen.\n\n[ref1]: https://example.com/wzd"
        );
        let wrapped = wrap_text_default(&text);

        assert!(
            !wrapped.contains("__REF"),
            "placeholder left behind in output:\n{wrapped}"
        );
        assert!(
            wrapped.contains(link),
            "reference link not restored intact:\n{wrapped}"
        );
        assert!(
            wrapped.contains("[ref1]: https://example.com/wzd"),
            "reference definition lost:\n{wrapped}"
        );
    }

    #[test]
    fn test_wrap_text_restores_multiple_overlong_reference_links() {
        // Several overlong links in one paragraph: every one of them must come
        // back, in its own place.
        let first = "[artikel 3.2.1, eerste lid, onderdeel a, van de Wet langdurige zorg zoals die gold voor de wijziging][ref1]";
        let second = "[artikel 1 van de Wet zorg en dwang psychogeriatrische en verstandelijk gehandicapte clienten][ref2]";
        let text = format!(
            "Zie {first} en daarnaast ook {second} voor de nadere invulling van dit begrip in de praktijk."
        );

        let wrapped = wrap_text(&text, 60);

        assert!(
            !wrapped.contains("__REF"),
            "placeholder left behind in output:\n{wrapped}"
        );
        assert!(wrapped.contains(first), "first link lost:\n{wrapped}");
        assert!(wrapped.contains(second), "second link lost:\n{wrapped}");
    }

    #[test]
    fn test_overlong_plain_word_overflows_only_beside_a_reference_link() {
        // Turning word breaking off applies to the whole paragraph, not just to
        // the placeholders: an unrelated word wider than the wrap width now
        // overflows its line instead of being hard-broken, but only in a
        // paragraph that also holds a reference link. Harvested law text has no
        // such words today, and hard-breaking one mid-word is no better than
        // letting it overflow -- pin the difference so it stays a decision.
        let overlong = "a".repeat(90);
        let sentence = "Zie {} en verder in deze bepaling.";

        let beside_link = wrap_text(
            &sentence.replace("{}", &format!("[artikel 4][ref1] en {overlong}")),
            60,
        );
        let on_its_own = wrap_text(&sentence.replace("{}", &overlong), 60);

        assert!(
            beside_link.contains(&overlong),
            "overlong word was broken next to a reference link:\n{beside_link}"
        );
        assert!(
            !on_its_own.contains(&overlong),
            "overlong word should still be hard-broken without a reference link:\n{on_its_own}"
        );
    }

    #[test]
    fn test_normalize_text_missing_space_after_comma() {
        // Real example from Wet op de zorgtoeslag source XML
        assert_eq!(normalize_text("lid,van"), "lid, van");
        assert_eq!(
            normalize_text("eerste of derde lid,van die wet"),
            "eerste of derde lid, van die wet"
        );
    }

    #[test]
    fn test_normalize_text_preserves_correct_spacing() {
        // Should not change text with correct spacing
        assert_eq!(normalize_text("lid, van"), "lid, van");
        assert_eq!(normalize_text("correct, spacing"), "correct, spacing");
    }

    #[test]
    fn test_normalize_text_preserves_numbers() {
        // Should not add space in numbers like "1,000"
        assert_eq!(normalize_text("€ 1,000"), "€ 1,000");
        assert_eq!(normalize_text("bedrag van 1,50"), "bedrag van 1,50");
    }

    #[test]
    fn test_normalize_text_multiple_occurrences() {
        // Should fix multiple occurrences
        assert_eq!(normalize_text("a,b,c,d"), "a, b, c, d");
    }

    /// Convenience: classify a source text through the same normalize+wrap
    /// path the writer uses.
    fn classify(source: &str, width: usize) -> TextStyle {
        let normalized = normalize_text(source);
        let emitted = wrap_text(&normalized, width);
        classify_text_style(&normalized, &emitted)
    }

    #[test]
    fn test_classify_single_line_emission_is_literal() {
        // Nothing to fold when the emitted string has no newlines.
        assert_eq!(
            classify_text_style("korte tekst", "korte tekst"),
            TextStyle::Literal
        );
    }

    #[test]
    fn test_classify_wrapped_prose_is_folded() {
        let prose = "Indien de normpremie voor een verzekerde in het berekeningsjaar minder bedraagt dan de standaardpremie in dat jaar, heeft de verzekerde aanspraak op een zorgtoeslag.";
        assert_eq!(classify(prose, 60), TextStyle::Folded);
    }

    #[test]
    fn test_classify_multi_paragraph_prose_is_folded() {
        let prose = "Eerste paragraaf die lang genoeg is om over de wrap-breedte heen te gaan bij het serialiseren.\n\nTweede paragraaf die eveneens lang genoeg is om gewrapt te worden door de writer.";
        assert_eq!(classify(prose, 60), TextStyle::Folded);
    }

    #[test]
    fn test_classify_reference_definitions_are_literal() {
        // [refN]: lines are line-oriented markdown; folding would join them.
        let text = "Zie [artikel 4][ref1] voor de premie die hier verder wordt toegelicht in een lange zin.\n\n[ref1]: https://example.com/a\n[ref2]: https://example.com/b";
        assert_eq!(classify(text, 60), TextStyle::Literal);
    }

    #[test]
    fn test_classify_non_ref_label_definitions_are_literal() {
        // The invariant covers ANY markdown reference definition label, not
        // just the `refN` labels the harvester generates today.
        let text = "Zie [artikel 4][wet] voor de premie die hier verder wordt toegelicht in een lange zin.\n\n[wet]: https://example.com/a";
        assert_eq!(classify(text, 60), TextStyle::Literal);
    }

    #[test]
    fn test_classify_hard_single_newline_is_literal() {
        // A single \n in the source is a deliberate hard break (e.g. a list
        // the splitter built); folding would join it into prose.
        let text = "- eerste onderdeel van de opsomming die hier staat\n- tweede onderdeel van de opsomming die hier staat";
        assert_eq!(classify(text, 60), TextStyle::Literal);
    }

    #[test]
    fn test_classify_indented_line_is_literal() {
        // More-indented lines switch folded-scalar semantics; never fold them.
        let text = "Aanhef van het artikel dat lang genoeg is om te wrappen op de ingestelde breedte:\n\n  - *onderdeel* met inspringing";
        assert_eq!(classify(text, 60), TextStyle::Literal);
    }

    #[test]
    fn test_classify_triple_newline_is_literal() {
        // \n\n\n would need three blank lines in a folded block (yamllint
        // empty-lines max 2), so it must stay literal.
        let text = "Eerste stuk tekst dat lang genoeg is om gewrapt te worden door de yaml writer.\n\n\nTweede stuk tekst na een dubbele lege regel dat ook lang genoeg is.";
        assert_eq!(classify(text, 60), TextStyle::Literal);
    }
}
