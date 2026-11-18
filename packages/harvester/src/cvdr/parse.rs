//! CVDR XML parsing into Article structs.
//!
//! CVDR XML uses a similar but different structure compared to BWB XML:
//! - Root: `<regeling>` → `<regeling-tekst>` → `<body>`
//! - Articles: `<artikel>` with `<kop>/<nr>` for numbering
//! - Paragraphs: `<lid>` with `<lidnr>` for numbering
//! - Text: `<al>` elements
//! - Lists: `<lijst>` with `<li>` items
//!
//! We reuse the existing splitting module where possible since the inner
//! article structure (artikel > lid > lijst > li) is identical to BWB.

use roxmltree::Document;

use crate::error::Result;
use crate::splitting::{LeafSplitStrategy, SplitContext, SplitEngine, create_dutch_law_hierarchy};
use crate::types::Article;
use crate::xml::{find_by_path, get_tag_name, get_text};

/// Parsed content from CVDR XML.
pub struct ParsedCvdrContent {
    /// Parsed articles.
    pub articles: Vec<Article>,

    /// Non-fatal warnings encountered during parsing.
    pub warnings: Vec<String>,
}

/// Parse articles from CVDR XML content.
///
/// The CVDR XML structure differs from BWB at the top level:
/// ```text
/// <cvdr:body>
///   <regeling>
///     <regeling-tekst>
///       <body>
///         <hoofdstuk>       (optional)
///           <artikel>...
/// ```
///
/// But the inner article structure (artikel > lid > lijst > li) is the same,
/// so we reuse the existing `SplitEngine` for article splitting.
///
/// # Arguments
/// * `xml` - Raw CVDR XML string
/// * `base_url` - Base URL for the regulation on lokaleregelgeving.overheid.nl
///
/// # Returns
/// `ParsedCvdrContent` with articles and warnings
pub fn parse_cvdr_articles(xml: &str, base_url: &str) -> Result<ParsedCvdrContent> {
    let doc = Document::parse(xml)?;
    let mut articles = Vec::new();
    let mut all_warnings: Vec<String> = Vec::new();

    // Create split engine (reuses BWB article splitting logic). Article-level
    // granularity is the default: one entry per artikel with leden/onderdelen
    // folded inline (enumerators kept).
    let hierarchy = create_dutch_law_hierarchy();
    let engine = SplitEngine::new(hierarchy, LeafSplitStrategy).with_article_level(true);

    // Find all artikel elements regardless of their position in the tree
    // CVDR structure varies: may be under body, hoofdstuk, paragraaf, afdeling, etc.
    for artikel in doc
        .descendants()
        .filter(|n| n.is_element() && get_tag_name(*n) == "artikel")
    {
        // Get article number
        let artikel_nr = if let Some(nr_node) = find_by_path(artikel, "kop/nr") {
            get_text(nr_node)
        } else if let Some(label) = artikel.attribute("label") {
            label.strip_prefix("Artikel ").unwrap_or(label).to_string()
        } else {
            continue; // Skip articles without number
        };

        // Build article URL
        let artikel_nr_url = artikel_nr.replace(' ', "_");
        let article_url = format!("{base_url}#Artikel{artikel_nr_url}");

        // Create split context
        // Use an empty string for bwb_id since CVDR doesn't use BWB references
        let context = SplitContext::new("", "", article_url);

        // Split the artikel using the shared engine
        let components = engine.split(artikel, context);

        // Convert components to articles and collect warnings
        for component in components {
            for warning in &component.warnings {
                all_warnings.push(format!("Article {}: {}", component.to_number(), warning));
            }
            articles.push(component.to_article());
        }
    }

    Ok(ParsedCvdrContent {
        articles,
        warnings: all_warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_cvdr_article() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cvdr:body xmlns:cvdr="https://purl.overheid.nl/cvdr/def/">
  <regeling>
    <regeling-tekst>
      <body>
        <artikel>
          <kop><nr>1</nr><titel>Begripsbepalingen</titel></kop>
          <al>In deze verordening wordt verstaan onder gemeente: de gemeente Amsterdam.</al>
        </artikel>
      </body>
    </regeling-tekst>
  </regeling>
</cvdr:body>"#;

        let result =
            parse_cvdr_articles(xml, "https://lokaleregelgeving.overheid.nl/CVDR681386").unwrap();

        assert_eq!(result.articles.len(), 1);
        assert_eq!(result.articles[0].number, "1");
        assert!(result.articles[0].text.contains("gemeente Amsterdam"));
    }

    #[test]
    fn test_parse_cvdr_article_with_lid() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<body>
  <artikel>
    <kop><nr>2</nr><titel>Toepassingsgebied</titel></kop>
    <lid>
      <lidnr>1.</lidnr>
      <al>Eerste lid tekst.</al>
    </lid>
    <lid>
      <lidnr>2.</lidnr>
      <al>Tweede lid tekst.</al>
    </lid>
  </artikel>
</body>"#;

        let result =
            parse_cvdr_articles(xml, "https://lokaleregelgeving.overheid.nl/CVDR123456").unwrap();

        // Article-level granularity: one entry per artikel, leden folded inline
        // with their lidnr enumerators.
        assert_eq!(result.articles.len(), 1);
        assert_eq!(result.articles[0].number, "2");
        let text = &result.articles[0].text;
        assert!(text.contains("1. Eerste lid tekst."), "got:\n{text}");
        assert!(text.contains("2. Tweede lid tekst."), "got:\n{text}");
    }

    #[test]
    fn test_parse_cvdr_article_with_lijst() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<body>
  <artikel>
    <kop><nr>3</nr></kop>
    <lid>
      <lidnr>1.</lidnr>
      <al>In deze verordening wordt verstaan onder:</al>
      <lijst>
        <li><li.nr>a.</li.nr><al>begrip a: definitie a;</al></li>
        <li><li.nr>b.</li.nr><al>begrip b: definitie b.</al></li>
      </lijst>
    </lid>
  </artikel>
</body>"#;

        let result =
            parse_cvdr_articles(xml, "https://lokaleregelgeving.overheid.nl/CVDR123456").unwrap();

        // Article-level granularity: one entry per artikel, onderdelen folded
        // inline with their li.nr enumerators (no longer split into 3.1.a/3.1.b).
        assert_eq!(result.articles.len(), 1);
        assert_eq!(result.articles[0].number, "3");
        let text = &result.articles[0].text;
        assert!(text.contains("In deze verordening wordt verstaan onder:"));
        assert!(text.contains("a. begrip a: definitie a;"), "got:\n{text}");
        assert!(text.contains("b. begrip b: definitie b."), "got:\n{text}");
    }

    #[test]
    fn test_parse_cvdr_article_in_hoofdstuk() {
        // CVDR articles can be nested inside hoofdstuk (chapter) elements
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<body>
  <hoofdstuk>
    <kop><nr>1</nr><titel>Algemene bepalingen</titel></kop>
    <artikel>
      <kop><nr>1</nr></kop>
      <al>Eerste artikel in hoofdstuk.</al>
    </artikel>
    <artikel>
      <kop><nr>2</nr></kop>
      <al>Tweede artikel in hoofdstuk.</al>
    </artikel>
  </hoofdstuk>
</body>"#;

        let result =
            parse_cvdr_articles(xml, "https://lokaleregelgeving.overheid.nl/CVDR123456").unwrap();

        assert_eq!(result.articles.len(), 2);
        assert_eq!(result.articles[0].number, "1");
        assert_eq!(result.articles[1].number, "2");
    }

    #[test]
    fn test_parse_cvdr_empty_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><body/>"#;

        let result =
            parse_cvdr_articles(xml, "https://lokaleregelgeving.overheid.nl/CVDR123456").unwrap();

        assert!(result.articles.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_parse_cvdr_article_url_format() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<body>
  <artikel>
    <kop><nr>1</nr></kop>
    <al>Test text.</al>
  </artikel>
</body>"#;

        let result =
            parse_cvdr_articles(xml, "https://lokaleregelgeving.overheid.nl/CVDR681386").unwrap();

        assert_eq!(
            result.articles[0].url,
            "https://lokaleregelgeving.overheid.nl/CVDR681386#Artikel1"
        );
    }
}
