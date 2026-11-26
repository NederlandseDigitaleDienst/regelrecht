//! What one law says about itself, extracted once and kept.
//!
//! The chain of an article (which articles, in which laws, a change to it
//! reaches) and the hotspots of a traject (where enrichment would help most)
//! both walk the same per-law facts: which articles have a model, which
//! cite or bind which, which outputs and open terms they declare, and what is
//! broken in them. Reading those off the YAML is the expensive part, so it
//! happens here, per law, and a caller keeps the result keyed by the law's
//! content (editor-api keys it by blob sha). The walk over several laws
//! ([`super::closure::walk_closure`]) then needs no YAML at all.
//!
//! Pure: no I/O, no clock.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_corpus::consumers_index::{SourceBinding, source_bindings_of};
use serde_yaml_ng::Value;

use super::checks::dangling_bindings;
use super::closure::{LoadedLaw, loaded};
use super::refgraph::{Binding, Graph, Node, top_article};

/// Text this short that says "vervallen" is a lapsed article: there is
/// nothing left to model, so its missing `machine_readable` is no gap.
const LAPSED_TEXT_MAX_CHARS: usize = 40;

/// An `implements` declaration: this article fills an open term of another
/// law.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Implements {
    /// `$id` of the law whose open term is filled.
    pub law_id: String,
    /// The article of that law that declares the open term, when named.
    pub article: Option<String>,
    /// The open term's `id`.
    pub open_term: String,
}

/// The facts of one top-level article.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArticleFacts {
    /// Top-level article number.
    pub number: String,
    /// Whether any of its entries has a `machine_readable` section.
    pub has_mr: bool,
    /// Whether it lacks one while its text says something: a lapsed article
    /// or an empty one has nothing to model.
    pub missing_mr: bool,
    /// Whether it is a begripsbepaling.
    pub is_definition: bool,
    /// What its text or `references` names, other laws included.
    pub cites: Vec<Node>,
    /// What its model reads ([`Graph::binds_to`]), unresolved.
    pub binds: Vec<Binding>,
    /// The open terms of other laws it fills.
    pub implements: Vec<Implements>,
    /// The outputs its model declares.
    pub outputs: Vec<String>,
    /// The `id`s of the open terms its model declares.
    pub open_terms: Vec<String>,
    /// Names its model uses that nothing defines, and intra-law sources that
    /// name an output the law does not declare.
    pub dangling: usize,
    /// Untranslatables and markings: what the model says it cannot express.
    pub markings: usize,
}

/// The facts of one law.
#[derive(Debug, Clone, Default)]
pub struct LawFacts {
    /// `$id`.
    pub law_id: String,
    pub bwb_id: String,
    /// `regulatory_layer`.
    pub layer: String,
    /// The reference graph, for the walk.
    pub graph: Graph,
    /// Top-level articles that are begripsbepalingen.
    pub definitions: BTreeSet<String>,
    /// Its top-level articles in document order.
    pub articles: Vec<ArticleFacts>,
    /// Its bindings to other laws per entry, as the consumers index reads
    /// them.
    pub source_bindings: Vec<SourceBinding>,
}

impl LawFacts {
    /// Facts of a law body; `None` when it does not parse.
    #[must_use]
    pub fn from_yaml(yaml: &str) -> Option<Self> {
        let doc: Value = serde_yaml_ng::from_str(yaml).ok()?;
        Some(Self::from_doc(&doc))
    }

