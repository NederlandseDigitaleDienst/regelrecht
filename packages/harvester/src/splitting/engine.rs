//! Split engine that orchestrates article splitting using the hierarchy registry.

use roxmltree::Node;

use super::registry::HierarchyRegistry;
use super::strategy::SplitStrategy;
use super::types::{ArticleComponent, ElementSpec, SplitContext};
use crate::registry::{create_content_registry, ParseContext, ParseEngine, ReferenceCollector};
use crate::xml::get_tag_name;

/// Engine for splitting articles using hierarchy schema.
///
/// Walks the XML tree according to the hierarchy specification and
/// produces `ArticleComponent` objects at split points.
pub struct SplitEngine<S: SplitStrategy> {
    hierarchy: HierarchyRegistry,
    strategy: S,
    parse_engine: ParseEngine,
    /// When true, each `artikel` is emitted as a SINGLE component with its
    /// leden/onderdelen folded inline into the text (keeping their enumerators),
    /// instead of being split into one component per lid/onderdeel. This is the
    /// harvester default; finer splitting is an explicit author choice made later
    /// in the editor. Leaf-splitting is retained behind `false` for tests.
    article_level: bool,
}

impl<S: SplitStrategy> SplitEngine<S> {
    /// Create a new split engine (leaf-splitting; `article_level` off).
    #[must_use]
    pub fn new(hierarchy: HierarchyRegistry, strategy: S) -> Self {
        let registry = create_content_registry();
        let parse_engine = ParseEngine::new(registry);
        Self {
            hierarchy,
            strategy,
            parse_engine,
            article_level: false,
        }
    }

    /// Enable article-level extraction: one component per `artikel`, with
    /// leden/onderdelen folded inline (enumerators preserved).
    #[must_use]
    pub fn with_article_level(mut self, article_level: bool) -> Self {
        self.article_level = article_level;
        self
    }

    /// Split an element into components based on hierarchy.
    pub fn split(&self, node: Node<'_, '_>, context: SplitContext) -> Vec<ArticleComponent> {
        let tag = get_tag_name(node);
        let Some(spec) = self.hierarchy.get_spec(tag) else {
            tracing::warn!(
                tag = %tag,
                "Unknown element in splitting hierarchy, skipping"
            );
            return Vec::new();
        };

        let mut components = Vec::new();

        // Get number for this element and update context
        let context = if let Some(number) = self.strategy.get_number(node, spec) {
            context.with_number(number)
        } else {
            context
        };

        // Multi-version articles: extract as single component, don't split by lid
        // These have <tussenkop> elements that separate different versions
        if tag == "artikel" && self.has_version_separator(node) {
            if let Some(component) = self.extract_full_article_content(node, spec, &context) {
                return vec![component];
            }
            return Vec::new();
        }

        // Article-level mode (harvester default): emit one component per artikel
        // with leden/onderdelen folded inline (enumerators preserved), instead of
        // splitting into one component per lid/onderdeel.
        if tag == "artikel" && self.article_level {
            if let Some(component) = self.extract_article_level_content(node, spec, &context) {
                return vec![component];
            }
            return Vec::new();
        }

        // Find structural children
        let structural_children = self.find_structural_children(node, spec);

        if !structural_children.is_empty() {
            // Has structural children - extract intro and recurse
            components.extend(self.process_with_structural_children(
                node,
                spec,
                &context,
                &structural_children,
            ));
        } else if self.strategy.should_split_here(node, spec, &context) {
            // Leaf node - extract content
            if let Some(component) = self.extract_leaf_content(node, spec, &context) {
                components.push(component);
            }
        }

        components
    }

