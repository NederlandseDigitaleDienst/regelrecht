//! Versions per in-force date, built offline from a trimmed Awir (BWBR0018472).
//!
//! The fixture has three consolidations: 2025-01-01, 2026-01-01 and
//! 2026-10-01. Article 13a is enacted early but only comes into force on
//! 2026-10-01; chapter 9 (article 24a) comes into force on 2027-01-01, a date
//! BWB has no consolidation for yet.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too. `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use regelrecht_harvester::harvester::{build_version, parse_law};
use regelrecht_harvester::manifest::{consolidation_dates_between, parse_manifest, BwbManifest};
use regelrecht_harvester::types::Law;
use regelrecht_harvester::wti::{parse_wti_metadata, WtiParseResult};
use regelrecht_harvester::yaml::generate_yaml;

const BWB_ID: &str = "BWBR0018472";

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/versies")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to load {}: {e}", path.display()))
}

fn manifest() -> BwbManifest {
    parse_manifest(&fixture("manifest.xml"), BWB_ID).unwrap()
}

fn wti() -> WtiParseResult {
    let xml = fixture("wti.xml");
    let doc = roxmltree::Document::parse(&xml).unwrap();
    parse_wti_metadata(&doc)
}

/// Build every version in the period the way `download_law_versions` does,
/// with the content downloads replaced by fixture files.
fn versions(from: &str, to: &str) -> Vec<Law> {
    let manifest = manifest();
    let wti = wti();
    consolidation_dates_between(&manifest, from, to)
        .iter()
        .map(|date| {
            let xml = fixture(&format!("content_{date}.xml"));
            build_version(wti.clone(), &manifest, &xml, BWB_ID, date).unwrap()
        })
        .collect()
}

fn numbers(law: &Law) -> Vec<&str> {
    law.articles.iter().map(|a| a.number.as_str()).collect()
}

#[test]
fn nogniet_article_is_left_out_with_a_warning() {
    let xml = fixture("content_2025-01-01.xml");
    let law = parse_law(wti(), &xml, BWB_ID, "2025-01-01").unwrap();

    assert_eq!(numbers(&law), ["1", "13"]);
    assert!(
        law.warnings
            .iter()
            .any(|w| w.starts_with("Article 13a: not in force on 2025-01-01")),
        "warnings: {:?}",
        law.warnings
    );
}

#[test]
fn one_version_per_in_force_date() {
    let laws = versions("2025-01-01", "2026-12-31");

    let dates: Vec<Option<&str>> = laws
        .iter()
        .map(|l| l.metadata.effective_date.as_deref())
        .collect();
    assert_eq!(
        dates,
        [Some("2025-01-01"), Some("2026-01-01"), Some("2026-10-01")]
    );

    // 13a appears only in the version of its own in-force date; chapter 9
    // (24a) is in force in none of them.
    assert_eq!(numbers(&laws[0]), ["1", "13"]);
    assert_eq!(numbers(&laws[1]), ["1", "13"]);
    assert_eq!(numbers(&laws[2]), ["1", "13", "13a"]);

    // Each version's YAML has its own in-force date as valid_from.
    for law in &laws {
        let date = law.metadata.effective_date.as_deref().unwrap();
        let yaml = generate_yaml(law, date).unwrap();
        assert!(yaml.contains(&format!("valid_from: '{date}'")), "{yaml}");
    }

    // Only the current consolidation has no successor, and it is open-ended.
    assert!(laws.iter().all(|l| l.metadata.valid_to.is_none()));
}

#[test]
fn version_points_to_where_a_pending_article_applies() {
    let laws = versions("2026-01-01", "2026-10-01");

    // 13a has a consolidation of its own: the warning names it.
    assert!(
        laws[0].warnings.iter().any(|w| w
            == "Article 13a: not in force on 2026-01-01; left out of this version, it is in the version of 2026-10-01"),
        "warnings: {:?}",
        laws[0].warnings
    );
    // 24a comes into force on a date BWB has no consolidation for yet.
    assert!(
        laws[1].warnings.iter().any(|w| w
            == "Article 24a: not in force on 2026-10-01; it comes into force on 2027-01-01, but BWB has no consolidation for that date yet"),
        "warnings: {:?}",
        laws[1].warnings
    );
}

#[test]
fn period_bounds_are_inclusive_and_may_be_empty() {
    let manifest = manifest();
    assert_eq!(
        consolidation_dates_between(&manifest, "2026-10-01", "2026-10-01"),
        ["2026-10-01"]
    );
    assert!(consolidation_dates_between(&manifest, "2025-01-02", "2025-12-31").is_empty());
}

#[test]
fn publication_date_is_latest_publication_in_force() {
    let laws = versions("2025-01-01", "2026-12-31");
    let publication_dates: Vec<Option<&str>> = laws
        .iter()
        .map(|l| l.metadata.publication_date.as_deref())
        .collect();

    // Not the first WTI date (2022-12-27), and not the publication of 13a
    // (2025-06-01) or chapter 9 (2026-09-30) while those are not in force.
    assert_eq!(
        publication_dates,
        [Some("2024-12-24"), Some("2025-12-24"), Some("2025-12-24")]
    );
}

#[test]
fn celex_reference_links_to_eur_lex() {
    let laws = versions("2025-01-01", "2025-01-01");
    let art_1 = &laws[0].articles[0];

    assert!(
        art_1.text.contains(
            "[Richtlijn 2001/55/EG](https://eur-lex.europa.eu/legal-content/NL/TXT/?uri=CELEX:32001L0055)"
        ),
        "got:\n{}",
        art_1.text
    );
    assert!(!art_1.text.contains("](32001L0055)"));
}
