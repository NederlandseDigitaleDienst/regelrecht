//! Core data types for the harvester.
//!
//! These types represent Dutch legal documents and their components,
//! matching the Python harvester's `models.py`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

use crate::config::wetten_url;

/// Re-export the canonical regulatory layer types from the shared crate.
pub use regelrecht_shared::RegulatoryLayer;

/// Parse a WTI "soort-regeling" text into a `RegulatoryLayer`.
///
/// Returns `(layer, warning)` where the warning is `Some` for ambiguous mappings
/// or unknown types. Direct matches produce no warning.
#[must_use]
pub fn regulatory_layer_from_soort_regeling(text: &str) -> (RegulatoryLayer, Option<String>) {
    match text.to_lowercase().as_str() {
        "grondwet" => (RegulatoryLayer::Grondwet, None),
        "wet" => (RegulatoryLayer::Wet, None),
        "amvb" | "algemene maatregel van bestuur" => (RegulatoryLayer::Amvb, None),
        "ministeriele regeling" | "ministeriële regeling" => {
            (RegulatoryLayer::MinisterieleRegeling, None)
        }
        "beleidsregel" => (RegulatoryLayer::Beleidsregel, None),
        "eu-verordening" => (RegulatoryLayer::EuVerordening, None),
        "eu-richtlijn" => (RegulatoryLayer::EuRichtlijn, None),
        "verdrag" => (RegulatoryLayer::Verdrag, None),
        "uitvoeringsbeleid" => (RegulatoryLayer::Uitvoeringsbeleid, None),
        "gemeentelijke verordening" => (RegulatoryLayer::GemeentelijkeVerordening, None),
        "provinciale verordening" => (RegulatoryLayer::ProvincialeVerordening, None),
        "waterschapsverordening" => (RegulatoryLayer::WaterschapsVerordening, None),
        // Ambiguous mappings - produce warnings
        "koninklijk besluit" | "kb" => (
            RegulatoryLayer::Amvb,
            Some(format!(
                "Mapped soort-regeling '{text}' to AMVB (closest schema match)"
            )),
        ),
        "regeling" => (
            RegulatoryLayer::MinisterieleRegeling,
            Some(format!(
                "Mapped soort-regeling '{text}' to MINISTERIELE_REGELING (closest schema match)"
            )),
        ),
        "verordening" => (
            RegulatoryLayer::GemeentelijkeVerordening,
            Some(format!(
                "Mapped soort-regeling '{text}' to GEMEENTELIJKE_VERORDENING (closest schema match)"
            )),
        ),
        unknown => {
            tracing::warn!(
                soort_regeling = %unknown,
                "Unknown soort-regeling type, defaulting to WET"
            );
            (
                RegulatoryLayer::Wet,
                Some(format!(
                    "Unknown soort-regeling '{unknown}', defaulting to WET"
                )),
            )
        }
    }
}

/// Metadata extracted from WTI file or CVDR SRU search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawMetadata {
    /// BWB identifier (e.g., "BWBR0018451").
    /// For CVDR laws, this is set to the CVDR ID.
    pub bwb_id: String,

    /// CVDR identifier (e.g., "CVDR681386"), if this law came from CVDR.
    pub cvdr_id: Option<String>,

    /// Official title (citeertitel).
    pub title: String,

    /// Type of regulatory document.
    pub regulatory_layer: RegulatoryLayer,

    /// Publication date (optional).
    pub publication_date: Option<String>,

    /// Effective date (optional, usually set from request).
    pub effective_date: Option<String>,

    /// Last date on which the law is in force (inclusive, einddatum). Set only when the
    /// law is terminated (vervalt/ingetrokken); `None` means valid until superseded. RFC-019.
    pub valid_to: Option<String>,

    /// Organisation name (e.g., "Gemeente Amsterdam"), for CVDR laws.
    pub creator: Option<String>,

    /// Scope code (e.g., "GM0363", "WS0155", "PV27"), for CVDR laws.
    pub scope_code: Option<String>,
}

/// Regex for slug generation - matches non-word characters.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static SLUG_NON_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\w\s-]").expect("valid regex"));

/// Regex for slug generation - matches whitespace and dashes.
#[allow(clippy::expect_used)] // Static regex that is guaranteed to be valid
static SLUG_SPACE_DASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[-\s]+").expect("valid regex"));

/// Maximum byte length for a slug used as a path component. Linux caps a single
/// path component at 255 bytes; we stay well under that as a conservative margin
/// below the per-component limit.
const MAX_SLUG_BYTES: usize = 200;

