//! XML utility functions for navigating and extracting data from DOM trees.

use roxmltree::Node;

/// Get the tag name without namespace prefix.
///
/// # Arguments
/// * `node` - XML node
///
/// # Returns
/// Tag name without namespace (e.g., "artikel" not "{ns}artikel")
///
/// # Examples
/// ```
/// use roxmltree::Document;
/// use regelrecht_harvester::xml::get_tag_name;
///
/// let xml = r#"<root><artikel>text</artikel></root>"#;
/// let doc = Document::parse(xml).unwrap();
/// let artikel = doc.root_element().first_element_child().unwrap();
/// assert_eq!(get_tag_name(artikel), "artikel");
/// ```
pub fn get_tag_name<'a>(node: Node<'a, '_>) -> &'a str {
    node.tag_name().name()
}

/// Find the first child element with the given tag name.
///
/// # Arguments
/// * `node` - Parent node to search in
/// * `tag` - Tag name to search for
///
/// # Returns
/// First matching child element, or `None` if not found
///
/// # Examples
/// ```
/// use roxmltree::Document;
/// use regelrecht_harvester::xml::find_child;
///
/// let xml = r#"<root><child1/><child2/></root>"#;
/// let doc = Document::parse(xml).unwrap();
/// let root = doc.root_element();
///
/// assert!(find_child(root, "child1").is_some());
/// assert!(find_child(root, "missing").is_none());
/// ```
pub fn find_child<'a, 'input>(node: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && get_tag_name(*child) == tag)
}

/// Find all child elements with the given tag name.
///
/// # Arguments
/// * `node` - Parent node to search in
/// * `tag` - Tag name to search for
///
/// # Returns
/// Iterator over matching child elements
///
/// # Examples
/// ```
/// use roxmltree::Document;
/// use regelrecht_harvester::xml::find_children;
///
/// let xml = r#"<root><item>1</item><item>2</item><other/></root>"#;
/// let doc = Document::parse(xml).unwrap();
/// let root = doc.root_element();
///
/// let items: Vec<_> = find_children(root, "item").collect();
/// assert_eq!(items.len(), 2);
/// ```
pub fn find_children<'a, 'input>(
    node: Node<'a, 'input>,
    tag: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children()
        .filter(move |child| child.is_element() && get_tag_name(*child) == tag)
}

/// Find a descendant element matching a path of tag names.
///
/// # Arguments
/// * `node` - Starting node
/// * `path` - Slash-separated path of tag names (e.g., "kop/nr")
///
/// # Returns
/// Matching element, or `None` if path not found
///
/// # Examples
/// ```
/// use roxmltree::Document;
/// use regelrecht_harvester::xml::find_by_path;
///
/// let xml = r#"<artikel><kop><nr>1</nr></kop></artikel>"#;
/// let doc = Document::parse(xml).unwrap();
/// let artikel = doc.root_element();
///
/// let nr = find_by_path(artikel, "kop/nr");
/// assert!(nr.is_some());
/// assert_eq!(nr.unwrap().text(), Some("1"));
/// ```
pub fn find_by_path<'a, 'input>(node: Node<'a, 'input>, path: &str) -> Option<Node<'a, 'input>> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut current = node;

    for part in parts {
        current = find_child(current, part)?;
    }

    Some(current)
}

/// Get the text content of a node, trimmed.
///
/// # Arguments
/// * `node` - Node to get text from
///
/// # Returns
/// Trimmed text content, or empty string if no text
pub fn get_text(node: Node<'_, '_>) -> String {
    node.text()
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Get an attribute value from a node.
///
/// # Arguments
/// * `node` - Node to get attribute from
/// * `name` - Attribute name
///
/// # Returns
/// Attribute value, or `None` if not found
pub fn get_attribute<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attribute(name)
}

/// Check if a node has a specific tag name.
///
/// # Arguments
/// * `node` - Node to check
/// * `tag` - Expected tag name
///
/// # Returns
/// `true` if the node has the specified tag name
pub fn has_tag(node: Node<'_, '_>, tag: &str) -> bool {
    node.is_element() && get_tag_name(node) == tag
}

/// Get all element children of a node.
///
/// # Arguments
/// * `node` - Parent node
///
/// # Returns
/// Iterator over element children (excludes text nodes, comments, etc.)
pub fn element_children<'a, 'input>(
    node: Node<'a, 'input>,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(|child| child.is_element())
}

