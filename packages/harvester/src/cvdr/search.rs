//! SRU search API for CVDR metadata retrieval.
//!
//! Uses the Overheid.nl SRU (Search/Retrieve via URL) service to look up
//! CVDR regulation metadata including title, creator, organisation type,
//! and the URL to the XML content.

use reqwest::Client;
use roxmltree::Document;

use crate::config::cvdr_sru_search_url;
use crate::error::{HarvesterError, Result};
use crate::http::{bytes_to_string, download_bytes_default};
use crate::types::{LawMetadata, RegulatoryLayer};

/// Metadata extracted from CVDR SRU search results.
#[derive(Debug, Clone)]
pub struct CvdrMetadata {
    /// CVDR identifier (e.g., "CVDR681386").
    pub cvdr_id: String,

    /// Official title of the regulation.
    pub title: String,

    /// Creator organisation name (e.g., "Gemeente Amsterdam").
    pub creator: String,

    /// Organisation type (e.g., "gemeenten", "provincies", "waterschappen").
    pub organisation_type: String,

    /// Mapped regulatory layer based on organisation type.
    pub regulatory_layer: RegulatoryLayer,

    /// Publication date (optional).
    pub publication_date: Option<String>,

    /// Effective date (optional).
    pub effective_date: Option<String>,

    /// URL to the XML content of the regulation.
    pub xml_url: String,

    /// Non-fatal warnings encountered during metadata extraction.
    pub warnings: Vec<String>,
}

impl CvdrMetadata {
    /// Convert CVDR metadata to a `LawMetadata` struct.
    ///
    /// # Arguments
    /// * `effective_date` - The effective date to use
    #[must_use]
    pub fn to_law_metadata(&self, effective_date: &str) -> LawMetadata {
        LawMetadata {
            bwb_id: String::new(),
            cvdr_id: Some(self.cvdr_id.clone()),
            title: self.title.clone(),
            regulatory_layer: self.regulatory_layer,
            publication_date: self.publication_date.clone(),
            effective_date: Some(effective_date.to_string()),
            valid_to: None,
            creator: Some(self.creator.clone()),
            scope_code: None, // Could be extracted from SRU if available
        }
    }
}

/// Map CVDR organisation type to `RegulatoryLayer`.
///
/// Accepts both plural forms (from SRU metadata, e.g., "gemeenten") and
/// singular forms (e.g., "gemeente") for robustness.
///
/// Returns `(layer, warning)` where warning is present for unknown types.
fn regulatory_layer_from_organisation_type(org_type: &str) -> (RegulatoryLayer, Option<String>) {
    match org_type.to_lowercase().as_str() {
        "gemeenten" | "gemeente" => (RegulatoryLayer::GemeentelijkeVerordening, None),
        "provincies" | "provincie" => (RegulatoryLayer::ProvincialeVerordening, None),
        "waterschappen" | "waterschap" => (RegulatoryLayer::WaterschapsVerordening, None),
        unknown => {
            tracing::warn!(
                organisation_type = %unknown,
                "Unknown CVDR organisation type, defaulting to GEMEENTELIJKE_VERORDENING"
            );
            (
                RegulatoryLayer::GemeentelijkeVerordening,
                Some(format!(
                    "Unknown organisation type '{unknown}', defaulting to GEMEENTELIJKE_VERORDENING"
                )),
            )
        }
    }
}

/// Search CVDR SRU API for regulation metadata.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `cvdr_id` - The CVDR identifier (e.g., "CVDR681386_1")
///
/// # Returns
/// `CvdrMetadata` with extracted metadata
pub async fn search_cvdr(client: &Client, cvdr_id: &str) -> Result<CvdrMetadata> {
    let url = cvdr_sru_search_url(cvdr_id);

    let bytes = download_bytes_default(client, &url).await.map_err(|e| {
        if let HarvesterError::Http(source) = e {
            HarvesterError::CvdrContentDownload {
                cvdr_id: cvdr_id.to_string(),
                source,
            }
        } else {
            e
        }
    })?;

    let xml_string = bytes_to_string(bytes, &format!("SRU search for {cvdr_id}"));
    parse_sru_response(&xml_string, cvdr_id)
}

