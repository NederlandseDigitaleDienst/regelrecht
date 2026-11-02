//! Inline element handlers for text-level elements.
//!
//! These handlers process elements that appear inline within text,
//! such as emphasis (nadruk), external references (extref), and
//! internal references (intref).

use regex::Regex;
use roxmltree::Node;
use std::sync::LazyLock;

use crate::config::sanitize_fragment;
use crate::registry::handler::{extract_text_with_tail, ElementHandler, RecurseFn};
use crate::registry::types::{ElementType, ParseContext, ParseResult};
use crate::types::Reference;

// Static regexes for parsing JCI references - all patterns are guaranteed to be valid
#[allow(clippy::expect_used)]
/// Regex for extracting BWB ID from JCI reference.
static BWB_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"BWBR\d+").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for extracting article number from JCI reference.
///
/// Captures the full value up to the next JCI parameter (`&`), like the sibling
/// patterns below. Article numbers can be numeric (`1`, `1a`, `12bis`), dotted
/// sub-articles (`2.5a`, `3.1`, `2.10`), or Roman numeral (`IV`). The previous
/// `\w`-only pattern truncated dotted numbers at the dot (`2.5a` → `2`), which
/// produced wrong reference metadata and `#Artikel2`-style anchor URLs.
static ARTIKEL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"artikel=([^&]+)").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for extracting lid (paragraph) from JCI reference.
static LID_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"lid=([^&]+)").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for extracting onderdeel (subdivision) from JCI reference.
static ONDERDEEL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"onderdeel=([^&]+)").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for extracting hoofdstuk (chapter) from JCI reference.
static HOOFDSTUK_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"hoofdstuk=([^&]+)").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for extracting paragraaf (section) from JCI reference.
static PARAGRAAF_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"paragraaf=([^&]+)").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for extracting afdeling (division) from JCI reference.
static AFDELING_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"afdeling=([^&]+)").expect("valid regex"));

#[allow(clippy::expect_used)]
/// Regex for a CELEX number, the identifier of an EU act on EUR-Lex: sector,
/// year, document type and number, with an optional suffix (e.g. `32001L0055`,
/// `32016R0679`, `32019R2088(01)`). BWB marks these with `reeks="Celex"`.
static CELEX_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9CE][0-9]{4}[A-Z]{1,2}[0-9]{4}(\([0-9]{1,2}\))?$").expect("valid regex")
});

/// Build the link target for an external reference's `doc` attribute.
///
/// - JCI (and other) references to a BWB id link to wetten.overheid.nl.
/// - A CELEX number links to the act on EUR-Lex.
/// - An absolute http(s) URL is kept as is.
///
/// Anything else (such as BWB's `doc="onbekend"`) has no target the reader can
/// follow, so it yields `None` and the reference stays plain text instead of
/// becoming a relative link that leads nowhere.
fn reference_url(doc: &str) -> Option<String> {
    if BWB_PATTERN.is_match(doc) {
        return Some(convert_jci_to_url(doc));
    }
    if CELEX_PATTERN.is_match(doc) {
        // Parentheses would end a markdown link target early.
        let celex = doc.replace('(', "%28").replace(')', "%29");
        return Some(format!(
            "https://eur-lex.europa.eu/legal-content/NL/TXT/?uri=CELEX:{celex}"
        ));
    }
    if doc.starts_with("https://") || doc.starts_with("http://") {
        return Some(doc.to_string());
    }
    None
}

/// Convert JCI reference to wetten.overheid.nl URL.
///
/// Anchor priority: artikel > hoofdstuk > afdeling > paragraaf
///
/// All fragment values are sanitized to prevent injection of problematic characters.
fn convert_jci_to_url(jci_ref: &str) -> String {
    let Some(bwb_match) = BWB_PATTERN.find(jci_ref) else {
        return jci_ref.to_string();
    };
    let bwb_id = bwb_match.as_str();

    // Anchor priority order: first match wins. Sanitised before interpolation
    // so user-controlled JCI fragments cannot break out of the URL fragment.
    let anchors: &[(&LazyLock<Regex>, &str)] = &[
        (&ARTIKEL_PATTERN, "Artikel"),
        (&HOOFDSTUK_PATTERN, "Hoofdstuk"),
        (&AFDELING_PATTERN, "Afdeling"),
        (&PARAGRAAF_PATTERN, "Paragraaf"),
    ];
    for (pattern, prefix) in anchors {
        if let Some(caps) = pattern.captures(jci_ref) {
            let value = sanitize_fragment(&caps[1].replace(' ', "_"));
            return format!("https://wetten.overheid.nl/{bwb_id}#{prefix}{value}");
        }
    }
    format!("https://wetten.overheid.nl/{bwb_id}")
}

