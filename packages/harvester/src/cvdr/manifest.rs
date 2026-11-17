//! CVDR manifest parsing for version resolution.
//!
//! The CVDR repository stores manifest files listing all versions of a regulation.
//! When a user provides a bare CVDR ID (e.g., `CVDR691525`), we need to resolve the
//! latest versioned identifier (e.g., `CVDR691525_2`) before querying the SRU API.
//!
//! Manifest URL pattern:
//! `https://repository.officiele-overheidspublicaties.nl/cvdr/{CVDR_ID}/manifest.xml`

use reqwest::Client;
use roxmltree::Document;

use crate::config::cvdr_manifest_url;
use crate::error::{HarvesterError, Result};
use crate::http::{bytes_to_string, download_bytes_default};

/// A resolved CVDR version from the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CvdrVersion {
    /// The versioned CVDR identifier (e.g., "CVDR691525_2").
    pub versioned_id: String,

    /// The version number (e.g., 2).
    pub version: u32,

    /// Direct URL to the XML content for this version.
    pub xml_url: String,
}

/// Resolve the latest version of a CVDR regulation from its manifest.
///
/// Downloads the manifest XML from the CVDR repository, parses all available
/// versions, and returns the latest one (either marked with `_latestItem` or
/// the highest version number).
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `cvdr_id` - The bare CVDR identifier (e.g., "CVDR691525")
///
/// # Returns
/// A `CvdrVersion` with the resolved versioned ID and XML URL
pub async fn resolve_latest_cvdr_version(client: &Client, cvdr_id: &str) -> Result<CvdrVersion> {
    let url = cvdr_manifest_url(cvdr_id);

    tracing::debug!(cvdr_id = %cvdr_id, url = %url, "Downloading CVDR manifest");

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

    let xml_string = bytes_to_string(bytes, &format!("CVDR manifest for {cvdr_id}"));
    parse_cvdr_manifest(&xml_string, cvdr_id)
}

/// Parse a CVDR manifest XML to find the latest version.
///
/// The manifest structure contains expression elements with versioned filenames.
/// We look for the `_latestItem` attribute first, then fall back to finding the
/// highest version number among all expressions.
fn parse_cvdr_manifest(xml: &str, cvdr_id: &str) -> Result<CvdrVersion> {
    let doc = Document::parse(xml)?;

    // Try to find a _latestItem attribute on the work element
    let latest_from_attr = doc
        .descendants()
        .find(|n| n.is_element() && n.has_tag_name("work"))
        .and_then(|n| n.attribute("_latestItem"))
        .and_then(|item| extract_version_from_path(item, cvdr_id));

    // Collect all versions from expression elements
    let mut versions: Vec<CvdrVersion> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }

        // Look for expression elements or item elements that contain version info
        if (node.has_tag_name("expression") || node.has_tag_name("item"))
            && let Some(label) = node.attribute("label")
            && let Some(version) = extract_version_from_path(label, cvdr_id)
        {
            // Check for duplicates
            if !versions.iter().any(|v| v.version == version.version) {
                versions.push(version);
            }
        }
    }

    // If we got a _latestItem match and it exists in our versions, prefer it
    if let Some(ref latest) = latest_from_attr
        && versions.iter().any(|v| v.version == latest.version)
    {
        tracing::debug!(
            cvdr_id = %cvdr_id,
            version = latest.version,
            versioned_id = %latest.versioned_id,
            "Resolved CVDR version from _latestItem"
        );
        return Ok(latest.clone());
    }

    // Fall back to the highest version number
    if let Some(latest) = versions.into_iter().max_by_key(|v| v.version) {
        tracing::debug!(
            cvdr_id = %cvdr_id,
            version = latest.version,
            versioned_id = %latest.versioned_id,
            "Resolved CVDR version from highest version number"
        );
        return Ok(latest);
    }

    // If _latestItem matched but wasn't in versions list, still use it
    if let Some(latest) = latest_from_attr {
        tracing::debug!(
            cvdr_id = %cvdr_id,
            version = latest.version,
            versioned_id = %latest.versioned_id,
            "Resolved CVDR version from _latestItem (no expressions found)"
        );
        return Ok(latest);
    }

    Err(HarvesterError::CvdrSearchFailed {
        cvdr_id: cvdr_id.to_string(),
        message: "No versions found in CVDR manifest".to_string(),
    })
}

/// Extract a `CvdrVersion` from a manifest path or label.
///
/// Handles patterns like:
/// - `CVDR691525_2.xml`
/// - `CVDR691525_2/xml/CVDR691525_2.xml`
/// - `1/xml/CVDR691525_1.xml`
/// - `2` (just the version number as label for expression elements)
fn extract_version_from_path(path: &str, cvdr_id: &str) -> Option<CvdrVersion> {
    // Try to find a versioned identifier pattern: CVDR{digits}_{version}
    let prefix = format!("{cvdr_id}_");

    // Search for the pattern in the path
    if let Some(start) = path.find(&prefix) {
        let after_prefix = &path[start + prefix.len()..];
        // Extract digits until we hit a non-digit character
        let version_str: String = after_prefix
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(version) = version_str.parse::<u32>() {
            let versioned_id = format!("{cvdr_id}_{version}");
            let xml_url = cvdr_xml_url(cvdr_id, version);
            return Some(CvdrVersion {
                versioned_id,
                version,
                xml_url,
            });
        }
    }

    // Try parsing the path as just a version number (expression label)
    let trimmed = path.trim();
    if let Ok(version) = trimmed.parse::<u32>() {
        let versioned_id = format!("{cvdr_id}_{version}");
        let xml_url = cvdr_xml_url(cvdr_id, version);
        return Some(CvdrVersion {
            versioned_id,
            version,
            xml_url,
        });
    }

    None
}

