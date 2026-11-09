//! Element handler trait definition.

use roxmltree::Node;

use super::types::{ElementType, ParseContext, ParseResult};

/// Function type for recursive processing of child elements.
pub type RecurseFn<'a, 'input> =
    dyn Fn(Node<'a, 'input>, &mut ParseContext<'_>) -> ParseResult + 'a;

/// Trait for element handlers.
///
/// Handlers are responsible for processing a specific type of XML element
/// and returning its text content. They receive a `recurse` function to
/// process child elements.
pub trait ElementHandler: Send + Sync {
    /// Return the type classification of this element.
    fn element_type(&self) -> ElementType;

    /// Check if this handler can process the given element.
    ///
    /// Default implementation always returns true.
    fn can_handle(&self, _node: Node<'_, '_>, _context: &ParseContext<'_>) -> bool {
        true
    }

    /// Process the element and return parsed text.
    ///
    /// # Arguments
    /// * `node` - The XML element to process
    /// * `context` - Current parsing context
    /// * `recurse` - Function to call for recursive child processing
    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult;
}

/// Extract text from element including children and tail text.
///
/// This is the common pattern for inline/passthrough elements:
/// - Include element's direct text
/// - Recurse into children
/// - Include tail text after each child
pub fn extract_text_with_tail<'a, 'input>(
    node: Node<'a, 'input>,
    context: &mut ParseContext<'_>,
    recurse: &RecurseFn<'a, 'input>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Add element's direct text
    if let Some(text) = node.text() {
        parts.push(text.to_string());
    }

    // Process children
    for child in node.children() {
        if child.is_element() {
            let result = recurse(child, context);
            if !result.text.is_empty() {
                parts.push(result.text);
            }
        }

        // Add tail text after the child (text that follows the child element)
        if let Some(tail) = child.tail() {
            if !tail.trim().is_empty() {
                parts.push(tail.to_string());
            } else if tail.contains(' ') {
                // Preserve word-separating spaces between inline elements
                // e.g., <nadruk>A</nadruk> <nadruk>B</nadruk> should produce "A B" not "AB"
                parts.push(" ".to_string());
            }
        }
    }

    parts.join("").trim().to_string()
}
