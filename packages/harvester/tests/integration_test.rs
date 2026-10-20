//! End-to-end integration tests for the harvester pipeline.
//!
//! Tests the complete pipeline from XML parsing to YAML generation
//! using fixture data from the Wet op de zorgtoeslag (BWBR0018451).

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use regelrecht_harvester::harvester::parse_law;
use regelrecht_harvester::types::{Law, RegulatoryLayer};
use regelrecht_harvester::wti::parse_wti_metadata;
use regelrecht_harvester::yaml::generate_yaml;

/// Load fixture file content.
fn load_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("zorgtoeslag")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to load {}: {}", path.display(), e))
}

/// Run the production harvester pipeline on the zorgtoeslag fixtures.
///
/// Only the downloads are replaced by fixture files; parsing goes through
/// [`parse_law`], the same function `download_law` uses.
fn run_pipeline() -> Law {
    let wti_xml = load_fixture("wti.xml");
    let content_xml = load_fixture("content.xml");

    let wti_doc = roxmltree::Document::parse(&wti_xml).expect("Failed to parse WTI XML");
    let wti_result = parse_wti_metadata(&wti_doc);
    let bwb_id = wti_result.metadata.bwb_id.clone();

    parse_law(wti_result, &content_xml, &bwb_id, "2025-01-01").expect("Failed to parse content")
}

#[test]
fn test_pipeline_article_count() {
    let law = run_pipeline();

    // One component per artikel (article-level, as in production); the
    // preamble is separate.
    let numbers: Vec<&str> = law.articles.iter().map(|a| a.number.as_str()).collect();
    assert_eq!(numbers, ["1", "2", "3", "4", "4a", "5", "6", "7", "8"]);

    // Preamble should be present
    assert!(law.preamble.is_some(), "Should have preamble");
}

#[test]
fn test_pipeline_metadata() {
    let law = run_pipeline();

    assert_eq!(law.metadata.bwb_id, "BWBR0018451");
    assert_eq!(law.metadata.title, "Wet op de zorgtoeslag");
    assert_eq!(law.metadata.regulatory_layer, RegulatoryLayer::Wet);
}

#[test]
fn test_pipeline_preamble() {
    let law = run_pipeline();

    let preamble = law.preamble.as_ref().expect("Should have preamble");
    assert!(
        preamble.text.contains("Wij Beatrix"),
        "Preamble should contain 'Wij Beatrix'"
    );
    assert!(
        preamble.text.contains("zorgtoeslag") || preamble.text.contains("zorgverzekering"),
        "Preamble should mention zorgtoeslag or zorgverzekering"
    );
    assert!(
        preamble.url.contains("Aanhef"),
        "Preamble URL should contain 'Aanhef'"
    );
}

#[test]
fn test_pipeline_article_1_folds_leden_and_onderdelen() {
    let law = run_pipeline();

    let art_1 = law
        .articles
        .iter()
        .find(|a| a.number == "1")
        .expect("artikel 1");
    let blocks: Vec<&str> = art_1.text.split("\n\n").collect();
    assert_eq!(blocks.len(), 11, "got:\n{}", art_1.text);

    // Lid 1 intro, its onderdelen a..h, then lid 2, each with its enumerator,
    // followed by the reference definitions.
    assert!(
        blocks[0].starts_with("1 In deze wet"),
        "got:\n{}",
        art_1.text
    );
    assert_eq!(
        blocks[1],
        "a. Onze Minister: Onze Minister van Volksgezondheid, Welzijn en Sport;"
    );
    assert!(blocks[9].starts_with("2 De hoogte van de zorgtoeslag"));
    assert!(blocks[10].trim_start().starts_with("[ref1]: "));
}

#[test]
fn test_pipeline_article_urls() {
    let law = run_pipeline();

    for (number, url) in [
        (
            "1",
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1",
        ),
        (
            "8",
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel8",
        ),
    ] {
        let art = law
            .articles
            .iter()
            .find(|a| a.number == number)
            .unwrap_or_else(|| panic!("artikel {number}"));
        assert_eq!(art.url, url);
    }
}

#[test]
fn test_pipeline_citeertitel() {
    let law = run_pipeline();

    let art_8 = law
        .articles
        .iter()
        .find(|a| a.number == "8")
        .expect("artikel 8");
    assert_eq!(
        art_8.text,
        "Deze wet wordt aangehaald als: Wet op de zorgtoeslag."
    );
}

/// Golden: the generated YAML for the zorgtoeslag fixture matches
/// `fixtures/zorgtoeslag/expected.yaml` byte-for-byte. The fixture documents
/// the writer's real-world output shape (folded prose blocks, literal blocks
/// for reference-bearing articles, quoting, indentation). After an
/// intentional writer change, regenerate it with:
/// `UPDATE_EXPECTED=1 cargo test --test integration_test`
#[test]
fn test_yaml_matches_expected_fixture() {
    let law = run_pipeline();
    let yaml = generate_yaml(&law, "2025-01-01").expect("Failed to generate YAML");
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zorgtoeslag/expected.yaml");
    if std::env::var_os("UPDATE_EXPECTED").is_some() {
        fs::write(&path, &yaml).expect("failed to update expected.yaml");
    }
    let expected = fs::read_to_string(&path).expect("failed to read expected.yaml");
    assert_eq!(
        yaml, expected,
        "generated YAML diverges from expected.yaml; rerun with UPDATE_EXPECTED=1 after an intentional writer change"
    );
}