/// Information about a bijlage (appendix) ancestor.
#[derive(Debug, Clone, PartialEq)]
pub struct BijlageContext {
    /// The bijlage number (e.g., "1", "2", "3").
    pub number: String,
}

/// Find the nearest bijlage ancestor and extract its number.
///
/// Walks up the XML tree from the given node to find a `<bijlage>` ancestor.
/// If found, extracts the number from `<kop>/<nr>`.
///
/// # Arguments
/// * `node` - Starting node to search from
///
/// # Returns
/// `Some(BijlageContext)` if the node is inside a bijlage, `None` otherwise
///
/// # Examples
/// ```
/// use roxmltree::Document;
/// use regelrecht_harvester::xml::find_bijlage_context;
///
/// let xml = r#"<bijlage><kop><nr>1</nr></kop><artikel/></bijlage>"#;
/// let doc = Document::parse(xml).unwrap();
/// let artikel = doc.descendants().find(|n| n.has_tag_name("artikel")).unwrap();
///
/// let ctx = find_bijlage_context(artikel);
/// assert!(ctx.is_some());
/// assert_eq!(ctx.unwrap().number, "1");
/// ```
pub fn find_bijlage_context(node: Node<'_, '_>) -> Option<BijlageContext> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.is_element() && get_tag_name(parent) == "bijlage" {
            let number = find_by_path(parent, "kop/nr")
                .map(|nr| get_text(nr))
                .filter(|s| !s.is_empty())?;
            return Some(BijlageContext { number });
        }
        current = parent.parent();
    }
    None
}