/// Parse SRU XML response to extract CVDR metadata.
fn parse_sru_response(xml: &str, cvdr_id: &str) -> Result<CvdrMetadata> {
    let doc = Document::parse(xml)?;

    let mut warnings = Vec::new();

    // Check for records in the SRU response
    let record = doc
        .descendants()
        .find(|n| n.is_element() && local_name(n) == "recordData")
        .ok_or_else(|| HarvesterError::CvdrSearchFailed {
            cvdr_id: cvdr_id.to_string(),
            message: "No records found in SRU response".to_string(),
        })?;

    // Extract the gzd (overheid metadata) element within the record
    let gzd = record
        .descendants()
        .find(|n| n.is_element() && local_name(n) == "gzd")
        .unwrap_or(record);

    // Extract title from dcterms.title or dc.title
    let title = find_element_text(&gzd, "title").unwrap_or_default();

    // Extract creator (organisation name)
    let creator = find_element_text(&gzd, "creator").unwrap_or_default();

    // Extract organisation type from overheid.organisatietype
    let organisation_type = find_element_text(&gzd, "organisatietype").unwrap_or_default();

    // Map organisation type to regulatory layer
    let (regulatory_layer, layer_warning) =
        regulatory_layer_from_organisation_type(&organisation_type);
    if let Some(warning) = layer_warning {
        warnings.push(warning);
    }

    // Extract publication date (dcterms.available or dcterms.issued)
    let publication_date =
        find_element_text(&gzd, "issued").or_else(|| find_element_text(&gzd, "available"));

    // Extract effective date (overheidproduct:inwerkingtredingDatum)
    let effective_date = find_element_text(&gzd, "inwerkingtredingDatum");

    // Find the XML content URL from enrichedData or meta
    let xml_url = find_xml_content_url(&doc, cvdr_id)?;

    Ok(CvdrMetadata {
        cvdr_id: cvdr_id.to_string(),
        title,
        creator,
        organisation_type,
        regulatory_layer,
        publication_date,
        effective_date,
        xml_url,
        warnings,
    })
}

/// Get the local name of an XML element (without namespace).
fn local_name<'a>(node: &roxmltree::Node<'a, '_>) -> &'a str {
    node.tag_name().name()
}