    /// Facts of a parsed law.
    #[must_use]
    pub fn from_doc(doc: &Value) -> Self {
        let LoadedLaw { graph, definitions } = loaded(doc);
        let text_of = |v: &Value, key: &str| {
            v.get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let mut dangling: BTreeMap<String, usize> = BTreeMap::new();
        for finding in dangling_bindings(doc) {
            if let Some(article) = finding.article {
                *dangling
                    .entry(top_article(&article).to_string())
                    .or_default() += 1;
            }
        }
        let mut articles: Vec<ArticleFacts> = graph
            .articles
            .iter()
            .map(|number| ArticleFacts {
                number: number.clone(),
                is_definition: definitions.contains(number),
                cites: graph
                    .depends_on
                    .get(&Node::new(&graph.bwb_id, number))
                    .map(|t| t.iter().cloned().collect())
                    .unwrap_or_default(),
                binds: graph
                    .binds_to
                    .get(number)
                    .map(|b| b.iter().cloned().collect())
                    .unwrap_or_default(),
                dangling: dangling.get(number).copied().unwrap_or(0),
                ..ArticleFacts::default()
            })
            .collect();
        let mut text: BTreeMap<String, String> = BTreeMap::new();
        for entry in doc
            .get("articles")
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
        {
            let Some(number) = entry_number(entry) else {
                continue;
            };
            let top = top_article(&number).to_string();
            let Some(facts) = articles.iter_mut().find(|a| a.number == top) else {
                continue;
            };
            let entry_text = entry.get("text").and_then(Value::as_str).unwrap_or("");
            let collected = text.entry(top).or_default();
            collected.push_str(entry_text);
            collected.push(' ');
            if let Some(mr) = entry.get("machine_readable") {
                facts.has_mr = true;
                add_model_facts(mr, facts);
            }
        }
        for facts in &mut articles {
            let body = text.get(&facts.number).map(String::as_str).unwrap_or("");
            facts.missing_mr = !facts.has_mr && says_something(body);
        }
        Self {
            law_id: text_of(doc, "$id"),
            bwb_id: graph.bwb_id.clone(),
            layer: text_of(doc, "regulatory_layer"),
            graph,
            definitions,
            articles,
            source_bindings: source_bindings_of(doc),
        }
    }

    /// The facts of one top-level article.
    #[must_use]
    pub fn article(&self, number: &str) -> Option<&ArticleFacts> {
        self.articles.iter().find(|a| a.number == number)
    }

    /// The law as the closure walk loads it.
    #[must_use]
    pub fn loaded(&self) -> LoadedLaw {
        LoadedLaw {
            graph: self.graph.clone(),
            definitions: self.definitions.clone(),
        }
    }
}

/// What one entry's model adds to its article's facts.
fn add_model_facts(mr: &Value, facts: &mut ArticleFacts) {
    let names = |seq: Option<&Value>, key: &str| -> Vec<String> {
        seq.and_then(Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(|v| v.get(key).and_then(Value::as_str))
            .map(str::to_string)
            .collect()
    };
    let execution = mr.get("execution");
    facts
        .outputs
        .extend(names(execution.and_then(|e| e.get("output")), "name"));
    facts.open_terms.extend(names(mr.get("open_terms"), "id"));
    for item in mr
        .get("implements")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        let Some(law_id) = item.get("law").and_then(Value::as_str) else {
            continue;
        };
        facts.implements.push(Implements {
            law_id: law_id.to_string(),
            article: item.get("article").map(scalar_text),
            open_term: item
                .get("open_term")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    for key in ["untranslatables", "markings"] {
        facts.markings += mr.get(key).and_then(Value::as_sequence).map_or(0, Vec::len);
    }
}

/// Whether an article's text leaves something to model: not empty, and not
/// the one word a lapsed article keeps.
fn says_something(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lapsed = trimmed.chars().count() <= LAPSED_TEXT_MAX_CHARS
        && trimmed.to_lowercase().contains("vervallen");
    !lapsed
}

/// Entry number as a string, however the YAML spelled it.
fn entry_number(entry: &Value) -> Option<String> {
    entry.get("number").map(scalar_text)
}

/// A scalar as text: a quoted `'4'` and a bare `4` are the same number.
fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_yaml_ng::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const LAW: &str = r"
$id: wet_op_de_zorgtoeslag
bwb_id: BWBR0018451
regulatory_layer: WET
articles:
  - number: '1'
    text: In deze wet wordt verstaan onder verzekerde de persoon die verzekerd is.
  - number: '2'
    text: De verzekerde heeft aanspraak op een zorgtoeslag, bedoeld in artikel 3.
    machine_readable:
      execution:
        input:
          - name: inkomen
            source:
              regulation: awir
              output: toetsingsinkomen
        output:
          - name: heeft_recht
      untranslatables:
        - construct: redelijkerwijs
  - number: '3'
    text: De hoogte van de zorgtoeslag wordt bij ministeriele regeling bepaald.
  - number: '3a'
    text: Vervallen.
  - number: '4'
    text: De standaardpremie.
    machine_readable:
      open_terms:
        - id: standaardpremie
      execution:
        output:
          - name: premie
          ";

    #[test]
    fn reads_what_each_article_has_and_lacks() {
        let facts = LawFacts::from_yaml(LAW).unwrap();
        assert_eq!(facts.law_id, "wet_op_de_zorgtoeslag");
        assert_eq!(facts.bwb_id, "BWBR0018451");
        assert_eq!(facts.layer, "WET");
        let numbers: Vec<&str> = facts.articles.iter().map(|a| a.number.as_str()).collect();
        assert_eq!(numbers, ["1", "2", "3", "3a", "4"]);

        let one = facts.article("1").unwrap();
        assert!(one.is_definition);
        assert!(one.missing_mr);

        let two = facts.article("2").unwrap();
        assert!(two.has_mr && !two.missing_mr);
        assert_eq!(two.outputs, ["heeft_recht"]);
        assert_eq!(two.markings, 1);
        assert_eq!(two.binds.len(), 1);
        assert_eq!(two.binds[0].regulation.as_deref(), Some("awir"));
        assert!(two.cites.contains(&Node::new("BWBR0018451", "3")));

        // A lapsed article has nothing to model.
        assert!(!facts.article("3a").unwrap().missing_mr);
        assert_eq!(facts.article("4").unwrap().open_terms, ["standaardpremie"]);
    }

    #[test]
    fn the_loaded_law_carries_the_graph_and_the_definitions() {
        let loaded = LawFacts::from_yaml(LAW).unwrap().loaded();
        assert_eq!(loaded.graph.bwb_id, "BWBR0018451");
        assert_eq!(loaded.graph.articles, ["1", "2", "3", "3a", "4"]);
        assert!(loaded.definitions.contains("1"));
    }

    #[test]
    fn only_a_short_text_that_says_vervallen_is_lapsed() {
        assert!(!says_something(""));
        assert!(!says_something("  Vervallen.  "));
        assert!(!says_something("[Vervallen per 01-01-2024]"));
        // Short, but not lapsed: a one-line article still has a norm.
        assert!(says_something("De standaardpremie."));
        // Says "vervallen", but at length: the word is part of a norm.
        assert!(says_something(
            "Het recht op zorgtoeslag is vervallen indien de verzekerde niet langer verzekerd is."
        ));
    }

    #[test]
    fn reads_an_implements_declaration() {
        let facts = LawFacts::from_yaml(
            r"
$id: regeling_standaardpremie
bwb_id: BWBR0050536
regulatory_layer: MINISTERIELE_REGELING
articles:
  - number: 1
    text: De standaardpremie bedraagt.
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: 4
          open_term: standaardpremie
",
        )
        .unwrap();
        let one = facts.article("1").unwrap();
        assert_eq!(
            one.implements,
            [Implements {
                law_id: "wet_op_de_zorgtoeslag".into(),
                article: Some("4".into()),
                open_term: "standaardpremie".into(),
            }]
        );
    }

    #[test]
    fn counts_a_dangling_binding_on_its_article() {
        let facts = LawFacts::from_yaml(
            r"
$id: wet
bwb_id: BWBX
articles:
  - number: '1'
    text: Iets.
    machine_readable:
      execution:
        input:
          - name: x
            source:
              output: bestaat_niet
        output:
          - name: y
",
        )
        .unwrap();
        assert_eq!(facts.article("1").unwrap().dangling, 1);
    }

    #[test]
    fn an_unparseable_body_has_no_facts() {
        assert!(LawFacts::from_yaml("articles: [").is_none());
    }
}
