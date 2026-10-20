//! Splitting strategies for article extraction.

use roxmltree::Node;

use super::types::{ElementSpec, SplitContext};
use crate::xml::{find_by_path, get_text};

/// Trait for configurable splitting strategies.
///
/// Implementations determine where to split and how to extract numbers.
pub trait SplitStrategy {
    /// Determine if this element should produce a component.
    fn should_split_here(
        &self,
        node: Node<'_, '_>,
        spec: &ElementSpec,
        context: &SplitContext,
    ) -> bool;

    /// Extract the number/identifier for this element.
    fn get_number(&self, node: Node<'_, '_>, spec: &ElementSpec) -> Option<String>;
}

/// Strategy that splits at leaf nodes (deepest split points).
///
/// This is the default strategy for Dutch law article splitting.
pub struct LeafSplitStrategy;

impl SplitStrategy for LeafSplitStrategy {
    fn should_split_here(
        &self,
        _node: Node<'_, '_>,
        spec: &ElementSpec,
        context: &SplitContext,
    ) -> bool {
        // Only split at elements marked as split points
        if !spec.is_split_point {
            return false;
        }

        // Respect max depth if set
        if let Some(max_depth) = context.max_depth {
            if context.depth >= max_depth {
                return true;
            }
        }

        true
    }

    fn get_number(&self, node: Node<'_, '_>, spec: &ElementSpec) -> Option<String> {
        // Try primary number source first
        if let Some(source) = spec.number_source.as_deref() {
            if let Some(nr_node) = find_by_path(node, source) {
                let nr = get_text(nr_node);
                if let Some(cleaned) = self.clean_number(&nr) {
                    return Some(cleaned);
                }
            }
        }

        // Fallback: try label attribute for artikel elements (used by repealed articles)
        // Repealed articles ("Vervallen") often lack <kop><nr> but retain the label attribute
        // Example: <artikel label="Artikel 4"> with text "Vervallen"
        if spec.tag == "artikel" {
            if let Some(label) = node.attribute("label") {
                if let Some(nr) = label.strip_prefix("Artikel ") {
                    if let Some(cleaned) = self.clean_number(nr) {
                        return Some(cleaned);
                    }
                }
            }
        }

        None
    }
}

