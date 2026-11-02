//! WTI (Wetstechnische Informatie) metadata file parsing.
//!
//! WTI files contain metadata about Dutch laws, including:
//! - BWB identifier
//! - Official title (citeertitel)
//! - Type of regulation (soort-regeling)
//!
//! The publication date of a version is not taken from the WTI: it lists every
//! amending act of the law, not the one a given version depends on. It comes
//! from the content XML instead (see `harvester::parse_law`).

use reqwest::Client;
use roxmltree::Document;

use crate::config::wti_url;
use crate::error::{HarvesterError, Result};
use crate::http::{bytes_to_string, download_bytes_default};
use crate::types::{LawMetadata, RegulatoryLayer};

/// Download WTI (metadata) XML content for a law.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
///
/// # Returns
/// Raw XML content as a string
pub async fn download_wti_xml(client: &Client, bwb_id: &str) -> Result<String> {
    let url = wti_url(bwb_id);
    let bytes = download_bytes_default(client, &url).await.map_err(|e| {
        if let HarvesterError::Http(source) = e {
            HarvesterError::WtiDownload {
                bwb_id: bwb_id.to_string(),
                source,
            }
        } else {
            e
        }
    })?;

    Ok(bytes_to_string(
        bytes,
        &format!("WTI metadata for {bwb_id}"),
    ))
}

/// Download and parse WTI metadata for a law.
///
/// This is a convenience function that downloads the WTI XML and parses it
/// into `LawMetadata`.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0018451")
///
/// # Returns
/// `WtiParseResult` with extracted metadata and any warnings
pub async fn download_wti(client: &Client, bwb_id: &str) -> Result<WtiParseResult> {
    let xml = download_wti_xml(client, bwb_id).await?;
    let doc = Document::parse(&xml)?;
    Ok(parse_wti_metadata(&doc))
}

/// Parsed WTI metadata including any warnings encountered.
#[derive(Debug, Clone)]
pub struct WtiParseResult {
    /// The extracted metadata.
    pub metadata: LawMetadata,
    /// Warnings encountered during parsing (e.g., ambiguous regulatory layer mapping).
    pub warnings: Vec<String>,
}

/// Extract metadata from WTI XML document.
///
/// # Arguments
/// * `doc` - Parsed WTI XML document
///
/// # Returns
/// `WtiParseResult` with extracted metadata and any warnings
pub fn parse_wti_metadata(doc: &Document<'_>) -> WtiParseResult {
    let root = doc.root_element();

    // BWB ID from attribute
    let bwb_id = root.attribute("bwb-id").unwrap_or_default().to_string();

    // Title - prefer citeertitel with status="officieel"
    let title = find_official_title(doc).unwrap_or_else(|| find_any_title(doc).unwrap_or_default());

    // Regulatory layer from soort-regeling
    let (regulatory_layer, layer_warning) = find_regulatory_layer(doc);

    let mut warnings = Vec::new();
    if let Some(warning) = layer_warning {
        warnings.push(warning);
    }

    WtiParseResult {
        metadata: LawMetadata {
            bwb_id,
            cvdr_id: None,
            title,
            regulatory_layer,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        },
        warnings,
    }
}

/// Find official title (citeertitel with status="officieel").
fn find_official_title(doc: &Document<'_>) -> Option<String> {
    doc.descendants()
        .find(|n| n.has_tag_name("citeertitel") && n.attribute("status") == Some("officieel"))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Find any title (fallback).
fn find_any_title(doc: &Document<'_>) -> Option<String> {
    doc.descendants()
        .find(|n| n.has_tag_name("citeertitel"))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Find regulatory layer from soort-regeling.
///
/// Returns `(layer, warning)` where the warning is present for ambiguous or unknown types.
fn find_regulatory_layer(doc: &Document<'_>) -> (RegulatoryLayer, Option<String>) {
    doc.descendants()
        .find(|n| n.has_tag_name("soort-regeling"))
        .and_then(|n| n.text())
        .map(crate::types::regulatory_layer_from_soort_regeling)
        .unwrap_or((RegulatoryLayer::Wet, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WTI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0018451">
  <citeertitel status="officieel">Wet op de zorgtoeslag</citeertitel>
  <citeertitel>Zorgtoeslag</citeertitel>
  <soort-regeling>wet</soort-regeling>
  <publicatiedatum>2005-12-29</publicatiedatum>
</wti-metagegevens>"#;

    #[test]
    fn test_parse_wti_metadata_basic() {
        let doc = Document::parse(SAMPLE_WTI).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(result.metadata.bwb_id, "BWBR0018451");
        assert_eq!(result.metadata.title, "Wet op de zorgtoeslag");
        assert_eq!(result.metadata.regulatory_layer, RegulatoryLayer::Wet);
        // The WTI is not the source of a version's publication date.
        assert_eq!(result.metadata.publication_date, None);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_wti_metadata_fallback_title() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0000001">
  <citeertitel>Fallback Title</citeertitel>
</wti-metagegevens>"#;

        let doc = Document::parse(xml).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(result.metadata.title, "Fallback Title");
    }

    #[test]
    fn test_parse_wti_metadata_amvb() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0000001">
  <citeertitel status="officieel">Test AMVB</citeertitel>
  <soort-regeling>amvb</soort-regeling>
</wti-metagegevens>"#;

        let doc = Document::parse(xml).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(result.metadata.regulatory_layer, RegulatoryLayer::Amvb);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_wti_metadata_ministeriele_regeling() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0000001">
  <citeertitel status="officieel">Test Regeling</citeertitel>
  <soort-regeling>ministeriële regeling</soort-regeling>
</wti-metagegevens>"#;

        let doc = Document::parse(xml).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(
            result.metadata.regulatory_layer,
            RegulatoryLayer::MinisterieleRegeling
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_wti_metadata_missing_optional_fields() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0000001">
</wti-metagegevens>"#;

        let doc = Document::parse(xml).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(result.metadata.bwb_id, "BWBR0000001");
        assert_eq!(result.metadata.title, "");
        assert_eq!(result.metadata.regulatory_layer, RegulatoryLayer::Wet);
        assert_eq!(result.metadata.publication_date, None);
    }

    #[test]
    fn test_parse_wti_metadata_koninklijk_besluit_maps_to_amvb_with_warning() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0000001">
  <citeertitel status="officieel">Test KB</citeertitel>
  <soort-regeling>koninklijk besluit</soort-regeling>
</wti-metagegevens>"#;

        let doc = Document::parse(xml).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(result.metadata.regulatory_layer, RegulatoryLayer::Amvb);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("koninklijk besluit"));
    }

    #[test]
    fn test_parse_wti_metadata_unknown_type_warns() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wti-metagegevens bwb-id="BWBR0000001">
  <citeertitel status="officieel">Unknown</citeertitel>
  <soort-regeling>something_unknown</soort-regeling>
</wti-metagegevens>"#;

        let doc = Document::parse(xml).unwrap();
        let result = parse_wti_metadata(&doc);

        assert_eq!(result.metadata.regulatory_layer, RegulatoryLayer::Wet);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("something_unknown"));
    }
}