#[test]
fn test_yaml_folds_wrapped_prose() {
    let law = run_pipeline();
    let yaml = generate_yaml(&law, "2025-01-01").expect("Failed to generate YAML");

    // The preamble is long multi-paragraph prose: it must be emitted as a
    // folded block so the cosmetic 115-char wrapping does not become real
    // newlines in the loaded text.
    assert!(
        yaml.contains("  text: >-\n"),
        "preamble should emit a folded block\n{yaml}"
    );

    let parsed: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&yaml).expect("Generated YAML should be valid");
    let preamble_text = parsed
        .get("preamble")
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .expect("preamble.text should be a string");

    // Every newline that survives loading must be a paragraph break: single
    // newlines were cosmetic wraps and fold back to spaces.
    let mut rest = preamble_text;
    while let Some(pos) = rest.find('\n') {
        rest = &rest[pos..];
        let run = rest.chars().take_while(|&c| c == '\n').count();
        assert_eq!(
            run, 2,
            "loaded preamble may only contain paragraph breaks (\\n\\n), found a run of {run}:\n{preamble_text}"
        );
        rest = &rest[run..];
    }
    assert!(
        preamble_text.contains("Wij Beatrix"),
        "folded preamble should round-trip its content"
    );
}

#[test]
fn test_references_extracted() {
    let law = run_pipeline();

    // Artikel 1 refers to the Zorgverzekeringswet (BWBR0018450).
    let art_1 = law
        .articles
        .iter()
        .find(|a| a.number == "1")
        .expect("artikel 1");
    assert!(
        art_1.references.iter().any(|r| r.bwb_id == "BWBR0018450"),
        "Should have reference to Zorgverzekeringswet (BWBR0018450): {:?}",
        art_1.references
    );
}

/// A skipped `<illustratie>` must leave a mark in the text.
///
/// Artikel 22a Participatiewet states the kostendelersnorm as a formula, and in
/// the BWB XML that formula is a PNG (`<illustratie>`), not text. The element is
/// on the skip list, so the sentence that introduces it ("is de norm per
/// kalendermaand voor de belanghebbende:") used to end on its colon and the norm
/// itself simply disappeared. Nothing downstream could tell that from a law that
/// genuinely says nothing there.
///
/// Dropping the image is fine; dropping it silently is not. The harvester leaves
/// a marker so a reader, and the schema validation behind it, can see that the
/// text is incomplete at that point.
#[test]
fn test_illustratie_leaves_a_marker_in_the_text() {
    use regelrecht_harvester::splitting::{
        create_dutch_law_hierarchy, LeafSplitStrategy, SplitContext, SplitEngine,
    };

    let xml = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formule/artikel22a_lid1.xml"),
    )
    .expect("fixture readable");
    let doc = roxmltree::Document::parse(&xml).expect("fixture parses");

    let hierarchy = create_dutch_law_hierarchy();
    let engine = SplitEngine::new(hierarchy, LeafSplitStrategy).with_article_level(true);
    let artikel = doc.root_element();
    let context = SplitContext::new(
        "BWBR0015703",
        "2022-03-15",
        "https://wetten.overheid.nl/BWBR0015703/2022-03-15#Artikel22a".to_string(),
    );

    let components = engine.split(artikel, context);
    let text = components
        .iter()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        text.contains("is de norm per kalendermaand voor de belanghebbende:"),
        "the sentence introducing the formula should survive; got:\n{text}"
    );
    assert!(
        text.contains("[formule niet in tekst beschikbaar]"),
        "a skipped illustratie should leave a marker so the gap is visible; got:\n{text}"
    );
}

/// Load a formula fixture and return the harvested article text.
fn harvest_formule_fixture(name: &str) -> String {
    use regelrecht_harvester::splitting::{
        create_dutch_law_hierarchy, LeafSplitStrategy, SplitContext, SplitEngine,
    };
    let xml = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/formule")
            .join(name),
    )
    .expect("fixture readable");
    let doc = roxmltree::Document::parse(&xml).expect("fixture parses");
    let engine =
        SplitEngine::new(create_dutch_law_hierarchy(), LeafSplitStrategy).with_article_level(true);
    let context = SplitContext::new(
        "BWBR0015703",
        "2022-03-15",
        "https://example.invalid/artikel".to_string(),
    );
    engine
        .split(doc.root_element(), context)
        .iter()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The marker never replaces text that was actually there.
///
/// A `<formule>` does not always wrap an image; it can carry a readable
/// fallback in its children. Emitting the marker over that would be worse than
/// the silent drop it replaces: it would state the formula is unavailable at
/// the moment it was available and was discarded.
#[test]
fn test_formule_with_readable_text_keeps_the_text() {
    let text = harvest_formule_fixture("formule_met_tekst.xml");
    assert!(
        text.contains("A = B x C"),
        "readable formula text must survive; got:\n{text}"
    );
    assert!(
        !text.contains("[formule niet in tekst beschikbaar]"),
        "no marker when the text is there; got:\n{text}"
    );
}

/// An `<illustratie>` outside a `<plaatje>` still leaves a marker.
///
/// The usual shape wraps it, but nothing in the format guarantees that, and a
/// bare one used to vanish silently — the exact failure this all fixes.
#[test]
fn test_bare_illustratie_still_leaves_a_marker() {
    let text = harvest_formule_fixture("losse_illustratie.xml");
    assert!(
        text.contains("[formule niet in tekst beschikbaar]"),
        "a bare illustratie must not vanish; got:\n{text}"
    );
}

/// A nested `<plaatje><illustratie/></plaatje>` yields exactly one marker.
///
/// Both elements carry the handler now, so the parent renders the child's
/// marker as its text rather than adding a second one.
#[test]
fn test_nested_plaatje_illustratie_marks_once() {
    let text = harvest_formule_fixture("artikel22a_lid1.xml");
    assert_eq!(
        text.matches("[formule niet in tekst beschikbaar]").count(),
        1,
        "expected exactly one marker; got:\n{text}"
    );
}