/// Extract first capture group from a regex match.
fn extract_capture(pattern: &Regex, text: &str) -> Option<String> {
    pattern.captures(text).map(|c| c[1].to_string())
}

/// Parse JCI reference to Reference object.
fn parse_jci_reference(jci_ref: &str) -> Option<Reference> {
    let bwb_id = BWB_PATTERN.find(jci_ref)?.as_str().to_string();

    Some(Reference {
        id: String::new(), // Will be set by collector
        bwb_id,
        artikel: extract_capture(&ARTIKEL_PATTERN, jci_ref),
        lid: extract_capture(&LID_PATTERN, jci_ref),
        onderdeel: extract_capture(&ONDERDEEL_PATTERN, jci_ref),
        hoofdstuk: extract_capture(&HOOFDSTUK_PATTERN, jci_ref),
        paragraaf: extract_capture(&PARAGRAAF_PATTERN, jci_ref),
        afdeling: extract_capture(&AFDELING_PATTERN, jci_ref),
    })
}

/// Handler for `<nadruk>` (emphasis) elements.
///
/// Converts emphasis to markdown: `**text**` for bold (type="vet"),
/// `*text*` for italic (default).
pub struct NadrukHandler;

impl ElementHandler for NadrukHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Inline
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        // Use extract_text_with_tail to properly handle nested elements
        let text = extract_text_with_tail(node, context, recurse);
        let nadruk_type = node.attribute("type").unwrap_or_default();

        let formatted = if nadruk_type == "vet" {
            format!("**{text}**")
        } else {
            format!("*{text}*")
        };

        ParseResult::new(formatted)
    }
}

/// Common logic for handling reference elements (extref/intref).
///
/// Converts references to markdown links using reference-style formatting
/// when a collector is available, or inline links otherwise.
fn handle_reference_element(node: Node<'_, '_>, context: &mut ParseContext<'_>) -> ParseResult {
    let ref_text = node.text().unwrap_or_default();
    let doc_attr = node.attribute("doc").unwrap_or_default();

    if let Some(collector) = &mut context.collector {
        // Try to parse as JCI reference
        if let Some(reference) = parse_jci_reference(doc_attr) {
            let ref_id = collector.add_full_reference(reference);
            return ParseResult::new(format!("[{ref_text}][{ref_id}]"));
        }
    }

    // Fallback to inline link
    if let Some(url) = reference_url(doc_attr) {
        return ParseResult::new(format!("[{ref_text}]({url})"));
    }

    ParseResult::new(ref_text)
}

/// Handler for `<extref>` (external reference) elements.
///
/// Converts external references to markdown links using reference-style
/// formatting when a collector is available, or inline links otherwise.
pub struct ExtrefHandler;

impl ElementHandler for ExtrefHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Inline
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        handle_reference_element(node, context)
    }
}

/// Handler for `<intref>` (internal reference) elements.
///
/// Converts internal references to markdown links using reference-style
/// formatting when a collector is available, or inline links otherwise.
pub struct IntrefHandler;

impl ElementHandler for IntrefHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Inline
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        handle_reference_element(node, context)
    }
}

/// Handler for `<al>` (paragraph/alinea) elements.
///
/// Extracts inline text including child elements like extref, intref,
/// and nadruk. This is the main text container element.
pub struct AlHandler;

impl ElementHandler for AlHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Inline
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        ParseResult::new(extract_text_with_tail(node, context, recurse))
    }
}

/// Handler for `<redactie>` (editorial note) elements.
///
/// Editorial notes are NOT law text - they are annotations from editors.
/// Examples:
/// - "Dit artikel is gewijzigd in verband met..."
/// - "Voor overige gevallen luidt het artikel als volgt:"
/// - "Vervallen."
///
/// These elements are SKIPPED (return empty text) during parsing.
pub struct RedactieHandler;

