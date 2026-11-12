//! Build the context an agent reads, from the corpus file plus the source
//! gate's sidecar. The worker assembles; the agent reads.
//!
//! Two problems this solves.
//!
//! The harvested corpus splits below article level: a lid is its own entry
//! (`3.2`, holding the chapeau) and each onderdeel another (`3.2.a`). An
//! agent working entry by entry reads onderdeel a without the lid it
//! qualifies and without the article it sits in, which makes a correct
//! reading impossible rather than unlikely. So the article is put back
//! together first.
//!
//! And the reading needs more than the article: the definitions the law
//! declares, the articles beside it, and where it sits in the document.
//! None of that is reachable from a sandboxed agent with no network, so it
//! is written to files the agent can open.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use super::source_gate::ContextSidecar;

/// Where an article sits in the document, or why that is not known.
///
/// Three states rather than a string, because "no enclosing container" and
/// "nobody established where this sits" are opposite facts that a plain
/// `String` collapses into the same empty value. The source gate writes the
/// sidecar that carries placement; the enrich path runs without it, and a
/// brief that renders an unknown placement as law-wide scope tells the agent
/// that a definition governs an article nobody has checked it governs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Placement {
    /// The document says this article sits inside these containers.
    Known(String),
    /// The document says nothing encloses this article, which is normal for a
    /// short law.
    NoContainer,
    /// Nobody established where it sits: the sidecar is absent.
    Unknown,
}

impl Placement {
    /// Whether the two placements are known to describe the same container or
    /// one enclosing the other, with `self` the wider of the two.
    ///
    /// `Unknown` on either side answers nothing: an unknown placement cannot
    /// establish containment and must not be read as failing to either.
    pub fn encloses(&self, inner: &Self) -> Option<bool> {
        match (self, inner) {
            (Self::Unknown, _) | (_, Self::Unknown) => None,
            (Self::NoContainer, _) => Some(true),
            (Self::Known(_), Self::NoContainer) => Some(false),
            (Self::Known(outer), Self::Known(inner)) => Some(inner.starts_with(outer.as_str())),
        }
    }
}

/// One article, put back together from however many corpus entries carry it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledArticle {
    /// Article number as the statute gives it, without any lid path.
    pub number: String,
    /// The corpus entry numbers that make it up, in document order.
    pub fragments: Vec<String>,
    /// The whole article: chapeau, leden and items in order, each prefixed
    /// with the path it came from so the reader can address a lid.
    pub text: String,
    /// Where the article sits, e.g. "Hoofdstuk 3 Besluiten > Afdeling 3.3
    /// Advisering", or why that is not known.
    pub path: Placement,
    /// How far this article's definitions reach, when it is one.
    pub definition_scope: Option<DefinitionScope>,
    /// Whether any fragment already carries a translation.
    pub has_model: bool,
}

/// How far a begripsbepaling reaches, as the provision itself says.
///
/// A definition article announces its own scope: "in deze wet" reaches the
/// whole statute, "in deze afdeling" reaches one afdeling, "voor de toepassing
/// van dit artikel" reaches nothing outside itself. Placement alone cannot
/// tell these apart, and reading every begripsbepaling as law-wide is how
/// article 2.62 of the Wet basisregistratie personen, which says "in deze
/// afdeling", ends up presented as governing article 1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefinitionScope {
    /// "in deze wet" — every article of this statute.
    WholeLaw,
    /// "in dit hoofdstuk" — the chapter this article sits in.
    Chapter,
    /// "in deze afdeling" — the division this article sits in.
    Division,
    /// "in deze paragraaf" — the paragraph this article sits in.
    Paragraph,
    /// "voor de toepassing van dit artikel" — nothing but this article.
    ArticleOnly,
}

impl DefinitionScope {
    /// The container level this scope is bounded by, as it appears in a
    /// placement path, or `None` when the scope needs no container.
    pub fn container_prefix(self) -> Option<&'static str> {
        match self {
            Self::WholeLaw | Self::ArticleOnly => None,
            Self::Chapter => Some("Hoofdstuk"),
            Self::Division => Some("Afdeling"),
            Self::Paragraph => Some("Paragraaf"),
        }
    }

    /// What the brief calls this scope.
    pub fn describe(self) -> &'static str {
        match self {
            Self::WholeLaw => "whole law",
            Self::Chapter => "this chapter",
            Self::Division => "this division",
            Self::Paragraph => "this paragraph",
            Self::ArticleOnly => "this article only",
        }
    }
}

