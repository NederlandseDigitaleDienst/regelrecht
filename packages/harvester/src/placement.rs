//! Where an article sits in the document: the containers that enclose it.
//!
//! An article in a BWB or CVDR toestand is nested in containers (`hoofdstuk`,
//! `afdeling`, `paragraaf`, ...), each with a number and an opschrift in its
//! `kop`. The splitter starts at `artikel` and never sees them, so they are
//! read here, from the article upwards, and written to the article's
//! `placement` (schema v0.7.0 and later).
//!
//! The opschrift is condensed legal classification written by the legislator.
//! It tells an editor how the law is organised, and it decides questions the
//! article text alone cannot answer: a definition at the head of an afdeling
//! governs that afdeling without any article referring to it.

use roxmltree::Node;
use serde::Serialize;

use crate::xml::{find_child, get_tag_name};

/// One container that encloses an article: its number and its opschrift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlacementLevel {
    /// Number as the law gives it (e.g. `"II"`, `"3.3"`). Empty when the
    /// container has an opschrift but no number.
    pub number: String,

    /// Opschrift of the container (e.g. `"Archiefbescheiden in het algemeen"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
}

/// The containers that enclose an article, one per kind the schema knows.
///
/// Empty for an article that no container encloses, which is normal in a
/// short law. Field order is the schema's, from most general to most specific.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boek: Option<PlacementLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deel: Option<PlacementLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<PlacementLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titeldeel: Option<PlacementLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<PlacementLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<PlacementLevel>,
}

impl Placement {
    /// True when no container encloses the article.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// The slot for a container element, or `None` for any other element.
    ///
    /// Elements the schema has no slot for (`sub-paragraaf`, `bijlage`,
    /// `divisie`) are not containers here.
    fn slot_mut(&mut self, tag: &str) -> Option<&mut Option<PlacementLevel>> {
        match tag {
            "boek" => Some(&mut self.boek),
            "deel" => Some(&mut self.deel),
            "hoofdstuk" => Some(&mut self.hoofdstuk),
            "titeldeel" => Some(&mut self.titeldeel),
            "afdeling" => Some(&mut self.afdeling),
            "paragraaf" => Some(&mut self.paragraaf),
            _ => None,
        }
    }
}

/// Read the containers that enclose `artikel`.
///
/// Ancestors are applied from the outermost inwards, so when a kind occurs
/// twice in the chain the innermost one is recorded, even when its `kop` says
/// nothing and the slot ends up empty: the article sits in that unnamed
/// container, not in the named one around it. Containers inside a `bijlage`
/// count like any other; the bijlage itself has no slot.
#[must_use]
pub fn find_placement(artikel: Node<'_, '_>) -> Placement {
    let ancestors: Vec<Node<'_, '_>> = artikel
        .ancestors()
        .skip(1)
        .filter(Node::is_element)
        .collect();

    let mut placement = Placement::default();
    for container in ancestors.into_iter().rev() {
        if let Some(slot) = placement.slot_mut(get_tag_name(container)) {
            *slot = read_level(container);
        }
    }
    placement
}

/// The number and opschrift from a container's `kop`, if it has either.
fn read_level(container: Node<'_, '_>) -> Option<PlacementLevel> {
    let kop = find_child(container, "kop")?;
    let number = find_child(kop, "nr").map(plain_text).unwrap_or_default();
    let heading = find_child(kop, "titel")
        .map(plain_text)
        .filter(|s| !s.is_empty());
    if number.is_empty() && heading.is_none() {
        return None;
    }
    Some(PlacementLevel { number, heading })
}