impl LawMetadata {
    /// Generate a URL-friendly slug from the title.
    ///
    /// Uses NFKD normalization to convert accented characters (like é, ë, ü)
    /// to their ASCII equivalents before filtering.
    ///
    /// # Examples
    /// ```
    /// use regelrecht_harvester::types::{LawMetadata, RegulatoryLayer};
    ///
    /// let metadata = LawMetadata {
    ///     bwb_id: "BWBR0018451".to_string(),
    ///     cvdr_id: None,
    ///     title: "Wet op de zorgtoeslag".to_string(),
    ///     regulatory_layer: RegulatoryLayer::Wet,
    ///     publication_date: None,
    ///     effective_date: None,
    ///     valid_to: None,
    ///     creator: None,
    ///     scope_code: None,
    /// };
    /// assert_eq!(metadata.to_slug(), "wet_op_de_zorgtoeslag");
    /// ```
    #[must_use]
    pub fn to_slug(&self) -> String {
        // Normalize using NFKD to decompose accented characters,
        // then filter to ASCII only (this removes the combining diacritics)
        let text: String = self
            .title
            .to_lowercase()
            .nfkd()
            .filter(|c| c.is_ascii())
            .collect();
        let text = SLUG_NON_WORD.replace_all(&text, "");
        let text = SLUG_SPACE_DASH.replace_all(&text, "_");
        let slug = text.trim_matches('_').to_string();
        self.cap_slug(slug)
    }

    /// Cap an over-long slug so it stays a valid path component.
    ///
    /// The slug is used as the per-law directory name, which on Linux is
    /// limited to 255 bytes; some official titles produce slugs well over that
    /// (e.g. long "Wijzigingswet …" titles), which previously failed harvesting
    /// with `Filename too long (os error 36)`. When a slug exceeds
    /// [`MAX_SLUG_BYTES`] it is truncated on a `_` word boundary and suffixed
    /// with the law's unique id (BWB or CVDR) so distinct long-titled laws can
    /// never collide and the directory stays traceable to its source. Slugs
    /// within the limit are returned unchanged, so normal laws are unaffected.
    fn cap_slug(&self, slug: String) -> String {
        // The slug is pure ASCII (see `to_slug`), so byte length == char count
        // and byte-index truncation never splits a multi-byte char.
        if slug.len() <= MAX_SLUG_BYTES {
            return slug;
        }

        let id = if !self.bwb_id.is_empty() {
            self.bwb_id.as_str()
        } else {
            self.cvdr_id.as_deref().unwrap_or_default()
        };
        let suffix = format!("_{}", id.to_lowercase());
        let budget = MAX_SLUG_BYTES.saturating_sub(suffix.len());

        let mut head = slug;
        head.truncate(budget);
        // Prefer cutting on a word boundary; fall back to the hard cut when the
        // truncated head is a single unbroken word.
        if let Some(i) = head.rfind('_') {
            head.truncate(i);
        }
        let head = head.trim_end_matches('_');
        format!("{head}{suffix}")
    }
}

/// A reference to another article or law.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// Unique identifier for this reference (e.g., "ref1").
    pub id: String,

    /// BWB identifier of the referenced law.
    pub bwb_id: String,

    /// Article number (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artikel: Option<String>,

    /// Paragraph number (lid) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,

    /// Subdivision (onderdeel) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onderdeel: Option<String>,

    /// Chapter (hoofdstuk) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<String>,

    /// Section (paragraaf) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<String>,

    /// Division (afdeling) (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<String>,
}

impl Reference {
    /// Create a new reference with just BWB ID.
    #[must_use]
    pub fn new(id: impl Into<String>, bwb_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            bwb_id: bwb_id.into(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: None,
        }
    }

    /// Generate wetten.overheid.nl URL.
    ///
    /// # Arguments
    /// * `date` - Optional date for versioned URL
    #[must_use]
    pub fn to_wetten_url(&self, date: Option<&str>) -> String {
        wetten_url(
            &self.bwb_id,
            date,
            self.artikel.as_deref(),
            self.hoofdstuk.as_deref(),
            self.afdeling.as_deref(),
            self.paragraaf.as_deref(),
        )
    }
}