/// Build the direct XML download URL for a CVDR version.
fn cvdr_xml_url(cvdr_id: &str, version: u32) -> String {
    use crate::config::CVDR_REPOSITORY_URL;
    format!("{CVDR_REPOSITORY_URL}/{cvdr_id}/{version}/xml/{cvdr_id}_{version}.xml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version_from_xml_filename() {
        let result = extract_version_from_path("CVDR691525_2.xml", "CVDR691525");
        assert_eq!(
            result,
            Some(CvdrVersion {
                versioned_id: "CVDR691525_2".to_string(),
                version: 2,
                xml_url: "https://repository.officiele-overheidspublicaties.nl/cvdr/CVDR691525/2/xml/CVDR691525_2.xml".to_string(),
            })
        );
    }

    #[test]
    fn test_extract_version_from_full_path() {
        let result = extract_version_from_path("1/xml/CVDR756485_1.xml", "CVDR756485");
        assert_eq!(
            result,
            Some(CvdrVersion {
                versioned_id: "CVDR756485_1".to_string(),
                version: 1,
                xml_url: "https://repository.officiele-overheidspublicaties.nl/cvdr/CVDR756485/1/xml/CVDR756485_1.xml".to_string(),
            })
        );
    }

    #[test]
    fn test_extract_version_from_number_label() {
        let result = extract_version_from_path("2", "CVDR691525");
        assert_eq!(
            result,
            Some(CvdrVersion {
                versioned_id: "CVDR691525_2".to_string(),
                version: 2,
                xml_url: "https://repository.officiele-overheidspublicaties.nl/cvdr/CVDR691525/2/xml/CVDR691525_2.xml".to_string(),
            })
        );
    }

    #[test]
    fn test_extract_version_no_match() {
        let result = extract_version_from_path("no_match_here", "CVDR691525");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_version_wrong_cvdr_id() {
        // Path references a different CVDR ID
        let result = extract_version_from_path("CVDR999999_1.xml", "CVDR691525");
        assert!(result.is_none());
    }

    #[test]
    fn test_cvdr_xml_url() {
        assert_eq!(
            cvdr_xml_url("CVDR691525", 2),
            "https://repository.officiele-overheidspublicaties.nl/cvdr/CVDR691525/2/xml/CVDR691525_2.xml"
        );
    }

    #[test]
    fn test_parse_manifest_single_version() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="1/xml/CVDR756485_1.xml">
    <expression label="1">
      <manifestation label="xml">
        <item label="CVDR756485_1.xml" />
      </manifestation>
    </expression>
  </work>
</repository>"#;

        let result = parse_cvdr_manifest(xml, "CVDR756485").unwrap();
        assert_eq!(result.versioned_id, "CVDR756485_1");
        assert_eq!(result.version, 1);
        assert_eq!(
            result.xml_url,
            "https://repository.officiele-overheidspublicaties.nl/cvdr/CVDR756485/1/xml/CVDR756485_1.xml"
        );
    }

    #[test]
    fn test_parse_manifest_multiple_versions() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2/xml/CVDR691525_2.xml">
    <expression label="1">
      <manifestation label="xml">
        <item label="CVDR691525_1.xml" />
      </manifestation>
    </expression>
    <expression label="2">
      <manifestation label="xml">
        <item label="CVDR691525_2.xml" />
      </manifestation>
    </expression>
  </work>
</repository>"#;

        let result = parse_cvdr_manifest(xml, "CVDR691525").unwrap();
        assert_eq!(result.versioned_id, "CVDR691525_2");
        assert_eq!(result.version, 2);
    }

    #[test]
    fn test_parse_manifest_latest_item_wins() {
        // Even though version 3 is higher, _latestItem says version 2
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2/xml/CVDR691525_2.xml">
    <expression label="1">
      <manifestation label="xml">
        <item label="CVDR691525_1.xml" />
      </manifestation>
    </expression>
    <expression label="2">
      <manifestation label="xml">
        <item label="CVDR691525_2.xml" />
      </manifestation>
    </expression>
    <expression label="3">
      <manifestation label="xml">
        <item label="CVDR691525_3.xml" />
      </manifestation>
    </expression>
  </work>
</repository>"#;

        let result = parse_cvdr_manifest(xml, "CVDR691525").unwrap();
        assert_eq!(result.versioned_id, "CVDR691525_2");
        assert_eq!(result.version, 2);
    }

    #[test]
    fn test_parse_manifest_no_versions_fails() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work>
  </work>
</repository>"#;

        let result = parse_cvdr_manifest(xml, "CVDR691525");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_manifest_falls_back_to_highest_version() {
        // No _latestItem attribute
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work>
    <expression label="1">
      <manifestation label="xml">
        <item label="CVDR691525_1.xml" />
      </manifestation>
    </expression>
    <expression label="3">
      <manifestation label="xml">
        <item label="CVDR691525_3.xml" />
      </manifestation>
    </expression>
    <expression label="2">
      <manifestation label="xml">
        <item label="CVDR691525_2.xml" />
      </manifestation>
    </expression>
  </work>
</repository>"#;

        let result = parse_cvdr_manifest(xml, "CVDR691525").unwrap();
        // Should pick version 3 as the highest
        assert_eq!(result.versioned_id, "CVDR691525_3");
        assert_eq!(result.version, 3);
    }
}
