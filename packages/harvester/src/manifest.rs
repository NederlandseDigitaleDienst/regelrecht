//! BWB manifest parsing for consolidation date resolution.
//!
//! The BWB repository doesn't have a consolidation for every date — the manifest.xml
//! file contains all available consolidation dates with their validity periods.
//! This module downloads and parses the manifest to find the correct consolidation date.

use reqwest::Client;
use roxmltree::Document;

use crate::config::{DEFAULT_MAX_RESPONSE_SIZE, manifest_url};
use crate::error::{HarvesterError, NoTextReason, Result};
use crate::http::{bytes_to_string, download_bytes};

/// Parsed BWB manifest containing available consolidations.
#[derive(Debug)]
pub struct BwbManifest {
    /// The `_latestItem` attribute from the `<work>` element (e.g. "2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml").
    pub latest_item: String,
    /// All consolidation expressions with their validity periods.
    pub expressions: Vec<Consolidation>,
}

/// A single consolidation expression from the manifest.
#[derive(Debug)]
pub struct Consolidation {
    /// Label attribute (e.g. "2026-02-04_0").
    pub label: String,
    /// Start of validity period (e.g. "2026-02-04").
    pub datum_inwerkingtreding: String,
    /// End of validity period (e.g. "9999-12-31" for current version).
    pub einddatum: String,
    /// Whether the XML item for this expression has been deleted from the repository.
    /// Deleted items return redirect loops instead of 404s, so they must be skipped.
    pub deleted: bool,
}

/// Download and parse the BWB manifest for a law.
///
/// # Arguments
/// * `client` - HTTP client to use
/// * `bwb_id` - The BWB identifier (e.g., "BWBR0015703")
pub async fn download_manifest(client: &Client, bwb_id: &str) -> Result<BwbManifest> {
    let url = manifest_url(bwb_id);
    let bytes = download_bytes(client, &url, DEFAULT_MAX_RESPONSE_SIZE)
        .await
        .map_err(|e| {
            if let HarvesterError::Http(source) = e {
                HarvesterError::ManifestDownload {
                    bwb_id: bwb_id.to_string(),
                    source,
                }
            } else {
                e
            }
        })?;

    let xml = bytes_to_string(bytes, &format!("manifest for {bwb_id}"));
    parse_manifest(&xml, bwb_id)
}

/// Parse manifest XML into a `BwbManifest`.
fn parse_manifest(xml: &str, bwb_id: &str) -> Result<BwbManifest> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();

    // Find the <work> element
    let work = root
        .descendants()
        .find(|n| n.has_tag_name("work"))
        .ok_or_else(|| HarvesterError::MissingElement {
            element: "work".to_string(),
            context: format!("manifest for {bwb_id}"),
        })?;

    let mut expressions = Vec::new();
    for expr in work.descendants().filter(|n| n.has_tag_name("expression")) {
        let label = expr.attribute("label").unwrap_or_default().to_string();

        let datum_inwerkingtreding = expr
            .descendants()
            .find(|n| n.has_tag_name("datum_inwerkingtreding"))
            .and_then(|n| n.text())
            .unwrap_or_default()
            .to_string();

        let einddatum = expr
            .descendants()
            .find(|n| n.has_tag_name("einddatum"))
            .and_then(|n| n.text())
            .unwrap_or("9999-12-31")
            .to_string();

        let deleted = expr
            .descendants()
            .find(|n| n.has_tag_name("manifestation") && n.attribute("label") == Some("xml"))
            .and_then(|m| m.descendants().find(|n| n.has_tag_name("item")))
            .and_then(|n| n.attribute("_deleted"))
            .is_some_and(|v| v == "true");

        if !label.is_empty() && !datum_inwerkingtreding.is_empty() {
            expressions.push(Consolidation {
                label,
                datum_inwerkingtreding,
                einddatum,
                deleted,
            });
        }
    }

    // `_latestItem` only exists once the work has at least one consolidated
    // version. When it is absent and there are no expressions, the work has no
    // consolidated text at all — classify why (withdrawn / not yet in force /
    // announced) so the pipeline can skip it terminally instead of treating it
    // as a retryable error.
    let latest_item = match work.attribute("_latestItem") {
        Some(value) => value.to_string(),
        None if expressions.is_empty() => {
            let metadata_date = |tag: &str| {
                work.descendants()
                    .find(|n| n.has_tag_name(tag))
                    .and_then(|n| n.text())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            // Intrekking takes precedence: a law withdrawn on its own
            // in-force date (intrekking == inwerkingtreding) never applied.
            let reason = if let Some(date) = metadata_date("datum_intrekking") {
                NoTextReason::Withdrawn { date }
            } else if let Some(date) = metadata_date("datum_inwerkingtreding") {
                NoTextReason::NotYetInForce { date }
            } else {
                NoTextReason::Announced
            };
            return Err(HarvesterError::NoConsolidatedText {
                bwb_id: bwb_id.to_string(),
                reason,
            });
        }
        // Has consolidated expressions but no latest pointer — unexpected/malformed.
        None => {
            return Err(HarvesterError::MissingElement {
                element: "_latestItem attribute".to_string(),
                context: format!("manifest for {bwb_id}"),
            });
        }
    };

    Ok(BwbManifest {
        latest_item,
        expressions,
    })
}