/// Everything the agent gets for one law.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LawContext {
    pub law_id: String,
    pub bwb_id: String,
    pub valid_from: String,
    /// The definition provisions of this law, whole, because a term's
    /// meaning and its scope decide questions the article alone cannot.
    pub definitions: Vec<AssembledArticle>,
    /// Every article, in document order.
    pub articles: Vec<AssembledArticle>,
}

impl LawContext {
    /// Article numbers in document order.
    pub fn numbers(&self) -> Vec<&str> {
        self.articles.iter().map(|a| a.number.as_str()).collect()
    }

    /// The articles beside `number` within the same container, which is
    /// where an establishing article and the article conferring its
    /// competences almost always sit relative to each other.
    pub fn siblings(&self, number: &str) -> Vec<&AssembledArticle> {
        let Some(target) = self.articles.iter().find(|a| a.number == number) else {
            return Vec::new();
        };
        self.articles
            .iter()
            .filter(|a| a.number != number && a.path == target.path)
            .collect()
    }
}

/// Split a corpus entry number into its article and the path within it.
/// `known` is the set of article numbers the source has; the longest one
/// that prefixes the entry wins, so in a law with articles `5` and `5.2`
/// the entry `5.2.1` belongs to `5.2`.
pub fn split_number<'a>(known: &'a [String], entry: &str) -> (&'a str, String) {
    let mut best: Option<(&str, String)> = None;
    for candidate in known {
        if entry == candidate {
            return (candidate.as_str(), String::new());
        }
        if let Some(rest) = entry.strip_prefix(candidate.as_str())
            && let Some(path) = rest.strip_prefix('.')
            && best
                .as_ref()
                .is_none_or(|(cur, _)| cur.len() < candidate.len())
        {
            best = Some((candidate.as_str(), path.to_string()));
        }
    }
    best.unwrap_or(("", entry.to_string()))
}

