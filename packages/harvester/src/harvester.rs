//! Main harvester service that ties all components together.

use roxmltree::Document;

use reqwest::Client;

use crate::config::{validate_bwb_id, validate_date, wetten_url, DEFAULT_MAX_RESPONSE_SIZE};
use crate::content::download_content_xml;
use crate::error::Result;
use crate::splitting::{create_dutch_law_hierarchy, LeafSplitStrategy, SplitContext, SplitEngine};
use crate::types::{Law, Preamble};
use crate::wti::{download_wti, WtiParseResult};
use crate::xml::{find_bijlage_context, find_by_path, find_children, get_tag_name, get_text};

/// Download and parse a Dutch law.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
/// * `date` - The effective date in YYYY-MM-DD format
///
/// # Returns
/// A `Law` object containing metadata, articles, and any warnings encountered during parsing
pub async fn download_law(client: &Client, bwb_id: &str, date: &str) -> Result<Law> {
    download_law_with_max_size(
        client,
        bwb_id,
        date,
        DEFAULT_MAX_RESPONSE_SIZE / (1024 * 1024),
    )
    .await
}

/// Download and parse a Dutch law with configurable max response size.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
/// * `date` - The effective date in YYYY-MM-DD format
/// * `max_size_mb` - Maximum response size in megabytes
///
/// # Returns
/// A `Law` object containing metadata, articles, and any warnings encountered during parsing
pub async fn download_law_with_max_size(
    client: &Client,
    bwb_id: &str,
    date: &str,
    max_size_mb: u64,
) -> Result<Law> {
    // Validate inputs
    validate_bwb_id(bwb_id)?;
    validate_date(date)?;

    let max_size_bytes = max_size_mb * 1024 * 1024;

    // Download and parse WTI metadata
    let wti_result = download_wti(client, bwb_id).await?;

    // Download content XML
    let content_xml = download_content_xml(client, bwb_id, date, max_size_bytes).await?;

    parse_law(wti_result, &content_xml, bwb_id, date)
}

/// Build a `Law` from parsed WTI metadata and downloaded content XML.
///
/// This is the offline half of [`download_law_with_max_size`]: everything after
/// the downloads. It is public so the pipeline can be tested on fixture files.
///
/// # Errors
/// Returns an error if the content XML cannot be parsed.
pub fn parse_law(
    wti_result: WtiParseResult,
    content_xml: &str,
    bwb_id: &str,
    date: &str,
) -> Result<Law> {
    // Parse articles from content
    let parsed = parse_articles(content_xml, bwb_id, date)?;

    // Combine warnings from WTI parsing and content parsing
    let mut warnings = wti_result.warnings;
    warnings.extend(parsed.warnings);

    Ok(Law {
        metadata: wti_result.metadata,
        preamble: parsed.preamble,
        articles: parsed.articles,
        warnings,
    })
}

/// Parsed content from XML.
struct ParsedContent {
    preamble: Option<Preamble>,
    articles: Vec<crate::types::Article>,
    warnings: Vec<String>,
}

/// Parse articles from content XML.
///
/// Returns parsed content including optional preamble, articles, and warnings.
fn parse_articles(xml: &str, bwb_id: &str, date: &str) -> Result<ParsedContent> {
    let doc = Document::parse(xml)?;
    let mut articles = Vec::new();
    let mut all_warnings: Vec<String> = Vec::new();

    // Extract aanhef (preamble)
    let preamble = extract_aanhef(&doc, bwb_id, date);

    // Create split engine. Article-level granularity is the harvester default:
    // one entry per artikel with leden/onderdelen folded inline (enumerators kept).
    // Finer splitting stays an explicit author choice made later in the editor.
    let hierarchy = create_dutch_law_hierarchy();
    let engine = SplitEngine::new(hierarchy, LeafSplitStrategy).with_article_level(true);

    // Find all artikel elements
    for artikel in doc
        .descendants()
        .filter(|n| n.is_element() && get_tag_name(*n) == "artikel")
    {
        // Get article number
        let artikel_nr = if let Some(nr_node) = find_by_path(artikel, "kop/nr") {
            get_text(nr_node)
        } else if let Some(label) = artikel.attribute("label") {
            label.strip_prefix("Artikel ").unwrap_or(label).to_string()
        } else {
            continue; // Skip articles without number
        };

        // Detect bijlage context
        let bijlage_context = find_bijlage_context(artikel);

        // Build base URL
        let artikel_nr_url = artikel_nr.replace(' ', "_");
        let base_url = wetten_url(bwb_id, Some(date), Some(&artikel_nr_url), None, None, None);

        // Create split context with bijlage prefix if applicable
        let mut context = SplitContext::new(bwb_id, date, base_url);
        if let Some(ctx) = bijlage_context {
            context = context.with_bijlage_prefix(format!("B{}", ctx.number));
        }

        // Split the artikel
        let components = engine.split(artikel, context);

        // Convert components to articles and collect warnings
        for component in components {
            // Collect warnings with article context
            for warning in &component.warnings {
                all_warnings.push(format!("Article {}: {}", component.to_number(), warning));
            }
            articles.push(component.to_article());
        }
    }

    Ok(ParsedContent {
        preamble,
        articles,
        warnings: all_warnings,
    })
}

/// Extract the aanhef (preamble) as a separate Preamble object.
fn extract_aanhef(doc: &Document<'_>, bwb_id: &str, date: &str) -> Option<Preamble> {
    let aanhef = doc
        .descendants()
        .find(|n| n.is_element() && get_tag_name(*n) == "aanhef")?;

    let mut parts: Vec<String> = Vec::new();

    // Extract <wij> element
    if let Some(wij) = find_children(aanhef, "wij").next() {
        if let Some(text) = wij.text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }
    }

    // Extract <considerans> elements
    if let Some(considerans) = find_children(aanhef, "considerans").next() {
        for al in considerans
            .descendants()
            .filter(|n| n.is_element() && get_tag_name(*n) == "considerans.al")
        {
            let text = extract_simple_text(al);
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }

    // Extract <afkondiging> element
    if let Some(afkondiging) = find_children(aanhef, "afkondiging").next() {
        for al in find_children(afkondiging, "al") {
            let text = extract_simple_text(al);
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }

    if parts.is_empty() {
        return None;
    }

    let text = parts.join("\n\n");
    let url = wetten_url(bwb_id, Some(date), Some("Aanhef"), None, None, None);

    Some(Preamble { text, url })
}

/// Simple text extraction from a node.
fn extract_simple_text(node: roxmltree::Node<'_, '_>) -> String {
    let mut text = String::new();

    if let Some(t) = node.text() {
        text.push_str(t);
    }

    for child in node.children() {
        if child.is_element() {
            text.push_str(&extract_simple_text(child));
        }
        if let Some(tail) = child.tail() {
            text.push_str(tail);
        }
    }

    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_text() {
        let xml = "<al>Hello <nadruk>world</nadruk>!</al>";
        let doc = Document::parse(xml).unwrap();
        let text = extract_simple_text(doc.root_element());
        assert_eq!(text, "Hello world!");
    }
}