/// Extract the date from a `_latestItem` path or label.
///
/// Handles both full paths like "2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"
/// and labels like "2026-02-04_0".
fn extract_date_from_item(item: &str) -> Option<&str> {
    // Get the first path segment (before any '/')
    let segment = item.split('/').next().unwrap_or(item);
    // Strip the trailing "_0" (or "_N") version suffix to get the date
    segment.rsplit_once('_').map(|(date, _)| date)
}

/// Resolve the correct consolidation date from a manifest.
///
/// - `None` date: returns the latest available consolidation date (from `_latestItem`)
/// - `Some(date)`: finds the consolidation where `datum_inwerkingtreding <= date <= einddatum`
///
/// # Arguments
/// * `manifest` - Parsed BWB manifest
/// * `date` - Optional target date in YYYY-MM-DD format
///
/// # Returns
/// The consolidation date to use (YYYY-MM-DD format)
pub fn resolve_consolidation_date(manifest: &BwbManifest, date: Option<&str>) -> Result<String> {
    match date {
        None => {
            // No date specified: use the latest consolidation
            extract_date_from_item(&manifest.latest_item)
                .map(|d| d.to_string())
                .ok_or_else(|| HarvesterError::MissingElement {
                    element: "date in _latestItem".to_string(),
                    context: format!("_latestItem: {}", manifest.latest_item),
                })
        }
        Some(target_date) => {
            // Find the consolidation whose validity period covers the target date.
            // Skip deleted expressions — BWB returns redirect loops for deleted items.
            for consolidation in manifest.expressions.iter().filter(|c| !c.deleted) {
                if consolidation.datum_inwerkingtreding.as_str() <= target_date
                    && target_date <= consolidation.einddatum.as_str()
                {
                    return Ok(consolidation.datum_inwerkingtreding.clone());
                }
            }

            // Fallback: return the non-deleted consolidation with the highest einddatum.
            // This handles withdrawn laws where the target date is after the last
            // validity period (e.g. law withdrawn in 2007, target date is 2021).
            let latest = manifest
                .expressions
                .iter()
                .filter(|c| !c.deleted)
                .max_by_key(|c| &c.einddatum);

            match latest {
                Some(consolidation) => {
                    let bwb_id = extract_bwb_id_from_latest(&manifest.latest_item);
                    tracing::warn!(
                        bwb_id = %bwb_id,
                        target_date = %target_date,
                        fallback_date = %consolidation.datum_inwerkingtreding,
                        fallback_einddatum = %consolidation.einddatum,
                        "No consolidation covers target date, falling back to latest consolidation"
                    );
                    Ok(consolidation.datum_inwerkingtreding.clone())
                }
                None => Err(HarvesterError::NoConsolidation {
                    bwb_id: extract_bwb_id_from_latest(&manifest.latest_item),
                    date: target_date.to_string(),
                }),
            }
        }
    }
}

/// Determine the law's instrument end date (`valid_to`) for the consolidation at
/// `version_date`, per RFC-019.
///
/// Returns `Some(einddatum)` only when that consolidation is the **final** one
/// (no later `datum_inwerkingtreding` among non-deleted consolidations) and its
/// `einddatum` is finite — i.e. not the `9999-12-31` "still in force" sentinel.
/// An intermediate consolidation's `einddatum` is merely the day before the next
/// version begins and does **not** mean the law ended, so it yields `None`.
pub fn resolve_valid_to(manifest: &BwbManifest, version_date: &str) -> Option<String> {
    let consolidation = manifest
        .expressions
        .iter()
        .filter(|c| !c.deleted)
        .find(|c| c.datum_inwerkingtreding == version_date)?;

    // BWB dates are ISO 8601 (YYYY-MM-DD), so lexicographic order equals
    // chronological order; no date parsing needed for the comparison.
    let is_final = !manifest
        .expressions
        .iter()
        .filter(|c| !c.deleted)
        .any(|c| c.datum_inwerkingtreding.as_str() > version_date);

    if is_final && consolidation.einddatum != "9999-12-31" && !consolidation.einddatum.is_empty() {
        Some(consolidation.einddatum.clone())
    } else {
        None
    }
}

