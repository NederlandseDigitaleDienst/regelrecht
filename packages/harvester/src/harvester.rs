//! Main harvester service that ties all components together.

use roxmltree::Document;

use reqwest::Client;

use crate::config::{
    validate_bwb_id, validate_date, validate_date_format, wetten_url, DEFAULT_MAX_RESPONSE_SIZE_MB,
};
use crate::content::download_content_xml;
use crate::error::{HarvesterError, NoTextReason, Result};
use crate::manifest::{
    consolidation_dates_between, download_manifest, has_consolidation, resolve_consolidation_date,
    resolve_valid_to, BwbManifest,
};
use crate::splitting::{create_dutch_law_hierarchy, LeafSplitStrategy, SplitContext, SplitEngine};
use crate::types::{Law, Preamble};
use crate::wti::{download_wti, WtiParseResult};
use crate::xml::{
    find_bijlage_context, find_by_path, find_children, get_tag_name, get_text, not_in_force_on,
};

/// Download and parse a Dutch law.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
/// * `date` - The date in YYYY-MM-DD format; the version in force on that date is harvested
///
/// # Returns
/// A `Law` object containing metadata, articles, and any warnings encountered during parsing
pub async fn download_law(client: &Client, bwb_id: &str, date: &str) -> Result<Law> {
    download_law_with_max_size(client, bwb_id, date, DEFAULT_MAX_RESPONSE_SIZE_MB).await
}

/// Download and parse a Dutch law with configurable max response size.
///
/// Same as [`download_law_version`] with a required date.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
/// * `date` - The date in YYYY-MM-DD format; the version in force on that date is harvested
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
    download_law_version(client, bwb_id, Some(date), max_size_mb).await
}

/// Download and parse the version of a Dutch law that is in force on `date`.
///
/// The BWB manifest maps `date` to the consolidation (toestand) that covers
/// it; `None` means today. A consolidation that starts after that date is
/// never taken, also not as the manifest's fallback for a law that has no
/// consolidation in force yet: harvest a period with
/// [`download_law_versions`] for that. The returned law carries that
/// consolidation date as `metadata.effective_date`, which is the version's
/// `valid_from`.
///
/// # Errors
/// Returns an error on an invalid id or date, a failed download, or a manifest
/// without a usable consolidation.
pub async fn download_law_version(
    client: &Client,
    bwb_id: &str,
    date: Option<&str>,
    max_size_mb: u64,
) -> Result<Law> {
    let date = date
        .map(str::to_string)
        .unwrap_or_else(regelrecht_shared::dates::today_str);
    validate_date(&date)?;
    let (manifest, wti) = download_sources(client, bwb_id).await?;
    let version_date = resolve_version_date(&manifest, bwb_id, &date)?;
    download_version(client, bwb_id, &manifest, wti, &version_date, max_size_mb).await
}

/// Download one version of a Dutch law per in-force date between `from` and
/// `to` (both inclusive).
///
/// Every consolidation in the BWB manifest starts on a date on which an
/// article changes or comes into force, so each one becomes a version.
/// `on_version` receives the versions in date order, each with its
/// consolidation date as `metadata.effective_date`, as soon as it is built,
/// so a caller can save it before the next download starts. Returns how many
/// versions there were; `0` means the law has no in-force date in the
/// period. The period may reach into the future.
///
/// # Errors
/// Returns an error on an invalid id or date, `from` after `to`, a failed
/// download of a version, or an error from `on_version`. The versions handed
/// over before that stay handed over.
pub async fn download_law_versions(
    client: &Client,
    bwb_id: &str,
    from: &str,
    to: &str,
    max_size_mb: u64,
    mut on_version: impl FnMut(Law) -> Result<()>,
) -> Result<usize> {
    // The bounds only filter the manifest's dates, so they may lie in the
    // future: BWB publishes a consolidation as soon as its date is set.
    validate_date_format(from)?;
    validate_date_format(to)?;
    if from > to {
        return Err(HarvesterError::InvalidPeriod {
            from: from.to_string(),
            to: to.to_string(),
        });
    }
    let (manifest, wti) = download_sources(client, bwb_id).await?;
    let dates = consolidation_dates_between(&manifest, from, to);
    for date in &dates {
        let law =
            download_version(client, bwb_id, &manifest, wti.clone(), date, max_size_mb).await?;
        on_version(law)?;
    }
    Ok(dates.len())
}

/// The start date of the consolidation in force on `date`.
///
/// The manifest falls back to its last consolidation when none covers `date`.
/// For a law that is enacted but has no consolidation in force yet, that one
/// starts after `date`: the law has no text on `date`, which is reported as
/// [`NoTextReason::NotYetInForce`] so the pipeline does not retry it.
fn resolve_version_date(manifest: &BwbManifest, bwb_id: &str, date: &str) -> Result<String> {
    let version_date = resolve_consolidation_date(manifest, Some(date))?;
    if version_date.as_str() > date {
        return Err(HarvesterError::NoConsolidatedText {
            bwb_id: bwb_id.to_string(),
            reason: NoTextReason::NotYetInForce { date: version_date },
        });
    }
    Ok(version_date)
}

