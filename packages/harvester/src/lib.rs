//! RegelRecht Harvester - Download Dutch legislation from BWB and CVDR repositories.
//!
//! This crate provides functionality to download Dutch laws from the
//! Basiswettenbestand (BWB) repository and decentrale regelgeving from
//! CVDR (Centrale Voorziening Decentrale Regelgeving), converting them
//! to schema-compliant YAML format.
//!
//! # Example
//!
//! ```
//! use regelrecht_harvester::config;
//!
//! // Validate BWB ID and date
//! assert!(config::validate_bwb_id("BWBR0018451").is_ok());
//! assert!(config::validate_date("2025-01-01").is_ok());
//!
//! // Validate CVDR ID
//! assert!(config::validate_cvdr_id("CVDR681386").is_ok());
//! ```
//!
//! # Architecture
//!
//! The harvester is organized into several modules:
//!
//! - [`config`]: Configuration constants and validation
//! - [`types`]: Core data types (Law, Article, Reference, etc.)
//! - [`error`]: Error types and Result alias
//! - [`http`]: HTTP client for downloading from BWB/CVDR
//! - [`wti`]: WTI metadata parsing (BWB)
//! - [`manifest`]: BWB manifest parsing for consolidation date resolution
//! - [`content`]: Content XML downloading (BWB)
//! - [`cvdr`]: CVDR harvester (search, download, parse)
//! - [`xml`]: XML utilities
//! - [`registry`]: Extensible element handler system
//! - [`splitting`]: Article splitting logic
//! - [`placement`]: The containers (hoofdstuk, afdeling, ...) around an article
//! - [`yaml`]: YAML output generation
//! - `cli`: Command-line interface (feature `cli`, on by default)
//! - [`harvester`]: Main harvester service
//! - [`source`]: Strategy trait for law sources (BWB, CVDR)

#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod content;
pub mod cvdr;
pub mod error;
pub mod harvester;
pub mod http;
pub mod manifest;
pub mod placement;
pub mod registry;
pub mod source;
pub mod splitting;
pub mod types;
pub mod wti;
pub mod xml;
pub mod yaml;

// Re-export main functions
pub use cvdr::download_cvdr_law;
pub use harvester::{download_law, download_law_with_max_size};

// Re-export source trait and detection
pub use source::{BwbSource, CvdrSource, LawSource, LawSourceType, detect_source};

// Re-export commonly used items
pub use config::{validate_bwb_id, validate_cvdr_id, validate_date};
pub use error::{HarvesterError, NoTextReason, Result};
pub use types::{Article, Law, LawMetadata, Preamble, Reference, RegulatoryLayer};
