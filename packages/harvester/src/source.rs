//! Strategy trait for downloading laws from different sources.
//!
//! Provides a unified interface over BWB (national laws) and CVDR
//! (decentral regulations), so consumers can work with a single
//! `dyn LawSource` instead of if/else routing.

use async_trait::async_trait;
use reqwest::Client;

use crate::config::{
    lokaleregelgeving_url, validate_bwb_id, validate_cvdr_id, DEFAULT_MAX_RESPONSE_SIZE,
};
use crate::error::{HarvesterError, Result};
use crate::types::Law;

/// Enum identifying the law source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawSourceType {
    /// National law from BWB (Basiswettenbestand).
    Bwb,
    /// Decentral regulation from CVDR.
    Cvdr,
}

/// Strategy trait for downloading laws from different sources.
#[async_trait]
pub trait LawSource: Send + Sync {
    /// Validate that the given ID is valid for this source.
    fn validate_id(&self, id: &str) -> Result<()>;

    /// Download and parse a law.
    async fn download(&self, client: &Client, id: &str, date: Option<&str>) -> Result<Law>;

    /// Build the public URL for a law.
    fn public_url(&self, id: &str) -> String;

    /// Source name for logging/display.
    fn name(&self) -> &'static str;

    /// Source type enum for type-safe dispatch.
    fn source_type(&self) -> LawSourceType;
}

/// BWB (Basiswettenbestand) source for national laws.
#[derive(Debug, Default)]
pub struct BwbSource {
    /// Maximum response size in megabytes.
    pub max_size_mb: Option<u64>,
}

#[async_trait]
impl LawSource for BwbSource {
    fn validate_id(&self, id: &str) -> Result<()> {
        validate_bwb_id(id)
    }

    async fn download(&self, client: &Client, id: &str, date: Option<&str>) -> Result<Law> {
        // BWB requires a date; default to today if not provided.
        let effective_date = date
            .map(String::from)
            .unwrap_or_else(regelrecht_shared::dates::today_str);

        let max_mb = self
            .max_size_mb
            .unwrap_or(DEFAULT_MAX_RESPONSE_SIZE / (1024 * 1024));

        crate::harvester::download_law_with_max_size(client, id, &effective_date, max_mb).await
    }

    fn public_url(&self, id: &str) -> String {
        format!("https://wetten.overheid.nl/{id}")
    }

    fn name(&self) -> &'static str {
        "BWB"
    }

    fn source_type(&self) -> LawSourceType {
        LawSourceType::Bwb
    }
}

/// CVDR (Centrale Voorziening Decentrale Regelgeving) source for decentral regulations.
#[derive(Debug, Default)]
pub struct CvdrSource;

#[async_trait]
impl LawSource for CvdrSource {
    fn validate_id(&self, id: &str) -> Result<()> {
        validate_cvdr_id(id)
    }

    async fn download(&self, client: &Client, id: &str, date: Option<&str>) -> Result<Law> {
        crate::cvdr::download_cvdr_law(client, id, date).await
    }

    fn public_url(&self, id: &str) -> String {
        lokaleregelgeving_url(id)
    }

    fn name(&self) -> &'static str {
        "CVDR"
    }

    fn source_type(&self) -> LawSourceType {
        LawSourceType::Cvdr
    }
}

/// Detect the law source from an ID and return the appropriate strategy.
///
/// # Errors
/// Returns `HarvesterError::InvalidLawId` if the ID prefix is not recognized.
///
/// # Examples
/// ```
/// use regelrecht_harvester::source::detect_source;
///
/// let source = detect_source("BWBR0018451").unwrap();
/// assert_eq!(source.name(), "BWB");
///
/// let source = detect_source("CVDR681386").unwrap();
/// assert_eq!(source.name(), "CVDR");
///
/// assert!(detect_source("INVALID").is_err());
/// ```
pub fn detect_source(law_id: &str) -> Result<Box<dyn LawSource>> {
    let source: Box<dyn LawSource> = if law_id.starts_with("BWBR") {
        Box::new(BwbSource::default())
    } else if law_id.starts_with("CVDR") {
        Box::new(CvdrSource)
    } else {
        return Err(HarvesterError::InvalidLawId(law_id.to_string()));
    };
    source.validate_id(law_id)?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bwb_source() {
        let source = detect_source("BWBR0018451").unwrap();
        assert_eq!(source.name(), "BWB");
    }

    #[test]
    fn detect_cvdr_source() {
        let source = detect_source("CVDR681386").unwrap();
        assert_eq!(source.name(), "CVDR");
    }

    #[test]
    fn detect_invalid_source() {
        assert!(detect_source("INVALID").is_err());
        assert!(detect_source("").is_err());
    }

    #[test]
    fn bwb_source_validates_id() {
        let source = BwbSource::default();
        assert!(source.validate_id("BWBR0018451").is_ok());
        assert!(source.validate_id("INVALID").is_err());
    }

    #[test]
    fn cvdr_source_validates_id() {
        let source = CvdrSource;
        assert!(source.validate_id("CVDR681386").is_ok());
        assert!(source.validate_id("INVALID").is_err());
    }

    #[test]
    fn bwb_source_public_url() {
        let source = BwbSource::default();
        assert_eq!(
            source.public_url("BWBR0018451"),
            "https://wetten.overheid.nl/BWBR0018451"
        );
    }

    #[test]
    fn cvdr_source_public_url() {
        let source = CvdrSource;
        assert_eq!(
            source.public_url("CVDR681386"),
            "https://lokaleregelgeving.overheid.nl/CVDR681386"
        );
    }
}