/// Format references as markdown reference definitions.
///
/// # Arguments
/// * `references` - List of references to format
///
/// # Returns
/// Markdown reference definitions, e.g.:
/// ```text
/// [ref1]: https://wetten.overheid.nl/BWBR0018451#Artikel4
/// [ref2]: https://wetten.overheid.nl/BWBR0018450#Artikel1
/// ```
#[must_use]
pub fn format_reference_definitions(references: &[Reference]) -> String {
    if references.is_empty() {
        return String::new();
    }

    references
        .iter()
        .map(|r| format!("[{}]: {}", r.id, r.to_wetten_url(None)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Preamble (aanhef) section of a law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preamble {
    /// Preamble text content.
    pub text: String,

    /// URL to the preamble on wetten.overheid.nl.
    pub url: String,
}

/// A single article from a law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    /// Article number (e.g., "1", "1.1", "1.1.a").
    pub number: String,

    /// Article text content.
    pub text: String,

    /// URL to the article on wetten.overheid.nl.
    pub url: String,

    /// References contained in this article.
    pub references: Vec<Reference>,
}

impl Article {
    /// Create a new article.
    #[must_use]
    pub fn new(number: impl Into<String>, text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            text: text.into(),
            url: url.into(),
            references: Vec::new(),
        }
    }

    /// Create an article with references.
    #[must_use]
    pub fn with_references(mut self, references: Vec<Reference>) -> Self {
        self.references = references;
        self
    }
}

/// Complete law with metadata and articles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Law {
    /// Metadata from WTI file.
    pub metadata: LawMetadata,

    /// Preamble (aanhef) section, if present.
    pub preamble: Option<Preamble>,

    /// List of articles.
    pub articles: Vec<Article>,

    /// Non-fatal warnings encountered during parsing.
    ///
    /// These indicate data that may have been skipped or improperly handled.
    /// Callers can inspect this to assess data quality.
    pub warnings: Vec<String>,
}