impl ElementHandler for RedactieHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Skip // Editorial content is always skipped
    }

    fn handle<'a, 'input>(
        &self,
        _node: Node<'a, 'input>,
        _context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        // Return empty - editorial content is not law text
        ParseResult::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ReferenceCollector;
    use roxmltree::Document;

    fn parse_and_handle<H: ElementHandler>(handler: &H, xml: &str) -> ParseResult {
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");
        let recurse = |_: Node<'_, '_>, _: &mut ParseContext<'_>| ParseResult::empty();
        handler.handle(node, &mut context, &recurse)
    }

    #[test]
    fn test_nadruk_handler_vet() {
        let result = parse_and_handle(&NadrukHandler, r#"<nadruk type="vet">bold</nadruk>"#);
        assert_eq!(result.text, "**bold**");
    }

    #[test]
    fn test_nadruk_handler_default() {
        let result = parse_and_handle(&NadrukHandler, "<nadruk>italic</nadruk>");
        assert_eq!(result.text, "*italic*");
    }

    #[test]
    fn test_nadruk_handler_nested() {
        // Test that nadruk can contain nested elements and still extract text properly
        let xml = r#"<nadruk type="vet">bold text with <sub>subscript</sub> inside</nadruk>"#;
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");
        // Simple recurse that just returns text for any child
        let recurse = |child: Node<'_, '_>, _: &mut ParseContext<'_>| {
            ParseResult::new(child.text().unwrap_or_default())
        };
        let result = NadrukHandler.handle(node, &mut context, &recurse);
        // The nested content should be extracted (exact format depends on extract_text_with_tail)
        assert!(result.text.starts_with("**"));
        assert!(result.text.ends_with("**"));
        assert!(result.text.contains("bold text"));
    }

    #[test]
    fn test_convert_jci_to_url() {
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&artikel=4"),
            "https://wetten.overheid.nl/BWBR0018451#Artikel4"
        );

        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451"),
            "https://wetten.overheid.nl/BWBR0018451"
        );
    }

    #[test]
    fn test_convert_jci_to_url_chapter() {
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0009950&hoofdstuk=5a"),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5a"
        );

        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0009950&hoofdstuk=5c"),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5c"
        );
    }

    #[test]
    fn test_convert_jci_to_url_section() {
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0009950&afdeling=3.1"),
            "https://wetten.overheid.nl/BWBR0009950#Afdeling3.1"
        );
    }

    #[test]
    fn test_convert_jci_to_url_paragraph() {
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0009950&paragraaf=2"),
            "https://wetten.overheid.nl/BWBR0009950#Paragraaf2"
        );
    }

    #[test]
    fn test_convert_jci_to_url_priority() {
        // Article takes priority over chapter
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0009950&hoofdstuk=5a&artikel=1"),
            "https://wetten.overheid.nl/BWBR0009950#Artikel1"
        );

        // Chapter takes priority over section
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0009950&afdeling=3.1&hoofdstuk=5a"),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5a"
        );
    }

    #[test]
    fn test_extref_handler_with_collector() {
        let xml = r#"<extref doc="jci1.3:c:BWBR0018451&amp;artikel=4">artikel 4</extref>"#;
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();

        let mut collector = ReferenceCollector::new();
        let mut context =
            ParseContext::new("BWBR0000000", "2025-01-01").with_collector(&mut collector);

        let recurse = |_: Node<'_, '_>, _: &mut ParseContext<'_>| ParseResult::empty();
        let result = ExtrefHandler.handle(node, &mut context, &recurse);

        assert_eq!(result.text, "[artikel 4][ref1]");
    }

    #[test]
    fn test_extref_handler_without_collector() {
        let xml = r#"<extref doc="jci1.3:c:BWBR0018451&amp;artikel=4">artikel 4</extref>"#;
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();

        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");

        let recurse = |_: Node<'_, '_>, _: &mut ParseContext<'_>| ParseResult::empty();
        let result = ExtrefHandler.handle(node, &mut context, &recurse);

        assert_eq!(
            result.text,
            "[artikel 4](https://wetten.overheid.nl/BWBR0018451#Artikel4)"
        );
    }

    #[test]
    fn test_extref_celex_links_to_eur_lex() {
        // Awir art. 1: a directive reference. Both with and without a collector
        // it must become an absolute EUR-Lex link, never a relative "(32001L0055)".
        let xml = r#"<extref doc="32001L0055" label="richtlijn" reeks="Celex">Richtlijn 2001/55/EG</extref>"#;
        let expected = "[Richtlijn 2001/55/EG](https://eur-lex.europa.eu/legal-content/NL/TXT/?uri=CELEX:32001L0055)";

        assert_eq!(parse_and_handle(&ExtrefHandler, xml).text, expected);

        let doc = Document::parse(xml).unwrap();
        let mut collector = ReferenceCollector::new();
        let mut context =
            ParseContext::new("BWBR0000000", "2025-01-01").with_collector(&mut collector);
        let recurse = |_: Node<'_, '_>, _: &mut ParseContext<'_>| ParseResult::empty();
        let result = ExtrefHandler.handle(doc.root_element(), &mut context, &recurse);
        assert_eq!(result.text, expected);
    }

    #[test]
    fn test_reference_url() {
        assert_eq!(
            reference_url("32019R2088(01)").as_deref(),
            Some("https://eur-lex.europa.eu/legal-content/NL/TXT/?uri=CELEX:32019R2088%2801%29")
        );
        assert_eq!(
            reference_url("https://example.org/a").as_deref(),
            Some("https://example.org/a")
        );
        // No target to follow: plain text, not a relative link.
        assert_eq!(reference_url("onbekend"), None);
        assert_eq!(reference_url(""), None);
        assert_eq!(
            parse_and_handle(&ExtrefHandler, r#"<extref doc="onbekend">bijlage</extref>"#).text,
            "bijlage"
        );
    }

    #[test]
    fn test_parse_jci_reference() {
        let jci = "jci1.3:c:BWBR0018451&artikel=4&lid=2";
        let reference = parse_jci_reference(jci).unwrap();

        assert_eq!(reference.bwb_id, "BWBR0018451");
        assert_eq!(reference.artikel, Some("4".to_string()));
        assert_eq!(reference.lid, Some("2".to_string()));
    }

    #[test]
    fn test_convert_jci_to_url_roman_numerals() {
        // Roman numeral article numbers (common in transitional provisions)
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&artikel=I"),
            "https://wetten.overheid.nl/BWBR0018451#ArtikelI"
        );
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&artikel=IV"),
            "https://wetten.overheid.nl/BWBR0018451#ArtikelIV"
        );
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&artikel=XIIa"),
            "https://wetten.overheid.nl/BWBR0018451#ArtikelXIIa"
        );
    }

    #[test]
    fn test_parse_jci_reference_roman_numerals() {
        let jci = "jci1.3:c:BWBR0018451&artikel=IV&lid=1";
        let reference = parse_jci_reference(jci).unwrap();

        assert_eq!(reference.bwb_id, "BWBR0018451");
        assert_eq!(reference.artikel, Some("IV".to_string()));
        assert_eq!(reference.lid, Some("1".to_string()));
    }

    #[test]
    fn test_parse_jci_reference_dotted_artikel() {
        // Dotted/sub-article numbers (e.g. besluit_zorgverzekering internal
        // refs) must keep their full number, not truncate at the dot.
        let jci =
            "jci1.3:c:BWBR0018492&hoofdstuk=2&paragraaf=1&artikel=2.5a&z=2026-01-01&g=2026-01-01";
        let reference = parse_jci_reference(jci).unwrap();
        assert_eq!(reference.bwb_id, "BWBR0018492");
        assert_eq!(reference.artikel, Some("2.5a".to_string()));
        assert_eq!(reference.hoofdstuk, Some("2".to_string()));
        assert_eq!(reference.paragraaf, Some("1".to_string()));

        // Trailing param and end-of-string both bound the value at '&'/end.
        assert_eq!(
            parse_jci_reference("jci1.3:c:BWBR0018492&artikel=3.1&z=2026-01-01")
                .unwrap()
                .artikel,
            Some("3.1".to_string())
        );
        assert_eq!(
            parse_jci_reference("jci1.3:c:BWBR0018492&artikel=2.10")
                .unwrap()
                .artikel,
            Some("2.10".to_string())
        );
    }

    #[test]
    fn test_convert_jci_to_url_dotted_artikel() {
        // The anchor must use the full dotted article number.
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018492&hoofdstuk=2&paragraaf=1&artikel=2.5a"),
            "https://wetten.overheid.nl/BWBR0018492#Artikel2.5a"
        );
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018492&artikel=3.1&z=2026-01-01"),
            "https://wetten.overheid.nl/BWBR0018492#Artikel3.1"
        );
    }

    #[test]
    fn test_convert_jci_to_url_sanitizes_fragments() {
        // All anchor patterns now match [^&]+, so any special chars in the
        // captured value must be stripped by sanitize_fragment before they
        // reach the URL fragment.

        // artikel pattern matches [^&]+ (since the dotted-artikel fix), so a
        // crafted value must still be sanitized in the anchor.
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&artikel=2.5a<script>"),
            "https://wetten.overheid.nl/BWBR0018451#Artikel2.5ascript"
        );

        // hoofdstuk pattern matches [^&]+ which could include special chars
        // Quotes and other special chars should be stripped by sanitize_fragment
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&hoofdstuk=5<script>alert"),
            "https://wetten.overheid.nl/BWBR0018451#Hoofdstuk5scriptalert"
        );

        // Test afdeling with special chars
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&afdeling=3.1\"onclick"),
            "https://wetten.overheid.nl/BWBR0018451#Afdeling3.1onclick"
        );

        // Test paragraaf with angle brackets
        assert_eq!(
            convert_jci_to_url("jci1.3:c:BWBR0018451&paragraaf=2<>"),
            "https://wetten.overheid.nl/BWBR0018451#Paragraaf2"
        );
    }
}