impl LeafSplitStrategy {
    /// Clean up and normalize an article number.
    ///
    /// - Removes trailing punctuation (period, degree symbol)
    /// - Filters out bullet-only markers
    /// - Transforms asterisk suffix to "_bis" for transition articles
    fn clean_number(&self, nr: &str) -> Option<String> {
        // Clean up the number:
        // - Remove trailing punctuation (period, degree symbol for Dutch "1°", "2°")
        // - Trim whitespace
        let nr = nr.trim();
        let nr = nr
            .strip_suffix('.')
            .or_else(|| nr.strip_suffix('°'))
            .or_else(|| nr.strip_suffix("°."))
            .unwrap_or(nr)
            .trim();

        // Filter out bullet-only markers (used for formula variables, not article numbers)
        // Unicode bullet: U+2022 (•)
        // These appear in laws like Participatiewet article 22a for formula variable definitions
        if nr.is_empty() || nr == "•" {
            return None;
        }

        // Transform asterisk suffix to "_bis" for transition articles (overgangsartikelen)
        // Example: "78ee*" becomes "78ee_bis"
        // The asterisk in Dutch law denotes transition provisions added later
        let nr = if let Some(base) = nr.strip_suffix('*') {
            format!("{base}_bis")
        } else {
            nr.to_string()
        };

        Some(nr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::Document;

    #[test]
    fn test_leaf_strategy_should_split() {
        let strategy = LeafSplitStrategy;
        let spec = ElementSpec::new("artikel").with_split_point(true);
        let context = SplitContext::new("BWBR0000000", "2025-01-01", "url");

        let xml = "<artikel/>";
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();

        assert!(strategy.should_split_here(node, &spec, &context));
    }

    #[test]
    fn test_leaf_strategy_no_split_non_split_point() {
        let strategy = LeafSplitStrategy;
        let spec = ElementSpec::new("lijst").with_split_point(false);
        let context = SplitContext::new("BWBR0000000", "2025-01-01", "url");

        let xml = "<lijst/>";
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();

        assert!(!strategy.should_split_here(node, &spec, &context));
    }

    #[test]
    fn test_leaf_strategy_get_number() {
        /// (case, tag, number_source, xml, expected)
        type Case = (
            &'static str,
            &'static str,
            Option<&'static str>,
            &'static str,
            Option<&'static str>,
        );
        let cases: &[Case] = &[
            (
                "simple",
                "lid",
                Some("lidnr"),
                "<lid><lidnr>1.</lidnr></lid>",
                Some("1"),
            ),
            (
                "nested path",
                "artikel",
                Some("kop/nr"),
                "<artikel><kop><nr>5</nr></kop></artikel>",
                Some("5"),
            ),
            ("no number source", "lijst", None, "<lijst/>", None),
            // Dutch lists often use degree symbol: "1°", "2°"
            (
                "degree symbol",
                "li",
                Some("li.nr"),
                "<li><li.nr>1°</li.nr></li>",
                Some("1"),
            ),
            (
                "whitespace",
                "lid",
                Some("lidnr"),
                "<lid><lidnr>  2.  </lidnr></lid>",
                Some("2"),
            ),
            // Bullet-only markers (used for formula variables like A, B in Participatiewet)
            // should be treated as unnumbered, so content is extracted inline
            (
                "bullet only",
                "li",
                Some("li.nr"),
                "<li><li.nr>•</li.nr></li>",
                None,
            ),
            (
                "bullet with whitespace",
                "li",
                Some("li.nr"),
                "<li><li.nr>  •  </li.nr></li>",
                None,
            ),
            // Asterisk suffix should be transformed to "_bis"
            // This is used for Dutch "overgangsartikelen" (transition provisions)
            (
                "asterisk to _bis",
                "artikel",
                Some("kop/nr"),
                "<artikel><kop><nr>78ee*</nr></kop></artikel>",
                Some("78ee_bis"),
            ),
            // Period is stripped first, then asterisk is transformed
            (
                "asterisk with period",
                "lid",
                Some("lidnr"),
                "<lid><lidnr>1*.</lidnr></lid>",
                Some("1_bis"),
            ),
            (
                "no asterisk unchanged",
                "artikel",
                Some("kop/nr"),
                "<artikel><kop><nr>78ee</nr></kop></artikel>",
                Some("78ee"),
            ),
            // Repealed articles ("Vervallen") often lack <kop><nr> but have label attribute
            // The strategy should fall back to extracting the number from label
            (
                "label fallback",
                "artikel",
                Some("kop/nr"),
                r#"<artikel label="Artikel 4"><al>Vervallen</al></artikel>"#,
                Some("4"),
            ),
            (
                "label fallback, complex number",
                "artikel",
                Some("kop/nr"),
                r#"<artikel label="Artikel 16a"><al>Vervallen</al></artikel>"#,
                Some("16a"),
            ),
            // When both kop/nr and label exist, kop/nr should take precedence
            (
                "kop/nr over label",
                "artikel",
                Some("kop/nr"),
                r#"<artikel label="Artikel 99"><kop><nr>5</nr></kop></artikel>"#,
                Some("5"),
            ),
        ];

        let strategy = LeafSplitStrategy;
        for &(case, tag, source, xml, expected) in cases {
            let spec = match source {
                Some(source) => ElementSpec::new(tag).with_number_source(source),
                None => ElementSpec::new(tag),
            };
            let doc = Document::parse(xml).unwrap();

            assert_eq!(
                strategy.get_number(doc.root_element(), &spec).as_deref(),
                expected,
                "{case}"
            );
        }
    }
}