/// Find a descendant element by local name and return its trimmed text content.
///
/// Works for any namespace (Dublin Core, overheid, overheidproduct, etc.)
/// because it matches on local name only.
fn find_element_text(parent: &roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    parent
        .descendants()
        .find(|n| n.is_element() && local_name(n) == name)
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Find the XML content URL from the SRU response.
///
/// Looks for the URL in enrichedData or constructs it from the CVDR ID.
fn find_xml_content_url(doc: &Document<'_>, cvdr_id: &str) -> Result<String> {
    // Try to find in enrichedData section (stukIdentifier or contentUrl)
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let name = local_name(&node);

        // Look for the XML content URL in various locations
        if name == "content" || name == "url" || name == "locatie" {
            if let Some(href) = node
                .attribute("src")
                .or_else(|| node.attribute("resourceIdentifier"))
                && (href.ends_with(".xml") || href.contains("/xml/"))
            {
                return Ok(href.to_string());
            }
            if let Some(text) = node.text() {
                let text = text.trim();
                if (text.ends_with(".xml") || text.contains("/xml/")) && text.starts_with("http") {
                    return Ok(text.to_string());
                }
            }
        }

        // Look for gzd:enrichedData containing the XML URL
        if name == "enrichedData" {
            for child in node.descendants() {
                if child.is_element()
                    && local_name(&child) == "stukIdentifier"
                    && let Some(text) = child.text()
                {
                    let text = text.trim();
                    if !text.is_empty() {
                        // The stukIdentifier often contains a URL to the XML
                        if text.starts_with("http") {
                            return Ok(text.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fallback: construct URL from CVDR ID using the known pattern
    // CVDR regulations are available at: https://lokaleregelgeving.overheid.nl/CVDRXXXXX/1 (XML endpoint)
    // Or via the SRU result itself which may contain the XML inline
    // Try the direct XML download pattern
    Ok(format!(
        "https://repository.overheid.nl/frbr/cvdr/{}/1/xml/{}_1.xml",
        cvdr_id.get(4..).unwrap_or_default(), // Strip "CVDR" prefix to get numeric part
        cvdr_id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regulatory_layer_from_organisation_type() {
        let (layer, warning) = regulatory_layer_from_organisation_type("gemeenten");
        assert_eq!(layer, RegulatoryLayer::GemeentelijkeVerordening);
        assert!(warning.is_none());

        let (layer, warning) = regulatory_layer_from_organisation_type("provincies");
        assert_eq!(layer, RegulatoryLayer::ProvincialeVerordening);
        assert!(warning.is_none());

        let (layer, warning) = regulatory_layer_from_organisation_type("waterschappen");
        assert_eq!(layer, RegulatoryLayer::WaterschapsVerordening);
        assert!(warning.is_none());

        let (layer, warning) = regulatory_layer_from_organisation_type("unknown_type");
        assert_eq!(layer, RegulatoryLayer::GemeentelijkeVerordening);
        assert!(warning.is_some());
    }

    #[test]
    fn test_regulatory_layer_from_organisation_type_case_insensitive() {
        let (layer, _) = regulatory_layer_from_organisation_type("Gemeenten");
        assert_eq!(layer, RegulatoryLayer::GemeentelijkeVerordening);

        let (layer, _) = regulatory_layer_from_organisation_type("WATERSCHAPPEN");
        assert_eq!(layer, RegulatoryLayer::WaterschapsVerordening);
    }

    #[test]
    fn test_regulatory_layer_from_organisation_type_singular() {
        let (layer, warning) = regulatory_layer_from_organisation_type("gemeente");
        assert_eq!(layer, RegulatoryLayer::GemeentelijkeVerordening);
        assert!(warning.is_none());

        let (layer, warning) = regulatory_layer_from_organisation_type("provincie");
        assert_eq!(layer, RegulatoryLayer::ProvincialeVerordening);
        assert!(warning.is_none());

        let (layer, warning) = regulatory_layer_from_organisation_type("waterschap");
        assert_eq!(layer, RegulatoryLayer::WaterschapsVerordening);
        assert!(warning.is_none());
    }

    #[test]
    fn test_parse_sru_response_basic() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<searchRetrieveResponse xmlns="http://www.loc.gov/zing/srw/">
  <numberOfRecords>1</numberOfRecords>
  <records>
    <record>
      <recordData>
        <gzd xmlns:dcterms="http://purl.org/dc/terms/"
             xmlns:overheidproduct="http://standaarden.overheid.nl/product/"
             xmlns:overheid="http://standaarden.overheid.nl/owms/terms/">
          <originalData>
            <overheidcvdr:meta xmlns:overheidcvdr="http://standaarden.overheid.nl/cvdr/terms/">
              <owmskern xmlns:dcterms="http://purl.org/dc/terms/">
                <title>Verordening maatschappelijke ondersteuning gemeente Amsterdam 2020</title>
                <creator>Gemeente Amsterdam</creator>
                <identifier>CVDR681386</identifier>
              </owmskern>
              <owmsmantel>
                <issued>2020-01-01</issued>
              </owmsmantel>
            </overheidcvdr:meta>
          </originalData>
          <enrichedData>
            <organisatietype>gemeenten</organisatietype>
            <inwerkingtredingDatum>2020-03-01</inwerkingtredingDatum>
          </enrichedData>
        </gzd>
      </recordData>
    </record>
  </records>
</searchRetrieveResponse>"#;

        let result = parse_sru_response(xml, "CVDR681386").unwrap();
        assert_eq!(result.cvdr_id, "CVDR681386");
        assert_eq!(
            result.title,
            "Verordening maatschappelijke ondersteuning gemeente Amsterdam 2020"
        );
        assert_eq!(result.creator, "Gemeente Amsterdam");
        assert_eq!(result.organisation_type, "gemeenten");
        assert_eq!(
            result.regulatory_layer,
            RegulatoryLayer::GemeentelijkeVerordening
        );
        assert_eq!(result.publication_date, Some("2020-01-01".to_string()));
        assert_eq!(result.effective_date, Some("2020-03-01".to_string()));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_sru_response_no_records() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<searchRetrieveResponse xmlns="http://www.loc.gov/zing/srw/">
  <numberOfRecords>0</numberOfRecords>
  <records/>
</searchRetrieveResponse>"#;

        let result = parse_sru_response(xml, "CVDR000000");
        assert!(result.is_err());
    }

    #[test]
    fn test_cvdr_metadata_to_law_metadata() {
        let metadata = CvdrMetadata {
            cvdr_id: "CVDR681386".to_string(),
            title: "Test Verordening".to_string(),
            creator: "Gemeente Test".to_string(),
            organisation_type: "gemeenten".to_string(),
            regulatory_layer: RegulatoryLayer::GemeentelijkeVerordening,
            publication_date: Some("2020-01-01".to_string()),
            effective_date: Some("2020-03-01".to_string()),
            xml_url: "https://example.com/test.xml".to_string(),
            warnings: Vec::new(),
        };

        let law_meta = metadata.to_law_metadata("2025-01-01");
        assert_eq!(law_meta.bwb_id, "");
        assert_eq!(law_meta.cvdr_id, Some("CVDR681386".to_string()));
        assert_eq!(law_meta.title, "Test Verordening");
        assert_eq!(
            law_meta.regulatory_layer,
            RegulatoryLayer::GemeentelijkeVerordening
        );
        assert_eq!(law_meta.creator, Some("Gemeente Test".to_string()));
        assert_eq!(law_meta.effective_date, Some("2025-01-01".to_string()));
    }
}