/// Extract BWB ID from the `_latestItem` path for error messages.
#[allow(clippy::expect_used)]
fn extract_bwb_id_from_latest(latest_item: &str) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static BWB_FINDER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"BWB[A-Z]\d{7}").expect("valid regex"));

    BWB_FINDER
        .find(latest_item)
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml">
    <expression label="2024-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2024-01-01</datum_inwerkingtreding>
        <einddatum>2024-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0015703_2024-01-01_0.xml" _deleted="false" /></manifestation>
    </expression>
    <expression label="2025-07-01_0">
      <metadata>
        <datum_inwerkingtreding>2025-07-01</datum_inwerkingtreding>
        <einddatum>2025-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0015703_2025-07-01_0.xml" _deleted="false" /></manifestation>
    </expression>
    <expression label="2026-02-04_0">
      <metadata>
        <datum_inwerkingtreding>2026-02-04</datum_inwerkingtreding>
        <einddatum>9999-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0015703_2026-02-04_0.xml" _deleted="false" /></manifestation>
    </expression>
  </work>
</repository>"#;

    #[test]
    fn test_parse_manifest() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        assert_eq!(
            manifest.latest_item,
            "2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"
        );
        assert_eq!(manifest.expressions.len(), 3);

        assert_eq!(manifest.expressions[0].label, "2024-01-01_0");
        assert_eq!(manifest.expressions[0].datum_inwerkingtreding, "2024-01-01");
        assert_eq!(manifest.expressions[0].einddatum, "2024-12-31");

        assert_eq!(manifest.expressions[2].label, "2026-02-04_0");
        assert_eq!(manifest.expressions[2].einddatum, "9999-12-31");
    }

    #[test]
    fn test_resolve_valid_to_in_force_law_has_none() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();
        // Final consolidation is still in force (9999-12-31) → no end date.
        assert_eq!(resolve_valid_to(&manifest, "2026-02-04"), None);
        // An intermediate consolidation's einddatum is mere succession → no end date.
        assert_eq!(resolve_valid_to(&manifest, "2024-01-01"), None);
    }

    #[test]
    fn test_resolve_valid_to_repealed_law_has_end_date() {
        // A law whose final consolidation carries a finite einddatum: repealed, no successor.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2005-01-01_0/xml/BWBR0001234_2005-01-01_0.xml">
    <expression label="2005-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2005-01-01</datum_inwerkingtreding>
        <einddatum>2007-06-30</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0001234_2005-01-01_0.xml" _deleted="false" /></manifestation>
    </expression>
  </work>
</repository>"#;
        let manifest = parse_manifest(xml, "BWBR0001234").unwrap();
        assert_eq!(
            resolve_valid_to(&manifest, "2005-01-01"),
            Some("2007-06-30".to_string())
        );
    }

    #[test]
    fn test_parse_manifest_withdrawn_law_classified() {
        // BWBR0025304 (Wet ambulancezorg): withdrawn, no consolidated text.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><work label="BWBR0025304">
            <metadata><datum_intrekking>2013-01-01</datum_intrekking></metadata></work>"#;
        match parse_manifest(xml, "BWBR0025304") {
            Err(HarvesterError::NoConsolidatedText { bwb_id, reason }) => {
                assert_eq!(bwb_id, "BWBR0025304");
                assert_eq!(
                    reason,
                    NoTextReason::Withdrawn {
                        date: "2013-01-01".to_string()
                    }
                );
            }
            other => panic!("expected NoConsolidatedText/Withdrawn, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_manifest_future_law_classified() {
        // BWBR0051692: adopted, in force 2027-01-01, no text yet.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><work label="BWBR0051692">
            <metadata><datum_inwerkingtreding>2027-01-01</datum_inwerkingtreding></metadata></work>"#;
        match parse_manifest(xml, "BWBR0051692") {
            Err(HarvesterError::NoConsolidatedText {
                reason: NoTextReason::NotYetInForce { date },
                ..
            }) => assert_eq!(date, "2027-01-01"),
            other => panic!("expected NotYetInForce, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_manifest_intrekking_takes_precedence_over_inwerkingtreding() {
        // BWBR0045723: withdrawn on its own in-force date → never applied.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><work label="BWBR0045723"><metadata>
            <datum_intrekking>2022-01-01</datum_intrekking>
            <datum_inwerkingtreding>2022-01-01</datum_inwerkingtreding></metadata></work>"#;
        match parse_manifest(xml, "BWBR0045723") {
            Err(HarvesterError::NoConsolidatedText {
                reason: NoTextReason::Withdrawn { date },
                ..
            }) => assert_eq!(date, "2022-01-01"),
            other => panic!("expected Withdrawn (precedence), got {other:?}"),
        }
    }

    #[test]
    fn test_parse_manifest_announced_law_classified() {
        // BWBR0051671: only WTI metadata, no dates.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><work label="BWBR0051671">
            <metadata><wti_locatie>BWBR0051671.WTI</wti_locatie></metadata></work>"#;
        match parse_manifest(xml, "BWBR0051671") {
            Err(HarvesterError::NoConsolidatedText {
                reason: NoTextReason::Announced,
                ..
            }) => {}
            other => panic!("expected Announced, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_manifest_expressions_without_latest_item_is_malformed() {
        // Has a consolidated expression but no `_latestItem` pointer — treated as
        // a malformed manifest (retryable), NOT a no-text classification.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><work label="BWBR0000001">
            <expression label="2024-01-01_0">
                <datum_inwerkingtreding>2024-01-01</datum_inwerkingtreding>
                <manifestation label="xml"><item label="x.xml" _deleted="false" /></manifestation>
            </expression></work>"#;
        match parse_manifest(xml, "BWBR0000001") {
            Err(HarvesterError::MissingElement { element, .. }) => {
                assert_eq!(element, "_latestItem attribute");
            }
            other => panic!("expected MissingElement, got {other:?}"),
        }
    }

    #[test]
    fn test_resolve_no_date_returns_latest() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();
        let result = resolve_consolidation_date(&manifest, None).unwrap();
        assert_eq!(result, "2026-02-04");
    }

    #[test]
    fn test_resolve_date_within_period() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Date falls in the 2025-07-01 to 2025-12-31 period
        let result = resolve_consolidation_date(&manifest, Some("2025-09-15")).unwrap();
        assert_eq!(result, "2025-07-01");

        // Date falls in the 2024-01-01 to 2024-12-31 period
        let result = resolve_consolidation_date(&manifest, Some("2024-06-15")).unwrap();
        assert_eq!(result, "2024-01-01");

        // Date falls in the current (open-ended) period
        let result = resolve_consolidation_date(&manifest, Some("2026-03-01")).unwrap();
        assert_eq!(result, "2026-02-04");
    }

    #[test]
    fn test_resolve_date_on_boundary() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Exactly on datum_inwerkingtreding
        let result = resolve_consolidation_date(&manifest, Some("2025-07-01")).unwrap();
        assert_eq!(result, "2025-07-01");

        // Exactly on einddatum
        let result = resolve_consolidation_date(&manifest, Some("2024-12-31")).unwrap();
        assert_eq!(result, "2024-01-01");
    }

    #[test]
    fn test_resolve_date_no_match_falls_back_to_latest() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Date before any consolidation — falls back to latest (highest einddatum = 9999-12-31)
        let result = resolve_consolidation_date(&manifest, Some("2023-01-01")).unwrap();
        assert_eq!(result, "2026-02-04");
    }

    #[test]
    fn test_resolve_date_in_gap_falls_back_to_latest() {
        let manifest = parse_manifest(SAMPLE_MANIFEST, "BWBR0015703").unwrap();

        // Date between two periods (2025-01-01 to 2025-06-30 is a gap)
        // Falls back to latest consolidation (highest einddatum = 9999-12-31)
        let result = resolve_consolidation_date(&manifest, Some("2025-03-15")).unwrap();
        assert_eq!(result, "2026-02-04");
    }

    #[test]
    fn test_resolve_date_after_withdrawal() {
        // Manifest for a withdrawn law: all consolidations have finite einddatum
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2006-01-01_0/xml/BWBR0002089_2006-01-01_0.xml">
    <expression label="2003-06-01_0">
      <metadata>
        <datum_inwerkingtreding>2003-06-01</datum_inwerkingtreding>
        <einddatum>2005-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0002089_2003-06-01_0.xml" _deleted="false" /></manifestation>
    </expression>
    <expression label="2006-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2006-01-01</datum_inwerkingtreding>
        <einddatum>2007-06-30</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0002089_2006-01-01_0.xml" _deleted="false" /></manifestation>
    </expression>
  </work>
</repository>"#;

        let manifest = parse_manifest(xml, "BWBR0002089").unwrap();

        // Target date 2021 is well after withdrawal (2007-06-30)
        // Should fall back to the consolidation with highest einddatum (2006-01-01)
        let result = resolve_consolidation_date(&manifest, Some("2021-01-01")).unwrap();
        assert_eq!(result, "2006-01-01");
    }

    #[test]
    fn test_resolve_skips_deleted_expressions() {
        // Reproduces BWBR0029244 bug: a deleted expression with einddatum=9999-12-31
        // would match before the correct non-deleted expression.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2026-01-01_0/xml/BWBR0029244_2026-01-01_0.xml">
    <expression label="2019-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2019-01-01</datum_inwerkingtreding>
        <einddatum>9999-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0029244_2019-01-01_0.xml" _deleted="true" /></manifestation>
    </expression>
    <expression label="2026-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2026-01-01</datum_inwerkingtreding>
        <einddatum>2026-02-20</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0029244_2026-01-01_0.xml" _deleted="false" /></manifestation>
    </expression>
  </work>
</repository>"#;

        let manifest = parse_manifest(xml, "BWBR0029244").unwrap();

        // Should resolve to 2026-01-01, NOT 2019-01-01 (which is deleted)
        let result = resolve_consolidation_date(&manifest, Some("2026-01-01")).unwrap();
        assert_eq!(result, "2026-01-01");
    }

    #[test]
    fn test_parse_manifest_detects_deleted() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2026-01-01_0/xml/BWBR0029244_2026-01-01_0.xml">
    <expression label="2019-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2019-01-01</datum_inwerkingtreding>
        <einddatum>9999-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0029244_2019-01-01_0.xml" _deleted="true" /></manifestation>
    </expression>
    <expression label="2026-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2026-01-01</datum_inwerkingtreding>
        <einddatum>2026-02-20</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0029244_2026-01-01_0.xml" _deleted="false" /></manifestation>
    </expression>
  </work>
</repository>"#;

        let manifest = parse_manifest(xml, "BWBR0029244").unwrap();
        assert_eq!(manifest.expressions.len(), 2);
        assert!(manifest.expressions[0].deleted);
        assert!(!manifest.expressions[1].deleted);
    }

    #[test]
    fn test_parse_manifest_missing_item_not_deleted() {
        // Expressions without <item> elements should default to not deleted
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2026-01-01_0/xml/BWBR0029244_2026-01-01_0.xml">
    <expression label="2026-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2026-01-01</datum_inwerkingtreding>
        <einddatum>9999-12-31</einddatum>
      </metadata>
    </expression>
  </work>
</repository>"#;

        let manifest = parse_manifest(xml, "BWBR0029244").unwrap();
        assert!(!manifest.expressions[0].deleted);
    }

    #[test]
    fn test_resolve_no_date_with_deleted_expressions() {
        // When requesting the latest consolidation (None date), the result comes from
        // `_latestItem` regardless of whether some expressions are deleted.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<repository>
  <work _latestItem="2026-01-01_0/xml/BWBR0029244_2026-01-01_0.xml">
    <expression label="2019-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2019-01-01</datum_inwerkingtreding>
        <einddatum>9999-12-31</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0029244_2019-01-01_0.xml" _deleted="true" /></manifestation>
    </expression>
    <expression label="2026-01-01_0">
      <metadata>
        <datum_inwerkingtreding>2026-01-01</datum_inwerkingtreding>
        <einddatum>2026-02-20</einddatum>
      </metadata>
      <manifestation label="xml"><item label="BWBR0029244_2026-01-01_0.xml" _deleted="false" /></manifestation>
    </expression>
  </work>
</repository>"#;

        let manifest = parse_manifest(xml, "BWBR0029244").unwrap();
        let result = resolve_consolidation_date(&manifest, None).unwrap();
        assert_eq!(result, "2026-01-01");
    }

    #[test]
    fn test_extract_date_from_item() {
        assert_eq!(
            extract_date_from_item("2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"),
            Some("2026-02-04")
        );
        assert_eq!(extract_date_from_item("2025-07-01_0"), Some("2025-07-01"));
        assert_eq!(extract_date_from_item("invalid"), None);
    }

    #[test]
    fn test_extract_bwb_id_from_latest() {
        assert_eq!(
            extract_bwb_id_from_latest("2026-02-04_0/xml/BWBR0015703_2026-02-04_0.xml"),
            "BWBR0015703"
        );
        assert_eq!(extract_bwb_id_from_latest("no_id_here"), "unknown");
    }
}
