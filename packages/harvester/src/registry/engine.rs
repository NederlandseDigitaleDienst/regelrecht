//! Parse engine that orchestrates element parsing using the registry.

use std::cell::RefCell;

use roxmltree::Node;

use super::core::ElementRegistry;
use super::types::{ParseContext, ParseResult};
use crate::error::{HarvesterError, Result};
use crate::xml::get_tag_name;

/// Engine that orchestrates element parsing using the registry.
///
/// The engine walks the XML tree and dispatches elements to their
/// registered handlers. It raises `UnknownElement` error for any element
/// that has no handler and is not marked as skip.
pub struct ParseEngine {
    registry: ElementRegistry,
}

impl ParseEngine {
    /// Create a new engine with the given registry.
    #[must_use]
    pub fn new(registry: ElementRegistry) -> Self {
        Self { registry }
    }

    /// Get a reference to the underlying registry.
    #[must_use]
    pub fn registry(&self) -> &ElementRegistry {
        &self.registry
    }

    /// Parse an element tree recursively.
    ///
    /// # Arguments
    /// * `node` - The XML element to parse
    /// * `context` - Current parsing context
    ///
    /// # Returns
    /// `ParseResult` containing the extracted text
    ///
    /// # Errors
    /// Returns `UnknownElement` error if an element has no handler and is not skipped.
    pub fn parse(&self, node: Node<'_, '_>, context: &mut ParseContext<'_>) -> Result<ParseResult> {
        let tag_name = get_tag_name(node);

        // Skip marked elements
        if self.registry.should_skip(tag_name) {
            return Ok(ParseResult::empty());
        }

        // Get handler
        if let Some(handler) = self.registry.get_handler(node, context) {
            // Collect errors during recursive parsing
            let collected_errors: RefCell<Vec<String>> = RefCell::new(Vec::new());

            // Create recursive parsing closure that collects errors and continues parsing
            let recurse = |child: Node<'_, '_>, ctx: &mut ParseContext<'_>| -> ParseResult {
                match self.parse(child, ctx) {
                    Ok(mut result) => {
                        // Collect any errors from the child result
                        collected_errors
                            .borrow_mut()
                            .extend(result.errors.drain(..));
                        result
                    }
                    Err(err) => {
                        let error_msg = format!("Error parsing <{}>: {}", get_tag_name(child), err);
                        tracing::warn!(
                            error = %err,
                            tag = %get_tag_name(child),
                            "Error parsing child element, skipping"
                        );
                        collected_errors.borrow_mut().push(error_msg);
                        ParseResult::empty()
                    }
                }
            };

            let mut result = handler.handle(node, context, &recurse);
            // Merge collected errors into the result
            result.errors.extend(collected_errors.into_inner());
            return Ok(result);
        }

        // No handler - raise error with parent context
        let parent_context = node
            .parent_element()
            .map(|p| format!("<{}>", get_tag_name(p)));
        Err(HarvesterError::UnknownElement {
            tag_name: tag_name.to_string(),
            context: parent_context,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ElementType;
    use crate::registry::handler::{ElementHandler, RecurseFn};
    use roxmltree::Document;

    struct TestHandler {
        output: String,
    }

    impl ElementHandler for TestHandler {
        fn element_type(&self) -> ElementType {
            ElementType::Inline
        }

        fn handle<'a, 'input>(
            &self,
            _node: Node<'a, 'input>,
            _context: &mut ParseContext<'_>,
            _recurse: &RecurseFn<'a, 'input>,
        ) -> ParseResult {
            ParseResult::new(&self.output)
        }
    }

    #[test]
    fn test_engine_parse_with_handler() {
        let mut registry = ElementRegistry::new();
        registry.register(
            "test",
            TestHandler {
                output: "hello".to_string(),
            },
        );
        let engine = ParseEngine::new(registry);

        let xml = "<test/>";
        let doc = Document::parse(xml).unwrap();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");

        let result = engine.parse(doc.root_element(), &mut context).unwrap();
        assert_eq!(result.text, "hello");
    }

    #[test]
    fn test_engine_parse_skip() {
        let mut registry = ElementRegistry::new();
        registry.skip(["meta-data"]);
        let engine = ParseEngine::new(registry);

        let xml = "<meta-data/>";
        let doc = Document::parse(xml).unwrap();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");

        let result = engine.parse(doc.root_element(), &mut context).unwrap();
        assert_eq!(result.text, "");
    }

    #[test]
    fn test_engine_parse_unknown() {
        let registry = ElementRegistry::new();
        let engine = ParseEngine::new(registry);

        let xml = "<unknown/>";
        let doc = Document::parse(xml).unwrap();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");

        let result = engine.parse(doc.root_element(), &mut context);
        assert!(result.is_err());
    }
}