/// Assemble a corpus law file into the context an agent reads.
///
/// `known_articles` comes from the source gate: the article numbers the
/// statute actually has. Without it the split is guesswork, because a
/// corpus entry `5.2.1` is ambiguous on its own.
pub fn assemble(
    doc: &Value,
    known_articles: &[String],
    sidecar: Option<&ContextSidecar>,
) -> LawContext {
    struct Fragment {
        article: String,
        entry: String,
        path: String,
        text: String,
        has_model: bool,
    }

    let empty = Vec::new();
    let fragments: Vec<Fragment> = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty)
        .iter()
        .filter_map(|entry| {
            let number = entry.get("number").and_then(Value::as_str)?;
            let (article, path) = split_number(known_articles, number);
            Some(Fragment {
                article: if article.is_empty() {
                    number.to_string()
                } else {
                    article.to_string()
                },
                entry: number.to_string(),
                path,
                text: entry
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                has_model: entry.get("machine_readable").is_some(),
            })
        })
        .collect();

    // Document order, not sorted order: the statute's own sequence is the
    // one a reader needs.
    let mut order: Vec<String> = Vec::new();
    let mut grouped: BTreeMap<String, Vec<&Fragment>> = BTreeMap::new();
    for f in &fragments {
        if !grouped.contains_key(&f.article) {
            order.push(f.article.clone());
        }
        grouped.entry(f.article.clone()).or_default().push(f);
    }

    let mut articles = Vec::new();
    for number in &order {
        let Some(parts) = grouped.get(number) else {
            continue;
        };
        let text = parts
            .iter()
            .map(|f| {
                if f.path.is_empty() {
                    f.text.clone()
                } else {
                    // Prefix with the path so a reader can say "lid 2,
                    // onderdeel a" and mean something checkable.
                    format!("[{}] {}", f.path, f.text)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let definition_scope = definition_scope(&text);
        articles.push(AssembledArticle {
            number: number.clone(),
            fragments: parts.iter().map(|f| f.entry.clone()).collect(),
            path: match sidecar {
                None => Placement::Unknown,
                Some(s) => match s.articles.get(number) {
                    None => Placement::Unknown,
                    Some(a) if a.path.is_empty() => Placement::NoContainer,
                    Some(a) => Placement::Known(a.path.clone()),
                },
            },
            text,
            definition_scope,
            has_model: parts.iter().any(|f| f.has_model),
        });
    }

    let definitions = articles
        .iter()
        .filter(|a| a.definition_scope.is_some())
        .cloned()
        .collect();

    LawContext {
        law_id: doc
            .get("$id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        bwb_id: doc
            .get("bwb_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        valid_from: doc
            .get("valid_from")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        definitions,
        articles,
    }
}

/// How far a definition provision reaches, or `None` when it is not one.
///
/// A begripsbepaling announces itself in fixed words, and in the same breath
/// it announces its own reach: "in deze wet", "in dit hoofdstuk", "in deze
/// afdeling", "voor de toepassing van dit artikel". Keeping which one it was
/// is the whole point — a scope thrown away can only be guessed back as the
/// widest one, and the widest one is wrong in exactly the cases that matter.
///
/// The narrowest phrase present wins. An article that says "in deze wet" in
/// its first lid and "voor de toepassing van dit artikel" in its fourth is
/// still law-wide for the terms in the first, so widest-wins would be the
/// defensible reading there; but the reverse combination is far more common in
/// the corpus, and a definition offered too narrowly costs the agent a lookup
/// while one offered too widely costs it a wrong rule.
fn definition_scope(text: &str) -> Option<DefinitionScope> {
    let lower = text.to_lowercase();
    // Without one of these the article is not a begripsbepaling at all, and a
    // scope phrase on its own ("in deze afdeling geldt een termijn van ...")
    // is an ordinary rule.
    let declares = lower.contains("wordt verstaan onder")
        || lower.contains("wordt in deze")
        || lower.contains("verstaan onder:");
    if !declares {
        return None;
    }
    // Narrowest first.
    for (needles, scope) in [
        (
            ["voor de toepassing van dit artikel", "in dit artikel"].as_slice(),
            DefinitionScope::ArticleOnly,
        ),
        (
            ["in deze paragraaf", "voor de toepassing van deze paragraaf"].as_slice(),
            DefinitionScope::Paragraph,
        ),
        (
            ["in deze afdeling", "voor de toepassing van deze afdeling"].as_slice(),
            DefinitionScope::Division,
        ),
        (
            ["in dit hoofdstuk", "voor de toepassing van dit hoofdstuk"].as_slice(),
            DefinitionScope::Chapter,
        ),
        (
            [
                "in deze wet",
                "voor de toepassing van deze wet",
                "in dit besluit",
                "in deze regeling",
            ]
            .as_slice(),
            DefinitionScope::WholeLaw,
        ),
    ] {
        if needles.iter().any(|n| lower.contains(n)) {
            return Some(scope);
        }
    }
    // It defines something and says nothing about how far that reaches. The
    // article is the only container the text itself establishes, so that is
    // what it gets: presenting it law-wide would be an assertion the provision
    // never made.
    Some(DefinitionScope::ArticleOnly)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn known() -> Vec<String> {
        ["1", "3", "5.2"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn split_number_prefers_the_longest_article() {
        let k = known();
        assert_eq!(split_number(&k, "5.2.1"), ("5.2", "1".to_string()));
        assert_eq!(split_number(&k, "3.2.a"), ("3", "2.a".to_string()));
        assert_eq!(split_number(&k, "3"), ("3", String::new()));
    }

    #[test]
    fn fragments_are_put_back_into_one_article() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: test_wet
articles:
  - number: '3.1'
    text: Eerste lid.
  - number: '3.2'
    text: 'Chapeau van het tweede lid:'
  - number: 3.2.a
    text: onderdeel a;
  - number: 3.2.b
    text: onderdeel b.
"#,
        )
        .unwrap();
        let ctx = assemble(&doc, &known(), None);
        assert_eq!(ctx.articles.len(), 1);
        let a = &ctx.articles[0];
        assert_eq!(a.number, "3");
        assert_eq!(a.fragments, vec!["3.1", "3.2", "3.2.a", "3.2.b"]);
        // The reader gets the chapeau together with the items it governs,
        // which is the whole point of assembling.
        assert!(a.text.contains("[1] Eerste lid."), "{}", a.text);
        assert!(a.text.contains("[2] Chapeau"), "{}", a.text);
        assert!(a.text.contains("[2.a] onderdeel a;"), "{}", a.text);
        assert!(!a.has_model);
    }

    #[test]
    fn an_unfragmented_article_keeps_its_text_as_is() {
        let doc: Value = serde_yaml_ng::from_str(
            "$id: test_wet\narticles:\n  - number: '1'\n    text: De hele tekst.\n    machine_readable: {}\n",
        )
        .unwrap();
        let ctx = assemble(&doc, &known(), None);
        assert_eq!(ctx.articles[0].text, "De hele tekst.");
        assert!(ctx.articles[0].has_model);
    }

    #[test]
    fn a_real_fragmented_law_assembles_into_whole_articles() {
        // The shape the harvested corpus actually has for Awir article 3:
        // the lid carries the chapeau, the onderdelen stand apart.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: algemene_wet_inkomensafhankelijke_regelingen
articles:
  - number: '3.1'
    text: Belanghebbende is degene die aanspraak maakt.
  - number: '3.2'
    text: 'In aanvulling op het eerste lid wordt mede verstaan onder partner degene die:'
  - number: 3.2.a
    text: uit wiens relatie met de belanghebbende een kind is geboren;
  - number: 3.2.b
    text: die een kind van de belanghebbende heeft erkend;
  - number: '4.1'
    text: Onder kind wordt verstaan een eigen kind.
"#,
        )
        .unwrap();
        let known: Vec<String> = ["3", "4"].iter().map(|s| s.to_string()).collect();
        let ctx = assemble(&doc, &known, None);
        assert_eq!(ctx.numbers(), vec!["3", "4"]);
        let a3 = &ctx.articles[0];
        assert_eq!(a3.fragments.len(), 4);
        // Without assembly an agent would read onderdeel a with no idea
        // that it completes "degene die:".
        assert!(a3.text.contains("mede verstaan onder partner degene die:"));
        assert!(a3.text.contains("[2.a] uit wiens relatie"));
    }

    #[test]
    fn a_definition_keeps_the_scope_its_own_words_state() {
        // Thrown away, this can only be guessed back as the widest scope, and
        // the widest one is wrong in exactly the cases that matter.
        for (text, want) in [
            (
                "In deze wet wordt verstaan onder: a. toeslag: ...",
                DefinitionScope::WholeLaw,
            ),
            (
                "In dit hoofdstuk wordt verstaan onder: b. peildatum: ...",
                DefinitionScope::Chapter,
            ),
            (
                "In deze afdeling en de daarop berustende bepalingen wordt verstaan onder een aangewezen bestuursorgaan: ...",
                DefinitionScope::Division,
            ),
            (
                "In deze paragraaf wordt verstaan onder: c. termijn: ...",
                DefinitionScope::Paragraph,
            ),
            (
                "Voor de toepassing van dit artikel wordt verstaan onder: d. jaar: ...",
                DefinitionScope::ArticleOnly,
            ),
            // States no scope at all, so it claims nothing beyond itself.
            (
                "Hieronder wordt verstaan onder kind: een eigen kind.",
                DefinitionScope::ArticleOnly,
            ),
        ] {
            assert_eq!(definition_scope(text), Some(want), "{text}");
        }
        assert_eq!(
            definition_scope("De toeslag bedraagt het standaardbedrag."),
            None
        );
        // A scope phrase without a definition is an ordinary rule.
        assert_eq!(
            definition_scope("In deze afdeling geldt een termijn van zes weken."),
            None
        );
    }

    #[test]
    fn placement_says_whether_anybody_looked() {
        let doc: Value = serde_yaml_ng::from_str(
            "$id: test_wet\narticles:\n  - number: '1'\n    text: De tekst.\n",
        )
        .unwrap();
        // No sidecar: nobody established where article 1 sits.
        assert_eq!(
            assemble(&doc, &known(), None).articles[0].path,
            Placement::Unknown
        );

        let sidecar: ContextSidecar = serde_yaml_ng::from_str(
            "bwb_id: BWBR0000000\nvalid_from: '2026-01-01'\narticles:\n  '1':\n    verdict: verified\n",
        )
        .unwrap();
        // Sidecar present and silent about containers: none enclose it.
        assert_eq!(
            assemble(&doc, &known(), Some(&sidecar)).articles[0].path,
            Placement::NoContainer
        );
    }

    #[test]
    fn definition_articles_are_singled_out() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: test_wet
articles:
  - number: '1'
    text: 'In deze wet wordt verstaan onder: a. verzekerde: degene die ...'
  - number: '3'
    text: De verzekerde heeft aanspraak.
"#,
        )
        .unwrap();
        let ctx = assemble(&doc, &known(), None);
        assert_eq!(ctx.definitions.len(), 1);
        assert_eq!(ctx.definitions[0].number, "1");
    }
}
