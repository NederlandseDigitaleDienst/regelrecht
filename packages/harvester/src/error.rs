//! Error types for the harvester.
//!
//! Uses the dual-error pattern: `HarvesterError` for library consumers
//! with detailed error context, and specific error types for internal use.

use serde::Serialize;
use thiserror::Error;

/// Why a BWB *work* has no consolidated text available to harvest.
///
/// A work only carries a `_latestItem` (and consolidated `<expression>`s) once
/// it has at least one consolidated version. Some works legitimately have none:
/// they were withdrawn, are not yet in force, or are only registered. This
/// distinguishes those expected cases from a genuinely malformed manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoTextReason {
    /// Withdrawn (`datum_intrekking` set): no current consolidated text exists.
    Withdrawn { date: String },
    /// Adopted but not yet in force (`datum_inwerkingtreding` in the future):
    /// consolidated text appears on/after that date.
    NotYetInForce { date: String },
    /// Only registered (WTI metadata), with no dates or text yet.
    Announced,
}

/// Main error type for the harvester library.
#[derive(Debug, Error)]
pub enum HarvesterError {
    /// Invalid BWB ID format.
    #[error("Invalid BWB ID format: '{0}'. Expected BWBRXXXXXXX (e.g., BWBR0018451)")]
    InvalidBwbId(String),

    /// Invalid date format.
    #[error("Invalid date format: '{0}'. Expected YYYY-MM-DD (e.g., 2025-01-01)")]
    InvalidDate(String),

    /// A date range whose start lies after its end.
    #[error("Invalid period: start date {from} is after end date {to}")]
    InvalidPeriod { from: String, to: String },

    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// All HTTP retry attempts exhausted.
    #[error("HTTP request failed after {attempts} attempts: {message}")]
    RetriesExhausted { attempts: u32, message: String },

    /// Failed to download WTI metadata.
    #[error("Failed to download WTI metadata for {bwb_id}: {source}")]
    WtiDownload {
        bwb_id: String,
        #[source]
        source: reqwest::Error,
    },

    /// Failed to download content XML.
    #[error("Failed to download content for {bwb_id} at date {date}: {source}")]
    ContentDownload {
        bwb_id: String,
        date: String,
        #[source]
        source: reqwest::Error,
    },

    /// XML parsing failed.
    #[error("XML parsing failed: {0}")]
    XmlParse(#[from] roxmltree::Error),

    /// Missing required XML element.
    #[error("Missing required XML element: {element} in {context}")]
    MissingElement { element: String, context: String },

    /// Unknown XML element encountered.
    #[error("No handler for element <{tag_name}>{}", .context.as_ref().map(|c| format!(" in {c}")).unwrap_or_default())]
    UnknownElement {
        tag_name: String,
        context: Option<String>,
    },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML serialization error.
    #[error("YAML serialization failed: {0}")]
    YamlSerialization(#[from] serde_yaml_ng::Error),

    /// A serialized `text: |-` block could not be aligned with any remaining
    /// fold-plan entry: its reconstructed content matched no planned text (in
    /// document order). This means the writer's emission invariant broke;
    /// folding must never guess which block gets which style, so the write
    /// fails instead. Leftover plan entries are legal (serde may emit a text
    /// with tabs/control characters as a quoted scalar rather than a block).
    #[error("YAML text-block fold plan mismatch: {detail}")]
    FoldPlanMismatch { detail: String },

    /// No BWB ID found in JCI reference.
    #[error("No BWB ID found in JCI reference: {0}")]
    InvalidJciReference(String),

    /// HTTP response too large.
    #[error("HTTP response too large: {actual_bytes} bytes exceeds limit of {max_bytes} bytes")]
    ResponseTooLarge { max_bytes: u64, actual_bytes: u64 },

    /// Failed to download manifest.
    #[error("Failed to download manifest for {bwb_id}: {source}")]
    ManifestDownload {
        bwb_id: String,
        #[source]
        source: reqwest::Error,
    },

    /// No consolidation found for the given date.
    #[error("No consolidation found for {bwb_id} at date {date}")]
    NoConsolidation { bwb_id: String, date: String },

    /// The work has no consolidated text at all (withdrawn / not yet in force /
    /// only announced). This is an expected outcome, not a malformed manifest:
    /// the pipeline maps it to a terminal "skipped" law status rather than a
    /// retryable failure.
    #[error("No consolidated text for {bwb_id} ({reason:?})")]
    NoConsolidatedText {
        bwb_id: String,
        reason: NoTextReason,
    },

    /// Invalid CVDR ID format.
    #[error("Invalid CVDR ID format: '{0}'. Expected CVDR followed by 3 or more digits (e.g., CVDR681386)")]
    InvalidCvdrId(String),

    /// Invalid law ID format (neither BWB nor CVDR).
    #[error("Invalid law ID format: '{0}'. Expected BWB ID (BWBRXXXXXXX) or CVDR ID (CVDRXXX...)")]
    InvalidLawId(String),

    /// CVDR SRU search failed.
    #[error("CVDR SRU search failed for {cvdr_id}: {message}")]
    CvdrSearchFailed { cvdr_id: String, message: String },

    /// CVDR content download failed.
    #[error("Failed to download CVDR content for {cvdr_id}: {source}")]
    CvdrContentDownload {
        cvdr_id: String,
        #[source]
        source: reqwest::Error,
    },
}

/// Result type alias for harvester operations.
pub type Result<T> = std::result::Result<T, HarvesterError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = HarvesterError::InvalidBwbId("INVALID".to_string());
        assert!(err.to_string().contains("INVALID"));
        assert!(err.to_string().contains("BWBRXXXXXXX"));
    }

    #[test]
    fn test_unknown_element_with_context() {
        let err = HarvesterError::UnknownElement {
            tag_name: "foo".to_string(),
            context: Some("artikel".to_string()),
        };
        assert_eq!(err.to_string(), "No handler for element <foo> in artikel");
    }

    #[test]
    fn test_unknown_element_without_context() {
        let err = HarvesterError::UnknownElement {
            tag_name: "foo".to_string(),
            context: None,
        };
        assert_eq!(err.to_string(), "No handler for element <foo>");
    }
}