/// Download the manifest and WTI metadata, which every version shares.
async fn download_sources(client: &Client, bwb_id: &str) -> Result<(BwbManifest, WtiParseResult)> {
    validate_bwb_id(bwb_id)?;
    let manifest = download_manifest(client, bwb_id).await?;
    let wti = download_wti(client, bwb_id).await?;
    Ok((manifest, wti))
}

/// Download the content XML of the consolidation at `version_date` and build
/// the version from it.
async fn download_version(
    client: &Client,
    bwb_id: &str,
    manifest: &BwbManifest,
    wti: WtiParseResult,
    version_date: &str,
    max_size_mb: u64,
) -> Result<Law> {
    let max_size_bytes = max_size_mb * 1024 * 1024;
    let content_xml = download_content_xml(client, bwb_id, version_date, max_size_bytes).await?;
    build_version(wti, manifest, &content_xml, bwb_id, version_date)
}

/// Build the version of a law at consolidation date `version_date`.
///
/// This is the offline half of [`download_law_version`] and
/// [`download_law_versions`]: [`parse_law`] plus what the manifest adds. It
/// sets the instrument end date (`valid_to`, RFC-019) and, for every article
/// left out because it is not yet in force, says whether BWB already has the
/// consolidation in which it does apply.
///
/// # Errors
/// Returns an error if the content XML cannot be parsed.
pub fn build_version(
    wti_result: WtiParseResult,
    manifest: &BwbManifest,
    content_xml: &str,
    bwb_id: &str,
    version_date: &str,
) -> Result<Law> {
    let mut law = assemble_law(
        wti_result,
        content_xml,
        bwb_id,
        version_date,
        Some(manifest),
    )?;
    law.metadata.valid_to = resolve_valid_to(manifest, version_date);
    Ok(law)
}

/// Build a `Law` from parsed WTI metadata and downloaded content XML.
///
/// This is the offline half of [`download_law_with_max_size`] without the
/// manifest: everything after the downloads. It is public so the pipeline can
/// be tested on fixture files.
///
/// Only what is in force on `date` ends up in the law: an article that is
/// enacted but not yet in force (BWB status `nogniet`, or an `inwerking` date
/// after `date`) is left out with a warning. `publication_date` is the latest
/// publication (`publicatie_bron`) among the elements in force on `date`.
///
/// Callers must pass a `bwb_id` and `date` that already passed
/// [`validate_bwb_id`] and [`validate_date`]; both feed the generated article
/// URLs. `bwb_id` should be the id the WTI was fetched for, which normally
/// equals `wti_result.metadata.bwb_id`; this function does not check that.
///
/// # Errors
/// Returns an error if the content XML cannot be parsed.
pub fn parse_law(
    wti_result: WtiParseResult,
    content_xml: &str,
    bwb_id: &str,
    date: &str,
) -> Result<Law> {
    assemble_law(wti_result, content_xml, bwb_id, date, None)
}

/// Shared core of [`parse_law`] and [`build_version`].
fn assemble_law(
    wti_result: WtiParseResult,
    content_xml: &str,
    bwb_id: &str,
    date: &str,
    manifest: Option<&BwbManifest>,
) -> Result<Law> {
    // Parse articles from content
    let parsed = parse_articles(content_xml, bwb_id, date)?;

    // Combine warnings from WTI parsing and content parsing
    let mut warnings = wti_result.warnings;
    warnings.extend(parsed.warnings);
    warnings.extend(
        parsed
            .not_in_force
            .iter()
            .map(|article| article.warning(date, manifest)),
    );

    if parsed.publication_date.is_none() {
        warnings.push(format!(
            "No publicatie_bron found among the elements in force on {date}; publication_date falls back to valid_from"
        ));
    }

    let mut metadata = wti_result.metadata;
    metadata.publication_date = parsed.publication_date;
    metadata.effective_date = Some(date.to_string());

    Ok(Law {
        metadata,
        preamble: parsed.preamble,
        articles: parsed.articles,
        warnings,
    })
}

/// An article left out of a version because it is not yet in force.
struct NotInForce {
    number: String,
    /// The `inwerking` date of the element that holds it back, if BWB knows it.
    in_force_from: Option<String>,
}

