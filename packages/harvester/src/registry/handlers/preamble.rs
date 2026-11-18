//! Preamble element handlers for law introductions.
//!
//! These handlers process elements in the preamble (aanhef) of Dutch laws,
//! including the royal introduction and considerations.

use roxmltree::Node;

use crate::registry::handler::{ElementHandler, RecurseFn, extract_text_with_tail};
use crate::registry::types::{ElementType, ParseContext, ParseResult};
use crate::xml::get_tag_name;

/// Handler for `<wij>` (royal "We") elements.
///
/// The opening of the preamble, typically "Wij Beatrix..." or "Wij Willem-Alexander...".
pub struct WijHandler;

impl ElementHandler for WijHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Inline
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        _context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let text = node.text().map(|s| s.trim()).unwrap_or_default();
        ParseResult::new(text)
    }
}

/// Recurse into all element children, joining non-empty results with
/// `separator`. Shared body for handlers that just delegate every element
/// child to the dispatcher (`<considerans>`, `<aanhef>`).
fn join_all_element_children<'a, 'input>(
    node: Node<'a, 'input>,
    context: &mut ParseContext<'_>,
    recurse: &RecurseFn<'a, 'input>,
    separator: &str,
) -> ParseResult {
    let mut parts: Vec<String> = Vec::new();
    for child in node.children() {
        if child.is_element() {
            let result = recurse(child, context);
            if !result.text.is_empty() {
                parts.push(result.text);
            }
        }
    }
    ParseResult::new(parts.join(separator))
}

/// Handler for `<considerans>` (considerations) elements.
///
/// Contains the legal considerations for the law.
pub struct ConsideransHandler;

impl ElementHandler for ConsideransHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        join_all_element_children(node, context, recurse, "\n\n")
    }
}

/// Handler for `<considerans.al>` (consideration paragraph) elements.
pub struct ConsideransAlHandler;

impl ElementHandler for ConsideransAlHandler {
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

/// Handler for `<afkondiging>` (proclamation) elements.
pub struct AfkondigingHandler;

impl ElementHandler for AfkondigingHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let mut parts: Vec<String> = Vec::new();

        for child in node.children() {
            if child.is_element() && get_tag_name(child) == "al" {
                let result = recurse(child, context);
                if !result.text.is_empty() {
                    parts.push(result.text);
                }
            }
        }

        ParseResult::new(parts.join("\n\n"))
    }
}

/// Handler for `<aanhef>` (preamble) elements.
pub struct AanhefHandler;

impl ElementHandler for AanhefHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        join_all_element_children(node, context, recurse, "\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::Document;

    fn parse_and_handle<H: ElementHandler>(handler: &H, xml: &str) -> ParseResult {
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");
        let recurse = |_: Node<'_, '_>, _: &mut ParseContext<'_>| ParseResult::empty();
        handler.handle(node, &mut context, &recurse)
    }

    #[test]
    fn test_wij_handler() {
        let result = parse_and_handle(
            &WijHandler,
            "<wij>Wij Willem-Alexander, bij de gratie Gods, Koning der Nederlanden</wij>",
        );
        assert!(result.text.contains("Willem-Alexander"));
    }

    #[test]
    fn test_considerans_al_handler() {
        let result = parse_and_handle(
            &ConsideransAlHandler,
            "<considerans.al>Overwegende dat...</considerans.al>",
        );
        assert!(result.text.contains("Overwegende"));
    }
}