/// Find the element that keeps `node` out of force on `date`, if any.
///
/// A BWB toestand marks every structural element (artikel, paragraaf,
/// hoofdstuk, ...) with a `status` and usually an `inwerking` date. An element
/// that is already enacted but not yet in force has `status="nogniet"`, with an
/// `inwerking` date once that date is known. This walks from `node` up to the
/// root and returns the first element that is `nogniet` or whose `inwerking`
/// lies after `date`. `None` means the node is in force on `date`.
///
/// Dates are ISO 8601 (YYYY-MM-DD), so string order is date order.
///
/// # Examples
/// ```
/// use roxmltree::Document;
/// use regelrecht_harvester::xml::not_in_force_on;
///
/// let xml = r#"<hoofdstuk status="goed"><artikel status="nogniet" inwerking="2026-10-01"/></hoofdstuk>"#;
/// let doc = Document::parse(xml).unwrap();
/// let artikel = doc.descendants().find(|n| n.has_tag_name("artikel")).unwrap();
///
/// let blocker = not_in_force_on(artikel, "2026-01-01").unwrap();
/// assert_eq!(blocker.attribute("inwerking"), Some("2026-10-01"));
/// ```
pub fn not_in_force_on<'a, 'input>(node: Node<'a, 'input>, date: &str) -> Option<Node<'a, 'input>> {
    node.ancestors().filter(Node::is_element).find(|n| {
        n.attribute("status") == Some("nogniet")
            || n.attribute("inwerking").is_some_and(|from| from > date)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::Document;

    #[test]
    fn test_get_tag_name() {
        let xml = r#"<root><child/></root>"#;
        let doc = Document::parse(xml).unwrap();
        assert_eq!(get_tag_name(doc.root_element()), "root");
    }

    #[test]
    fn test_get_tag_name_with_namespace() {
        let xml = r#"<ns:root xmlns:ns="http://example.com"><ns:child/></ns:root>"#;
        let doc = Document::parse(xml).unwrap();
        assert_eq!(get_tag_name(doc.root_element()), "root");
    }

    #[test]
    fn test_find_child() {
        let xml = r#"<root><a/><b/><c/></root>"#;
        let doc = Document::parse(xml).unwrap();
        let root = doc.root_element();

        assert!(find_child(root, "a").is_some());
        assert!(find_child(root, "b").is_some());
        assert!(find_child(root, "d").is_none());
    }

    #[test]
    fn test_find_children() {
        let xml = r#"<root><item>1</item><other/><item>2</item></root>"#;
        let doc = Document::parse(xml).unwrap();
        let root = doc.root_element();

        let items: Vec<_> = find_children(root, "item").collect();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_find_by_path() {
        let xml = r#"<root><level1><level2><target>found</target></level2></level1></root>"#;
        let doc = Document::parse(xml).unwrap();
        let root = doc.root_element();

        let target = find_by_path(root, "level1/level2/target");
        assert!(target.is_some());
        assert_eq!(get_text(target.unwrap()), "found");

        assert!(find_by_path(root, "missing/path").is_none());
    }

    #[test]
    fn test_get_text() {
        let xml = r#"<root>  trimmed text  </root>"#;
        let doc = Document::parse(xml).unwrap();
        assert_eq!(get_text(doc.root_element()), "trimmed text");
    }

    #[test]
    fn test_get_attribute() {
        let xml = r#"<root attr="value"/>"#;
        let doc = Document::parse(xml).unwrap();
        let root = doc.root_element();

        assert_eq!(get_attribute(root, "attr"), Some("value"));
        assert_eq!(get_attribute(root, "missing"), None);
    }

    #[test]
    fn test_has_tag() {
        let xml = r#"<artikel/>"#;
        let doc = Document::parse(xml).unwrap();
        let root = doc.root_element();

        assert!(has_tag(root, "artikel"));
        assert!(!has_tag(root, "other"));
    }

    #[test]
    fn test_element_children() {
        let xml = r#"<root>text<child1/>more<child2/></root>"#;
        let doc = Document::parse(xml).unwrap();
        let root = doc.root_element();

        let children: Vec<_> = element_children(root).collect();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_find_bijlage_context_inside_bijlage() {
        let xml = r#"<wet>
            <bijlage>
                <kop><nr>1</nr><titel>Bijlage 1</titel></kop>
                <artikel><kop><nr>1</nr></kop></artikel>
            </bijlage>
        </wet>"#;
        let doc = Document::parse(xml).unwrap();
        let artikel = doc
            .descendants()
            .find(|n| n.is_element() && get_tag_name(*n) == "artikel")
            .unwrap();

        let ctx = find_bijlage_context(artikel);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().number, "1");
    }

    #[test]
    fn test_find_bijlage_context_outside_bijlage() {
        let xml = r#"<wet>
            <artikel><kop><nr>1</nr></kop></artikel>
        </wet>"#;
        let doc = Document::parse(xml).unwrap();
        let artikel = doc
            .descendants()
            .find(|n| n.is_element() && get_tag_name(*n) == "artikel")
            .unwrap();

        let ctx = find_bijlage_context(artikel);
        assert!(ctx.is_none());
    }

    #[test]
    fn test_find_bijlage_context_nested_bijlage() {
        let xml = r#"<wet>
            <bijlage>
                <kop><nr>2</nr></kop>
                <deel>
                    <artikel><kop><nr>1.a</nr></kop></artikel>
                </deel>
            </bijlage>
        </wet>"#;
        let doc = Document::parse(xml).unwrap();
        let artikel = doc
            .descendants()
            .find(|n| n.is_element() && get_tag_name(*n) == "artikel")
            .unwrap();

        let ctx = find_bijlage_context(artikel);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().number, "2");
    }

    #[test]
    fn test_not_in_force_on() {
        let xml = r#"<wet>
            <artikel id="goed" status="goed" inwerking="2020-01-01"/>
            <artikel id="later" status="goed" inwerking="2026-10-01"/>
            <artikel id="nogniet" status="nogniet"/>
            <hoofdstuk status="nogniet" inwerking="2027-01-01"><artikel id="binnen" status="goed"/></hoofdstuk>
        </wet>"#;
        let doc = Document::parse(xml).unwrap();
        let by_id = |id: &str| {
            doc.descendants()
                .find(|n| n.attribute("id") == Some(id))
                .unwrap()
        };

        assert!(not_in_force_on(by_id("goed"), "2025-01-01").is_none());
        assert!(not_in_force_on(by_id("later"), "2025-01-01").is_some());
        assert!(not_in_force_on(by_id("later"), "2026-10-01").is_none());
        assert!(not_in_force_on(by_id("nogniet"), "2099-01-01").is_some());
        let blocker = not_in_force_on(by_id("binnen"), "2026-01-01").unwrap();
        assert_eq!(get_tag_name(blocker), "hoofdstuk");
    }
}