impl Law {
    /// Create a new law with metadata.
    #[must_use]
    pub fn new(metadata: LawMetadata) -> Self {
        Self {
            metadata,
            preamble: None,
            articles: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Add an article to the law.
    pub fn add_article(&mut self, article: Article) {
        self.articles.push(article);
    }

    /// Add a warning to the law.
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Get the number of warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regulatory_layer_as_str() {
        assert_eq!(RegulatoryLayer::Wet.as_str(), "WET");
        assert_eq!(RegulatoryLayer::Amvb.as_str(), "AMVB");
        assert_eq!(
            RegulatoryLayer::MinisterieleRegeling.as_str(),
            "MINISTERIELE_REGELING"
        );
    }

    #[test]
    fn test_regulatory_layer_as_dir_name() {
        assert_eq!(RegulatoryLayer::Wet.as_dir_name(), "wet");
        assert_eq!(
            RegulatoryLayer::MinisterieleRegeling.as_dir_name(),
            "ministeriele_regeling"
        );
    }

    #[test]
    fn test_regulatory_layer_from_soort_regeling() {
        // Direct matches - no warning
        assert_eq!(
            regulatory_layer_from_soort_regeling("wet"),
            (RegulatoryLayer::Wet, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("WET"),
            (RegulatoryLayer::Wet, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("amvb"),
            (RegulatoryLayer::Amvb, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("algemene maatregel van bestuur"),
            (RegulatoryLayer::Amvb, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("ministeriële regeling"),
            (RegulatoryLayer::MinisterieleRegeling, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("grondwet"),
            (RegulatoryLayer::Grondwet, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("eu-verordening"),
            (RegulatoryLayer::EuVerordening, None)
        );
        assert_eq!(
            regulatory_layer_from_soort_regeling("verdrag"),
            (RegulatoryLayer::Verdrag, None)
        );

        // Ambiguous mappings - produce warnings
        let (layer, warning) = regulatory_layer_from_soort_regeling("koninklijk besluit");
        assert_eq!(layer, RegulatoryLayer::Amvb);
        assert!(warning.is_some());

        let (layer, warning) = regulatory_layer_from_soort_regeling("regeling");
        assert_eq!(layer, RegulatoryLayer::MinisterieleRegeling);
        assert!(warning.is_some());

        let (layer, warning) = regulatory_layer_from_soort_regeling("verordening");
        assert_eq!(layer, RegulatoryLayer::GemeentelijkeVerordening);
        assert!(warning.is_some());

        // Unknown defaults to Wet with warning
        let (layer, warning) = regulatory_layer_from_soort_regeling("unknown");
        assert_eq!(layer, RegulatoryLayer::Wet);
        assert!(warning.is_some());
    }

    #[test]
    fn test_law_metadata_to_slug() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0018451".to_string(),
            cvdr_id: None,
            title: "Wet op de zorgtoeslag".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        assert_eq!(metadata.to_slug(), "wet_op_de_zorgtoeslag");
    }

    #[test]
    fn test_law_metadata_to_slug_long_title_is_capped() {
        // A real-world over-long title (BWBR0049301 produced a 267-char slug),
        // which used to fail harvesting with "Filename too long (os error 36)".
        let title = "Besluit vaststelling aantal tijdstippen waarvan in de \
            Omgevingswet en daarmee verband houdende wet- en regelgeving is \
            aangegeven dat deze bij koninklijk besluit worden bepaald en \
            vaststelling tijdstip inwerkingtreding van diverse onderdelen \
            daarvan alsmede van enkele samenhangende wetten en besluiten";
        let metadata = LawMetadata {
            bwb_id: "BWBR0049301".to_string(),
            cvdr_id: None,
            title: title.to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        let slug = metadata.to_slug();
        assert!(
            slug.len() <= MAX_SLUG_BYTES,
            "slug too long: {}",
            slug.len()
        );
        assert!(
            slug.ends_with("_bwbr0049301"),
            "slug must carry the unique id suffix: {slug}"
        );
        assert!(!slug.contains("__"), "no empty boundary segments: {slug}");
        assert!(slug.starts_with("besluit_vaststelling_aantal_tijdstippen"));
    }

    #[test]
    fn test_law_metadata_to_slug_long_titles_dont_collide() {
        // Two distinct laws sharing a long common prefix must get distinct slugs.
        let prefix = "wijzigingswet ".repeat(30); // ~420 chars, well over the cap
        let make = |bwb: &str| LawMetadata {
            bwb_id: bwb.to_string(),
            cvdr_id: None,
            title: format!("{prefix} {bwb}"),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        let a = make("BWBR0009790").to_slug();
        let b = make("BWBR0027415").to_slug();
        assert_ne!(a, b);
        assert!(a.len() <= MAX_SLUG_BYTES && b.len() <= MAX_SLUG_BYTES);
        assert!(a.ends_with("_bwbr0009790") && b.ends_with("_bwbr0027415"));
    }

    #[test]
    fn test_law_metadata_to_slug_cap_uses_cvdr_id_when_no_bwb() {
        let metadata = LawMetadata {
            bwb_id: String::new(),
            cvdr_id: Some("CVDR123456".to_string()),
            title: "x ".repeat(200), // forces the cap
            regulatory_layer: RegulatoryLayer::GemeentelijkeVerordening,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        let slug = metadata.to_slug();
        assert!(slug.len() <= MAX_SLUG_BYTES);
        assert!(slug.ends_with("_cvdr123456"), "got: {slug}");
    }

    #[test]
    fn test_law_metadata_to_slug_special_chars() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0000000".to_string(),
            cvdr_id: None,
            title: "Wet (test) - special!".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        assert_eq!(metadata.to_slug(), "wet_test_special");
    }

    #[test]
    fn test_law_metadata_to_slug_diacritics() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0000000".to_string(),
            cvdr_id: None,
            title: "Ministeriële regeling".to_string(),
            regulatory_layer: RegulatoryLayer::MinisterieleRegeling,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        // ë should be normalized to e
        assert_eq!(metadata.to_slug(), "ministeriele_regeling");
    }

    #[test]
    fn test_law_metadata_to_slug_various_diacritics() {
        let metadata = LawMetadata {
            bwb_id: "BWBR0000000".to_string(),
            cvdr_id: None,
            title: "Café résumé naïve".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: None,
            effective_date: None,
            valid_to: None,
            creator: None,
            scope_code: None,
        };
        // é, é, ï should all be normalized to their ASCII equivalents
        assert_eq!(metadata.to_slug(), "cafe_resume_naive");
    }

    #[test]
    fn test_reference_to_wetten_url() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0018451".to_string(),
            artikel: Some("4".to_string()),
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0018451#Artikel4"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_chapter() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: Some("5a".to_string()),
            paragraaf: None,
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Hoofdstuk5a"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_section() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: Some("3.1".to_string()),
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Afdeling3.1"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_paragraph() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: None,
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: Some("2".to_string()),
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Paragraaf2"
        );
    }

    #[test]
    fn test_reference_to_wetten_url_priority() {
        // Article takes priority over chapter
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0009950".to_string(),
            artikel: Some("1".to_string()),
            lid: None,
            onderdeel: None,
            hoofdstuk: Some("5a".to_string()),
            paragraaf: None,
            afdeling: None,
        };

        assert_eq!(
            reference.to_wetten_url(None),
            "https://wetten.overheid.nl/BWBR0009950#Artikel1"
        );
    }

    #[test]
    fn test_format_reference_definitions() {
        let references = vec![
            Reference::new("ref1", "BWBR0018451"),
            Reference {
                id: "ref2".to_string(),
                bwb_id: "BWBR0018450".to_string(),
                artikel: Some("1".to_string()),
                ..Reference::new("", "")
            },
        ];

        let result = format_reference_definitions(&references);
        assert!(result.contains("[ref1]: https://wetten.overheid.nl/BWBR0018451"));
        assert!(result.contains("[ref2]: https://wetten.overheid.nl/BWBR0018450#Artikel1"));
    }

    #[test]
    fn test_format_reference_definitions_empty() {
        let references: Vec<Reference> = vec![];
        assert_eq!(format_reference_definitions(&references), "");
    }

    #[test]
    fn test_regulatory_layer_serialization() {
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::Wet).unwrap(),
            "\"WET\""
        );
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::MinisterieleRegeling).unwrap(),
            "\"MINISTERIELE_REGELING\""
        );
    }
}