impl NotInForce {
    fn warning(&self, date: &str, manifest: Option<&BwbManifest>) -> String {
        let number = &self.number;
        // A `nogniet` element can carry an `inwerking` that has already passed;
        // that date says nothing about when the article will apply.
        let from = self.in_force_from.as_deref().filter(|from| *from > date);
        match (from, manifest) {
            (None, _) => format!(
                "Article {number}: not in force on {date} and no in-force date is set yet; left out of this version"
            ),
            (Some(from), Some(manifest)) if has_consolidation(manifest, from) => format!(
                "Article {number}: not in force on {date}; left out of this version, it is in the version of {from}"
            ),
            (Some(from), Some(_)) => format!(
                "Article {number}: not in force on {date}; it comes into force on {from}, but BWB has no consolidation for that date yet"
            ),
            (Some(from), None) => format!(
                "Article {number}: not in force on {date}; it comes into force on {from}, left out of this version"
            ),
        }
    }
}

/// Parsed content from XML.
struct ParsedContent {
    preamble: Option<Preamble>,
    articles: Vec<crate::types::Article>,
    warnings: Vec<String>,
    not_in_force: Vec<NotInForce>,
    publication_date: Option<String>,
}

/// Parse articles from content XML.
///
/// Returns parsed content including optional preamble, articles, and warnings.
fn parse_articles(xml: &str, bwb_id: &str, date: &str) -> Result<ParsedContent> {
    let doc = Document::parse(xml)?;
    let mut articles = Vec::new();
    let mut all_warnings: Vec<String> = Vec::new();
    let mut not_in_force = Vec::new();

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

        // An enacted article that is not yet in force belongs to a later version.
        if let Some(blocker) = not_in_force_on(artikel, date) {
            not_in_force.push(NotInForce {
                number: artikel_nr,
                in_force_from: blocker.attribute("inwerking").map(str::to_string),
            });
            continue;
        }

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
        not_in_force,
        publication_date: latest_publication(&doc, date),
    })
}

/// The latest publication date among the elements in force on `date`.
///
/// Every element of a BWB toestand records the publication of the act that
/// gave it its current text (`publicatie_bron`). The newest one among the
/// elements in force is the last publication this version depends on, which
/// is the version's publication date. Elements that are not yet in force are
/// skipped: their publication belongs to a later version.
fn latest_publication(doc: &Document<'_>, date: &str) -> Option<String> {
    doc.descendants()
        .filter_map(|n| n.attribute("publicatie_bron").map(|d| (n, d)))
        .filter(|(_, published)| validate_date_format(published).is_ok())
        .filter(|(n, _)| not_in_force_on(*n, date).is_none())
        .map(|(_, published)| published)
        .max()
        .map(str::to_string)
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
    fn test_resolve_version_date_never_takes_a_future_consolidation() {
        let xml = r#"<work label="BWBR0000001" _latestItem="2027-01-01_0/xml/BWBR0000001_2027-01-01_0.xml">
            <expression label="2027-01-01_0"><metadata>
              <datum_inwerkingtreding>2027-01-01</datum_inwerkingtreding>
            </metadata></expression></work>"#;
        let manifest = crate::manifest::parse_manifest(xml, "BWBR0000001").unwrap();

        match resolve_version_date(&manifest, "BWBR0000001", "2026-11-02") {
            Err(HarvesterError::NoConsolidatedText {
                reason: NoTextReason::NotYetInForce { date },
                ..
            }) => assert_eq!(date, "2027-01-01"),
            other => panic!("expected NotYetInForce, got {other:?}"),
        }
        assert_eq!(
            resolve_version_date(&manifest, "BWBR0000001", "2027-03-01").unwrap(),
            "2027-01-01"
        );
    }

    #[test]
    fn test_missing_publication_date_is_warned_about() {
        let xml = r#"<toestand><artikel label="Artikel 1" status="goed"><kop><nr>1</nr></kop><al>Tekst.</al></artikel></toestand>"#;
        let wti = WtiParseResult {
            metadata: crate::types::LawMetadata {
                bwb_id: "BWBR0000001".to_string(),
                cvdr_id: None,
                title: "Test".to_string(),
                regulatory_layer: crate::types::RegulatoryLayer::Wet,
                publication_date: None,
                effective_date: None,
                valid_to: None,
                creator: None,
                scope_code: None,
            },
            warnings: Vec::new(),
        };
        let law = parse_law(wti, xml, "BWBR0000001", "2025-01-01").unwrap();
        assert_eq!(law.metadata.publication_date, None);
        assert!(law
            .warnings
            .iter()
            .any(|w| w.contains("publication_date falls back to valid_from")));
    }

    #[test]
    fn test_not_in_force_warning_ignores_a_past_inwerking() {
        let article = NotInForce {
            number: "13a".to_string(),
            in_force_from: Some("2020-01-01".to_string()),
        };
        assert_eq!(
            article.warning("2025-01-01", None),
            "Article 13a: not in force on 2025-01-01 and no in-force date is set yet; left out of this version"
        );
    }

    #[test]
    fn test_extract_simple_text() {
        let xml = "<al>Hello <nadruk>world</nadruk>!</al>";
        let doc = Document::parse(xml).unwrap();
        let text = extract_simple_text(doc.root_element());
        assert_eq!(text, "Hello world!");
    }
}