/// All text under `node` as one line, without footnotes.
///
/// An opschrift can carry inline markup (`nadruk`, `intref` in the Awb), whose
/// text belongs to the heading. A `noot` is a footnote and does not.
fn plain_text(node: Node<'_, '_>) -> String {
    let in_footnote = |text: &Node<'_, '_>| {
        text.ancestors()
            .take_while(|a| a != &node)
            .any(|a| a.is_element() && get_tag_name(a) == "noot")
    };
    let raw: String = node
        .descendants()
        .filter(|n| n.is_text() && !in_footnote(n))
        .filter_map(|n| n.text())
        .collect();
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::Document;

    fn artikel<'a, 'input>(doc: &'a Document<'input>, nr: &str) -> Node<'a, 'input> {
        doc.descendants()
            .filter(|n| n.has_tag_name("artikel"))
            .find(|n| {
                find_child(*n, "kop")
                    .and_then(|k| find_child(k, "nr"))
                    .map(plain_text)
                    .as_deref()
                    == Some(nr)
            })
            .unwrap_or_else(|| panic!("fixture has no artikel {nr}"))
    }

    fn level(number: &str, heading: &str) -> Option<PlacementLevel> {
        Some(PlacementLevel {
            number: number.to_string(),
            heading: Some(heading.to_string()),
        })
    }

    /// The shape of the Archiefwet 1995 (BWBR0007376): articles directly
    /// under numbered hoofdstukken, each with a `label`, `nr` and `titel`.
    const ARCHIEFWET: &str = r#"<wetgeving><wet-besluit><wettekst>
  <hoofdstuk label="Hoofdstuk I">
    <kop><label>Hoofdstuk</label><nr>I</nr><titel>Algemene bepalingen</titel></kop>
    <artikel label="Artikel 1"><kop><label>Artikel</label><nr>1</nr></kop><al>Begrippen.</al></artikel>
  </hoofdstuk>
  <hoofdstuk label="Hoofdstuk II">
    <kop><label>Hoofdstuk</label><nr>II</nr><titel>Archiefbescheiden in het algemeen</titel></kop>
    <artikel label="Artikel 3"><kop><label>Artikel</label><nr>3</nr></kop><al>Zorg.</al></artikel>
  </hoofdstuk>
</wettekst></wet-besluit></wetgeving>"#;

    #[test]
    fn articles_get_the_hoofdstuk_they_sit_in() {
        let doc = Document::parse(ARCHIEFWET).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "1")),
            Placement {
                hoofdstuk: level("I", "Algemene bepalingen"),
                ..Placement::default()
            }
        );
        assert_eq!(
            find_placement(artikel(&doc, "3")).hoofdstuk,
            level("II", "Archiefbescheiden in het algemeen")
        );
    }

    #[test]
    fn nested_containers_fill_every_level() {
        // Every kind the schema knows, each in its own slot. The Awb alone
        // nests hoofdstuk > titeldeel > afdeling > paragraaf.
        let xml = r#"<wettekst>
<boek><kop><nr>1</nr><titel>Boek</titel></kop>
<deel><kop><nr>A</nr><titel>Deel</titel></kop>
  <hoofdstuk><kop><nr>4</nr><titel>Bijzondere bepalingen over besluiten</titel></kop>
    <titeldeel><kop><nr>4.2</nr><titel>Subsidies</titel></kop>
      <afdeling><kop><nr>4.2.8</nr><titel>Subsidieplafond</titel></kop>
        <paragraaf><kop><nr>4.2.8.1</nr><titel>Algemeen</titel></kop>
          <artikel><kop><nr>4:22</nr></kop></artikel>
        </paragraaf>
      </afdeling>
    </titeldeel>
  </hoofdstuk>
</deel>
</boek>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "4:22")),
            Placement {
                boek: level("1", "Boek"),
                deel: level("A", "Deel"),
                hoofdstuk: level("4", "Bijzondere bepalingen over besluiten"),
                titeldeel: level("4.2", "Subsidies"),
                afdeling: level("4.2.8", "Subsidieplafond"),
                paragraaf: level("4.2.8.1", "Algemeen"),
            }
        );
    }

    #[test]
    fn heading_keeps_inline_text_and_drops_footnotes() {
        let xml = r#"<wettekst>
  <afdeling><kop><nr>3.4</nr><titel>Uitgebreide <nadruk type="cur">openbare</nadruk> voorbereidingsprocedure<noot><al>Vervallen.</al></noot></titel></kop>
    <artikel><kop><nr>3:10</nr></kop></artikel>
  </afdeling>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "3:10")).afdeling,
            level("3.4", "Uitgebreide openbare voorbereidingsprocedure")
        );
    }

    #[test]
    fn heading_joins_inline_elements_without_adding_spaces() {
        // As in the Awb: a reference in parentheses inside the opschrift.
        let xml = r#"<wettekst>
  <afdeling><kop><nr>8.2.6</nr><titel>Verzet (<intref doc="x">artikel 8:5</intref>)</titel></kop>
    <artikel><kop><nr>8:55</nr></kop></artikel>
  </afdeling>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "8:55")).afdeling,
            level("8.2.6", "Verzet (artikel 8:5)")
        );
    }

    #[test]
    fn article_outside_every_container_has_an_empty_placement() {
        let xml = r#"<wettekst><artikel><kop><nr>1</nr></kop></artikel></wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert!(find_placement(artikel(&doc, "1")).is_empty());
    }

    #[test]
    fn innermost_container_of_a_kind_wins() {
        let xml = r#"<wettekst>
  <paragraaf><kop><nr>1</nr><titel>Buiten</titel></kop>
    <paragraaf><kop><nr>1.1</nr><titel>Binnen</titel></kop>
      <artikel><kop><nr>1</nr></kop></artikel>
    </paragraaf>
  </paragraaf>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "1")).paragraaf,
            level("1.1", "Binnen")
        );
    }

    #[test]
    fn container_without_number_keeps_its_heading() {
        let xml = r#"<wettekst>
  <paragraaf><kop><titel>Slotbepalingen</titel></kop>
    <artikel><kop><nr>9</nr></kop></artikel>
  </paragraaf>
  <hoofdstuk><kop/>
    <artikel><kop><nr>10</nr></kop></artikel>
  </hoofdstuk>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "9")).paragraaf,
            Some(PlacementLevel {
                number: String::new(),
                heading: Some("Slotbepalingen".to_string()),
            })
        );
        // A container whose kop says nothing is not recorded.
        assert!(find_placement(artikel(&doc, "10")).is_empty());
    }

    #[test]
    fn unnamed_inner_container_empties_the_slot() {
        let xml = r#"<wettekst>
  <paragraaf><kop><nr>1</nr><titel>Buiten</titel></kop>
    <paragraaf>
      <artikel><kop><nr>1</nr></kop></artikel>
    </paragraaf>
  </paragraaf>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert!(find_placement(artikel(&doc, "1")).paragraaf.is_none());
    }

    #[test]
    fn bijlage_is_not_a_container_but_what_it_holds_is() {
        let xml = r#"<wettekst>
  <bijlage><kop><nr>1</nr><titel>Tabel</titel></kop>
    <hoofdstuk><kop><nr>A</nr><titel>Tarieven</titel></kop>
      <artikel><kop><nr>1</nr></kop></artikel>
    </hoofdstuk>
  </bijlage>
</wettekst>"#;
        let doc = Document::parse(xml).unwrap();

        assert_eq!(
            find_placement(artikel(&doc, "1")),
            Placement {
                hoofdstuk: level("A", "Tarieven"),
                ..Placement::default()
            }
        );
    }
}