    /// Find structural children according to spec.
    ///
    /// Checks children in priority order and returns the first
    /// matching type found. Unmarked lists (type="ongemarkeerd") are
    /// excluded - their content is extracted inline instead.
    ///
    /// **Multiple sibling lists:** When multiple `<lijst>` elements appear
    /// as direct siblings (not separated by `<lid>`), they are kept inline
    /// rather than split. This prevents duplicate numbers like `B2:1.a` when
    /// bijlagen have multiple lists that each start their own enumeration.
    fn find_structural_children<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        spec: &ElementSpec,
    ) -> Vec<Node<'a, 'input>> {
        for child_tag in &spec.children {
            let children: Vec<_> = node
                .children()
                .filter(|child| {
                    child.is_element()
                        && get_tag_name(*child) == child_tag
                        && !self.is_effectively_unmarked_list(*child)
                })
                .collect();

            if !children.is_empty() {
                // Multiple sibling lists at the same level cause duplicate numbers
                // (each list starts its own a, b, c enumeration). Keep them inline.
                if child_tag == "lijst" && children.len() > 1 {
                    return Vec::new();
                }
                return children;
            }
        }
        Vec::new()
    }

    /// Check if element is an unmarked list (explicit attribute).
    fn is_unmarked_list(&self, node: Node<'_, '_>) -> bool {
        get_tag_name(node) == "lijst" && node.attribute("type") == Some("ongemarkeerd")
    }

    /// Check if an artikel contains multiple versions (phased implementation).
    ///
    /// Articles with "gefaseerde inwerkingtreding" (phased implementation) contain
    /// multiple versions of the same content separated by `<tussenkop>` elements.
    /// These articles should NOT be split by lid to avoid duplicate numbers.
    fn has_version_separator(&self, node: Node<'_, '_>) -> bool {
        node.children()
            .any(|c| c.is_element() && get_tag_name(c) == "tussenkop")
    }

    /// Check if a node is editorial content (not law text).
    ///
    /// `<redactie>` elements contain editorial annotations like:
    /// - "Dit artikel is gewijzigd in verband met..."
    /// - "Voor overige gevallen luidt het artikel als volgt:"
    /// - "Vervallen."
    ///
    /// These must be excluded from extracted text as they are NOT part of the law.
    fn is_editorial_content(&self, node: Node<'_, '_>) -> bool {
        get_tag_name(node) == "redactie"
    }

    /// Check if a list is "effectively unmarked" - all items have non-addressable markers.
    ///
    /// This includes:
    /// - Lists with `type="ongemarkeerd"` attribute
    /// - Lists where ALL `li.nr` elements contain only bullets (•), dashes (–/-/—), or are empty
    ///
    /// These lists should be kept inline rather than split into separate components.
    fn is_effectively_unmarked_list(&self, node: Node<'_, '_>) -> bool {
        if get_tag_name(node) != "lijst" {
            return false;
        }

        // Explicit ongemarkeerd attribute
        if node.attribute("type") == Some("ongemarkeerd") {
            return true;
        }

        // Check all li children - if ALL have non-addressable markers, treat as unmarked
        let li_items: Vec<_> = node
            .children()
            .filter(|c| c.is_element() && get_tag_name(*c) == "li")
            .collect();

        if li_items.is_empty() {
            return false;
        }

        li_items.iter().all(|li| {
            let nr_text = li
                .children()
                .find(|c| c.is_element() && get_tag_name(*c) == "li.nr")
                .and_then(|n| n.text())
                .map(|s| s.trim())
                .unwrap_or("");

            // Non-addressable markers: empty, bullet, or various dash characters
            nr_text.is_empty()
                || nr_text == "•"
                || nr_text == "–" // en-dash
                || nr_text == "-" // hyphen
                || nr_text == "—" // em-dash
        })
    }

    /// Process an element that has structural children.
    fn process_with_structural_children<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        spec: &ElementSpec,
        context: &SplitContext,
        structural_children: &[Node<'a, 'input>],
    ) -> Vec<ArticleComponent> {
        let mut components = Vec::new();

        // Extract intro text before structural children
        if self.strategy.should_split_here(node, spec, context) {
            if let Some(intro) = self.extract_intro_text(node, spec, context, structural_children) {
                components.push(intro);
            }
        }

        // Recurse into structural children
        for child in structural_children {
            components.extend(self.split(*child, context.clone()));
        }

        components
    }

    /// Extract intro text that appears before structural children.
    fn extract_intro_text<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        spec: &ElementSpec,
        context: &SplitContext,
        structural_children: &[Node<'a, 'input>],
    ) -> Option<ArticleComponent> {
        let mut collector = ReferenceCollector::new();
        let mut parts: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let first_structural = structural_children.first();

        for child in node.children() {
            // Stop when we hit the first structural child
            if first_structural.is_some() && Some(&child) == first_structural {
                break;
            }

            if !child.is_element() {
                continue;
            }

            let child_tag = get_tag_name(child);

            // Skip number elements
            if spec.skip_for_number.contains(&child_tag.to_string()) {
                continue;
            }

            // Extract content from content tags
            if spec.content_tags.contains(&child_tag.to_string()) {
                let (text, errs) = self.extract_inline_text_with_warnings(
                    child,
                    &mut collector,
                    &context.bwb_id,
                    &context.date,
                );
                warnings.extend(errs);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }

        if parts.is_empty() {
            return None;
        }

        Some(
            ArticleComponent::new(
                context.number_parts.clone(),
                parts.join(" ").trim().to_string(),
                context.base_url.clone(),
            )
            .with_bijlage_prefix(context.bijlage_prefix.clone())
            .with_references(collector.into_references())
            .with_warnings(warnings),
        )
    }

    /// Extract all content from a leaf element.
    ///
    /// Lists encountered here are kept inline (they weren't treated as
    /// structural children, e.g., due to multiple sibling lists).
    fn extract_leaf_content(
        &self,
        node: Node<'_, '_>,
        spec: &ElementSpec,
        context: &SplitContext,
    ) -> Option<ArticleComponent> {
        let mut collector = ReferenceCollector::new();
        let mut parts: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        for child in node.children() {
            if !child.is_element() {
                continue;
            }

            let child_tag = get_tag_name(child);

            // Skip number elements
            if spec.skip_for_number.contains(&child_tag.to_string()) {
                continue;
            }

            // Extract content from content tags
            if spec.content_tags.contains(&child_tag.to_string()) {
                let (text, errs) = self.extract_inline_text_with_warnings(
                    child,
                    &mut collector,
                    &context.bwb_id,
                    &context.date,
                );
                warnings.extend(errs);
                if !text.is_empty() {
                    parts.push(text);
                }
            } else if child_tag == "lijst" {
                // Lists encountered here are kept inline (not split as structural children)
                // This happens for unmarked lists or multiple sibling lists
                let text = if self.is_effectively_unmarked_list(child) {
                    self.extract_unmarked_list_text(
                        child,
                        &mut collector,
                        &context.bwb_id,
                        &context.date,
                    )
                } else {
                    self.extract_marked_list_text(
                        child,
                        &mut collector,
                        &context.bwb_id,
                        &context.date,
                    )
                };
                if !text.is_empty() {
                    parts.push(text);
                }
            } else if !self.hierarchy.is_structural(child_tag) {
                // Also extract from non-structural elements
                let (text, errs) = self.extract_inline_text_with_warnings(
                    child,
                    &mut collector,
                    &context.bwb_id,
                    &context.date,
                );
                warnings.extend(errs);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }

        if parts.is_empty() {
            return None;
        }

        Some(
            ArticleComponent::new(
                context.number_parts.clone(),
                parts.join(" ").trim().to_string(),
                context.base_url.clone(),
            )
            .with_bijlage_prefix(context.bijlage_prefix.clone())
            .with_references(collector.into_references())
            .with_warnings(warnings),
        )
    }

    /// Extract full article content without splitting by lid.
    ///
    /// Used for articles with multiple versions (gefaseerde inwerkingtreding).
    /// These contain `<tussenkop>` elements that separate different versions
    /// and should NOT be split into individual lid components.
    fn extract_full_article_content(
        &self,
        node: Node<'_, '_>,
        spec: &ElementSpec,
        context: &SplitContext,
    ) -> Option<ArticleComponent> {
        let mut collector = ReferenceCollector::new();
        let mut parts: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        for child in node.children() {
            if !child.is_element() {
                continue;
            }

            let child_tag = get_tag_name(child);

            // Skip kop (number already extracted)
            if spec.skip_for_number.contains(&child_tag.to_string()) {
                continue;
            }

            // Skip meta-data elements
            if child_tag == "meta-data" {
                continue;
            }

            // Extract content recursively (extract_inline_text already skips redactie)
            let (text, errs) = self.extract_element_recursive_with_warnings(
                child,
                &mut collector,
                &context.bwb_id,
                &context.date,
            );
            warnings.extend(errs);
            if !text.is_empty() {
                parts.push(text);
            }
        }

        if parts.is_empty() {
            return None;
        }

        Some(
            ArticleComponent::new(
                context.number_parts.clone(),
                parts.join("\n\n").trim().to_string(),
                context.base_url.clone(),
            )
            .with_bijlage_prefix(context.bijlage_prefix.clone())
            .with_references(collector.into_references())
            .with_warnings(warnings),
        )
    }

    /// Extract a whole article at article-level granularity.
    ///
    /// Produces ONE component for the artikel, with its leden/onderdelen folded
    /// inline as blank-line-separated blocks that keep their enumerators
    /// (`1.`, `a.`, `1°`), matching how whole-article laws (e.g.
    /// `zorgverzekeringswet`) are already stored so the existing markdown
    /// rendering shows the structure. Marked lists become separate blocks;
    /// unmarked/dash/bullet lists stay inline; `redactie`/`tussenkop`/`meta-data`
    /// are skipped.
    fn extract_article_level_content(
        &self,
        node: Node<'_, '_>,
        spec: &ElementSpec,
        context: &SplitContext,
    ) -> Option<ArticleComponent> {
        let mut collector = ReferenceCollector::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut blocks: Vec<String> = Vec::new();

        for child in node.children() {
            if !child.is_element() {
                continue;
            }
            let child_tag = get_tag_name(child);

            // Skip kop (number already extracted) and meta-data.
            if spec.skip_for_number.contains(&child_tag.to_string()) || child_tag == "meta-data" {
                continue;
            }

            let (mut child_blocks, errs) =
                self.collect_article_blocks(child, &mut collector, &context.bwb_id, &context.date);
            warnings.extend(errs);
            blocks.append(&mut child_blocks);
        }

        if blocks.is_empty() {
            return None;
        }

        Some(
            ArticleComponent::new(
                context.number_parts.clone(),
                blocks.join("\n\n").trim().to_string(),
                context.base_url.clone(),
            )
            .with_bijlage_prefix(context.bijlage_prefix.clone())
            .with_references(collector.into_references())
            .with_warnings(warnings),
        )
    }

    /// Collect the blank-line-separated text blocks for one element, recursing
    /// through lid/lijst/li and re-attaching their enumerators as prefixes.
    ///
    /// - `al` → one block (inline text).
    /// - `lid` → its content blocks, with the `lidnr` prefixed onto the first.
    /// - `lijst` (marked) → one block per `li`; unmarked/dash/bullet lists are
    ///   kept inline as a single block (reusing `extract_unmarked_list_text`).
    /// - `li` → its content blocks, with the `li.nr` marker prefixed onto the first.
    ///
    /// Editorial (`redactie`), `tussenkop` and `meta-data` are skipped.
    fn collect_article_blocks(
        &self,
        node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> (Vec<String>, Vec<String>) {
        let tag = get_tag_name(node);
        let mut blocks: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        if self.is_editorial_content(node) || tag == "tussenkop" || tag == "meta-data" {
            return (blocks, warnings);
        }

        match tag {
            "al" => {
                let (text, errs) =
                    self.extract_inline_text_with_warnings(node, collector, bwb_id, date);
                warnings.extend(errs);
                if !text.is_empty() {
                    blocks.push(text);
                }
            }
            "lid" => {
                let prefix = Self::enumerator(node, "lidnr");
                let mut child_blocks = self.collect_marker_children(
                    node,
                    "lidnr",
                    collector,
                    bwb_id,
                    date,
                    &mut warnings,
                );
                Self::prepend_marker(&mut child_blocks, prefix);
                blocks.append(&mut child_blocks);
            }
            "li" => {
                let marker = Self::enumerator(node, "li.nr");
                let mut child_blocks = self.collect_marker_children(
                    node,
                    "li.nr",
                    collector,
                    bwb_id,
                    date,
                    &mut warnings,
                );
                Self::prepend_marker(&mut child_blocks, marker);
                blocks.append(&mut child_blocks);
            }
            "lijst" => {
                if self.is_effectively_unmarked_list(node) {
                    // Non-addressable markers (bullets/dashes/unmarked) stay inline.
                    let text = self.extract_unmarked_list_text(node, collector, bwb_id, date);
                    if !text.is_empty() {
                        blocks.push(text);
                    }
                } else {
                    for li in node.children() {
                        if !li.is_element() || get_tag_name(li) != "li" {
                            continue;
                        }
                        let (mut li_blocks, errs) =
                            self.collect_article_blocks(li, collector, bwb_id, date);
                        warnings.extend(errs);
                        blocks.append(&mut li_blocks);
                    }
                }
            }
            _ => {
                // Unknown/other inline element: extract as a single block.
                let (text, errs) =
                    self.extract_inline_text_with_warnings(node, collector, bwb_id, date);
                warnings.extend(errs);
                if !text.is_empty() {
                    blocks.push(text);
                }
            }
        }

        (blocks, warnings)
    }

    /// Collect blocks from a lid/li's children, skipping the marker element.
    fn collect_marker_children(
        &self,
        node: Node<'_, '_>,
        marker_tag: &str,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
        warnings: &mut Vec<String>,
    ) -> Vec<String> {
        let mut blocks: Vec<String> = Vec::new();
        for child in node.children() {
            if !child.is_element() {
                continue;
            }
            if get_tag_name(child) == marker_tag {
                continue;
            }
            let (mut child_blocks, errs) =
                self.collect_article_blocks(child, collector, bwb_id, date);
            warnings.extend(errs);
            blocks.append(&mut child_blocks);
        }
        blocks
    }

    /// Read and trim a marker element (e.g. `lidnr`, `li.nr`) directly under `node`.
    fn enumerator(node: Node<'_, '_>, marker_tag: &str) -> Option<String> {
        node.children()
            .find(|c| c.is_element() && get_tag_name(*c) == marker_tag)
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Prefix the enumerator onto the first block (e.g. `a.` + `wet: ...` →
    /// `a. wet: ...`), or stand alone if the element had no text blocks.
    fn prepend_marker(blocks: &mut Vec<String>, marker: Option<String>) {
        let Some(marker) = marker else { return };
        match blocks.first_mut() {
            Some(first) => *first = format!("{marker} {first}"),
            None => blocks.push(marker),
        }
    }

    /// Recursively extract text from an element and its children.
    ///
    /// Handles nested structures like lid > al, lijst > li > al, etc.
    /// Editorial content is automatically skipped.
    ///
    /// Returns `(text, warnings)` tuple where warnings are non-fatal parse errors.
    fn extract_element_recursive_with_warnings(
        &self,
        node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> (String, Vec<String>) {
        let tag = get_tag_name(node);

        // Skip editorial content
        if self.is_editorial_content(node) {
            return (String::new(), Vec::new());
        }

        // Skip tussenkop - it's just a version separator, not content
        if tag == "tussenkop" {
            return (String::new(), Vec::new());
        }

        // Skip meta-data
        if tag == "meta-data" {
            return (String::new(), Vec::new());
        }

        // Content elements - extract inline text
        if tag == "al" {
            return self.extract_inline_text_with_warnings(node, collector, bwb_id, date);
        }

        // Structural elements - recurse into children
        if tag == "lid" || tag == "lijst" || tag == "li" {
            let mut parts: Vec<String> = Vec::new();
            let mut warnings: Vec<String> = Vec::new();

            // For lid, include the lid number as prefix
            if tag == "lid" {
                if let Some(nr) = node
                    .children()
                    .find(|c| c.is_element() && get_tag_name(*c) == "lidnr")
                    .and_then(|n| n.text())
                {
                    parts.push(nr.trim().to_string());
                }
            }

            // For li, include the li.nr as prefix
            if tag == "li" {
                if let Some(nr) = node
                    .children()
                    .find(|c| c.is_element() && get_tag_name(*c) == "li.nr")
                    .and_then(|n| n.text())
                {
                    parts.push(nr.trim().to_string());
                }
            }

            for child in node.children() {
                if !child.is_element() {
                    continue;
                }

                let child_tag = get_tag_name(child);

                // Skip number elements - already handled above
                if child_tag == "lidnr" || child_tag == "li.nr" {
                    continue;
                }

                let (text, errs) =
                    self.extract_element_recursive_with_warnings(child, collector, bwb_id, date);
                warnings.extend(errs);
                if !text.is_empty() {
                    parts.push(text);
                }
            }

            return (parts.join(" "), warnings);
        }

        // Unknown element - try inline extraction
        self.extract_inline_text_with_warnings(node, collector, bwb_id, date)
    }

    /// Extract inline text from an element using the registry handlers.
    ///
    /// This processes `<extref>`, `<intref>`, `<nadruk>`, and other inline
    /// elements through their registered handlers, enabling proper markdown
    /// link generation and reference collection.
    ///
    /// Editorial content (`<redactie>`) is always skipped - it's not law text.
    ///
    /// Returns `(text, warnings)` tuple where warnings are non-fatal parse errors.
    fn extract_inline_text_with_warnings(
        &self,
        node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> (String, Vec<String>) {
        // Skip editorial content - not law text
        if self.is_editorial_content(node) {
            return (String::new(), Vec::new());
        }

        let mut parse_context = ParseContext::new(bwb_id, date).with_collector(collector);

        // Try to parse using the registry engine
        if let Ok(result) = self.parse_engine.parse(node, &mut parse_context) {
            return (result.text.trim().to_string(), result.errors);
        }

        // Fallback to simple text extraction if handler not found
        (
            self.extract_simple_text(node, collector, bwb_id, date),
            Vec::new(),
        )
    }

    /// Extract inline text from an element using the registry handlers.
    ///
    /// This is a convenience wrapper that discards warnings. Use
    /// `extract_inline_text_with_warnings` when warning collection is needed.
    fn extract_inline_text(
        &self,
        node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> String {
        self.extract_inline_text_with_warnings(node, collector, bwb_id, date)
            .0
    }

    /// Simple text extraction fallback.
    ///
    /// Editorial content (`<redactie>`) is always skipped - it's not law text.
    fn extract_simple_text(
        &self,
        node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> String {
        // Skip editorial content - not law text
        if self.is_editorial_content(node) {
            return String::new();
        }

        let mut text = String::new();

        if let Some(t) = node.text() {
            text.push_str(t);
        }

        for child in node.children() {
            if child.is_element() {
                // Try registry first, fall back to recursive simple extraction
                let child_text = self.extract_inline_text(child, collector, bwb_id, date);
                text.push_str(&child_text);
            }
            if let Some(tail) = child.tail() {
                text.push_str(tail);
            }
        }

        text
    }

    /// Extract text from an unmarked list, preserving list structure.
    fn extract_unmarked_list_text(
        &self,
        lijst_node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> String {
        let mut parts: Vec<String> = Vec::new();

        for li in lijst_node.children() {
            if !li.is_element() || get_tag_name(li) != "li" {
                continue;
            }

            let mut li_parts: Vec<String> = Vec::new();

            for child in li.children() {
                if !child.is_element() {
                    continue;
                }

                let child_tag = get_tag_name(child);

                if child_tag == "li.nr" {
                    continue; // Skip the dash/bullet marker
                }

                if child_tag == "al" {
                    let text = self.extract_inline_text(child, collector, bwb_id, date);
                    if !text.is_empty() {
                        li_parts.push(text);
                    }
                } else if child_tag == "lijst" {
                    // Handle nested lists (both marked and unmarked)
                    // For definitions with sub-items like "woonruimte: 1° ... 2° ..."
                    let nested = if self.is_unmarked_list(child) {
                        self.extract_unmarked_list_text(child, collector, bwb_id, date)
                    } else {
                        // Marked list - extract all items inline
                        self.extract_marked_list_text(child, collector, bwb_id, date)
                    };
                    if !nested.is_empty() {
                        li_parts.push(nested);
                    }
                }
            }

            if !li_parts.is_empty() {
                parts.push(format!("- {}", li_parts.join(" ")));
            }
        }

        parts.join("\n")
    }

    /// Extract text from a marked (expliciet) list, joining items with their markers.
    ///
    /// Used for nested marked lists within definition items (e.g., "woonruimte: 1° ... 2° ...").
    fn extract_marked_list_text(
        &self,
        lijst_node: Node<'_, '_>,
        collector: &mut ReferenceCollector,
        bwb_id: &str,
        date: &str,
    ) -> String {
        let mut parts: Vec<String> = Vec::new();

        for li in lijst_node.children() {
            if !li.is_element() || get_tag_name(li) != "li" {
                continue;
            }

            let mut li_parts: Vec<String> = Vec::new();

            // Get the marker (li.nr) text
            let marker = li
                .children()
                .find(|c| c.is_element() && get_tag_name(*c) == "li.nr")
                .and_then(|n| n.text())
                .map(|s| s.trim().to_string());

            for child in li.children() {
                if !child.is_element() {
                    continue;
                }

                let child_tag = get_tag_name(child);

                if child_tag == "li.nr" {
                    continue;
                }

                if child_tag == "al" {
                    let text = self.extract_inline_text(child, collector, bwb_id, date);
                    if !text.is_empty() {
                        li_parts.push(text);
                    }
                } else if child_tag == "lijst" {
                    // Recursively handle nested lists
                    let nested = if self.is_unmarked_list(child) {
                        self.extract_unmarked_list_text(child, collector, bwb_id, date)
                    } else {
                        self.extract_marked_list_text(child, collector, bwb_id, date)
                    };
                    if !nested.is_empty() {
                        li_parts.push(nested);
                    }
                }
            }

            if !li_parts.is_empty() {
                let item_text = li_parts.join(" ");
                if let Some(m) = marker {
                    parts.push(format!("{m} {item_text}"));
                } else {
                    parts.push(item_text);
                }
            }
        }

        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::splitting::config::create_dutch_law_hierarchy;
    use crate::splitting::strategy::LeafSplitStrategy;

    fn leaf_engine() -> SplitEngine<LeafSplitStrategy> {
        SplitEngine::new(create_dutch_law_hierarchy(), LeafSplitStrategy)
    }

    /// The configuration every production caller uses (`harvester.rs`,
    /// `cvdr/parse.rs`). Leaf mode is only reachable from tests.
    fn article_level_engine() -> SplitEngine<LeafSplitStrategy> {
        leaf_engine().with_article_level(true)
    }

    /// Both engine modes, labelled for assertion messages. Regressions that
    /// must hold regardless of granularity run against both, so the
    /// production (article-level) path is never left uncovered.
    fn both_engines() -> [(&'static str, SplitEngine<LeafSplitStrategy>); 2] {
        [
            ("leaf", leaf_engine()),
            ("article-level", article_level_engine()),
        ]
    }

    fn context() -> SplitContext {
        SplitContext::new("BWBR0018451", "2025-01-01", "https://example.com")
    }

    fn split_with(
        engine: &SplitEngine<LeafSplitStrategy>,
        xml: &str,
        context: SplitContext,
    ) -> Vec<ArticleComponent> {
        let doc = roxmltree::Document::parse(xml).unwrap();
        engine.split(doc.root_element(), context)
    }

    fn split_leaf(xml: &str) -> Vec<ArticleComponent> {
        split_with(&leaf_engine(), xml, context())
    }

    fn split_article(xml: &str) -> Vec<ArticleComponent> {
        split_with(&article_level_engine(), xml, context())
    }

    fn numbers(components: &[ArticleComponent]) -> Vec<String> {
        components.iter().map(ArticleComponent::to_number).collect()
    }

    #[test]
    fn test_split_simple_artikel() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al>Article text here.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            assert_eq!(components[0].to_number(), "1", "{mode}");
            assert_eq!(components[0].text, "Article text here.", "{mode}");
            assert!(components[0].warnings.is_empty(), "{mode}");
        }
    }

    #[test]
    fn test_split_artikel_with_lid() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>First paragraph text.</al>
            </lid>
            <lid>
                <lidnr>2.</lidnr>
                <al>Second paragraph text.</al>
            </lid>
        </artikel>"#;

        let components = split_leaf(xml);

        assert_eq!(numbers(&components), ["1.1", "1.2"]);
    }

    #[test]
    fn test_split_artikel_with_lijst() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>In deze wet wordt verstaan onder:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>first item;</al></li>
                    <li><li.nr>b.</li.nr><al>second item.</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        let components = split_leaf(xml);

        // Should have: intro + 2 list items = 3 components
        assert_eq!(numbers(&components), ["1.1", "1.1.a", "1.1.b"]);
    }

    #[test]
    fn test_split_artikel_with_extref() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al>verzekerde: de persoon, bedoeld in <extref doc="jci1.3:c:BWBR0018450&amp;artikel=1">artikel 1 van de Zorgverzekeringswet</extref>.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            // Should contain markdown link
            assert!(
                components[0]
                    .text
                    .contains("[artikel 1 van de Zorgverzekeringswet][ref1]"),
                "{mode}: {}",
                components[0].text
            );
            // Should have reference definition
            assert_eq!(components[0].references.len(), 1, "{mode}");
            assert_eq!(components[0].references[0].id, "ref1", "{mode}");
            assert_eq!(components[0].references[0].bwb_id, "BWBR0018450", "{mode}");
            assert_eq!(
                components[0].references[0].artikel,
                Some("1".to_string()),
                "{mode}"
            );
        }
    }

    #[test]
    fn test_split_artikel_with_multiple_refs() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al>Zie <extref doc="jci1.3:c:BWBR0018450&amp;artikel=1">artikel 1</extref> en <extref doc="jci1.3:c:BWBR0018450&amp;artikel=2">artikel 2</extref>.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            let refs = &components[0].references;
            assert_eq!(refs.len(), 2, "{mode}");
            assert_eq!(refs[0].id, "ref1", "{mode}");
            assert_eq!(refs[0].artikel, Some("1".to_string()), "{mode}");
            assert_eq!(refs[1].id, "ref2", "{mode}");
            assert_eq!(refs[1].artikel, Some("2".to_string()), "{mode}");
        }
    }

    #[test]
    fn test_split_artikel_with_intref() {
        let xml = r#"<artikel>
            <kop><nr>2</nr></kop>
            <al>Zie <intref doc="jci1.3:c:BWBR0018451&amp;artikel=1">artikel 1</intref> van deze wet.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            assert!(
                components[0].text.contains("[artikel 1][ref1]"),
                "{mode}: {}",
                components[0].text
            );
            assert_eq!(components[0].references.len(), 1, "{mode}");
            assert_eq!(components[0].references[0].bwb_id, "BWBR0018451", "{mode}");
        }
    }

    /// References inside leden and onderdelen are numbered across the whole
    /// artikel in article-level mode, since the artikel is one component with
    /// one reference list.
    #[test]
    fn test_article_level_numbers_refs_across_leden() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>Zie <extref doc="jci1.3:c:BWBR0018450&amp;artikel=1">artikel 1</extref>:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>en <extref doc="jci1.3:c:BWBR0018450&amp;artikel=2">artikel 2</extref>;</al></li>
                </lijst>
            </lid>
            <lid>
                <lidnr>2.</lidnr>
                <al>Zie <intref doc="jci1.3:c:BWBR0018451&amp;artikel=3">artikel 3</intref>.</al>
            </lid>
        </artikel>"#;

        let components = split_article(xml);

        assert_eq!(components.len(), 1);
        let text = &components[0].text;
        assert!(text.contains("1. Zie [artikel 1][ref1]:"), "got:\n{text}");
        assert!(text.contains("a. en [artikel 2][ref2];"), "got:\n{text}");
        assert!(text.contains("2. Zie [artikel 3][ref3]."), "got:\n{text}");
        let refs = &components[0].references;
        assert_eq!(
            refs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["ref1", "ref2", "ref3"]
        );
        assert_eq!(refs[2].bwb_id, "BWBR0018451");
        assert_eq!(refs[2].artikel, Some("3".to_string()));
    }

    #[test]
    fn test_split_artikel_with_bijlage_prefix() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>First paragraph in bijlage.</al>
            </lid>
        </artikel>"#;

        for (mode, engine, expected) in [
            ("leaf", leaf_engine(), "B1:1.1"),
            ("article-level", article_level_engine(), "B1:1"),
        ] {
            let components = split_with(&engine, xml, context().with_bijlage_prefix("B1"));

            assert_eq!(components.len(), 1, "{mode}");
            assert_eq!(components[0].to_number(), expected, "{mode}");
            assert_eq!(
                components[0].bijlage_prefix,
                Some("B1".to_string()),
                "{mode}"
            );
        }
    }

    #[test]
    fn test_split_artikel_without_bijlage_prefix() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al>Regular article text.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            assert_eq!(components[0].to_number(), "1", "{mode}");
            assert_eq!(components[0].bijlage_prefix, None, "{mode}");
        }
    }

    #[test]
    fn test_split_artikel_with_nested_marked_list_in_definition() {
        // Tests definitions like "woonruimte: 1° ... 2° ..." where a definition term
        // has sub-items in a nested marked list
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>In deze wet wordt verstaan onder:</al>
                <lijst type="ongemarkeerd">
                    <li>
                        <al><nadruk type="cur">term:</nadruk> simple definition;</al>
                    </li>
                    <li>
                        <al><nadruk type="cur">woonruimte:</nadruk></al>
                        <lijst type="expliciet">
                            <li>
                                <li.nr>1°</li.nr>
                                <al>besloten ruimte die bestemd is voor bewoning, en</al>
                            </li>
                            <li>
                                <li.nr>2°</li.nr>
                                <al>standplaats;</al>
                            </li>
                        </lijst>
                    </li>
                    <li>
                        <al><nadruk type="cur">other:</nadruk> another definition.</al>
                    </li>
                </lijst>
            </lid>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            let text = &components[0].text;

            // Should contain the term
            assert!(text.contains("*woonruimte:*"), "{mode}: {text}");

            // Should contain the nested list items with their markers
            assert!(
                text.contains("1° besloten ruimte"),
                "{mode}: missing nested list item 1°: {text}"
            );
            assert!(
                text.contains("2° standplaats"),
                "{mode}: missing nested list item 2°: {text}"
            );
        }
    }

    #[test]
    fn test_split_artikel_with_dash_list_inline() {
        // Lists with dash markers should be kept inline, not split into separate components
        // This fixes the issue where articles like Wetboek van Strafrecht 421.1.b
        // were incorrectly split because dash-marked lists weren't recognized as unmarked
        let xml = r#"<artikel>
            <kop><nr>421</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>Intro text:</al>
                <lijst>
                    <li><li.nr>–</li.nr><al>first item;</al></li>
                    <li><li.nr>–</li.nr><al>second item.</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            // Should produce 1 component with all items inline (not split into 3)
            assert_eq!(components.len(), 1, "{mode}: dash list should stay inline");
            let text = &components[0].text;
            assert!(text.contains("first item"), "{mode}: {text}");
            assert!(text.contains("second item"), "{mode}: {text}");
        }

        // Article-level keeps the whole dash list as one block after the lid
        // intro: the dashes are not onderdelen, so the items get no block of
        // their own.
        let text = &split_article(xml)[0].text;
        assert_eq!(text.matches("\n\n").count(), 1, "got:\n{text}");
    }

    #[test]
    fn test_split_artikel_with_bullet_list_inline() {
        // Lists with bullet markers (•) should also be kept inline
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>Requirements:</al>
                <lijst>
                    <li><li.nr>•</li.nr><al>requirement one;</al></li>
                    <li><li.nr>•</li.nr><al>requirement two.</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(
                components.len(),
                1,
                "{mode}: bullet list should stay inline"
            );
            let text = &components[0].text;
            assert!(text.contains("requirement one"), "{mode}: {text}");
            assert!(text.contains("requirement two"), "{mode}: {text}");
        }

        // Article-level keeps the whole bullet list as one block after the
        // lid intro, like the dash list above.
        let text = &split_article(xml)[0].text;
        assert_eq!(text.matches("\n\n").count(), 1, "got:\n{text}");
    }

    #[test]
    fn test_split_artikel_with_mixed_markers_still_splits() {
        // A list with mixed markers (some addressable, some not) should still be split
        // because the addressable items need their own components
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>Definitions:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>first definition;</al></li>
                    <li><li.nr>–</li.nr><al>note about definitions;</al></li>
                    <li><li.nr>b.</li.nr><al>second definition.</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        // The dash item keeps its own component, numbered by its marker.
        let components = split_leaf(xml);
        assert_eq!(numbers(&components), ["1.1", "1.1.a", "1.1.–", "1.1.b"]);
        assert_eq!(components[2].text, "note about definitions;");

        // Article-level: every item is its own block, keeping its marker.
        let components = split_article(xml);
        assert_eq!(components.len(), 1);
        let blocks: Vec<&str> = components[0].text.split("\n\n").collect();
        assert_eq!(
            blocks,
            [
                "1. Definitions:",
                "a. first definition;",
                "– note about definitions;",
                "b. second definition.",
            ]
        );
    }

    #[test]
    fn test_split_artikel_excludes_redactie() {
        // Editorial content (<redactie>) should not appear in extracted text
        // as it's not law text but editorial annotations
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al><redactie type="extra">This is editorial, not law text.</redactie></al>
            <al>This is actual law text.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            assert_eq!(components[0].text, "This is actual law text.", "{mode}");
        }
    }

    #[test]
    fn test_split_artikel_with_multiple_versions_stays_together() {
        // Articles with gefaseerde inwerkingtreding (phased implementation) contain
        // multiple versions separated by <tussenkop>. They should stay as one component.
        let xml = r#"<artikel>
            <kop><nr>8:36c</nr></kop>
            <al><redactie type="extra">Editorial note about this article.</redactie></al>
            <lid>
                <lidnr>1</lidnr>
                <al>Version 1 lid 1 text.</al>
            </lid>
            <lid>
                <lidnr>2</lidnr>
                <al>Version 1 lid 2 text.</al>
            </lid>
            <al><redactie type="extra">Voor overige gevallen luidt het artikel als volgt:</redactie></al>
            <tussenkop>Artikel 8:36c.</tussenkop>
            <lid>
                <lidnr>1</lidnr>
                <al>Version 2 lid 1 text.</al>
            </lid>
            <lid>
                <lidnr>2</lidnr>
                <al>Version 2 lid 2 text.</al>
            </lid>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            // Single component with both versions
            assert_eq!(components.len(), 1, "{mode}");
            assert_eq!(components[0].to_number(), "8:36c", "{mode}");

            let text = &components[0].text;
            // Both versions should be present
            assert!(text.contains("Version 1 lid 1"), "{mode}: {text}");
            assert!(text.contains("Version 2 lid 2"), "{mode}: {text}");

            // Editorial content should be excluded
            assert!(!text.contains("Editorial note"), "{mode}: {text}");
            assert!(!text.contains("overige gevallen"), "{mode}: {text}");
        }
    }

    #[test]
    fn test_split_artikel_redactie_in_lid() {
        // Redactie inside lid elements should also be excluded
        let xml = r#"<artikel>
            <kop><nr>5</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>Actual paragraph text.</al>
                <al><redactie type="vervanging">Vervallen.</redactie></al>
            </lid>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            let text = &components[0].text;
            assert!(!text.contains("Vervallen"), "{mode}: {text}");
            assert!(text.contains("Actual paragraph text"), "{mode}: {text}");
        }
    }

    #[test]
    fn test_split_artikel_with_multiple_sibling_lists_keeps_inline() {
        // Multiple sibling lists at the same level should be kept inline
        // to prevent duplicates like B2:1.a appearing multiple times
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al>Introduction to first list:</al>
            <lijst>
                <li><li.nr>a.</li.nr><al>first list item a;</al></li>
                <li><li.nr>b.</li.nr><al>first list item b.</al></li>
            </lijst>
            <al>Introduction to second list:</al>
            <lijst>
                <li><li.nr>a.</li.nr><al>second list item a;</al></li>
                <li><li.nr>b.</li.nr><al>second list item b.</al></li>
            </lijst>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context().with_bijlage_prefix("B2"));

            // Multiple sibling lists should be kept inline - single component
            assert_eq!(components.len(), 1, "{mode}");
            assert_eq!(components[0].to_number(), "B2:1", "{mode}");

            // All content should be present
            let text = &components[0].text;
            assert!(text.contains("first list item a"), "{mode}: {text}");
            assert!(text.contains("first list item b"), "{mode}: {text}");
            assert!(text.contains("second list item a"), "{mode}: {text}");
            assert!(text.contains("second list item b"), "{mode}: {text}");
        }
    }

    #[test]
    fn test_split_artikel_with_lid_wrapped_lists() {
        // Lists inside separate lids should still split (each lid has one list)
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>First lid intro:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>lid 1 item a;</al></li>
                    <li><li.nr>b.</li.nr><al>lid 1 item b.</al></li>
                </lijst>
            </lid>
            <lid>
                <lidnr>2.</lidnr>
                <al>Second lid intro:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>lid 2 item a;</al></li>
                    <li><li.nr>b.</li.nr><al>lid 2 item b.</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        let components = split_leaf(xml);

        // Each lid has ONE list, so they should split normally, with unique
        // numbers (no duplicates).
        assert_eq!(
            numbers(&components),
            ["1.1", "1.1.a", "1.1.b", "1.2", "1.2.a", "1.2.b"]
        );
    }

    #[test]
    fn test_article_level_folds_lid_and_onderdelen_into_one_component() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>In dit besluit wordt verstaan onder:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>wet: de Zorgverzekeringswet;</al></li>
                    <li><li.nr>b.</li.nr><al>verblijf: verblijf gedurende het etmaal;</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        let components = split_article(xml);

        // One component for the whole article (not split per onderdeel).
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].to_number(), "1");

        // Onderdeel enumerators are preserved, as blank-line-separated blocks.
        let text = &components[0].text;
        assert!(text.contains("In dit besluit wordt verstaan onder:"));
        assert!(
            text.contains("a. wet: de Zorgverzekeringswet;"),
            "missing onderdeel a marker, got:\n{text}"
        );
        assert!(
            text.contains("b. verblijf: verblijf gedurende het etmaal;"),
            "missing onderdeel b marker, got:\n{text}"
        );
        assert!(
            text.contains("\n\n"),
            "blocks should be blank-line separated"
        );
    }

    #[test]
    fn test_article_level_keeps_nested_sub_items_with_markers() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>wordt verstaan onder:</al>
                <lijst>
                    <li>
                        <li.nr>e.</li.nr>
                        <al>in-vitrofertilisatiepoging: zorg, inhoudende:</al>
                        <lijst>
                            <li><li.nr>1°</li.nr><al>de follikelpunctie;</al></li>
                            <li><li.nr>2°</li.nr><al>de bevruchting;</al></li>
                        </lijst>
                    </li>
                </lijst>
            </lid>
        </artikel>"#;

        let components = split_article(xml);

        assert_eq!(components.len(), 1);
        let text = &components[0].text;
        assert!(
            text.contains("e. in-vitrofertilisatiepoging"),
            "got:\n{text}"
        );
        assert!(text.contains("1° de follikelpunctie;"), "got:\n{text}");
        assert!(text.contains("2° de bevruchting;"), "got:\n{text}");
    }

    #[test]
    fn test_article_level_multiple_lids_one_component_with_lidnrs() {
        let xml = r#"<artikel>
            <kop><nr>2</nr></kop>
            <lid><lidnr>1.</lidnr><al>Eerste lid tekst.</al></lid>
            <lid><lidnr>2.</lidnr><al>Tweede lid tekst.</al></lid>
        </artikel>"#;

        let components = split_article(xml);

        assert_eq!(components.len(), 1);
        assert_eq!(components[0].to_number(), "2");
        let text = &components[0].text;
        assert!(text.contains("1. Eerste lid tekst."), "got:\n{text}");
        assert!(text.contains("2. Tweede lid tekst."), "got:\n{text}");
        // Two lid blocks, blank-line separated.
        assert_eq!(text.matches("\n\n").count(), 1, "got:\n{text}");
    }

    #[test]
    fn test_article_level_vervallen_lid_keeps_lone_enumerator() {
        // A repealed ("Vervallen") lid has only editorial content, so its body is
        // empty. We still emit the lid number as a lone block so the structure
        // reads "1." / "2." with the repealed lid visible (prepend_marker None arm).
        let xml = r#"<artikel>
            <kop><nr>3</nr></kop>
            <lid><lidnr>1.</lidnr><al><redactie type="vervanging">Vervallen.</redactie></al></lid>
            <lid><lidnr>2.</lidnr><al>Tweede lid tekst.</al></lid>
        </artikel>"#;

        let components = split_article(xml);

        assert_eq!(components.len(), 1);
        let text = &components[0].text;
        assert!(
            !text.contains("Vervallen"),
            "editorial excluded, got:\n{text}"
        );
        // The empty lid keeps its number as a standalone block.
        assert!(
            text.split("\n\n").any(|b| b.trim() == "1."),
            "repealed lid should keep a lone '1.' block, got:\n{text}"
        );
        assert!(text.contains("2. Tweede lid tekst."), "got:\n{text}");
    }

    #[test]
    fn test_split_artikel_with_unknown_element_produces_warning() {
        // An unknown element inside <al> should produce a warning but still extract text
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <al>Text before <unknown_element>unknown content</unknown_element> text after.</al>
        </artikel>"#;

        for (mode, engine) in both_engines() {
            let components = split_with(&engine, xml, context());

            assert_eq!(components.len(), 1, "{mode}");
            // Text should still be extracted (graceful degradation)
            let text = &components[0].text;
            assert!(text.contains("Text before"), "{mode}: {text}");
            assert!(text.contains("text after"), "{mode}: {text}");
            // Should have a warning about the unknown element
            let warnings = &components[0].warnings;
            assert_eq!(warnings.len(), 1, "{mode}: {warnings:?}");
            assert!(
                warnings[0].contains("unknown_element"),
                "{mode}: warning should name the element: {warnings:?}"
            );
        }
    }

    /// An unknown element nested in a lid and onderdeel still surfaces its
    /// warning at article level, where the walk goes through
    /// `collect_article_blocks` instead of the leaf extractor.
    #[test]
    fn test_article_level_unknown_element_in_onderdeel_produces_warning() {
        let xml = r#"<artikel>
            <kop><nr>1</nr></kop>
            <lid>
                <lidnr>1.</lidnr>
                <al>Intro:</al>
                <lijst>
                    <li><li.nr>a.</li.nr><al>item <unknown_element>x</unknown_element> tekst;</al></li>
                </lijst>
            </lid>
        </artikel>"#;

        let components = split_article(xml);

        assert_eq!(components.len(), 1);
        let warnings = &components[0].warnings;
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("unknown_element"), "{warnings:?}");
    }
}
